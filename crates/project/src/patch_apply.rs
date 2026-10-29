//! Applying an NCPatcher build to a project without ever leaving it broken. Port of
//! `libs/nsmb-project/src/patch_apply.cpp`.
//!
//! The project directory is also the NCPatcher project. A build runs with `--out` to a staging path, the ROM it
//! produced is validated there, and only a ROM that parses is moved to the `rom.output` the project configures. A
//! failed build, or one that writes something unreadable, leaves the project exactly as it was.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result, read_file_prefix};
use bigstar_nds::{DS_CARTRIDGE_HEADER_SIZE, parse_rom_header};
use bigstar_patching::{BuildObserver, BuildRequest, NCPatcherClient, ProcessCancel, ProjectConfigKind, RunSummary, detect_project_config};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct PatchApplyRequest {
    /// The project directory, which is also the NCPatcher project directory.
    pub project_dir: PathBuf,
    /// Which configured build variant to produce. Empty means the project configures none.
    pub variant: String,
    pub ncpatcher_executable: PathBuf,
    /// The clean ROM NCPatcher builds from, which `ncpatcher.yaml` does not name.
    pub clean_rom: PathBuf,
    /// Where the built ROM goes: the `rom.output` NCPatcher reports for the project.
    pub output: PathBuf,
    pub verbose: bool,
    /// Passed straight to the build; the apply itself only moves files that are already written.
    pub cancel: Option<Arc<ProcessCancel>>,
}

impl Default for PatchApplyRequest {
    fn default() -> Self {
        Self {
            project_dir: PathBuf::new(),
            variant: String::new(),
            ncpatcher_executable: PathBuf::from("ncpatcher"),
            clean_rom: PathBuf::new(),
            output: PathBuf::new(),
            verbose: false,
            cancel: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PatchApplyOutcome {
    /// NCPatcher reported a failure. Nothing was touched.
    #[default]
    BuildFailed,
    /// The build succeeded but its output could not be validated. Nothing was touched.
    OutputRejected,
    /// The built ROM is in place.
    Applied,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PatchApplyResult {
    pub outcome: PatchApplyOutcome,
    pub summary: RunSummary,
    /// The ROM the build produced, in its final place on success.
    pub built_rom: PathBuf,
    /// Set when the outcome is `OutputRejected`, saying what about the output was wrong.
    pub rejection: String,
}

/// How many backups of each kind a sweep leaves behind.
pub const DEFAULT_BACKUPS_KEPT: usize = 10;

/// The one directory the editor keeps its own files in: in a project, or beside a ROM edited directly.
pub const STATE_DIR: &str = ".bigstar";

pub fn patch_staging_dir(project_dir: &Path) -> PathBuf {
    project_dir.join(STATE_DIR).join("staging")
}

pub fn patch_backup_root(project_dir: &Path) -> PathBuf {
    project_dir.join(STATE_DIR).join("backups")
}

/// Removing a staging directory is best effort: failing to clean up is not a reason to refuse a build that otherwise
/// worked.
fn remove_quietly(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

/// `rename` fails across devices; falling back to copy-then-remove keeps the move working without giving up the
/// atomic path where it exists.
fn move_file(from: &Path, to: &Path) -> Result<()> {
    let _ = std::fs::remove_file(to);
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    std::fs::copy(from, to).map_err(|e| Error::new(ErrorCode::IoError, format!("could not move {} to {}: {e}", from.display(), to.display())))?;
    let _ = std::fs::remove_file(from);
    Ok(())
}

/// Deletes all but the `keep` most recent backups of each kind, and answers how many it removed. A backup is named
/// for what made it and then for when, and a timestamp that sorts lexically sorts chronologically, so the newest few
/// of a kind are simply the largest names beginning with it. Per kind rather than overall: one afternoon of level
/// saves must not sweep away the morning's patch import.
pub fn prune_patch_backups(project_dir: &Path, keep: usize) -> usize {
    let root = patch_backup_root(project_dir);
    let Ok(entries) = std::fs::read_dir(&root) else { return 0 };
    let mut kinds: BTreeMap<String, Vec<PathBuf>> = BTreeMap::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let kind = name.split('-').next().unwrap_or_default().to_owned();
        kinds.entry(kind).or_default().push(entry.path());
    }
    let mut removed = 0;
    for entries in kinds.values_mut() {
        if entries.len() <= keep {
            continue;
        }
        entries.sort_by(|a, b| b.cmp(a));
        for entry in entries.iter().skip(keep) {
            if std::fs::remove_dir_all(entry).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

/// Runs the build and, if the ROM it produced checks out, moves it into place.
pub fn apply_patch_transactionally(request: &PatchApplyRequest, observer: &mut BuildObserver) -> Result<PatchApplyResult> {
    if !request.project_dir.is_dir() {
        return Err(Error::new(ErrorCode::NotFound, format!("project directory does not exist: {}", request.project_dir.display())));
    }
    match detect_project_config(&request.project_dir).kind {
        ProjectConfigKind::None => return Err(Error::new(ErrorCode::NotFound, format!("no ncpatcher.yaml in {}", request.project_dir.display()))),
        ProjectConfigKind::V1Json => {
            return Err(Error::new(
                ErrorCode::Unsupported,
                "this project still uses the legacy ncpatcher.json; convert it with NCPatcher's migrate step first",
            ));
        }
        ProjectConfigKind::V2Yaml => {}
    }
    // NCPatcher with no output patches its ROM in place, and the ROM it is handed is the clean one.
    if request.output.as_os_str().is_empty() {
        return Err(Error::new(
            ErrorCode::InvalidArgument,
            "ncpatcher.yaml sets no rom.output, so a build would have nowhere to go but over the clean ROM",
        ));
    }

    let staging = patch_staging_dir(&request.project_dir);
    // A staging directory left behind by an interrupted run is stale by definition, and reusing it would apply a mix
    // of two builds.
    remove_quietly(&staging);
    std::fs::create_dir_all(&staging)
        .map_err(|_| Error::new(ErrorCode::IoError, format!("could not create the staging directory {}", staging.display())))?;

    let staged_rom = staging.join("patched.nds");
    let build = BuildRequest {
        project_dir: request.project_dir.clone(),
        variant: request.variant.clone(),
        rom_override: request.clean_rom.clone(),
        output_override: staged_rom.clone(),
        verbose: request.verbose,
        cancel: request.cancel.clone(),
    };
    let summary = match NCPatcherClient::new(&request.ncpatcher_executable).build(&build, observer) {
        Ok(summary) => summary,
        Err(error) => {
            remove_quietly(&staging);
            return Err(error);
        }
    };
    let mut result = PatchApplyResult { summary, ..PatchApplyResult::default() };
    if !result.summary.succeeded() {
        // NCPatcher said no. The project has not been touched, and the staging directory holds nothing worth keeping.
        result.outcome = PatchApplyOutcome::BuildFailed;
        remove_quietly(&staging);
        return Ok(result);
    }

    // From here the build claims to have worked. What is left is checking that what it produced can actually be used,
    // before the ROM the editor reads is replaced with it.
    let reject = |mut result: PatchApplyResult, why: String| {
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = why;
        remove_quietly(&staging);
        Ok(result)
    };
    if !staged_rom.is_file() {
        return reject(result, format!("NCPatcher reported success but wrote no ROM at {}", staged_rom.display()));
    }
    let header = match read_file_prefix(&staged_rom, DS_CARTRIDGE_HEADER_SIZE) {
        Ok(header) => header,
        Err(e) => return reject(result, format!("the built ROM could not be read: {}", e.message)),
    };
    if let Err(e) = parse_rom_header(&header) {
        return reject(result, format!("the built ROM does not parse as an NDS ROM: {}", e.message));
    }

    // The ROM is sound, so it becomes the one the editor reads: one file arriving, nothing unpacked, nothing moved
    // aside. NCPatcher rebuilds the previous build from the pristine binaries it keeps.
    let final_rom = request.output.clone();
    if let Some(parent) = final_rom.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = move_file(&staged_rom, &final_rom) {
        return reject(result, format!("the built ROM was fine, but could not be moved into place: {}", e.message));
    }
    remove_quietly(&staging);
    result.built_rom = final_rom;
    result.outcome = PatchApplyOutcome::Applied;
    Ok(result)
}
