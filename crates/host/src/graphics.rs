//! The open project's graphics as the host serves them: level tilesets as atlases, backgrounds, actor catalogs and
//! draw lists, every image delivered once by content hash.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::{BACKGROUND_PALETTE_OFFSET, BACKGROUND_TILE_OFFSET, FOREGROUND_PALETTE_OFFSET, FOREGROUND_TILE_OFFSET, RgbaImage};
use bigstar_level::{BackgroundCatalog, BackgroundDefinition, BackgroundLayer, LevelTilesetReferences, ModuleDataCatalog, SpriteDefinition};
use bigstar_patching::{FileManifest, ModuleGraph};
use bigstar_project::{
    ActorResource, ActorResourceKind, BackgroundAnimator, BackgroundResourceFiles, CatalogEntry, CatalogKind, EffectiveFilesystem, LuaActorCatalog,
    LuaActorDrawKind, LuaActorLayer, LuaActorRenderInput, LuaBackgroundLayer, LuaTileLayer, PROJECT_TILESET_LIST, RomReader, StageObjectIdentity,
    TileOverlayCatalog, TilesetCatalog, TilesetResourceFiles, background_palette_animator, background_tile_animator, builtin_tileset_catalog,
    filled_table_slots, has_lua_renderer, load_background_frames, load_background_graphics, load_level_tilesets, load_lua_actor_layers,
    load_lua_background_layers, load_lua_tile_layers, lua_actor_bounds, parse_tileset_catalog, render_lua_actor, resolve_file_layer,
    resolve_file_tileset, resolve_level_graphics,
};
use bigstar_protocol::{
    ATLAS_COLUMNS, ActorCatalog, ActorDraw, ActorInfo, ActorInput, DrawCommand, DrawKind, EntryKey, GraphicsSubject, ImageGraphics, LayerFrame,
    LevelGraphics, LevelId, LiquidSurface, Map16Overlay, ResourceBytes, SurfaceWave, TextureData, TextureId, TilemapSlot, TilesetGraphics,
    TilesetListEntry, TilesetListing, TilesetResources, TilesetSlot,
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
        LuaActorDrawKind::Liquid => DrawKind::Liquid,
    }
}

/// Per sprite type: the class id it maps to, and the tileset slot and object bank it needs.
#[derive(Debug, Clone, Default)]
struct SpriteTables {
    object_ids: Vec<u32>,
    banks: Vec<[u8; 2]>,
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
    /// The tileset list in force, and why the project's own was passed over when it was.
    tileset_list: Option<(Arc<TilesetCatalog>, Option<String>)>,
    actors: Option<LuaActorCatalog>,
    /// The class id and the object bank of each sprite type, read once.
    sprite_tables: Option<SpriteTables>,
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
            tileset_list: None,
            actors: None,
            sprite_tables: None,
            textures: TextureStore::default(),
            watched: Vec::new(),
        })
    }

    /// A ROM edited directly: read from memory with `edits` over it, drawn with the shipped scripts alone, since there
    /// is no project or module to bring scripts of their own.
    pub fn for_rom(data_dir: &Path, rom: RomReader, edits: &std::collections::BTreeMap<String, Vec<u8>>) -> Self {
        let mut files = EffectiveFilesystem::new();
        files.set_rom(Some(rom));
        for (address, bytes) in edits {
            files.set_edit(address.clone(), bytes.clone());
        }
        Self {
            directory: PathBuf::new(),
            data_dir: data_dir.to_path_buf(),
            files,
            modules: ModuleGraph::default(),
            catalog: ModuleDataCatalog::default(),
            overlays: None,
            backgrounds: None,
            tileset_list: None,
            actors: None,
            sprite_tables: None,
            textures: TextureStore::default(),
            watched: Vec::new(),
        }
    }

    /// Layers in the order `docs/lua-actors.md`, `docs/lua-tiles.md` and `docs/lua-backgrounds.md` give: the project's own, each enabled module's
    /// in module order, then the shipped directory.
    fn layers(&self, kind: &str, bare: bool) -> Vec<(String, PathBuf)> {
        let mut layers = Vec::new();
        if !self.directory.as_os_str().is_empty() {
            layers.push((format!("project bigstar/{kind}"), self.directory.join("bigstar").join(kind)));
            if bare {
                layers.push((format!("project {kind}"), self.directory.join(kind)));
            }
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
            // The tileset list names every layer; a script's own name stands only where the list has none. A slot the
            // list names and no script describes is there by name alone, drawn as it is.
            let list = self.tileset_list().0;
            let mut catalog = loaded.catalog;
            for definition in &mut catalog.definitions {
                if let Some(name) = list.table_name(catalog_kind(definition.layer), definition.id) {
                    definition.name = name.to_owned();
                }
            }
            for layer in [BackgroundLayer::Background, BackgroundLayer::Foreground] {
                for entry in list.layers(catalog_kind(layer)) {
                    let Some(id) = entry.table_id() else { continue };
                    if catalog.find(layer, id).is_none() {
                        catalog.definitions.push(BackgroundDefinition { layer, id, name: entry.name.clone(), ..BackgroundDefinition::default() });
                    }
                }
            }
            self.backgrounds = Some(catalog);
        }
        self.backgrounds.get_or_insert_with(BackgroundCatalog::default)
    }

    /// The tileset list in force: the editor's own with the project's `tilesets.yaml` laid over it when it has one that
    /// reads. Why the project's was passed over comes with it.
    pub fn tileset_list(&mut self) -> (Arc<TilesetCatalog>, Option<String>) {
        if let Some(list) = &self.tileset_list {
            return list.clone();
        }
        let mut catalog = builtin_tileset_catalog().clone();
        let file = self.directory.join(PROJECT_TILESET_LIST);
        let mut problem = None;
        if !self.directory.as_os_str().is_empty() && file.is_file() {
            let read = std::fs::read(&file)
                .map_err(|_| Error::new(ErrorCode::IoError, "it could not be read"))
                .and_then(|bytes| String::from_utf8(bytes).map_err(|_| Error::new(ErrorCode::ParseError, "it is not UTF-8")))
                .and_then(|text| parse_tileset_catalog(&text));
            match read {
                Ok(project) => catalog.merge(project),
                Err(error) => problem = Some(format!("{PROJECT_TILESET_LIST} was not used: {}", error.message)),
            }
        }
        let list = (Arc::new(catalog), problem);
        self.tileset_list = Some(list.clone());
        list
    }

    /// The tileset list as the Tilesets tab shows it, each slot marked with whether the game has files for it. A slot
    /// list that cannot be read marks every slot there.
    pub fn tileset_listing(&mut self) -> TilesetListing {
        let (list, problem) = self.tileset_list();
        let mut filled = |kind| filled_table_slots(&mut self.files, kind).unwrap_or_default();
        let filled = [filled(CatalogKind::Tileset), filled(CatalogKind::Background), filled(CatalogKind::Foreground)];
        let rows = |entries: Vec<(Option<u16>, &str)>, filled: &[bool], subject: fn(EntryKey) -> GraphicsSubject| -> Vec<TilesetListEntry> {
            entries
                .into_iter()
                .map(|(id, name)| {
                    let (key, available) = match id {
                        Some(id) => (EntryKey::Table(id), filled.get(usize::from(id)).copied().unwrap_or(true)),
                        None => (EntryKey::File(name.to_owned()), true),
                    };
                    TilesetListEntry { subject: subject(key), name: name.to_owned(), available }
                })
                .collect()
        };
        let fixed = [(EntryKey::Shared, "Shared"), (EntryKey::SubUnit, "Sub-unit")].map(|(key, name)| TilesetListEntry {
            subject: GraphicsSubject::Tileset(key),
            name: name.to_owned(),
            available: true,
        });
        let mut tilesets = fixed.to_vec();
        tilesets.extend(rows(pairs(&list.tilesets), &filled[0], GraphicsSubject::Tileset));
        TilesetListing {
            tilesets,
            backgrounds: rows(pairs(&list.backgrounds), &filled[1], GraphicsSubject::Background),
            foregrounds: rows(pairs(&list.foregrounds), &filled[2], GraphicsSubject::Foreground),
            scenes: list
                .scenes
                .iter()
                .map(|scene| TilesetListEntry { subject: GraphicsSubject::Scene(scene.name.clone()), name: scene.name.clone(), available: true })
                .collect(),
            problem,
        }
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
    /// What `address` reads as from now on, for a ROM edited directly.
    pub fn set_edit(&mut self, address: String, bytes: Vec<u8>) {
        self.files.set_edit(address, bytes);
    }

    pub fn read_file(&mut self, address: &str) -> Result<Vec<u8>> {
        self.files.read(address)
    }

    /// The files behind what a tileset workspace edits, and what the tile scripts draw in place of the level tileset's
    /// blocks. A level's subject brings the `references` it loads by; an entry of the tileset list is loaded as a level
    /// naming it alone would load it, and keeps only its own part. A port of `MainWindow::openTilesetEditor`'s loading.
    pub fn tileset_resources(&mut self, subject: GraphicsSubject, references: Option<LevelTilesetReferences>) -> Result<TilesetResources> {
        let none = LevelTilesetReferences {
            tileset_id: u16::MAX,
            background_graphics_id: u16::MAX,
            background_screen_id: u16::MAX,
            foreground_graphics_id: u16::MAX,
            foreground_screen_id: u16::MAX,
            background_palette_id: u16::MAX,
            foreground_palette_id: u16::MAX,
            tileset_palette_id: u16::MAX,
        };
        let not_listed = |what: &str, name: &str| Error::new(ErrorCode::NotFound, format!("the tileset list has no {what} named {name:?}"));
        let list = self.tileset_list().0;
        let bytes = |file: &bigstar_project::ResourceFile| ResourceBytes { rom_path: file.rom_path.clone(), bytes: file.data.clone() };
        let map = |files: &BackgroundResourceFiles| TilemapSlot {
            graphics: bytes(&files.graphics),
            palette: bytes(&files.palette),
            screen: bytes(&files.screen),
        };
        let slot = |files: &TilesetResourceFiles| TilesetSlot {
            graphics: bytes(&files.graphics),
            palette: bytes(&files.palette),
            map16: bytes(&files.map16),
            objects: bytes(&files.objects),
            object_index: bytes(&files.object_index),
            behaviors: bytes(&files.behaviors),
            tile_offset: files.tile_offset,
            palette_offset: files.palette_offset,
            tile_page_offset: files.tile_page_offset,
        };
        let mut result = TilesetResources { subject: subject.clone(), ..TilesetResources::default() };

        // A layer named by its files needs nothing else loaded.
        let layer = |kind: CatalogKind, key: &EntryKey| match key {
            EntryKey::File(name) => list.file_layer(kind, name).ok_or_else(|| not_listed("layer", name)).cloned().map(Some),
            _ => Ok(None),
        };
        match &subject {
            GraphicsSubject::Background(key @ EntryKey::File(_)) => {
                let files = layer(CatalogKind::Background, key)?.unwrap_or_default();
                result.background = map(&resolve_file_layer(&mut self.files, &files)?);
                return Ok(result);
            }
            GraphicsSubject::Foreground(key @ EntryKey::File(_)) => {
                let files = layer(CatalogKind::Foreground, key)?.unwrap_or_default();
                result.foreground = map(&resolve_file_layer(&mut self.files, &files)?);
                return Ok(result);
            }
            GraphicsSubject::Scene(name) => {
                let scene = list.scene(name).ok_or_else(|| not_listed("scene", name))?.clone();
                for (files, target) in [
                    (&scene.background, &mut result.background),
                    (&scene.tileset, &mut result.tileset_layer),
                    (&scene.foreground, &mut result.foreground),
                ] {
                    if let Some(files) = files {
                        *target = map(&resolve_file_layer(&mut self.files, files)?);
                    }
                }
                return Ok(result);
            }
            _ => {}
        }

        let references = match &subject {
            GraphicsSubject::Level(_) => {
                references.ok_or_else(|| Error::new(ErrorCode::InvalidArgument, "a level's tilesets need its references"))?
            }
            GraphicsSubject::Tileset(EntryKey::Table(id)) => LevelTilesetReferences { tileset_id: *id, tileset_palette_id: *id, ..none },
            GraphicsSubject::Background(EntryKey::Table(id)) => {
                LevelTilesetReferences { background_graphics_id: *id, background_screen_id: *id, background_palette_id: *id, ..none }
            }
            GraphicsSubject::Foreground(EntryKey::Table(id)) => {
                LevelTilesetReferences { foreground_graphics_id: *id, foreground_screen_id: *id, foreground_palette_id: *id, ..none }
            }
            _ => none,
        };
        let mut resources = resolve_level_graphics(&mut self.files, references)?;
        if let GraphicsSubject::Tileset(EntryKey::File(name)) = &subject {
            let files = list.file_tileset(name).ok_or_else(|| not_listed("tileset", name))?.clone();
            resources.tilesets[1] = resolve_file_tileset(&mut self.files, &files)?;
        }
        // A layer from the tables comes with the shared and sub-unit tilesets every level loads; they are not its own.
        if !matches!(subject, GraphicsSubject::Background(_) | GraphicsSubject::Foreground(_)) {
            result.tilesets = resources.tilesets.iter().map(slot).collect();
        }
        result.background = map(&resources.background);
        result.foreground = map(&resources.foreground);
        // Only a tileset from the tables has blocks the scripts know by number.
        let scripted = matches!(subject, GraphicsSubject::Level(_) | GraphicsSubject::Tileset(EntryKey::Table(_)));
        if scripted {
            result.overlays = self
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
        }
        result.references = references;
        Ok(result)
    }

    pub fn level_graphics(&mut self, level: LevelId, generation: u64, references: LevelTilesetReferences) -> LevelGraphics {
        // The scripts' catalog is the same for every level, so a level whose files will not resolve still names its
        // backgrounds and fails on its own error alone.
        let tileset_names =
            self.tileset_list().0.tilesets.iter().filter(|entry| entry.table_id().is_some()).map(|entry| entry.name.clone()).collect();
        let mut result = LevelGraphics { level, generation, backgrounds: self.backgrounds().clone(), tileset_names, ..LevelGraphics::default() };
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
                behaviors: decoded.behaviors,
                error: (!error.is_empty()).then_some(error),
            };
        }
        result.background = self.layer_graphics(BackgroundLayer::Background, &resources.background, references);
        result.foreground = self.layer_graphics(BackgroundLayer::Foreground, &resources.foreground, references);
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

    /// The class id each sprite type maps to and the object bank it needs, out of the ROM's own tables.
    fn sprite_tables(&mut self) -> &SpriteTables {
        let files = &mut self.files;
        self.sprite_tables.get_or_insert_with(|| {
            resolve_level_graphics(files, LevelTilesetReferences::default())
                .map(|resources| SpriteTables {
                    object_ids: resources.actors.iter().map(|actor| actor.object_id).collect(),
                    banks: resources.sprite_banks,
                })
                .unwrap_or_default()
        })
    }

    fn sprite_object_ids(&mut self) -> &[u32] {
        &self.sprite_tables().object_ids
    }

    pub fn actor_catalog(&mut self, generation: u64) -> ActorCatalog {
        self.ensure_actors();
        let SpriteTables { object_ids: sprite_object_ids, banks: sprite_banks } = self.sprite_tables().clone();
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
        ActorCatalog { generation, actors, sprite_object_ids, sprite_banks }
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
                            liquid: c.liquid.map(|liquid| LiquidSurface {
                                frames: liquid.frames.into_iter().map(|image| self.textures.add(image)).collect(),
                                frame_ticks: liquid.frame_ticks,
                                column_width: liquid.column_width,
                                depth_scale: liquid.depth_scale,
                                waves: liquid
                                    .waves
                                    .into_iter()
                                    .map(|wave| SurfaceWave { amplitude: wave.amplitude, step: wave.step, speed: wave.speed })
                                    .collect(),
                            }),
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

fn catalog_kind(layer: BackgroundLayer) -> CatalogKind {
    match layer {
        BackgroundLayer::Background => CatalogKind::Background,
        BackgroundLayer::Foreground => CatalogKind::Foreground,
    }
}

/// Each of `entries`' slot, when it names one, and name.
fn pairs<F>(entries: &[CatalogEntry<F>]) -> Vec<(Option<u16>, &str)> {
    entries.iter().map(|entry| (entry.table_id(), entry.name.as_str())).collect()
}
