//! The open project's graphics as the host serves them: level tilesets as atlases, backgrounds, actor catalogs and
//! draw lists, every image delivered once by content hash.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::RgbaImage;
use bigstar_level::{LevelTilesetReferences, ModuleDataCatalog};
use bigstar_patching::{FileManifest, ModuleGraph};
use bigstar_project::{
    ActorResource, ActorResourceKind, EffectiveFilesystem, LuaActorCatalog, LuaActorDrawKind, LuaActorLayer, LuaActorRenderInput, LuaTileLayer,
    RomReader, StageObjectIdentity, TileOverlayCatalog, has_lua_renderer, load_background_graphics, load_level_tilesets, load_lua_actor_layers,
    load_lua_tile_layers, lua_actor_bounds, render_lua_actor, resolve_level_graphics,
};
use bigstar_protocol::{
    ATLAS_COLUMNS, ActorCatalog, ActorDraw, ActorInfo, ActorInput, DrawCommand, DrawKind, ImageGraphics, LevelGraphics, LevelId, TextureData,
    TextureId, TilesetGraphics,
};
use sha2::{Digest, Sha256};

/// Every texture handed out, by id, kept alive so a frontend can ask for it later. Images an actor program holds are
/// also indexed by address, so a draw list does not hash the same still on every render.
#[derive(Default)]
pub(crate) struct TextureStore {
    images: HashMap<TextureId, Arc<RgbaImage>>,
    by_address: HashMap<usize, TextureId>,
}

impl TextureStore {
    pub fn add(&mut self, image: Arc<RgbaImage>) -> TextureId {
        let address = Arc::as_ptr(&image) as usize;
        if let Some(id) = self.by_address.get(&address) {
            return id.clone();
        }
        let mut hash = Sha256::new();
        hash.update((image.width as u32).to_le_bytes());
        hash.update((image.height as u32).to_le_bytes());
        hash.update(&image.pixels);
        let id: TextureId = hash.finalize().iter().take(16).map(|byte| format!("{byte:02x}")).collect();
        self.by_address.insert(address, id.clone());
        self.images.entry(id.clone()).or_insert(image);
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
            actors: None,
            sprite_object_ids: None,
            textures: TextureStore::default(),
            watched: Vec::new(),
        })
    }

    /// Layers in the order `docs/lua-actors.md` and `docs/lua-tiles.md` give: the project's own, each enabled module's
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
                atlas: atlas(&decoded.map16_tiles).map(|image| self.textures.add(Arc::new(image))),
                blocks: decoded.map16_tiles.len() as u32,
                objects: decoded.objects,
                error: (!error.is_empty()).then_some(error),
            };
        }
        let mut image = |files| match load_background_graphics(files) {
            Ok(image) => ImageGraphics { texture: Some(self.textures.add(Arc::new(image))), error: None },
            Err(error) => ImageGraphics { texture: None, error: Some(error.message) },
        };
        result.background = image(&resources.background);
        result.foreground = image(&resources.foreground);
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
        let actors = self
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
