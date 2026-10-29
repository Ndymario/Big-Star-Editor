//! Where a catalog area's two files live in a ROM, by the addresses the effective filesystem and module targets use.

use std::collections::BTreeSet;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_patching::FileManifest;
use bigstar_project::{AreaLocation, LevelCatalog, LevelCatalogArea, RomReader};

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

/// The ROM's file names, sorted, so an area's files are found by path.
pub(crate) struct RomIndex {
    sorted: Vec<String>,
}

impl RomIndex {
    pub fn new_empty() -> Self {
        Self { sorted: Vec::new() }
    }

    pub fn new(rom: &RomReader) -> Self {
        let mut sorted: Vec<String> = rom.files().iter().map(|file| file.path.clone()).collect();
        sorted.sort();
        Self { sorted }
    }

    fn contains(&self, path: &str) -> bool {
        self.sorted.binary_search_by(|probe| probe.as_str().cmp(path)).is_ok()
    }
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The area's course and background files, where the list says they are. A plain file the plan adds counts before a
/// build places it, since reads take the module's copy first; an archive's members are the ROM's.
pub(crate) fn locate(rom: Option<&mut RomReader>, index: &RomIndex, plan: Option<&FileManifest>, area: &LevelCatalogArea) -> Option<LevelFiles> {
    match &area.location {
        AreaLocation::File(base) => {
            let (course, bgdat) = (format!("{base}.bin"), format!("{base}_bgdat.bin"));
            let exists = |path: &str| index.contains(path) || plan.is_some_and(|plan| plan.find(path).is_some());
            let found = exists(&course) && exists(&bgdat);
            found.then_some(LevelFiles { course: Source::File(course), bgdat: Source::File(bgdat) })
        }
        AreaLocation::Member { archive, member } => {
            let rom = rom?;
            if !index.contains(archive) || !rom.is_archive(archive) {
                return None;
            }
            let members = rom.list_archive(archive).ok()?;
            let (course, bgdat) = (format!("{member}.bin"), format!("{member}_bgdat.bin"));
            let holds = |name: &str| members.iter().any(|m| m.path == name);
            (holds(&course) && holds(&bgdat)).then(|| LevelFiles {
                course: Source::Member { archive: archive.clone(), member: course },
                bgdat: Source::Member { archive: archive.clone(), member: bgdat },
            })
        }
    }
}

fn unsafe_name(name: &str) -> bool {
    name.starts_with('/') || name.contains('\\') || name.contains('\0') || name.split('/').any(|part| part == "..")
}

/// A level the list does not name: a module's own, found by its `_bgdat.bin` beside a course file of the same name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomLevel {
    /// The two files' shared path without its suffix, `z_new/coop/W2UnlockCutscene`, as the list would name it.
    pub token: String,
    /// The module the plan says supplies the background file; empty when nothing says.
    pub module: String,
    /// The level's file name, `W2UnlockCutscene`.
    pub name: String,
    pub files: LevelFiles,
}

impl CustomLevel {
    /// "W2UnlockCutscene (coop)".
    pub fn title(&self) -> String {
        if self.module.is_empty() { self.name.clone() } else { format!("{} ({})", self.name, self.module) }
    }
}

/// Every level the ROM or the plan holds besides the list's, by module and then name. A file the plan adds is listed
/// before a build places it, since reads take the module's copy first.
pub(crate) fn custom_levels(index: &RomIndex, plan: Option<&FileManifest>, catalog: &LevelCatalog) -> Vec<CustomLevel> {
    let listed: BTreeSet<&str> = catalog.areas().into_iter().map(|(_, area)| area.path.as_str()).collect();
    let paths: BTreeSet<&str> =
        index.sorted.iter().map(String::as_str).chain(plan.iter().flat_map(|plan| &plan.files).map(|file| file.path.as_str())).collect();
    let mut levels: Vec<CustomLevel> = paths
        .iter()
        .filter_map(|path| {
            let token = path.strip_suffix("_bgdat.bin")?;
            let name = file_name(token);
            let course = format!("{token}.bin");
            if name.is_empty() || listed.contains(token) || !paths.contains(course.as_str()) {
                return None;
            }
            let module = plan.and_then(|plan| plan.find(path)).map(|file| file.module.clone()).unwrap_or_default();
            Some(CustomLevel {
                token: token.to_owned(),
                module,
                name: name.to_owned(),
                files: LevelFiles { course: Source::File(course), bgdat: Source::File((*path).to_owned()) },
            })
        })
        .collect();
    levels.sort_by(|a, b| (&a.module, &a.name, &a.token).cmp(&(&b.module, &b.name, &b.token)));
    levels
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
