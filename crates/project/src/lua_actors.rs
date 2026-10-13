//! Actors as Lua scripts: metadata and settings fields as data, drawing as functions. Port of
//! `libs/nsmb-project/src/lua_actors.cpp`; the contract is `docs/lua-actors.md`.
//!
//! Loaders exist only while `init` runs and renderer methods only while `render` runs: both are `mlua` scoped
//! functions, so a script that keeps `ctx` or `renderer` and calls it later gets an error rather than disk I/O or a
//! stray draw.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, Weak};

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_formats::{
    Bgr555Color, GraphicsFormatKind, RgbaImage, decode_4bpp_tiles, decode_8bpp_tiles, decode_bgr555_palette, decompress_nitro_lz,
    identify_graphics_format, parse_ncgr, parse_nclr, parse_tex0, render_tex0_texture,
};
use bigstar_level::{SpriteBounds, SpriteDefinition, SpriteDefinitionCatalog, SpriteFieldChoice, SpriteFieldDefinition, read_sprite_field};
use mlua::{AnyUserData, ChunkMode, Function, HookTriggers, Lua, MetaMethod, MultiValue, Scope, Table, UserData, UserDataMethods, Value, VmState};
use serde::{Deserialize, Serialize};

use crate::lua_sandbox::{MAX_INSTRUCTIONS, MAX_SCRIPT_BYTES, boolean_field, integer_field, lua_message, new_state, string_field};

/// Graphics and Palette name indexed ROM resources, Model a TEX0 container, and Image a still that ships beside the
/// actor script rather than in the ROM.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActorResourceKind {
    Graphics,
    Palette,
    Model,
    Image,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LuaActorRenderInput {
    pub x: i32,
    pub y: i32,
    pub pixel_x: f64,
    pub pixel_y: f64,
    pub object_id: u32,
    pub r#type: u16,
    pub hash: Option<u32>,
    pub data: [u8; 6],
    pub settings: u32,
    pub valid: bool,
}

impl LuaActorRenderInput {
    pub fn new() -> Self {
        Self { valid: true, ..Self::default() }
    }
}

/// The stock editor draws more than stills: a few actors outline their reach with bars, rings and lines, so shapes
/// are commands too rather than something a script has to fake with a one-pixel image.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum LuaActorDrawKind {
    #[default]
    Image,
    Text,
    FillRect,
    StrokeRect,
    FillEllipse,
    Line,
}

/// Every command shares the destination rectangle so the painter treats them uniformly; `image`, `text`, `color` and
/// the line endpoint are the parts only some kinds use.
#[derive(Debug, Clone, PartialEq)]
pub struct LuaActorDrawCommand {
    pub kind: LuaActorDrawKind,
    pub image: Option<Arc<RgbaImage>>,
    pub text: String,
    pub source_x: i32,
    pub source_y: i32,
    pub source_width: u32,
    pub source_height: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub opacity: f64,
    /// Line endpoint, in scene pixels, for `Line`.
    pub x2: f64,
    pub y2: f64,
    /// Stroke thickness for `StrokeRect` and `Line`.
    pub stroke_width: f64,
    /// Non-premultiplied 0xRRGGBBAA.
    pub color: u32,
    pub flip_x: bool,
    pub flip_y: bool,
}

impl Default for LuaActorDrawCommand {
    fn default() -> Self {
        Self {
            kind: LuaActorDrawKind::Image,
            image: None,
            text: String::new(),
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 0.0,
            rotation: 0.0,
            opacity: 1.0,
            x2: 0.0,
            y2: 0.0,
            stroke_width: 1.0,
            color: 0xFFFF_FFFF,
            flip_x: false,
            flip_y: false,
        }
    }
}

/// What a render produced, and whether the script claimed the drawing as complete. `custom_rendered` false means the
/// script drew something but still wants the stock numbered marker, the way NSMBe's `customRendered = false` cases do.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LuaActorRender {
    pub commands: Vec<LuaActorDrawCommand>,
    pub custom_rendered: bool,
}

#[derive(Debug, Clone, Default)]
pub struct LuaActorDefinition {
    pub object_id: Option<u32>,
    pub hash: Option<u32>,
    pub sprite: SpriteDefinition,
    pub source: PathBuf,
    pub render_resources: Vec<PathBuf>,
    /// Set when `init` failed: the actor keeps its metadata and loses its renderer, rather than taking the whole
    /// catalog down with it.
    pub init_error: String,
    pub program: Option<Arc<LuaActorProgram>>,
}

#[derive(Debug, Clone, Default)]
pub struct LuaActorCatalog {
    pub actors: Vec<LuaActorDefinition>,
}

impl LuaActorCatalog {
    pub fn by_object_id(&self, object_id: u32) -> Option<&LuaActorDefinition> {
        self.actors.iter().find(|actor| actor.object_id == Some(object_id))
    }

    pub fn by_hash(&self, hash: u32) -> Option<&LuaActorDefinition> {
        self.actors.iter().find(|actor| actor.hash == Some(hash))
    }

    pub fn sprite_definitions(&self) -> SpriteDefinitionCatalog {
        SpriteDefinitionCatalog { definitions: self.actors.iter().map(|actor| actor.sprite.clone()).collect() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LuaActorLayer {
    pub name: String,
    pub directory: PathBuf,
}

/// What the host found for a resource name. The address is what identifies the format and keys the cache; `source`
/// is the file on disk it came from, empty when the ROM supplied it and therefore nothing for a watcher to follow.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActorResource {
    pub rom_path: String,
    pub source: PathBuf,
    pub data: Vec<u8>,
}

/// Finds a resource by kind and name. `NotFound` means the actor draws without it; any other error fails the call.
pub type ActorResourceResolver<'a> = &'a dyn Fn(ActorResourceKind, &str) -> Result<ActorResource>;
/// The project layer stays free of an image codec, so the host injects one. Without one, `loadImage` reports every
/// still as unavailable.
pub type ActorImageDecoder<'a> = &'a dyn Fn(&[u8]) -> Result<RgbaImage>;

/// The readable name a module's `stageobjects.json` gives a stage object -- `coop.CoopFlagActor.Default` -- turned
/// into the hash and class id the level format stores.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageObjectIdentity {
    pub hash: u32,
    pub object_id: u32,
}

pub type StageObjectNameResolver<'a> = &'a dyn Fn(&str) -> Option<StageObjectIdentity>;

/// Enough for a screen-wide run of 16px tiles. Past it a draw is dropped rather than raised, so an actor given
/// nonsense settings still draws the part that fits instead of vanishing.
const MAX_DRAW_COMMANDS: usize = 4096;
/// Sized past the sprite count a course reaches, so a level's steady state fits and only a drag -- which mints a key
/// per mouse position -- can fill it.
const MAX_CALL_CACHE_ENTRIES: usize = 1024;
const COORDINATE_LIMIT: f64 = 1_048_576.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourceType {
    Palette,
    Drawable,
}

#[derive(Debug, Clone)]
struct ResourceRef {
    kind: ResourceType,
    /// What the asset cache is keyed on: the file on disk when a module supplies this resource, and the ROM address
    /// otherwise.
    key: String,
    /// The file to watch, empty when the ROM supplied it.
    watch: PathBuf,
    palette: Option<Arc<Vec<Bgr555Color>>>,
    image: Option<Arc<RgbaImage>>,
}

/// The handle a script holds for a loaded resource. It reports its own pixel size, which ports need to place a
/// rotated still by its rotated top-left.
struct ResourceHandle {
    index: usize,
    width: i64,
    height: i64,
}

impl UserData for ResourceHandle {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |_, this, key: String| match key.as_str() {
            "width" => Ok(this.width),
            "height" => Ok(this.height),
            _ => Err(mlua::Error::runtime(format!("a resource has no '{key}'"))),
        });
    }
}

#[derive(Default)]
struct LuaAssetCache {
    palettes: HashMap<String, Weak<Vec<Bgr555Color>>>,
    images: HashMap<String, Weak<RgbaImage>>,
}

/// A render is pure in its input, so the answer is kept rather than recomputed. Position is part of the key because
/// commands carry absolute scene coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct CallCacheKey {
    x: i32,
    y: i32,
    pixel_x: u64,
    pixel_y: u64,
    object_id: u32,
    r#type: u16,
    hash: Option<u32>,
    data: [u8; 6],
    settings: u32,
    valid: bool,
}

impl CallCacheKey {
    fn new(input: &LuaActorRenderInput) -> Self {
        Self {
            x: input.x,
            y: input.y,
            pixel_x: input.pixel_x.to_bits(),
            pixel_y: input.pixel_y.to_bits(),
            object_id: input.object_id,
            r#type: input.r#type,
            hash: input.hash,
            data: input.data,
            settings: input.settings,
            valid: input.valid,
        }
    }
}

/// Which bounds function ran is part of the key, resolved rather than taken from the caller's flag: an actor that
/// declares no `renderBounds` answers both questions from `grabBounds`, and should not pay twice for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum BoundsFunction {
    Grab,
    Render,
}

/// A script that raises is cached too: an actor whose `render` is broken is the worst case for repaint cost
/// otherwise.
#[derive(Clone)]
struct RenderCacheEntry {
    render: LuaActorRender,
    error: String,
}

#[derive(Clone)]
struct BoundsCacheEntry {
    bounds: Option<SpriteBounds>,
    error: String,
}

/// One actor's Lua state and the functions it declared. Everything a call reads besides its input is fixed for the
/// program's lifetime, so the caches live here and die with it; reloading a layer builds new programs.
pub struct LuaActorProgram {
    lua: Lua,
    instructions: Arc<AtomicUsize>,
    resources: Vec<ResourceRef>,
    fields: Vec<SpriteFieldDefinition>,
    render: Option<Function>,
    grab_bounds: Option<Function>,
    render_bounds: Option<Function>,
    render_cache: Mutex<HashMap<CallCacheKey, RenderCacheEntry>>,
    bounds_cache: Mutex<HashMap<(CallCacheKey, BoundsFunction), BoundsCacheEntry>>,
}

impl std::fmt::Debug for LuaActorProgram {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LuaActorProgram")
            .field("render", &self.render.is_some())
            .field("grab_bounds", &self.grab_bounds.is_some())
            .field("render_bounds", &self.render_bounds.is_some())
            .finish_non_exhaustive()
    }
}

fn runtime(message: impl Into<String>) -> mlua::Error {
    mlua::Error::runtime(message.into())
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Dragging mints a fresh key every mouse position, so a map is capped and dropped whole once it fills.
fn remember<K: std::hash::Hash + Eq, V>(map: &mut HashMap<K, V>, key: K, entry: V) {
    if map.len() >= MAX_CALL_CACHE_ENTRIES {
        map.clear();
    }
    map.insert(key, entry);
}

fn arg(args: &MultiValue, position: usize) -> Value {
    args.get(position.wrapping_sub(1)).cloned().unwrap_or(Value::Nil)
}

/// `luaL_checkinteger`: an integer, a float with an integral value, or a string that reads as one.
fn check_integer(lua: &Lua, args: &MultiValue, position: usize) -> mlua::Result<i64> {
    lua.coerce_integer(arg(args, position))?.ok_or_else(|| runtime(format!("bad argument #{position} (number expected)")))
}

fn options_table(args: &MultiValue, position: usize) -> mlua::Result<Option<Table>> {
    match arg(args, position) {
        Value::Nil => Ok(None),
        Value::Table(table) => Ok(Some(table)),
        _ => Err(runtime(format!("bad argument #{position} (table expected)"))),
    }
}

fn option_integer(table: &Table, key: &str, fallback: i64, minimum: i64, maximum: i64) -> mlua::Result<i64> {
    match table.get::<Value>(key)? {
        Value::Nil => Ok(fallback),
        Value::Integer(value) if (minimum..=maximum).contains(&value) => Ok(value),
        Value::Integer(_) => Err(runtime(format!("{key} is outside its supported range"))),
        _ => Err(runtime(format!("{key} must be an integer"))),
    }
}

fn option_number(lua: &Lua, table: &Table, key: &str, fallback: f64, minimum: f64, maximum: f64) -> mlua::Result<f64> {
    let value = table.get::<Value>(key)?;
    if value.is_nil() {
        return Ok(fallback);
    }
    let number = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        Value::String(_) => lua.coerce_number(value)?.ok_or_else(|| runtime(format!("{key} must be a number")))?,
        _ => return Err(runtime(format!("{key} must be a number"))),
    };
    if !number.is_finite() || number < minimum || number > maximum {
        return Err(runtime(format!("{key} is outside its supported range")));
    }
    Ok(number)
}

fn option_boolean(table: &Table, key: &str, fallback: bool) -> mlua::Result<bool> {
    match table.get::<Value>(key)? {
        Value::Nil => Ok(fallback),
        Value::Boolean(value) => Ok(value),
        _ => Err(runtime(format!("{key} must be a boolean"))),
    }
}

fn actor_resource_bytes(bytes: &[u8]) -> Result<Vec<u8>> {
    match bytes.first() {
        Some(0x10 | 0x11) => decompress_nitro_lz(bytes),
        _ => Ok(bytes.to_vec()),
    }
}

/// The address when the ROM supplies this, the disk path when a module does.
fn resource_key(resource: &ActorResource) -> String {
    if resource.source.as_os_str().is_empty() { resource.rom_path.clone() } else { resource.source.to_string_lossy().into_owned() }
}

fn decode_bitmap(
    resource: &ActorResource,
    bpp: i64,
    mut width: usize,
    mut height: usize,
    tile_offset: usize,
    palette: &[Bgr555Color],
) -> Result<Arc<RgbaImage>> {
    let bytes = actor_resource_bytes(&resource.data)?;
    let mut characters = bytes.as_slice();
    let ncgr;
    if identify_graphics_format(&bytes, &resource.rom_path) == GraphicsFormatKind::NitroCharacterGraphics {
        ncgr = parse_ncgr(&bytes)?;
        characters = &ncgr.characters;
        if width == 0 && ncgr.width != 0xffff {
            width = usize::from(ncgr.width) * 8;
        }
        if height == 0 && ncgr.height != 0xffff {
            height = usize::from(ncgr.height) * 8;
        }
    }
    if width == 0 || !width.is_multiple_of(8) || width > 4096 {
        return Err(Error::new(ErrorCode::InvalidArgument, "loadBitmap needs a width divisible by 8"));
    }
    let tiles: Vec<[u8; 64]> = if bpp == 4 {
        decode_4bpp_tiles(characters)?.into_iter().map(|tile| tile.pixels).collect()
    } else {
        decode_8bpp_tiles(characters)?.into_iter().map(|tile| tile.pixels).collect()
    };
    if tile_offset > tiles.len() {
        return Err(Error::new(ErrorCode::OutOfBounds, "loadBitmap tileOffset is outside the resource"));
    }
    let tiles_wide = width / 8;
    let available = tiles.len() - tile_offset;
    if height == 0 {
        height = available.div_ceil(tiles_wide) * 8;
    }
    if height == 0 || !height.is_multiple_of(8) || height > 4096 || width > (4096 * 4096) / height {
        return Err(Error::new(ErrorCode::InvalidArgument, "loadBitmap height is invalid"));
    }
    let mut image = RgbaImage { width, height, pixels: vec![0; width * height * 4] };
    let wanted = tiles_wide * (height / 8);
    for (tile, pixels) in tiles.iter().skip(tile_offset).take(wanted.min(available)).enumerate() {
        let (tile_x, tile_y) = (tile % tiles_wide, tile / tiles_wide);
        for (i, &index) in pixels.iter().enumerate() {
            let Some(color) = palette.get(usize::from(index)) else { continue };
            let (x, y) = (i % 8, i / 8);
            let target = ((tile_y * 8 + y) * width + tile_x * 8 + x) * 4;
            if let Some(slot) = image.pixels.get_mut(target..target + 4) {
                slot.copy_from_slice(&[color.red, color.green, color.blue, if color.transparent { 0 } else { 255 }]);
            }
        }
    }
    Ok(Arc::new(image))
}

fn decode_palette(resource: &ActorResource, bank: usize, color_count: usize, transparent: bool) -> Result<Arc<Vec<Bgr555Color>>> {
    let bytes = actor_resource_bytes(&resource.data)?;
    let nclr;
    let mut raw = bytes.as_slice();
    if identify_graphics_format(&bytes, &resource.rom_path) == GraphicsFormatKind::NitroPalette {
        nclr = parse_nclr(&bytes)?;
        raw = &nclr.palette;
    }
    let colors = decode_bgr555_palette(raw, false)?;
    let count = if color_count == 0 { colors.len() } else { color_count };
    let start = bank * count;
    if count == 0 || start > colors.len() || count > colors.len() - start {
        return Err(Error::new(ErrorCode::OutOfBounds, "loadPalette bank is outside the resource"));
    }
    let mut result = colors.get(start..start + count).unwrap_or_default().to_vec();
    if transparent && let Some(first) = result.first_mut() {
        first.transparent = true;
    }
    Ok(Arc::new(result))
}

fn decode_texture(resource: &ActorResource, texture: usize, palette: usize) -> Result<Arc<RgbaImage>> {
    let bytes = actor_resource_bytes(&resource.data)?;
    let tex0 = parse_tex0(&bytes)?;
    let (Some(texture), Some(palette)) = (tex0.textures.get(texture), tex0.palettes.get(palette)) else {
        return Err(Error::new(ErrorCode::OutOfBounds, "loadTexture index is outside the resource"));
    };
    Ok(Arc::new(render_tex0_texture(texture, palette)?))
}

/// Everything the loaders in `init` share: where to find things, the decoded-asset cache, and the handles handed out
/// so far.
struct LoaderContext<'a> {
    resolve: ActorResourceResolver<'a>,
    decode_image: Option<ActorImageDecoder<'a>>,
    cache: &'a RefCell<LuaAssetCache>,
    layer: &'a Path,
    resources: RefCell<Vec<ResourceRef>>,
}

impl LoaderContext<'_> {
    fn resolve(&self, kind: ActorResourceKind, name: &str) -> mlua::Result<Option<ActorResource>> {
        match (self.resolve)(kind, name) {
            Ok(found) => Ok(Some(found)),
            Err(error) if error.code == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(runtime(error.message)),
        }
    }

    fn push(&self, lua: &Lua, resource: ResourceRef) -> mlua::Result<AnyUserData> {
        let mut resources = self.resources.borrow_mut();
        let (width, height) = resource.image.as_ref().map_or((0, 0), |image| (image.width as i64, image.height as i64));
        let handle = ResourceHandle { index: resources.len(), width, height };
        resources.push(resource);
        lua.create_userdata(handle)
    }

    fn cached_image(&self, key: &str) -> Option<Arc<RgbaImage>> {
        self.cache.borrow().images.get(key).and_then(Weak::upgrade)
    }

    fn remember_image(&self, key: String, image: &Arc<RgbaImage>) {
        self.cache.borrow_mut().images.insert(key, Arc::downgrade(image));
    }

    /// Stills live beside the actor script rather than in the ROM, so they resolve against the layer directory and
    /// may not climb out of it.
    fn load_image(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<AnyUserData> {
        let Value::String(name) = arg(args, 1) else { return Err(runtime("loadImage filename must be a string")) };
        let name = PathBuf::from(name.to_string_lossy());
        if name.is_absolute() || name.has_root() {
            return Err(runtime("loadImage filename must be relative to the actor directory"));
        }
        if name.components().any(|part| part == Component::ParentDir) {
            return Err(runtime("loadImage filename may not leave the actor directory"));
        }
        let mut resource = ResourceRef { kind: ResourceType::Drawable, key: String::new(), watch: PathBuf::new(), palette: None, image: None };
        let path = self.layer.join(&name);
        if path.exists() {
            resource.watch = path.clone();
            resource.key = path.to_string_lossy().into_owned();
            resource.image = self.cached_image(&resource.key);
            if resource.image.is_none()
                && let Some(decode) = self.decode_image
                && let Ok(bytes) = read_file(&path)
                && let Ok(decoded) = decode(&bytes)
            {
                let image = Arc::new(decoded);
                self.remember_image(resource.key.clone(), &image);
                resource.image = Some(image);
            }
        }
        self.push(lua, resource)
    }

    fn load_palette(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<AnyUserData> {
        let Value::String(name) = arg(args, 1) else { return Err(runtime("loadPalette filename must be a string")) };
        let options = options_table(args, 2)?;
        let (bank, colors, transparent) = match &options {
            Some(table) => (
                option_integer(table, "bank", 0, 0, 65535)?,
                option_integer(table, "colors", 0, 0, 65536)?,
                option_boolean(table, "transparent", true)?,
            ),
            None => (0, 0, true),
        };
        let found = self.resolve(ActorResourceKind::Palette, &name.to_string_lossy())?;
        let mut resource = ResourceRef {
            kind: ResourceType::Palette,
            key: found.as_ref().map(resource_key).unwrap_or_default(),
            watch: found.as_ref().map(|found| found.source.clone()).unwrap_or_default(),
            palette: None,
            image: None,
        };
        if let Some(found) = found {
            let key = format!("{}:{bank}:{colors}:{}", resource.key, if transparent { "1" } else { "0" });
            resource.palette = self.cache.borrow().palettes.get(&key).and_then(Weak::upgrade);
            if resource.palette.is_none()
                && let Ok(decoded) = decode_palette(&found, bank as usize, colors as usize, transparent)
            {
                self.cache.borrow_mut().palettes.insert(key, Arc::downgrade(&decoded));
                resource.palette = Some(decoded);
            }
        }
        self.push(lua, resource)
    }

    /// The palette is bound here, not at draw time: an actor that wants two palettes of the same characters loads
    /// the bitmap twice, and painting stays a blit.
    fn load_bitmap(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<AnyUserData> {
        let Value::String(name) = arg(args, 1) else { return Err(runtime("loadBitmap filename must be a string")) };
        let Value::Table(options) = arg(args, 2) else { return Err(runtime("bad argument #2 (table expected)")) };
        let bpp = option_integer(&options, "bpp", 0, 4, 8)?;
        if bpp != 4 && bpp != 8 {
            return Err(runtime("loadBitmap bpp must be 4 or 8"));
        }
        let width = option_integer(&options, "width", 0, 0, 4096)? as usize;
        let height = option_integer(&options, "height", 0, 0, 4096)? as usize;
        let offset = option_integer(&options, "tileOffset", 0, 0, 1 << 24)? as usize;
        let palette_value = options.get::<Value>("palette")?;
        if palette_value.is_nil() {
            return Err(runtime("loadBitmap needs a palette from loadPalette"));
        }
        let (palette_colors, palette_key) = {
            let resources = self.resources.borrow();
            let handle = handle_of(&palette_value, ResourceType::Palette, &resources, "loadBitmap palette must come from loadPalette")?;
            (handle.palette.clone(), handle.key.clone())
        };
        let found = self.resolve(ActorResourceKind::Graphics, &name.to_string_lossy())?;
        let mut resource = ResourceRef {
            kind: ResourceType::Drawable,
            key: found.as_ref().map(resource_key).unwrap_or_default(),
            watch: found.as_ref().map(|found| found.source.clone()).unwrap_or_default(),
            palette: None,
            image: None,
        };
        if let (Some(found), Some(colors)) = (found, palette_colors) {
            let key = format!("{}:{bpp}:{width}:{height}:{offset}:{palette_key}", resource.key);
            resource.image = self.cached_image(&key);
            if resource.image.is_none()
                && let Ok(decoded) = decode_bitmap(&found, bpp, width, height, offset, &colors)
            {
                self.remember_image(key, &decoded);
                resource.image = Some(decoded);
            }
        }
        self.push(lua, resource)
    }

    fn load_texture(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<AnyUserData> {
        let Value::String(name) = arg(args, 1) else { return Err(runtime("loadTexture filename must be a string")) };
        let options = options_table(args, 2)?;
        let (texture, palette) = match &options {
            Some(table) => (option_integer(table, "texture", 0, 0, 65535)?, option_integer(table, "palette", 0, 0, 65535)?),
            None => (0, 0),
        };
        let found = self.resolve(ActorResourceKind::Model, &name.to_string_lossy())?;
        let mut resource = ResourceRef {
            kind: ResourceType::Drawable,
            key: found.as_ref().map(resource_key).unwrap_or_default(),
            watch: found.as_ref().map(|found| found.source.clone()).unwrap_or_default(),
            palette: None,
            image: None,
        };
        if let Some(found) = found {
            let key = format!("{}:tex:{texture}:{palette}", resource.key);
            resource.image = self.cached_image(&key);
            if resource.image.is_none()
                && let Ok(decoded) = decode_texture(&found, texture as usize, palette as usize)
            {
                self.remember_image(key, &decoded);
                resource.image = Some(decoded);
            }
        }
        self.push(lua, resource)
    }
}

fn handle_of<'r>(value: &Value, expected: ResourceType, resources: &'r [ResourceRef], what: &str) -> mlua::Result<&'r ResourceRef> {
    let Value::UserData(userdata) = value else { return Err(runtime(what)) };
    let index = userdata.borrow::<ResourceHandle>().map_err(|_| runtime(what))?.index;
    resources.get(index).filter(|resource| resource.kind == expected).ok_or_else(|| runtime(what))
}

/// Two things come up in nearly every actor: an image that has to be drawn with a transform decided somewhere other
/// than the draw call, and a destination given as the top-left of the *drawn* result rather than as a rectangle to
/// rotate in place. `gfx` is the small Lua layer over `renderer:draw` that actor scripts share.
const ACTOR_PRELUDE: &str = r#"
gfx = {}

-- Pairs an image with a transform to apply wherever it is eventually drawn.
function gfx.transform(image, rotation, flipX, flipY)
    if type(image) == "table" then image = image.image end
    return { image = image, rotation = rotation, flipX = flipX, flipY = flipY }
end

-- The pixel size of an image or of a transformed image.
function gfx.width(image)
    if type(image) == "table" then image = image.image end
    return image and image.width or 0
end

function gfx.height(image)
    if type(image) == "table" then image = image.image end
    return image and image.height or 0
end

-- Gives a rectangle a colour so it can go straight to a shape method.
function gfx.colored(rect, color)
    rect.color = color
    return rect
end

-- Draws an image so that the visible result's top-left corner lands on x, y.
-- renderer:draw rotates a destination rectangle about its own centre, so a
-- quarter-turn needs the rectangle offset by half the difference of its sides.
function gfx.blit(renderer, image, x, y, w, h, source)
    local rotation, flipX, flipY = 0, false, false
    if type(image) == "table" then
        rotation, flipX, flipY, image = image.rotation, image.flipX, image.flipY, image.image
    end
    if not image then return false end
    local opts = { rotation = rotation, flipX = flipX, flipY = flipY }
    if source then
        opts.sourceX, opts.sourceY = source.x, source.y
        opts.sourceWidth, opts.sourceHeight = source.width, source.height
    end
    opts.width = w or (source and source.width) or image.width
    opts.height = h or (source and source.height) or image.height
    if rotation % 180 ~= 0 then
        x, y = x + (opts.height - opts.width) / 2, y + (opts.width - opts.height) / 2
    end
    opts.x, opts.y = x, y
    return renderer:draw(image, opts)
end
"#;

/// What a call into `render` or a bounds function sees: the input and the fields it reads through, and the commands
/// the renderer collects.
struct CallContext<'a> {
    input: &'a LuaActorRenderInput,
    fields: &'a [SpriteFieldDefinition],
    resources: &'a [ResourceRef],
    commands: RefCell<Vec<LuaActorDrawCommand>>,
}

fn actor_data_value(input: &LuaActorRenderInput) -> u64 {
    u64::from(input.settings) | (u64::from(input.data[1]) << 32) | (u64::from(input.data[0]) << 40)
}

impl CallContext<'_> {
    /// Scripts read settings through the fields they already declare, so the canvas and the properties panel decode
    /// the same bits the same way. A checkbox field yields a boolean because 0 is truthy in Lua.
    fn field(&self, lua: &Lua, key: Value) -> mlua::Result<Value> {
        let key = lua.coerce_string(key)?.map(|key| key.to_string_lossy()).ok_or_else(|| runtime("bad argument #2 (string expected)"))?;
        let Some(field) = self.fields.iter().find(|field| field.key == key) else {
            // Returning nil would let a mistyped key silently render the wrong thing.
            return Err(runtime(format!("actor has no declared field '{key}'")));
        };
        let value = read_sprite_field(actor_data_value(self.input), field);
        Ok(if field.checkbox_value != 0 { Value::Boolean(value != 0) } else { Value::Integer(i64::from(value)) })
    }

    fn drawable(&self, args: &MultiValue, position: usize) -> mlua::Result<Option<Arc<RgbaImage>>> {
        Ok(handle_of(
            &arg(args, position),
            ResourceType::Drawable,
            self.resources,
            "renderer methods need a handle from loadImage, loadBitmap or loadTexture",
        )?
        .image
        .clone())
    }

    #[allow(clippy::too_many_arguments)]
    fn emit_draw(
        &self,
        lua: &Lua,
        image: Option<Arc<RgbaImage>>,
        text: String,
        args: &MultiValue,
        options: usize,
        mut source_x: i64,
        mut source_y: i64,
        mut source_width: i64,
        mut source_height: i64,
    ) -> mlua::Result<bool> {
        let is_text = !text.is_empty();
        let (image_width, image_height) = image.as_ref().map_or((0, 0), |image| (image.width as i64, image.height as i64));
        if !is_text && (image.is_none() || image_width == 0 || image_height == 0) {
            return Ok(false);
        }
        let table = options_table(args, options)?;
        if let Some(table) = table.as_ref().filter(|_| !is_text) {
            source_x = option_integer(table, "sourceX", source_x, 0, 1 << 20)?;
            source_y = option_integer(table, "sourceY", source_y, 0, 1 << 20)?;
            source_width = option_integer(table, "sourceWidth", source_width, 1, 1 << 20)?;
            source_height = option_integer(table, "sourceHeight", source_height, 1, 1 << 20)?;
        }
        if !is_text && (source_x < 0 || source_y < 0 || source_x + source_width > image_width || source_y + source_height > image_height) {
            return Err(runtime("draw source rectangle is outside the resource"));
        }
        if self.commands.borrow().len() >= MAX_DRAW_COMMANDS {
            return Ok(false);
        }
        let input = self.input;
        let mut command = LuaActorDrawCommand {
            kind: if is_text { LuaActorDrawKind::Text } else { LuaActorDrawKind::Image },
            image,
            text,
            source_x: source_x as i32,
            source_y: source_y as i32,
            source_width: source_width as u32,
            source_height: source_height as u32,
            x: input.pixel_x,
            y: input.pixel_y,
            width: source_width as f64,
            height: source_height as f64,
            ..LuaActorDrawCommand::default()
        };
        if let Some(table) = &table {
            command.x = option_number(lua, table, "x", input.pixel_x, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
            command.y = option_number(lua, table, "y", input.pixel_y, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
            command.width = option_number(lua, table, "width", source_width as f64, 0.001, COORDINATE_LIMIT)?;
            command.height = option_number(lua, table, "height", source_height as f64, 0.001, COORDINATE_LIMIT)?;
            command.rotation = option_number(lua, table, "rotation", 0.0, -36000.0, 36000.0)?;
            command.opacity = option_number(lua, table, "opacity", 1.0, 0.0, 1.0)?;
            command.flip_x = option_boolean(table, "flipX", false)?;
            command.flip_y = option_boolean(table, "flipY", false)?;
        }
        self.commands.borrow_mut().push(command);
        Ok(true)
    }

    fn draw(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<bool> {
        let image = self.drawable(args, 2)?;
        let (width, height) = image.as_ref().map_or((1, 1), |image| (image.width as i64, image.height as i64));
        self.emit_draw(lua, image, String::new(), args, 3, 0, 0, width, height)
    }

    fn draw_tile(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<bool> {
        let image = self.drawable(args, 2)?;
        let tile = check_integer(lua, args, 3)?;
        if tile < 0 {
            return Err(runtime("tile index cannot be negative"));
        }
        let Some(found) = &image else { return Ok(false) };
        let tiles_wide = (found.width / 8) as i64;
        if tiles_wide == 0 {
            return Err(runtime("drawTile needs a resource at least one tile wide"));
        }
        let (source_x, source_y) = (tile % tiles_wide * 8, tile / tiles_wide * 8);
        if source_y >= found.height as i64 {
            return Ok(false);
        }
        self.emit_draw(lua, image, String::new(), args, 4, source_x, source_y, 8, 8)
    }

    fn draw_frame(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<bool> {
        let image = self.drawable(args, 2)?;
        let frame = check_integer(lua, args, 3)?;
        let width = check_integer(lua, args, 4)?;
        let height = check_integer(lua, args, 5)?;
        if frame < 0 || width <= 0 || height <= 0 || width > 4096 || height > 4096 {
            return Err(runtime("drawFrame arguments are invalid"));
        }
        let Some(found) = &image else { return Ok(false) };
        let frames_wide = found.width as i64 / width;
        if frames_wide == 0 {
            return Err(runtime("drawFrame width exceeds the resource"));
        }
        let (source_x, source_y) = (frame % frames_wide * width, frame / frames_wide * height);
        if source_y + height > found.height as i64 {
            return Ok(false);
        }
        self.emit_draw(lua, image, String::new(), args, 6, source_x, source_y, width, height)
    }

    /// The stock renderer prints numbers over a few actors, so text is a draw command rather than something scripts
    /// have to fake with graphics.
    fn draw_text(&self, lua: &Lua, args: &MultiValue) -> mlua::Result<bool> {
        let Value::String(text) = arg(args, 2) else { return Err(runtime("renderer:drawText needs a string")) };
        let text = text.to_string_lossy();
        if text.is_empty() {
            return Ok(false);
        }
        self.emit_draw(lua, None, text, args, 3, 0, 0, 0, 0)
    }

    fn emit_shape(&self, lua: &Lua, args: &MultiValue, kind: LuaActorDrawKind) -> mlua::Result<bool> {
        let Value::Table(table) = arg(args, 2) else { return Err(runtime("bad argument #2 (table expected)")) };
        if self.commands.borrow().len() >= MAX_DRAW_COMMANDS {
            return Ok(false);
        }
        let input = self.input;
        let mut command = LuaActorDrawCommand { kind, ..LuaActorDrawCommand::default() };
        command.x = option_number(lua, &table, "x", input.pixel_x, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
        command.y = option_number(lua, &table, "y", input.pixel_y, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
        if kind == LuaActorDrawKind::Line {
            command.x2 = option_number(lua, &table, "x2", command.x, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
            command.y2 = option_number(lua, &table, "y2", command.y, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
        } else {
            // A degenerate or inverted box draws nothing, the way GDI+ treats one, rather than failing the actor's
            // whole render.
            command.width = option_number(lua, &table, "width", 16.0, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
            command.height = option_number(lua, &table, "height", 16.0, -COORDINATE_LIMIT, COORDINATE_LIMIT)?;
            if command.width <= 0.0 || command.height <= 0.0 {
                return Ok(false);
            }
        }
        command.rotation = option_number(lua, &table, "rotation", 0.0, -36000.0, 36000.0)?;
        command.opacity = option_number(lua, &table, "opacity", 1.0, 0.0, 1.0)?;
        command.stroke_width = option_number(lua, &table, "strokeWidth", 1.0, 0.0, 4096.0)?;
        command.color = option_color(&table, 0xFFFF_FFFF)?;
        self.commands.borrow_mut().push(command);
        Ok(true)
    }
}

/// `#rgb`, `#rrggbb` and `#rrggbbaa` all land as 0xRRGGBBAA.
fn parse_color(text: &str) -> Result<u32> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if ![3, 6, 8].contains(&digits.len()) {
        return Err(Error::new(ErrorCode::ParseError, "a colour must be #rgb, #rrggbb or #rrggbbaa"));
    }
    let mut value = 0u32;
    for digit in digits.chars() {
        let nibble = digit.to_digit(16).ok_or_else(|| Error::new(ErrorCode::ParseError, "a colour has a non-hex digit"))?;
        value = (value << 4) | nibble;
        if digits.len() == 3 {
            value = (value << 4) | nibble;
        }
    }
    if digits.len() != 8 {
        value = (value << 8) | 0xFF;
    }
    Ok(value)
}

/// An integer is taken as the packed value directly so scripts can compute a colour.
fn option_color(table: &Table, fallback: u32) -> mlua::Result<u32> {
    match table.get::<Value>("color")? {
        Value::Nil => Ok(fallback),
        Value::Integer(value) => Ok(value as u32),
        Value::String(text) => parse_color(&text.to_string_lossy()).map_err(|e| runtime(e.message)),
        _ => Err(runtime("color must be a string or a packed integer")),
    }
}

/// The renderer a script draws through, alive for one `render` call.
fn renderer<'scope>(lua: &Lua, scope: &'scope Scope<'scope, '_>, call: &'scope CallContext<'_>) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.set("draw", scope.create_function(move |lua, args: MultiValue| call.draw(lua, &args))?)?;
    table.set("drawTile", scope.create_function(move |lua, args: MultiValue| call.draw_tile(lua, &args))?)?;
    table.set("drawFrame", scope.create_function(move |lua, args: MultiValue| call.draw_frame(lua, &args))?)?;
    table.set("drawText", scope.create_function(move |lua, args: MultiValue| call.draw_text(lua, &args))?)?;
    for (name, kind) in [
        ("fillRect", LuaActorDrawKind::FillRect),
        ("strokeRect", LuaActorDrawKind::StrokeRect),
        ("fillEllipse", LuaActorDrawKind::FillEllipse),
        ("drawLine", LuaActorDrawKind::Line),
    ] {
        table.set(name, scope.create_function(move |lua, args: MultiValue| call.emit_shape(lua, &args, kind))?)?;
    }
    Ok(table)
}

/// The `actor` table a script reads, alive for one call.
fn actor_table<'scope>(lua: &Lua, scope: &'scope Scope<'scope, '_>, call: &'scope CallContext<'_>) -> mlua::Result<Table> {
    let input = call.input;
    let table = lua.create_table()?;
    table.set("x", input.x)?;
    table.set("y", input.y)?;
    table.set("pixelX", Value::Number(input.pixel_x))?;
    table.set("pixelY", Value::Number(input.pixel_y))?;
    table.set("objectId", input.object_id)?;
    table.set("type", input.r#type)?;
    table.set("settings", input.settings)?;
    if let Some(hash) = input.hash {
        table.set("hash", hash)?;
    }
    table.set("valid", input.valid)?;
    let data = lua.create_table()?;
    for (index, &byte) in input.data.iter().enumerate() {
        data.raw_set(index as i64, byte)?;
    }
    table.set("data", data)?;
    // `actor:field("odd-key-name")`, for keys that are not identifiers.
    table.set("field", scope.create_function(move |lua, (_actor, key): (Value, Value)| call.field(lua, key))?)?;
    let fields = lua.create_table()?;
    let metatable = lua.create_table()?;
    metatable.set("__index", scope.create_function(move |lua, (_fields, key): (Value, Value)| call.field(lua, key))?)?;
    fields.set_metatable(Some(metatable))?;
    table.set("fields", fields)?;
    Ok(table)
}

fn function_field(table: &Table, key: &str, actor: &str) -> Result<Option<Function>> {
    match table.get::<Value>(key) {
        Ok(Value::Nil) => Ok(None),
        Ok(Value::Function(function)) => Ok(Some(function)),
        _ => Err(Error::new(ErrorCode::ParseError, format!("{actor} has an invalid {key}"))),
    }
}

fn field_mask(field: &SpriteFieldDefinition) -> i64 {
    (1i64 << field.bit_width) - 1
}

fn read_fields(table: &Table, name: &str) -> Result<Vec<SpriteFieldDefinition>> {
    let fields_table = match table.get::<Value>("fields") {
        Ok(Value::Nil) => return Ok(Vec::new()),
        Ok(Value::Table(fields)) => fields,
        _ => return Err(Error::new(ErrorCode::ParseError, format!("{name} has invalid fields"))),
    };
    let mut keys = HashSet::new();
    let mut fields = Vec::new();
    for i in 1..=fields_table.raw_len() {
        let Ok(Value::Table(field_table)) = fields_table.raw_get::<Value>(i) else {
            return Err(Error::new(ErrorCode::ParseError, format!("{name} has an invalid field")));
        };
        let mut field = SpriteFieldDefinition {
            key: string_field(&field_table, "key", name, true)?,
            label: string_field(&field_table, "label", name, true)?,
            notes: string_field(&field_table, "notes", name, false)?,
            ..SpriteFieldDefinition::default()
        };
        if !keys.insert(field.key.clone()) {
            return Err(Error::new(ErrorCode::ParseError, format!("{name} has a duplicate field key")));
        }
        let offset = integer_field(&field_table, "bitOffset", 0, 47, &field.key, true, 0)?;
        let width = integer_field(&field_table, "bitWidth", 1, 31.min(48 - offset), &field.key, true, 0)?;
        field.bit_offset = offset as u8;
        field.bit_width = width as u8;
        field.value_offset = integer_field(&field_table, "valueOffset", i64::from(i32::MIN), i64::from(i32::MAX), &field.key, false, 0)? as i32;
        field.checkbox_value = integer_field(&field_table, "checkboxValue", 0, field_mask(&field), &field.key, false, 0)? as u32;
        field.signed_value = boolean_field(&field_table, "signed", &field.key, false)?;
        match field_table.get::<Value>("choices") {
            Ok(Value::Nil) => {}
            Ok(Value::Table(choices)) => {
                for choice in 1..=choices.raw_len() {
                    let Ok(Value::Table(choice_table)) = choices.raw_get::<Value>(choice) else {
                        return Err(Error::new(ErrorCode::ParseError, format!("{} has an invalid choice", field.key)));
                    };
                    let value = integer_field(&choice_table, "value", 0, i64::from(u32::MAX), &field.key, true, 0)? as u32;
                    let label = string_field(&choice_table, "label", &field.key, true)?;
                    field.choices.push(SpriteFieldChoice { value, label });
                }
            }
            _ => return Err(Error::new(ErrorCode::ParseError, format!("{} has invalid choices", field.key))),
        }
        fields.push(field);
    }
    Ok(fields)
}

fn identity(table: &Table, key: &str) -> Result<Option<u32>> {
    match table.get::<Value>(key) {
        Ok(Value::Nil) => Ok(None),
        Ok(Value::Integer(value)) if (0..=i64::from(u32::MAX)).contains(&value) => Ok(Some(value as u32)),
        _ => Err(Error::new(ErrorCode::ParseError, format!("actor has an invalid {key}"))),
    }
}

fn parse_error(error: mlua::Error) -> Error {
    Error::new(ErrorCode::ParseError, lua_message(&error))
}

fn load_actor(
    path: &Path,
    layer: &Path,
    resolve: ActorResourceResolver<'_>,
    decode_image: Option<ActorImageDecoder<'_>>,
    stage_object: Option<StageObjectNameResolver<'_>>,
    cache: &RefCell<LuaAssetCache>,
) -> Result<LuaActorDefinition> {
    let bytes = read_file(path)?;
    if bytes.len() > MAX_SCRIPT_BYTES {
        return Err(Error::new(ErrorCode::OutOfBounds, "actor script is larger than 1 MiB"));
    }
    let lua = new_state()?;
    lua.load(ACTOR_PRELUDE)
        .set_name("=gfx")
        .exec()
        .map_err(|e| Error::new(ErrorCode::ParseError, format!("the gfx prelude failed: {}", lua_message(&e))))?;
    let instructions = Arc::new(AtomicUsize::new(0));
    let counter = instructions.clone();
    lua.set_hook(HookTriggers::new().every_nth_instruction(1000), move |_, _| {
        if counter.fetch_add(1000, Ordering::Relaxed) + 1000 > MAX_INSTRUCTIONS {
            return Err(runtime("actor script exceeded its instruction limit"));
        }
        Ok(VmState::Continue)
    })
    .map_err(parse_error)?;

    let filename = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let returned: Value =
        lua.load(bytes.as_slice()).set_name(format!("@{}", path.display())).set_mode(ChunkMode::Text).call(()).map_err(parse_error)?;
    let Value::Table(table) = returned else { return Err(Error::new(ErrorCode::ParseError, "actor script must return a table")) };

    let mut actor = LuaActorDefinition {
        source: path.to_path_buf(),
        object_id: identity(&table, "objectId")?,
        hash: identity(&table, "hash")?,
        ..Default::default()
    };
    let stage_object_name = string_field(&table, "stageObject", &filename, false)?;
    if !stage_object_name.is_empty() {
        if actor.object_id.is_some() || actor.hash.is_some() {
            return Err(Error::new(ErrorCode::ParseError, "actor must define exactly one of objectId, hash or stageObject"));
        }
        let Some(stage_object) = stage_object else {
            return Err(Error::new(ErrorCode::NotFound, format!("stageObject {stage_object_name} needs a built stage-object catalog")));
        };
        let Some(found) = stage_object(&stage_object_name) else {
            return Err(Error::new(ErrorCode::NotFound, format!("no stage object is named {stage_object_name}")));
        };
        // Exactly what writing `hash = 0x...` by hand would have done: the name is the stable spelling of the same
        // identity.
        actor.hash = Some(found.hash);
    }
    if actor.object_id.is_some() == actor.hash.is_some() {
        return Err(Error::new(ErrorCode::ParseError, "actor must define exactly one of objectId, hash or stageObject"));
    }
    actor.sprite.name = string_field(&table, "name", &filename, true)?;
    actor.sprite.object_id = actor.object_id.unwrap_or(0);
    actor.sprite.hash = actor.hash;
    let name = actor.sprite.name.clone();
    let category = string_field(&table, "category", &name, false)?;
    if !category.is_empty() {
        actor.sprite.category = category;
    }
    actor.sprite.notes = string_field(&table, "notes", &name, false)?;
    actor.sprite.fields = read_fields(&table, &name)?;
    let mut render = function_field(&table, "render", &name)?;
    let grab_bounds = function_field(&table, "grabBounds", &name)?;
    let render_bounds = function_field(&table, "renderBounds", &name)?;
    let init = function_field(&table, "init", &name)?;

    // init() is the only place resources load, so painting never touches disk. A failure here costs the actor its
    // renderer, not the whole catalog.
    let loader = LoaderContext { resolve, decode_image, cache, layer, resources: RefCell::new(Vec::new()) };
    if let Some(init) = init {
        instructions.store(0, Ordering::Relaxed);
        let ran = lua.scope(|scope| {
            let ctx = lua.create_table()?;
            let loader = &loader;
            ctx.set("loadImage", scope.create_function(move |lua, args: MultiValue| loader.load_image(lua, &args))?)?;
            ctx.set("loadPalette", scope.create_function(move |lua, args: MultiValue| loader.load_palette(lua, &args))?)?;
            ctx.set("loadBitmap", scope.create_function(move |lua, args: MultiValue| loader.load_bitmap(lua, &args))?)?;
            ctx.set("loadTexture", scope.create_function(move |lua, args: MultiValue| loader.load_texture(lua, &args))?)?;
            init.call::<()>(ctx)
        });
        if let Err(error) = ran {
            actor.init_error = lua_message(&error);
            render = None;
        }
    }
    let resources = loader.resources.into_inner();
    actor.render_resources =
        resources.iter().filter(|resource| !resource.watch.as_os_str().is_empty()).map(|resource| resource.watch.clone()).collect();
    actor.render_resources.sort();
    actor.render_resources.dedup();

    if render.is_some() || grab_bounds.is_some() || render_bounds.is_some() {
        actor.program = Some(Arc::new(LuaActorProgram {
            lua,
            instructions,
            resources,
            fields: actor.sprite.fields.clone(),
            render,
            grab_bounds,
            render_bounds,
            render_cache: Mutex::new(HashMap::new()),
            bounds_cache: Mutex::new(HashMap::new()),
        }));
    }

    // Evaluate the bounds once at zero settings and origin, so the absolute rect that comes back doubles as the
    // offset used for hit testing and thumbnails.
    if actor.program.is_some() {
        let probe = LuaActorRenderInput { object_id: actor.object_id.unwrap_or(0), hash: actor.hash, ..LuaActorRenderInput::new() };
        if let Ok(Some(grab)) = lua_actor_bounds(&actor, &probe, false) {
            actor.sprite.grab_bounds = grab;
        }
        actor.sprite.render_bounds = match lua_actor_bounds(&actor, &probe, true) {
            Ok(Some(render_box)) => render_box,
            _ => actor.sprite.grab_bounds,
        };
    }
    Ok(actor)
}

pub fn has_lua_renderer(actor: &LuaActorDefinition) -> bool {
    actor.program.as_ref().is_some_and(|program| program.render.is_some())
}

pub fn render_lua_actor(actor: &LuaActorDefinition, input: &LuaActorRenderInput) -> Result<LuaActorRender> {
    let Some(program) = actor.program.as_ref() else { return Ok(LuaActorRender::default()) };
    let Some(render) = program.render.as_ref() else { return Ok(LuaActorRender::default()) };
    let key = CallCacheKey::new(input);
    if let Some(cached) = lock(&program.render_cache).get(&key) {
        if !cached.error.is_empty() {
            return Err(Error::new(ErrorCode::ParseError, cached.error.clone()));
        }
        return Ok(cached.render.clone());
    }
    program.instructions.store(0, Ordering::Relaxed);
    let call = CallContext { input, fields: &program.fields, resources: &program.resources, commands: RefCell::new(Vec::new()) };
    let lua = &program.lua;
    let result = lua.scope(|scope| {
        let actor_value = actor_table(lua, scope, &call)?;
        let renderer_value = renderer(lua, scope, &call)?;
        render.call::<Value>((actor_value, renderer_value))
    });
    match result {
        Err(error) => {
            let message = lua_message(&error);
            remember(&mut lock(&program.render_cache), key, RenderCacheEntry { render: LuaActorRender::default(), error: message.clone() });
            Err(Error::new(ErrorCode::ParseError, message))
        }
        Ok(returned) => {
            // A falsy return is NSMBe's `customRendered = false`: whatever the script drew stands, and the stock
            // numbered marker is drawn over it as well.
            let custom_rendered = !matches!(returned, Value::Nil | Value::Boolean(false));
            let render = LuaActorRender { commands: call.commands.into_inner(), custom_rendered };
            remember(&mut lock(&program.render_cache), key, RenderCacheEntry { render: render.clone(), error: String::new() });
            Ok(render)
        }
    }
}

/// Bounds arrive in scene pixels derived from `pixelX`/`pixelY`, which carry a drag offset and so are not necessarily
/// integers.
fn bounds_number(lua: &Lua, table: &Table, key: &str, minimum: i64, maximum: i64, actor: &str) -> Result<i64> {
    let invalid = || Error::new(ErrorCode::ParseError, format!("{actor} has an invalid {key}"));
    let value = table.get::<Value>(key).map_err(|_| invalid())?;
    let number = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        Value::String(_) => lua.coerce_number(value).ok().flatten().ok_or_else(invalid)?,
        _ => return Err(invalid()),
    };
    let rounded = number.round();
    if !rounded.is_finite() || rounded < minimum as f64 || rounded > maximum as f64 {
        return Err(invalid());
    }
    Ok(rounded as i64)
}

/// The actor's box in scene pixels, or `None` when the actor declares no bounds function.
pub fn lua_actor_bounds(actor: &LuaActorDefinition, input: &LuaActorRenderInput, render_bounds: bool) -> Result<Option<SpriteBounds>> {
    let Some(program) = actor.program.as_ref() else { return Ok(None) };
    let (which, function) = match (render_bounds, &program.render_bounds, &program.grab_bounds) {
        (true, Some(function), _) => (BoundsFunction::Render, function),
        (_, _, Some(function)) => (BoundsFunction::Grab, function),
        _ => return Ok(None),
    };
    let key = (CallCacheKey::new(input), which);
    if let Some(cached) = lock(&program.bounds_cache).get(&key) {
        if !cached.error.is_empty() {
            return Err(Error::new(ErrorCode::ParseError, cached.error.clone()));
        }
        return Ok(cached.bounds);
    }
    program.instructions.store(0, Ordering::Relaxed);
    let call = CallContext { input, fields: &program.fields, resources: &program.resources, commands: RefCell::new(Vec::new()) };
    let lua = &program.lua;
    let returned = lua.scope(|scope| {
        let actor_value = actor_table(lua, scope, &call)?;
        function.call::<Value>(actor_value)
    });
    let name = &actor.sprite.name;
    let outcome: Result<Option<SpriteBounds>> = match returned {
        Err(error) => Err(Error::new(ErrorCode::ParseError, lua_message(&error))),
        Ok(Value::Nil) => Ok(None),
        Ok(Value::Table(table)) => (|| {
            let x = bounds_number(lua, &table, "x", -32768, 32767, name)?;
            let y = bounds_number(lua, &table, "y", -32768, 32767, name)?;
            let width = bounds_number(lua, &table, "width", 1, 65536, name)?;
            let height = bounds_number(lua, &table, "height", 1, 65536, name)?;
            Ok(Some(SpriteBounds { x: x as i16, y: y as i16, width: width as u16, height: height as u16 }))
        })(),
        Ok(_) => Err(Error::new(ErrorCode::ParseError, format!("{name} bounds function must return a table or nil"))),
    };
    let entry = match &outcome {
        Ok(bounds) => BoundsCacheEntry { bounds: *bounds, error: String::new() },
        Err(error) => BoundsCacheEntry { bounds: None, error: error.message.clone() },
    };
    remember(&mut lock(&program.bounds_cache), key, entry);
    outcome
}

/// Loads every `*.lua` in each layer, in layer order and then file-name order; the first definition for an object id
/// or hash wins.
pub fn load_lua_actor_layers(
    layers: &[LuaActorLayer],
    resolve: ActorResourceResolver<'_>,
    decode_image: Option<ActorImageDecoder<'_>>,
    stage_object: Option<StageObjectNameResolver<'_>>,
) -> Result<LuaActorCatalog> {
    let mut result = LuaActorCatalog::default();
    let cache = RefCell::new(LuaAssetCache::default());
    let (mut object_ids, mut hashes) = (HashSet::new(), HashSet::new());
    for layer in layers {
        match layer.directory.try_exists() {
            Err(_) => return Err(Error::new(ErrorCode::IoError, format!("could not inspect actor layer {}", layer.name))),
            Ok(false) => continue,
            Ok(true) => {}
        }
        if !layer.directory.is_dir() {
            return Err(Error::new(ErrorCode::InvalidArgument, format!("actor layer is not a directory: {}", layer.name)));
        }
        let entries =
            std::fs::read_dir(&layer.directory).map_err(|_| Error::new(ErrorCode::IoError, format!("could not read actor layer {}", layer.name)))?;
        let mut scripts: Vec<PathBuf> =
            entries.flatten().map(|entry| entry.path()).filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "lua")).collect();
        scripts.sort();
        for script in scripts {
            let actor = load_actor(&script, &layer.directory, resolve, decode_image, stage_object, &cache).map_err(|e| {
                let filename = script.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
                Error::new(e.code, format!("{} / {filename}: {}", layer.name, e.message))
            })?;
            let fresh = match (actor.object_id, actor.hash) {
                (Some(object_id), _) => object_ids.insert(object_id),
                (None, Some(hash)) => hashes.insert(hash),
                (None, None) => false,
            };
            if fresh {
                result.actors.push(actor);
            }
        }
    }
    Ok(result)
}
