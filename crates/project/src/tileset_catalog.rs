//! The tileset list: what the Layers tab offers to edit and what each is called. The editor ships one
//! (`tilesets/tilesets.yaml`); a project's own `tilesets.yaml` merges into it rather than taking its place, so a hack
//! that renames two backgrounds writes two entries. See `docs/tilesets-list.md`.

use std::sync::OnceLock;

use bigstar_core::{Error, ErrorCode, Result};
use serde::Deserialize;

/// The name a project's own list goes by, in the project's root.
pub const PROJECT_TILESET_LIST: &str = "tilesets.yaml";

/// How many slots each of the game's resource tables in overlay 0 has.
pub const TABLE_SLOTS: u16 = 76;

const TILESETS: &str = include_str!("../tilesets/tilesets.yaml");

/// A background, foreground or a scene's tileset layer, named by ROM path.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LayerFiles {
    pub graphics: String,
    pub palette: String,
    pub screen: String,
}

/// A tileset named by ROM path. Its behaviours are optional, as a level tileset's are to the game.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TilesetFiles {
    pub graphics: String,
    pub palette: String,
    pub map16: String,
    pub objects: String,
    pub object_index: String,
    pub behaviors: Option<String>,
}

/// Where an entry's files come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntrySource<F> {
    /// A slot of the game's tables, which say what it loads.
    Table(u16),
    /// Files named outright.
    Files(F),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry<F> {
    pub name: String,
    pub source: EntrySource<F>,
}

impl<F> CatalogEntry<F> {
    pub fn table_id(&self) -> Option<u16> {
        match self.source {
            EntrySource::Table(id) => Some(id),
            EntrySource::Files(_) => None,
        }
    }
}

/// Layers the game puts together outside a level, each where that BG goes: the title screen's castle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SceneEntry {
    pub name: String,
    pub background: Option<LayerFiles>,
    /// BG2, where a level's tileset goes, drawn from a screen instead of from Map16.
    pub tileset: Option<LayerFiles>,
    pub foreground: Option<LayerFiles>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TilesetCatalog {
    pub tilesets: Vec<CatalogEntry<TilesetFiles>>,
    pub backgrounds: Vec<CatalogEntry<LayerFiles>>,
    pub foregrounds: Vec<CatalogEntry<LayerFiles>>,
    pub scenes: Vec<SceneEntry>,
}

/// Which list of the catalog an entry is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogKind {
    Tileset,
    Background,
    Foreground,
}

impl TilesetCatalog {
    /// The name the list gives table slot `id` of `kind`.
    pub fn table_name(&self, kind: CatalogKind, id: u16) -> Option<&str> {
        match kind {
            CatalogKind::Tileset => slot_name(&self.tilesets, id),
            CatalogKind::Background | CatalogKind::Foreground => slot_name(self.layers(kind), id),
        }
    }

    pub fn layers(&self, kind: CatalogKind) -> &[CatalogEntry<LayerFiles>] {
        match kind {
            CatalogKind::Foreground => &self.foregrounds,
            _ => &self.backgrounds,
        }
    }

    /// The tileset named `name` that names its files.
    pub fn file_tileset(&self, name: &str) -> Option<&TilesetFiles> {
        self.tilesets.iter().find(|entry| entry.name == name).and_then(|entry| match &entry.source {
            EntrySource::Files(files) => Some(files),
            EntrySource::Table(_) => None,
        })
    }

    /// The background or foreground named `name` that names its files.
    pub fn file_layer(&self, kind: CatalogKind, name: &str) -> Option<&LayerFiles> {
        self.layers(kind).iter().find(|entry| entry.name == name).and_then(|entry| match &entry.source {
            EntrySource::Files(files) => Some(files),
            EntrySource::Table(_) => None,
        })
    }

    pub fn scene(&self, name: &str) -> Option<&SceneEntry> {
        self.scenes.iter().find(|scene| scene.name == name)
    }

    /// `project` laid over this list: a slot it names takes its name, and an entry or scene it names as one here
    /// takes that one's place. The rest it has are added after.
    pub fn merge(&mut self, project: TilesetCatalog) {
        merge_entries(&mut self.tilesets, project.tilesets);
        merge_entries(&mut self.backgrounds, project.backgrounds);
        merge_entries(&mut self.foregrounds, project.foregrounds);
        for scene in project.scenes {
            match self.scenes.iter_mut().find(|own| own.name == scene.name) {
                Some(own) => *own = scene,
                None => self.scenes.push(scene),
            }
        }
    }
}

fn slot_name<F>(entries: &[CatalogEntry<F>], id: u16) -> Option<&str> {
    entries.iter().find(|entry| entry.table_id() == Some(id)).map(|entry| entry.name.as_str())
}

fn merge_entries<F>(own: &mut Vec<CatalogEntry<F>>, project: Vec<CatalogEntry<F>>) {
    for entry in project {
        let same = |existing: &CatalogEntry<F>| match (&entry.source, &existing.source) {
            (EntrySource::Table(id), EntrySource::Table(other)) => id == other,
            (EntrySource::Files(_), EntrySource::Files(_)) => entry.name == existing.name,
            _ => false,
        };
        match own.iter().position(same).and_then(|index| own.get_mut(index)) {
            Some(existing) => *existing = entry,
            None => own.push(entry),
        }
    }
    // Slots stay in table order whatever order the project wrote them in; named entries follow them.
    own.sort_by_key(|entry| entry.table_id().map_or((1, 0), |id| (0, id)));
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawList {
    #[serde(default)]
    tilesets: Vec<RawTileset>,
    #[serde(default)]
    backgrounds: Vec<RawLayer>,
    #[serde(default)]
    foregrounds: Vec<RawLayer>,
    #[serde(default)]
    scenes: Vec<RawScene>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTileset {
    id: Option<u16>,
    name: String,
    graphics: Option<String>,
    palette: Option<String>,
    map16: Option<String>,
    objects: Option<String>,
    object_index: Option<String>,
    behaviors: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLayer {
    id: Option<u16>,
    name: String,
    graphics: Option<String>,
    palette: Option<String>,
    screen: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFiles {
    graphics: String,
    palette: String,
    screen: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScene {
    name: String,
    background: Option<RawFiles>,
    tileset: Option<RawFiles>,
    foreground: Option<RawFiles>,
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

/// `path`, when it is a path inside the ROM.
fn rom_path(path: String) -> Result<String> {
    let bad = path.contains(['\\', '\0']) || path.split('/').any(|part| part.is_empty() || part == "." || part == "..");
    if bad { Err(parse_error(format!("{path:?} is not a path inside the ROM"))) } else { Ok(path) }
}

fn named(name: String, what: &str) -> Result<String> {
    if name.trim().is_empty() { Err(parse_error(format!("a {what} has no name"))) } else { Ok(name) }
}

/// The source an entry with `id` and `files` has: a slot, or its files, never both or neither.
fn source<F>(id: Option<u16>, files: Option<Result<F>>, name: &str) -> Result<EntrySource<F>> {
    match (id, files) {
        (Some(id), None) if id < TABLE_SLOTS => Ok(EntrySource::Table(id)),
        (Some(id), None) => Err(parse_error(format!("{name:?}: the game's tables stop at {}, not {id}", TABLE_SLOTS - 1))),
        (None, Some(files)) => files.map(EntrySource::Files),
        (Some(_), Some(_)) => Err(parse_error(format!("{name:?} names a slot and files; a slot's files are the ones its tables say"))),
        (None, None) => Err(parse_error(format!("{name:?} names neither a slot nor its files"))),
    }
}

/// Every one of `parts` given, or none: `None` for none, an error naming the first missing for some.
fn all_or_none<const N: usize>(parts: [(&str, Option<String>); N], name: &str) -> Option<Result<[String; N]>> {
    if parts.iter().all(|(_, part)| part.is_none()) {
        return None;
    }
    if let Some((field, _)) = parts.iter().find(|(_, part)| part.is_none()) {
        return Some(Err(parse_error(format!("{name:?} names files but not its {field}"))));
    }
    Some(parts.map(|(_, part)| part.unwrap_or_default()).into_iter().map(rom_path).collect::<Result<Vec<_>>>().map(|paths| {
        let mut paths = paths.into_iter();
        std::array::from_fn(|_| paths.next().unwrap_or_default())
    }))
}

fn tileset(raw: RawTileset) -> Result<CatalogEntry<TilesetFiles>> {
    let name = named(raw.name, "tileset")?;
    let parts =
        [("graphics", raw.graphics), ("palette", raw.palette), ("map16", raw.map16), ("objects", raw.objects), ("object_index", raw.object_index)];
    let Some(files) = all_or_none(parts, &name) else {
        if raw.behaviors.is_some() {
            return Err(parse_error(format!("{name:?} names behaviours but not its other files")));
        }
        return Ok(CatalogEntry { source: source(raw.id, None, &name)?, name });
    };
    let files = files.and_then(|[graphics, palette, map16, objects, object_index]| {
        Ok(TilesetFiles { graphics, palette, map16, objects, object_index, behaviors: raw.behaviors.map(rom_path).transpose()? })
    });
    let files = Some(files);
    Ok(CatalogEntry { source: source(raw.id, files, &name)?, name })
}

fn layer_files(raw: RawFiles, name: &str) -> Result<LayerFiles> {
    let parts = [("graphics", Some(raw.graphics)), ("palette", Some(raw.palette)), ("screen", Some(raw.screen))];
    let [graphics, palette, screen] = all_or_none(parts, name).unwrap_or_else(|| Err(parse_error("a layer names no files")))?;
    Ok(LayerFiles { graphics, palette, screen })
}

fn layer(raw: RawLayer, what: &str) -> Result<CatalogEntry<LayerFiles>> {
    let name = named(raw.name, what)?;
    let parts = [("graphics", raw.graphics), ("palette", raw.palette), ("screen", raw.screen)];
    let files = all_or_none(parts, &name).map(|paths| paths.map(|[graphics, palette, screen]| LayerFiles { graphics, palette, screen }));
    Ok(CatalogEntry { source: source(raw.id, files, &name)?, name })
}

fn scene(raw: RawScene) -> Result<SceneEntry> {
    let name = named(raw.name, "scene")?;
    let part = |files: Option<RawFiles>| files.map(|files| layer_files(files, &name)).transpose();
    let scene = SceneEntry { background: part(raw.background)?, tileset: part(raw.tileset)?, foreground: part(raw.foreground)?, name: name.clone() };
    if scene.background.is_none() && scene.tileset.is_none() && scene.foreground.is_none() {
        return Err(parse_error(format!("scene {name:?} has no layers")));
    }
    Ok(scene)
}

/// An error when two of `entries` are the same slot, or the same named entry.
fn unique<F>(entries: &[CatalogEntry<F>], what: &str) -> Result<()> {
    for (index, entry) in entries.iter().enumerate() {
        let twice = entries.iter().take(index).any(|earlier| match (&earlier.source, &entry.source) {
            (EntrySource::Table(a), EntrySource::Table(b)) => a == b,
            (EntrySource::Files(_), EntrySource::Files(_)) => earlier.name == entry.name,
            _ => false,
        });
        if twice {
            return Err(parse_error(match entry.table_id() {
                Some(id) => format!("{what} {id} is listed twice"),
                None => format!("{what} {:?} is listed twice", entry.name),
            }));
        }
    }
    Ok(())
}

/// A tileset list as `tilesets.yaml` writes it.
pub fn parse_tileset_catalog(yaml: &str) -> Result<TilesetCatalog> {
    let raw: RawList = serde_norway::from_str(yaml).map_err(|e| parse_error(e.to_string()))?;
    let catalog = TilesetCatalog {
        tilesets: raw.tilesets.into_iter().map(tileset).collect::<Result<_>>()?,
        backgrounds: raw.backgrounds.into_iter().map(|raw| layer(raw, "background")).collect::<Result<_>>()?,
        foregrounds: raw.foregrounds.into_iter().map(|raw| layer(raw, "foreground")).collect::<Result<_>>()?,
        scenes: raw.scenes.into_iter().map(scene).collect::<Result<_>>()?,
    };
    unique(&catalog.tilesets, "tileset")?;
    unique(&catalog.backgrounds, "background")?;
    unique(&catalog.foregrounds, "foreground")?;
    if let Some((_, scene)) =
        catalog.scenes.iter().enumerate().find(|(index, scene)| catalog.scenes.iter().take(*index).any(|s| s.name == scene.name))
    {
        return Err(parse_error(format!("scene {:?} is listed twice", scene.name)));
    }
    Ok(catalog)
}

/// The list the editor ships. One serves every release, as it names slots and paths rather than file IDs.
pub fn builtin_tileset_catalog() -> &'static TilesetCatalog {
    static LIST: OnceLock<TilesetCatalog> = OnceLock::new();
    LIST.get_or_init(|| parse_tileset_catalog(TILESETS).unwrap_or_default())
}
