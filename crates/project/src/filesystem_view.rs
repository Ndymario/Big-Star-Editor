//! The Filesystem dock's model: a join of NCPatcher's plan, the last build's manifest and the ROM as it stands, plus
//! `module_file_target` for writing an edit back. Port of `libs/nsmb-project/src/filesystem_view.cpp`.
//!
//! Nothing here is resolved by this crate: layering, module order, `into:`, the archive-folder convention and
//! component subtraction all happen in NCPatcher, once.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_patching::{FileAction, FileManifest, ModuleGraph, ModuleInfo, RomArchiveMember, RomFileEntry};
use serde::{Deserialize, Serialize};

use crate::RomReader;
use crate::effective_filesystem::source_path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FilesystemStatus {
    #[default]
    Vanilla,
    Replaced,
    Added,
    Pending,
    Orphaned,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleVariantMapping {
    pub module: String,
    pub layer: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemEntry {
    pub path: String,
    pub status: FilesystemStatus,
    pub module: String,
    pub component: String,
    pub variant: String,
    pub id: Option<i64>,
    /// The id came from a plan rather than from the ROM, so it is what the next build would assign and not what the
    /// game can address today. Only a file the ROM does not hold yet can carry one.
    pub id_is_planned: bool,
    /// The member's file id inside its container, set only on a row whose path carries a `!`. Never both this and
    /// `id`: an archive numbers its members in its own name table.
    pub member_id: Option<i64>,
    pub size: u64,
    /// The module file supplying this entry, empty when the ROM does.
    pub source: PathBuf,
}

impl FilesystemEntry {
    pub fn inside_archive(&self) -> bool {
        self.member_id.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemView {
    pub entries: Vec<FilesystemEntry>,
    pub watched_directories: Vec<PathBuf>,
}

impl FilesystemView {
    pub fn has_pending(&self) -> bool {
        self.entries.iter().any(|entry| matches!(entry.status, FilesystemStatus::Pending | FilesystemStatus::Orphaned))
    }

    pub fn find(&self, path: &str) -> Option<&FilesystemEntry> {
        self.entries.iter().find(|entry| entry.path == path)
    }
}

/// What the view is a join of: the plan is NCPatcher's answer to "what would a build place", the manifest is the
/// last build's own record of what it did place, and the ROM is what the game holds now.
#[derive(Default)]
pub struct FilesystemViewRequest<'a> {
    /// The built ROM. Absent before a first build, which leaves the view with only what the plan and the last
    /// manifest say.
    pub rom: Option<&'a mut RomReader>,
    pub project_dir: PathBuf,
    /// Only for the directories to watch. The view never walks a module tree to decide what it produces.
    pub modules: ModuleGraph,
    /// `ncpatcher files plan`. Absent when NCPatcher could not be asked, which leaves the view reporting the last
    /// build rather than guessing past it.
    pub plan: Option<FileManifest>,
    /// The last build's `files-dump`. Absent before a first build.
    pub manifest: Option<FileManifest>,
    pub variant: String,
}

fn safe_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with('/') || path.ends_with('/') || path.contains('\\') {
        return false;
    }
    path.split('/').all(|part| !part.is_empty() && part != "." && part != ".." && part.len() <= 0x7f)
}

/// Does the module's copy of a file already match what the ROM holds? The FAT answers the size for nothing, and a
/// size that disagrees settles it without reading either.
fn source_matches_rom(source: &Path, rom: &mut RomReader, path: &str) -> bool {
    let Some(id) = rom.file_id(path) else { return false };
    let Some(size) = rom.file_size(id) else { return false };
    if !source.metadata().is_ok_and(|meta| meta.is_file() && meta.len() == u64::from(size)) {
        return false;
    }
    match (rom.read_id(id), read_file(source)) {
        (Ok(stored), Ok(edited)) => stored == edited,
        _ => false,
    }
}

/// Does the container the ROM holds already carry this member? Addressed by the file id the archive's own name table
/// gives it; a member path is a label, not an address.
fn member_matches_rom(rom: &mut RomReader, container: &str, project_dir: &Path, member: &RomArchiveMember) -> bool {
    // A hook writes this one, so there is nothing on disk to compare and nothing the user can do about it either way.
    if member.source_missing {
        return true;
    }
    let Ok(id) = usize::try_from(member.id) else { return false };
    match (rom.read_member_id(container, id), read_file(&source_path(project_dir, &member.source))) {
        (Ok(stored), Ok(expected)) => stored == expected,
        _ => false,
    }
}

fn same_member(built: &RomArchiveMember, planned: &RomArchiveMember) -> bool {
    built.path == planned.path && built.module == planned.module && built.component == planned.component && built.from_variant == planned.from_variant
}

/// The set of members a run wrote is part of what it did. A build that replaced three members and a plan that would
/// replace two disagree even if those two match byte for byte.
fn same_members(built: &[RomArchiveMember], planned: &[RomArchiveMember]) -> bool {
    built.len() == planned.len() && built.iter().zip(planned).all(|(a, b)| a.id == b.id && same_member(a, b))
}

/// Where in a module's tree a ROM path is written, which is the one direction no build document answers. Archive
/// addresses (`a.narc!b/c`) become the archive-folder spelling (`a_narc/b/c`) NCPatcher sweeps back into the
/// container.
pub fn module_file_target(module: &ModuleInfo, variant: &str, rom_path: &str, mappings: &[ModuleVariantMapping]) -> Result<PathBuf> {
    let invalid = |message: String| Error::new(ErrorCode::InvalidArgument, message);
    let tree = match &module.nitrofs {
        Some(tree) if module.enabled => tree,
        _ => return Err(invalid("target module has no enabled NitroFS tree".to_owned())),
    };
    let mut relative = rom_path.to_owned();
    if let Some((archive, member)) = rom_path.split_once('!') {
        // `a/b.narc!c/d` is written as `a/b_narc/c/d`, and the same for any other extension a game gives its
        // archives. Writing is unambiguous, because the address already said so.
        let dot = archive.rfind('.');
        let slash = archive.rfind('/');
        let Some(dot) = dot.filter(|&dot| dot + 1 < archive.len() && slash.is_none_or(|slash| dot > slash)) else {
            return Err(invalid(format!("archive destination has no extension to spell as a folder: {archive}")));
        };
        relative = format!("{}_{}/{member}", archive.get(..dot).unwrap_or_default(), archive.get(dot + 1..).unwrap_or_default());
    }
    if !tree.into.is_empty() {
        let prefix = format!("{}/", tree.into);
        let Some(rest) = relative.strip_prefix(&prefix) else {
            return Err(invalid("ROM path is outside the module's into prefix".to_owned()));
        };
        relative = rest.to_owned();
    }
    if !safe_path(&relative) {
        return Err(invalid("ROM path is unsafe".to_owned()));
    }
    let mut root = tree.dir.clone();
    if tree.layered {
        let mapped = mappings.iter().find(|mapping| mapping.module == module.key || mapping.module == module.id);
        let layer = match mapped {
            Some(mapping) => mapping.layer.clone(),
            None if variant.is_empty() => tree.base_variant.clone(),
            None => variant.to_owned(),
        };
        if layer.is_empty() {
            return Err(invalid("a variant is required for this layered module".to_owned()));
        }
        root.push(layer);
        if mapped.is_some() && !root.is_dir() {
            return Err(Error::new(ErrorCode::NotFound, format!("mapped module layer does not exist: {}", root.display())));
        }
    }
    Ok(root.join(relative))
}

/// Every directory under `dir`, `dir` included. A symlinked directory is listed but not entered, as
/// `std::filesystem::recursive_directory_iterator` does by default.
fn collect_directories(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        out.push(path.clone());
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            collect_directories(&path, out);
        }
    }
}

fn supplied(plan: Option<&RomFileEntry>) -> Option<&RomFileEntry> {
    // A build writes a file when a module supplies it, and also when it creates one out of nothing: z_new/reserved
    // has no source at all.
    plan.filter(|plan| plan.has_provenance() || plan.action != FileAction::Unchanged)
}

pub fn build_filesystem_view(request: FilesystemViewRequest<'_>) -> Result<FilesystemView> {
    let FilesystemViewRequest { mut rom, project_dir, modules, plan: plan_document, manifest: manifest_document, .. } = request;
    let mut view = FilesystemView::default();
    // Three records of the same ROM, none of them derived here: what a build would place, what the last build did
    // place, and what is on disk now.
    let planned: BTreeMap<&str, &RomFileEntry> = plan_document.iter().flat_map(|doc| &doc.files).map(|file| (file.path.as_str(), file)).collect();
    let manifested: BTreeMap<&str, &RomFileEntry> =
        manifest_document.iter().flat_map(|doc| &doc.files).map(|file| (file.path.as_str(), file)).collect();
    let plan_is_planned = plan_document.as_ref().is_some_and(|doc| doc.planned);

    let mut paths: BTreeSet<String> = BTreeSet::new();
    if let Some(rom) = rom.as_deref() {
        paths.extend(rom.files().iter().map(|file| file.path.clone()));
    }
    paths.extend(manifested.keys().map(|path| (*path).to_owned()));
    paths.extend(planned.keys().map(|path| (*path).to_owned()));

    for path in &paths {
        let mut entry = FilesystemEntry { path: path.clone(), ..FilesystemEntry::default() };
        let manifest = manifested.get(path.as_str()).copied();
        let plan = planned.get(path.as_str()).copied();

        if let Some(manifest) = manifest {
            entry.id = Some(manifest.id);
            entry.size = manifest.size as u64;
            entry.module = manifest.module.clone();
            entry.component = manifest.component.clone();
            entry.variant = manifest.from_variant.clone();
        } else if let Some(plan) = plan {
            // Only a file the ROM does not hold yet can have an id the ROM cannot confirm, and only a plan can predict
            // one.
            entry.id = Some(plan.id);
            entry.size = plan.size as u64;
            entry.id_is_planned = plan_is_planned && plan.action == FileAction::Created;
        } else if let Some(rom) = rom.as_deref()
            && let Some(id) = rom.file_id(path)
        {
            // Neither document mentions it, so the ROM is the only record there is -- the state before a first build.
            entry.id = Some(i64::from(id));
            entry.size = u64::from(rom.file_size(id).unwrap_or(0));
        }

        let Some(plan) = supplied(plan) else {
            entry.status = if manifest.is_some_and(RomFileEntry::has_provenance) { FilesystemStatus::Orphaned } else { FilesystemStatus::Vanilla };
            view.entries.push(entry);
            continue;
        };

        entry.module = plan.module.clone();
        entry.component = plan.component.clone();
        entry.variant = plan.from_variant.clone();
        if !plan.source.is_empty() {
            entry.source = source_path(&project_dir, &plan.source);
        }
        if !plan.source_missing {
            entry.size = plan.size as u64;
        }

        let mut agrees = manifest.is_some_and(|manifest| {
            manifest.module == plan.module && manifest.component == plan.component && manifest.from_variant == plan.from_variant
        }) && rom.is_some();

        // An archive is not a file with one answer. Each member the plan would write is judged against the ROM's
        // copy at that id on its own, so the view can say which member is stale instead of only that one is.
        let mut member_rows = Vec::with_capacity(plan.members.len());
        for member in &plan.members {
            let mut row = FilesystemEntry {
                path: format!("{path}!{}", member.path),
                member_id: Some(member.id),
                module: member.module.clone(),
                component: member.component.clone(),
                variant: member.from_variant.clone(),
                ..FilesystemEntry::default()
            };
            if !member.source.is_empty() {
                row.source = source_path(&project_dir, &member.source);
            }
            if !member.source_missing {
                row.size = member.size as u64;
            }
            let built = manifest.and_then(|manifest| manifest.members.iter().find(|built| built.id == member.id));
            let written = built.is_some_and(|built| same_member(built, member))
                && rom.as_deref_mut().is_some_and(|rom| member_matches_rom(rom, path, &project_dir, member));
            row.status = if written { FilesystemStatus::Replaced } else { FilesystemStatus::Pending };
            if !written {
                agrees = false;
            }
            member_rows.push(row);
        }
        if agrees && !plan.members.is_empty() {
            agrees = manifest.is_some_and(|manifest| same_members(&manifest.members, &plan.members));
        } else if agrees && plan.members.is_empty() && !entry.source.as_os_str().is_empty() && !plan.source_missing {
            // A source a pre-build hook writes is not on disk to be compared, so there is no honest way to call it
            // stale. Reporting the last build is better than a Pending mark nothing the user does can clear.
            agrees = rom.as_deref_mut().is_some_and(|rom| source_matches_rom(&entry.source, rom, path));
        }

        entry.status = if !agrees {
            FilesystemStatus::Pending
        } else if path.starts_with("z_new/") || manifest.is_some_and(|manifest| manifest.action == FileAction::Created) {
            FilesystemStatus::Added
        } else {
            FilesystemStatus::Replaced
        };
        view.entries.push(entry);
        view.entries.extend(member_rows);
    }

    for module in &modules.modules {
        if let Some(tree) = module.nitrofs.as_ref().filter(|_| module.enabled) {
            view.watched_directories.push(tree.dir.clone());
            collect_directories(&tree.dir, &mut view.watched_directories);
        }
    }
    view.watched_directories.sort();
    view.watched_directories.dedup();
    Ok(view)
}

pub fn filesystem_status_name(status: FilesystemStatus) -> &'static str {
    match status {
        FilesystemStatus::Vanilla => "Vanilla",
        FilesystemStatus::Replaced => "Replaced",
        FilesystemStatus::Added => "Added",
        FilesystemStatus::Pending => "Pending",
        FilesystemStatus::Orphaned => "Orphaned",
    }
}
