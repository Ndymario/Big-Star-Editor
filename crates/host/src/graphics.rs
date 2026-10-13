//! The open project's graphics as the host serves them: level tilesets as atlases, backgrounds, actor catalogs and
//! draw lists, every image delivered once by content hash.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::{BACKGROUND_PALETTE_OFFSET, BACKGROUND_TILE_OFFSET, FOREGROUND_PALETTE_OFFSET, FOREGROUND_TILE_OFFSET, RgbaImage};
use bigstar_level::{BackgroundCatalog, BackgroundLayer, LevelTilesetReferences, ModuleDataCatalog, SpriteDefinition};
use bigstar_patching::{FileManifest, ModuleGraph};
use bigstar_project::{
    ActorResource, ActorResourceKind, BackgroundAnimator, BackgroundResourceFiles, EffectiveFilesystem, LuaActorCatalog, LuaActorDrawKind,
    LuaActorLayer, LuaActorRenderInput, LuaBackgroundLayer, LuaTileLayer, RomReader, StageObjectIdentity, TileOverlayCatalog,
    background_palette_animator, background_tile_animator, has_lua_renderer, load_background_frames, load_background_graphics, load_level_tilesets,
    load_lua_actor_layers, load_lua_background_layers, load_lua_tile_layers, lua_actor_bounds, render_lua_actor, resolve_level_graphics,
};
use bigstar_protocol::{
    ATLAS_COLUMNS, ActorCatalog, ActorDraw, ActorInfo, ActorInput, DrawCommand, DrawKind, ImageGraphics, LayerFrame, LevelGraphics, LevelId,
    Map16Overlay, ResourceBytes, TextureData, TextureId, TilemapSlot, TilesetGraphics, TilesetResources, TilesetSlot,
};
use sha2::{Digest, Sha256};

/// Every texture handed out, by id, kept alive so a frontend can ask for it later. Images an actor program holds are
/// also indexed by address, so a draw list does not hash the same still on every render.
#[derive(Default)]
pub(crate) struct TextureStore {
    images: HashMap<TextureId, Arc<RgbaImage>>,
    /// Each entry holds its image, so the address it is keyed by is never freed and handed to another image.
    by_address: HashMap<usize, (TextureId, Arc<RgbaImage>)>,
}

impl TextureStore {
    fn id(image: &RgbaImage) -> TextureId {
        let mut hash = Sha256::new();
        hash.update((image.width as u32).to_le_bytes());
        hash.update((image.height as u32).to_le_bytes());
        hash.update(&image.pixels);
        hash.finalize().iter().take(16).map(|byte| format!("{byte:02x}")).collect()
    }

    /// An image someone else keeps: hashed once, then found by address.
    pub fn add(&mut self, image: Arc<RgbaImage>) -> TextureId {
        let address = Arc::as_ptr(&image) as usize;
        if let Some((id, _)) = self.by_address.get(&address) {
            return id.clone();
        }
        let id = Self::id(&image);
        self.by_address.insert(address, (id.clone(), Arc::clone(&image)));
        self.images.entry(id.clone()).or_insert(image);
        id
    }

    /// An image decoded for this call alone, which nothing would find by address again.
    pub fn add_owned(&mut self, image: RgbaImage) -> TextureId {
        let id = Self::id(&image);
        self.images.entry(id.clone()).or_insert_with(|| Arc::new(image));
        id
    }

    pub fn get(&self, id: &str) -> Option<TextureData> {
        self.images.get(id).map(|image| TextureData {
            id: id.to_owned(),
            width: image.width as u32,
            height: image.height as u32,
            rgba: image.pixels.clone(),
        })
    }
}

/// Decodes the stills that ship beside actor scripts.
fn decode_png(bytes: &[u8]) -> Result<RgbaImage> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| Error::new(ErrorCode::ParseError, format!("could not decode a PNG: {e}")))?
        .to_rgba8();
    Ok(RgbaImage { width: image.width() as usize, height: image.height() as usize, pixels: image.into_raw() })
}

/// Packs 16x16 Map16 blocks into rows of [`ATLAS_COLUMNS`].
fn atlas(blocks: &[RgbaImage]) -> Option<RgbaImage> {
    if blocks.is_empty() {
        return None;
    }
    let columns = ATLAS_COLUMNS as usize;
    let rows = blocks.len().div_ceil(columns);
    let width = columns * 16;
    let mut image = RgbaImage { width, height: rows * 16, pixels: vec![0; width * rows * 16 * 4] };
    for (index, block) in blocks.iter().enumerate() {
        let (left, top) = ((index % columns) * 16, (index / columns) * 16);
        for y in 0..block.height.min(16) {
            let from = y * block.width * 4;
            let to = ((top + y) * width + left) * 4;
            let length = block.width.min(16) * 4;
            if let (Some(row), Some(target)) = (block.pixels.get(from..from + length), image.pixels.get_mut(to..to + length)) {
                target.copy_from_slice(row);
            }
        }
    }
    Some(image)
}

fn draw_kind(kind: LuaActorDrawKind) -> DrawKind {
    match kind {
        LuaActorDrawKind::Image => DrawKind::Image,
        LuaActorDrawKind::Text => DrawKind::Text,
        LuaActorDrawKind::FillRect => DrawKind::FillRect,
        LuaActorDrawKind::StrokeRect => DrawKind::StrokeRect,
        LuaActorDrawKind::FillEllipse => DrawKind::FillEllipse,
        LuaActorDrawKind::Line => DrawKind::Line,
    }
}

/// What the host knows about one open project's graphics. Rebuilt whole when a watched file changes.
pub(crate) struct ProjectGraphics {
    directory: PathBuf,
    data_dir: PathBuf,
    files: EffectiveFilesystem,
    modules: ModuleGraph,
    catalog: ModuleDataCatalog,
    overlays: Option<TileOverlayCatalog>,
    backgrounds: Option<BackgroundCatalog>,
    actors: Option<LuaActorCatalog>,
    sprite_object_ids: Option<Vec<u32>>,
    pub textures: TextureStore,
    /// Files whose change changes what this has served.
    pub watched: Vec<PathBuf>,
}

impl ProjectGraphics {
    pub fn new(
        directory: &Path,
        data_dir: &Path,
        rom: Option<&Path>,
        plan: Option<FileManifest>,
        modules: ModuleGraph,
        catalog: ModuleDataCatalog,
    ) -> Result<Self> {
        let mut files = EffectiveFilesystem::new();
        if let Some(rom) = rom {
            files.set_rom(Some(RomReader::open(rom)?));
        }
        files.set_plan(directory, plan);
        Ok(Self {
            directory: directory.to_path_buf(),
            data_dir: data_dir.to_path_buf(),
            files,
            modules,
            catalog,
            overlays: None,
            backgrounds: None,
            actors: None,
            sprite_object_ids: None,
            textures: TextureStore::default(),
            watched: Vec::new(),
        })
    }

    /// Layers in the order `docs/lua-actors.md`, `docs/lua-tiles.md` and `docs/lua-backgrounds.md` give: the project's own, each enabled module's
    /// in module order, then the shipped directory.
    fn layers(&self, kind: &str, bare: bool) -> Vec<(String, PathBuf)> {
        let mut layers = vec![(format!("project bigstar/{kind}"), self.directory.join("bigstar").join(kind))];
        if bare {
            layers.push((format!("project {kind}"), self.directory.join(kind)));
        }
        for module in self.modules.modules.iter().filter(|module| module.enabled) {
            layers.push((format!("{} bigstar/{kind}", module.key), module.dir.join("bigstar").join(kind)));
            if bare {
                layers.push((format!("{} {kind}", module.key), module.dir.join(kind)));
            }
        }
        layers.push((format!("shipped {kind}"), self.data_dir.join(kind)));
        layers
    }

    fn resolve(files: &mut EffectiveFilesystem, name: &str) -> Result<ActorResource> {
        let data = files.read(name).map_err(|e| if e.code == ErrorCode::NotFound { e } else { Error::new(ErrorCode::NotFound, e.message) })?;
        Ok(ActorResource { rom_path: name.to_owned(), source: files.source(name).unwrap_or_default(), data })
    }

    fn overlays(&mut self) -> &TileOverlayCatalog {
        if self.overlays.is_none() {
            let layers: Vec<LuaTileLayer> =
                self.layers("tiles", false).into_iter().map(|(name, directory)| LuaTileLayer { name, directory }).collect();
            let files = std::cell::RefCell::new(&mut self.files);
            let resolve = |_: ActorResourceKind, name: &str| Self::resolve(&mut files.borrow_mut(), name);
            let loaded = load_lua_tile_layers(&layers, &resolve).unwrap_or_else(|error| {
                log::warn!("tile scripts: {}", error.message);
                TileOverlayCatalog::default()
            });
            for script in &loaded.scripts {
                self.watched.extend(script.resources.iter().cloned());
            }
            self.overlays = Some(loaded);
        }
        self.overlays.get_or_insert_with(TileOverlayCatalog::default)
    }

    /// What the background scripts say, read once. A script that will not load is logged and the rest stand.
    fn backgrounds(&mut self) -> &BackgroundCatalog {
        if self.backgrounds.is_none() {
            let layers: Vec<LuaBackgroundLayer> =
                self.layers("backgrounds", false).into_iter().map(|(name, directory)| LuaBackgroundLayer { name, directory }).collect();
            let loaded = load_lua_background_layers(&layers).unwrap_or_else(|error| {
                log::warn!("background scripts: {}", error.message);
                Default::default()
            });
            for error in &loaded.errors {
                log::warn!("background scripts: {error}");
            }
            self.watched.extend(loaded.sources);
            self.backgrounds = Some(loaded.catalog);
        }
        self.backgrounds.get_or_insert_with(BackgroundCatalog::default)
    }

    /// Everything the scripts have animating one layer of a level: the tiles its own screen and the other layer's
    /// overwrite in it, and the colours its palette cycles. An animation whose file will not read is logged and
    /// dropped, and the layer is drawn without it.
    fn animators(&mut self, layer: BackgroundLayer, references: LevelTilesetReferences) -> Vec<BackgroundAnimator> {
        let catalog = self.backgrounds().clone();
        let own_palette = match layer {
            BackgroundLayer::Background => references.background_palette_id,
            BackgroundLayer::Foreground => references.foreground_palette_id,
        };
        let screens =
            [(BackgroundLayer::Background, references.background_screen_id), (BackgroundLayer::Foreground, references.foreground_screen_id)];
        let mut animators = Vec::new();
        let read = |files: &mut EffectiveFilesystem, watched: &mut Vec<PathBuf>, name: &str| -> Option<Vec<u8>> {
            let data = files.read(name).map_err(|error| log::warn!("background scripts: {name}: {}", error.message)).ok()?;
            watched.extend(files.source(name));
            Some(data)
        };
        for (of, screen) in screens {
            let Some(tiles) = catalog.find(of, screen).and_then(|definition| definition.tiles.as_ref()) else { continue };
            if tiles.copies.iter().all(|copy| copy.layer != layer) {
                continue;
            }
            let Some(data) = read(&mut self.files, &mut self.watched, &tiles.file) else { continue };
            match background_tile_animator(tiles, layer, &data) {
                Ok(animator) => animators.extend(animator),
                Err(error) => log::warn!("background scripts: {}: {}", tiles.file, error.message),
            }
        }
        // The game picks the colours to cycle by the palette the layer loaded, which its own block names.
        if let Some(palette) = catalog.find(layer, own_palette).and_then(|definition| definition.palette.as_ref())
            && let Some(data) = read(&mut self.files, &mut self.watched, &palette.file)
        {
            match background_palette_animator(palette, &data) {
                Ok(animator) => animators.push(animator),
                Err(error) => log::warn!("background scripts: {}: {}", palette.file, error.message),
            }
        }
        animators
    }

    /// One layer of a level as a still and, where the scripts animate it, as the frames it plays.
    fn layer_graphics(&mut self, layer: BackgroundLayer, files: &BackgroundResourceFiles, references: LevelTilesetReferences) -> ImageGraphics {
        // A level without the layer has no files for it, which is not an error.
        if files.screen.is_empty() {
            return ImageGraphics::default();
        }
        let (tile_offset, palette_offset) = match layer {
            BackgroundLayer::Background => (BACKGROUND_TILE_OFFSET, BACKGROUND_PALETTE_OFFSET),
            BackgroundLayer::Foreground => (FOREGROUND_TILE_OFFSET, FOREGROUND_PALETTE_OFFSET),
        };
        let texture = match load_background_graphics(files, tile_offset, palette_offset) {
            Ok(image) => self.textures.add_owned(image),
            Err(error) => return ImageGraphics { texture: None, error: Some(error.message), frames: Vec::new() },
        };
        let animators = self.animators(layer, references);
        let frames = match load_background_frames(files, tile_offset, palette_offset, &animators) {
            Ok(played) => {
                let textures: Vec<TextureId> = played.images.into_iter().map(|image| self.textures.add_owned(image)).collect();
                played.frames.iter().filter_map(|&(image, ticks)| Some(LayerFrame { texture: textures.get(image)?.clone(), ticks })).collect()
            }
            Err(error) => {
                log::warn!("background scripts: {} animation: {}", layer.name(), error.message);
                Vec::new()
            }
        };
        ImageGraphics { texture: Some(texture), error: None, frames }
    }

    /// What NCPatcher now plans, after the host wrote module files: reads follow it.
    pub fn set_plan(&mut self, plan: Option<FileManifest>) {
        self.files.set_plan(&self.directory, plan);
    }

    /// A file as the active variant has it: the module's copy when there is one, else the ROM's.
    pub fn read_file(&mut self, address: &str) -> Result<Vec<u8>> {
        self.files.read(address)
    }

    /// The files behind a level's three tilesets and its backgrounds, and what its tileset's tile scripts draw in place
    /// of blocks, for the tileset workspace. A port of `MainWindow::openTilesetEditor`'s loading.
    pub fn tileset_resources(&mut self, level: LevelId, references: LevelTilesetReferences) -> Result<TilesetResources> {
        let resources = resolve_level_graphics(&mut self.files, references)?;
        let bytes = |file: &bigstar_project::ResourceFile| ResourceBytes { rom_path: file.rom_path.clone(), bytes: file.data.clone() };
        let tilesets = resources
            .tilesets
            .iter()
            .map(|files| TilesetSlot {
                graphics: bytes(&files.graphics),
                palette: bytes(&files.palette),
                map16: bytes(&files.map16),
                objects: bytes(&files.objects),
                object_index: bytes(&files.object_index),
                behaviors: bytes(&files.behaviors),
                tile_offset: files.tile_offset,
                palette_offset: files.palette_offset,
                tile_page_offset: files.tile_page_offset,
            })
            .collect();
        let map = |files: &BackgroundResourceFiles| TilemapSlot {
            graphics: bytes(&files.graphics),
            palette: bytes(&files.palette),
            screen: bytes(&files.screen),
        };
        let (background, foreground) = (map(&resources.background), map(&resources.foreground));
        let overlays = self
            .overlays()
            .blocks(references.tileset_id)
            .into_iter()
            .map(|(block, overlay)| Map16Overlay {
                block,
                image: overlay.image.as_deref().cloned(),
                remap: overlay.remap,
                replaces: overlay.replaces,
            })
            .collect();
        Ok(TilesetResources { level, tilesets, background, foreground, references, overlays })
    }

    pub fn level_graphics(&mut self, level: LevelId, generation: u64, references: LevelTilesetReferences) -> LevelGraphics {
        let mut result = LevelGraphics { level, generation, ..LevelGraphics::default() };
        let resources = match resolve_level_graphics(&mut self.files, references) {
            Ok(resources) => resources,
            Err(error) => {
                result.error = Some(error.message);
                return result;
            }
        };
        self.watched.extend(resources.watched_files.iter().cloned());
        self.overlays();
        let overlays = self.overlays.as_ref();
        let tilesets = load_level_tilesets(&resources.tilesets, references.tileset_id, overlays);
        for ((target, decoded), error) in result.tilesets.iter_mut().zip(tilesets.tilesets).zip(tilesets.errors) {
            *target = TilesetGraphics {
                atlas: atlas(&decoded.map16_tiles).map(|image| self.textures.add_owned(image)),
                blocks: decoded.map16_tiles.len() as u32,
                objects: decoded.objects,
                error: (!error.is_empty()).then_some(error),
            };
        }
        result.background = self.layer_graphics(BackgroundLayer::Background, &resources.background, references);
        result.foreground = self.layer_graphics(BackgroundLayer::Foreground, &resources.foreground, references);
        result.backgrounds = self.backgrounds().clone();
        result
    }

    fn ensure_actors(&mut self) {
        if self.actors.is_some() {
            return;
        }
        let layers: Vec<LuaActorLayer> = self.layers("actors", true).into_iter().map(|(name, directory)| LuaActorLayer { name, directory }).collect();
        let stage_objects: HashMap<String, StageObjectIdentity> = self
            .catalog
            .stage_objects
            .iter()
            .map(|object| (object.name.clone(), StageObjectIdentity { hash: object.hash, object_id: object.object_id }))
            .collect();
        let stage_object = |name: &str| stage_objects.get(name).copied();
        let files = std::cell::RefCell::new(&mut self.files);
        let resolve = |_: ActorResourceKind, name: &str| Self::resolve(&mut files.borrow_mut(), name);
        let catalog = load_lua_actor_layers(&layers, &resolve, Some(&decode_png), Some(&stage_object)).unwrap_or_else(|error| {
            log::warn!("actor scripts: {}", error.message);
            LuaActorCatalog::default()
        });
        for actor in &catalog.actors {
            self.watched.extend(actor.render_resources.iter().cloned());
            self.watched.push(actor.source.clone());
        }
        self.actors = Some(catalog);
    }

    /// The class id each sprite type maps to, out of the ROM's own table.
    fn sprite_object_ids(&mut self) -> &[u32] {
        if self.sprite_object_ids.is_none() {
            let references = LevelTilesetReferences::default();
            let ids =
                resolve_level_graphics(&mut self.files, references).map(|resources| resources.actors.iter().map(|actor| actor.object_id).collect());
            self.sprite_object_ids = Some(ids.unwrap_or_default());
        }
        self.sprite_object_ids.as_deref().unwrap_or_default()
    }

    pub fn actor_catalog(&mut self, generation: u64) -> ActorCatalog {
        self.ensure_actors();
        let sprite_object_ids = self.sprite_object_ids().to_vec();
        let mut actors: Vec<ActorInfo> = self
            .actors
            .as_ref()
            .map(|catalog| {
                catalog
                    .actors
                    .iter()
                    .map(|actor| ActorInfo {
                        definition: actor.sprite.clone(),
                        has_renderer: has_lua_renderer(actor),
                        init_error: (!actor.init_error.is_empty()).then(|| actor.init_error.clone()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        // A module actor nobody wrote a script for is still the project's own: it keeps the name it was declared
        // with and draws as the stock marker.
        let scripted: HashSet<u32> = actors.iter().filter_map(|actor| actor.definition.hash).collect();
        for object in self.catalog.stage_objects.iter().filter(|object| !scripted.contains(&object.hash)) {
            let definition =
                SpriteDefinition { object_id: object.object_id, hash: Some(object.hash), name: object.name.clone(), ..SpriteDefinition::default() };
            actors.push(ActorInfo { definition, has_renderer: false, init_error: None });
        }
        ActorCatalog { generation, actors, sprite_object_ids }
    }

    pub fn render_actors(&mut self, inputs: &[ActorInput]) -> Vec<ActorDraw> {
        self.ensure_actors();
        let object_ids = self.sprite_object_ids().to_vec();
        let Some(catalog) = self.actors.as_ref() else { return Vec::new() };
        let mut draws = Vec::with_capacity(inputs.len());
        for input in inputs {
            let object_id = object_ids.get(usize::from(input.sprite_type)).copied();
            let actor = match input.hash {
                Some(hash) => catalog.by_hash(hash),
                None => object_id.and_then(|id| catalog.by_object_id(id)),
            };
            let Some(actor) = actor else {
                draws.push(ActorDraw::default());
                continue;
            };
            let settings = input.settings;
            let script_input = LuaActorRenderInput {
                x: input.x,
                y: input.y,
                pixel_x: input.pixel_x,
                pixel_y: input.pixel_y,
                object_id: actor.sprite.object_id,
                r#type: input.sprite_type,
                hash: input.hash,
                data: [input.events[0], input.events[1], (settings >> 24) as u8, (settings >> 16) as u8, (settings >> 8) as u8, settings as u8],
                settings,
                valid: true,
            };
            let mut draw = ActorDraw { found: true, ..ActorDraw::default() };
            match lua_actor_bounds(actor, &script_input, false) {
                Ok(bounds) => draw.grab = bounds,
                Err(error) => draw.error = Some(error.message),
            }
            match lua_actor_bounds(actor, &script_input, true) {
                Ok(bounds) => draw.render = bounds,
                Err(error) => draw.error = draw.error.or(Some(error.message)),
            }
            match render_lua_actor(actor, &script_input) {
                Ok(rendered) => {
                    draw.custom_rendered = rendered.custom_rendered;
                    draw.commands = rendered
                        .commands
                        .into_iter()
                        .map(|c| DrawCommand {
                            kind: draw_kind(c.kind),
                            texture: c.image.map(|image| self.textures.add(image)),
                            text: c.text,
                            source: [c.source_x, c.source_y],
                            source_size: [c.source_width, c.source_height],
                            x: c.x,
                            y: c.y,
                            width: c.width,
                            height: c.height,
                            rotation: c.rotation,
                            opacity: c.opacity,
                            x2: c.x2,
                            y2: c.y2,
                            stroke_width: c.stroke_width,
                            color: c.color,
                            flip_x: c.flip_x,
                            flip_y: c.flip_y,
                        })
                        .collect();
                }
                Err(error) => draw.error = draw.error.or(Some(error.message)),
            }
            draws.push(draw);
        }
        draws
    }
}
