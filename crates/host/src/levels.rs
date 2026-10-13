//! Where a catalog area's two files live in a ROM, by the addresses the effective filesystem and module targets use.

use std::collections::HashMap;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_project::{LevelCatalogArea, RomReader};

/// One file of a level: a plain file of the ROM, or a member of an archive in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Source {
    File(String),
    Member { archive: String, member: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LevelFiles {
    pub course: Source,
    pub bgdat: Source,
}

/// The ROM's file names, so an area's files are found by name the way the Qt editor finds them.
pub(crate) struct RomIndex {
    /// Every path, sorted.
    sorted: Vec<String>,
    /// File name to the first path that bears it.
    by_name: HashMap<String, String>,
}

impl RomIndex {
    pub fn new_empty() -> Self {
        Self { sorted: Vec::new(), by_name: HashMap::new() }
    }

    pub fn new(rom: &RomReader) -> Self {
        let mut sorted: Vec<String> = rom.files().iter().map(|file| file.path.clone()).collect();
        sorted.sort();
        let mut by_name = HashMap::new();
        for path in &sorted {
            let name = path.rsplit('/').next().unwrap_or(path);
            by_name.entry(name.to_owned()).or_insert_with(|| path.clone());
        }
        Self { sorted, by_name }
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The area's course and background files. A download-play area lives in the archive its token names and nowhere else.
pub(crate) fn locate(rom: &mut RomReader, index: &RomIndex, area: &LevelCatalogArea) -> Option<LevelFiles> {
    let course_name = format!("{}.bin", area.course_token);
    let bgdat_name = format!("{}_bgdat.bin", area.course_token);
    let Some(narc) = &area.narc_name else {
        return Some(LevelFiles {
            course: Source::File(index.by_name.get(&course_name)?.clone()),
            bgdat: Source::File(index.by_name.get(&bgdat_name)?.clone()),
        });
    };

    let wanted = narc.to_lowercase();
    for archive in index.sorted.iter().filter(|path| path.to_lowercase().contains(&wanted)) {
        if !rom.is_archive(archive) {
            continue;
        }
        let Ok(members) = rom.list_archive(archive) else { continue };
        let find = |name: &str| members.iter().find(|member| file_name(&member.path) == name).map(|member| member.path.clone());
        if let (Some(course), Some(bgdat)) = (find(&course_name), find(&bgdat_name)) {
            return Some(LevelFiles {
                course: Source::Member { archive: archive.clone(), member: course },
                bgdat: Source::Member { archive: archive.clone(), member: bgdat },
            });
        }
    }
    None
}

fn unsafe_name(name: &str) -> bool {
    name.starts_with('/') || name.contains('\\') || name.contains('\0') || name.split('/').any(|part| part == "..")
}

/// A level file's address, as the filesystem view, the effective filesystem and module targets name it: a plain path,
/// or an archive and its member joined by `!`.
pub(crate) fn address(source: &Source) -> Result<String> {
    let refuse = || Error::new(ErrorCode::InvalidArgument, "the ROM names a file outside the project");
    match source {
        Source::File(path) if !unsafe_name(path) => Ok(path.clone()),
        Source::Member { archive, member } if !unsafe_name(archive) && !unsafe_name(member) => Ok(format!("{archive}!{member}")),
        _ => Err(refuse()),
    }
}
