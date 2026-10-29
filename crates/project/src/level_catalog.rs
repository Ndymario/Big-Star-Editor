//! The level list: groups of levels, each level's areas by their ROM path. The editor ships one for each ROM layout
//! (`levels/levels.yaml`, and `levels/levels_a2de.yaml` for the American ROM, the only one without a third W6 Castle
//! area); a project's own `levels.yaml` takes their place.

use std::sync::OnceLock;

use bigstar_core::{Error, ErrorCode, Result};
use serde::Deserialize;

/// The name a project's own list goes by, in the project's root.
pub const PROJECT_LEVEL_LIST: &str = "levels.yaml";

const LEVELS: &str = include_str!("../levels/levels.yaml");
const LEVELS_A2DE: &str = include_str!("../levels/levels_a2de.yaml");

/// Where an area's two files are, by their shared path without `.bin`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AreaLocation {
    /// A plain file of the ROM, `course/A01_1`.
    File(String),
    /// A member of an archive in it: `ARCHIVE/Dat_Ice.narc` and `Dat/Ice/J03_1`.
    Member { archive: String, member: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelCatalogArea {
    /// The path as the list writes it, which is also the area's token.
    pub path: String,
    pub display_name: String,
    pub location: AreaLocation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelCatalogLevel {
    pub display_name: String,
    pub areas: Vec<LevelCatalogArea>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelCatalogGroup {
    pub name: String,
    pub groups: Vec<LevelCatalogGroup>,
    pub levels: Vec<LevelCatalogLevel>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LevelCatalog {
    /// The unnamed group the list's top level makes.
    pub root: LevelCatalogGroup,
}

impl LevelCatalog {
    /// Every area with its level, in list order.
    pub fn areas(&self) -> Vec<(&LevelCatalogLevel, &LevelCatalogArea)> {
        fn walk<'a>(group: &'a LevelCatalogGroup, out: &mut Vec<(&'a LevelCatalogLevel, &'a LevelCatalogArea)>) {
            for child in &group.groups {
                walk(child, out);
            }
            for level in &group.levels {
                out.extend(level.areas.iter().map(|area| (level, area)));
            }
        }
        let mut out = Vec::new();
        walk(&self.root, &mut out);
        out
    }

    /// The area the list names `path`, with its level.
    pub fn find(&self, path: &str) -> Option<(&LevelCatalogLevel, &LevelCatalogArea)> {
        self.areas().into_iter().find(|(_, area)| area.path == path)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawList {
    #[serde(default)]
    levels: Vec<RawItem>,
}

/// `- group:` or `- level:`, a map of one key.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawItem {
    group: Option<RawGroup>,
    level: Option<RawLevel>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGroup {
    name: String,
    #[serde(default)]
    groups: Vec<RawItem>,
    #[serde(default)]
    levels: Vec<RawItem>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLevel {
    name: String,
    areas: Vec<String>,
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

/// Where `path` points: past the first archive it names, a member of that archive.
fn location(path: &str) -> Result<AreaLocation> {
    let parts: Vec<&str> = path.split('/').collect();
    if path.contains(['\\', '\0']) || parts.iter().any(|part| part.is_empty() || *part == "." || *part == "..") {
        return Err(parse_error(format!("{path:?} is not a path inside the ROM")));
    }
    let archive = parts.iter().position(|part| {
        let part = part.to_ascii_lowercase();
        part.ends_with(".narc") || part.ends_with(".carc")
    });
    let Some(index) = archive else { return Ok(AreaLocation::File(path.to_owned())) };
    match parts.split_at(index + 1) {
        (_, []) => Err(parse_error(format!("{path:?} names an archive, not an area inside it"))),
        (archive, member) => Ok(AreaLocation::Member { archive: archive.join("/"), member: member.join("/") }),
    }
}

fn level(raw: RawLevel) -> Result<LevelCatalogLevel> {
    if raw.areas.is_empty() {
        return Err(parse_error(format!("level {:?} has no areas", raw.name)));
    }
    let count = raw.areas.len();
    let areas = raw
        .areas
        .into_iter()
        .enumerate()
        .map(|(i, path)| {
            let display_name = if count == 1 { raw.name.clone() } else { format!("Area {}", i + 1) };
            Ok(LevelCatalogArea { location: location(&path)?, path, display_name })
        })
        .collect::<Result<_>>()?;
    Ok(LevelCatalogLevel { display_name: raw.name, areas })
}

/// `items` sorted into `group`, each list keeping its order.
fn fill(group: &mut LevelCatalogGroup, items: Vec<RawItem>) -> Result<()> {
    for item in items {
        match (item.group, item.level) {
            (Some(raw), None) => {
                if raw.name.trim().is_empty() {
                    return Err(parse_error("a group has no name"));
                }
                let mut child = LevelCatalogGroup { name: raw.name, ..LevelCatalogGroup::default() };
                fill(&mut child, raw.groups)?;
                fill(&mut child, raw.levels)?;
                group.groups.push(child);
            }
            (None, Some(raw)) => group.levels.push(level(raw)?),
            _ => return Err(parse_error("each item is either a group or a level")),
        }
    }
    Ok(())
}

/// A level list as `levels.yaml` writes it.
pub fn parse_level_catalog(yaml: &str) -> Result<LevelCatalog> {
    let raw: RawList = serde_norway::from_str(yaml).map_err(|e| parse_error(e.to_string()))?;
    let mut catalog = LevelCatalog::default();
    fill(&mut catalog.root, raw.levels)?;
    Ok(catalog)
}

/// The list the editor ships for a ROM of `game_code`; the American ROM lacks an area every other one has, its own kiosk
/// demo included.
pub fn builtin_level_catalog(game_code: Option<&str>) -> &'static LevelCatalog {
    static LIST: OnceLock<LevelCatalog> = OnceLock::new();
    static LIST_A2DE: OnceLock<LevelCatalog> = OnceLock::new();
    let (cell, yaml) = if game_code == Some("A2DE") { (&LIST_A2DE, LEVELS_A2DE) } else { (&LIST, LEVELS) };
    cell.get_or_init(|| parse_level_catalog(yaml).unwrap_or_default())
}
