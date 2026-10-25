//! The editor's per-project choices, kept in the project directory so every frontend of the host sees the same ones.

use std::path::{Path, PathBuf};

use bigstar_core::{Error, ErrorCode, Result, write_text_file_atomically};
use bigstar_protocol::ProjectSettings;
use serde::{Deserialize, Serialize};

pub(crate) const SETTINGS_FILE_NAME: &str = ".big-star-workspace.json";

/// What is written: the choices, not what NCPatcher reported, which is asked again each time.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Stored {
    #[serde(default)]
    active_variant: String,
    #[serde(default)]
    active_module: String,
    #[serde(default)]
    generated_path: String,
    /// The clean ROM NCPatcher builds from. Kept here rather than in `ncpatcher.yaml`, which is the same on every
    /// machine while where the ROM lies is not.
    #[serde(default)]
    clean_rom: String,
    #[serde(default = "default_autosave_minutes")]
    autosave_minutes: u32,
    #[serde(default = "default_backups_kept")]
    backups_kept: u32,
}

pub(crate) const DEFAULT_AUTOSAVE_MINUTES: u32 = 1;

fn default_autosave_minutes() -> u32 {
    DEFAULT_AUTOSAVE_MINUTES
}

fn default_backups_kept() -> u32 {
    bigstar_project::DEFAULT_BACKUPS_KEPT as u32
}

fn path(directory: &Path) -> PathBuf {
    directory.join(SETTINGS_FILE_NAME)
}

/// A missing or unreadable file is the defaults: the choices are conveniences, never a reason to refuse a project.
pub(crate) fn load_settings(directory: &Path) -> ProjectSettings {
    let stored: Option<Stored> = std::fs::read_to_string(path(directory)).ok().and_then(|text| serde_json::from_str(&text).ok());
    let stored = stored.unwrap_or_else(|| Stored {
        active_variant: String::new(),
        active_module: String::new(),
        generated_path: String::new(),
        clean_rom: String::new(),
        autosave_minutes: default_autosave_minutes(),
        backups_kept: default_backups_kept(),
    });
    ProjectSettings {
        active_variant: stored.active_variant,
        active_module: stored.active_module,
        generated_path: stored.generated_path,
        clean_rom: stored.clean_rom,
        autosave_minutes: stored.autosave_minutes.clamp(1, 60),
        backups_kept: stored.backups_kept.clamp(1, 100),
        ..Default::default()
    }
}

pub(crate) fn save_settings(directory: &Path, settings: &ProjectSettings) -> Result<()> {
    let stored = Stored {
        active_variant: settings.active_variant.clone(),
        active_module: settings.active_module.clone(),
        generated_path: settings.generated_path.clone(),
        clean_rom: settings.clean_rom.clone(),
        autosave_minutes: settings.autosave_minutes,
        backups_kept: settings.backups_kept,
    };
    let text = serde_json::to_string_pretty(&stored).map_err(|e| Error::new(ErrorCode::InvalidArgument, e.to_string()))?;
    write_text_file_atomically(&path(directory), &format!("{text}\n"))
}

/// The clean ROM a project's settings name, for a tool that builds it without opening it.
pub fn stored_clean_rom(directory: &Path) -> Option<PathBuf> {
    clean_rom_file(directory, &load_settings(directory))
}

/// The clean ROM the settings name, relative paths against the project. `None` when they name none.
pub(crate) fn clean_rom_file(directory: &Path, settings: &ProjectSettings) -> Option<PathBuf> {
    (!settings.clean_rom.is_empty()).then(|| directory.join(&settings.clean_rom))
}

/// `file` relative to the project when it is inside it, so a project moves with its ROM, and as it is otherwise. How
/// the settings store the clean ROM, and how a project's ROM is shown.
pub(crate) fn relative_to_project(directory: &Path, file: &Path) -> String {
    file.strip_prefix(directory).unwrap_or(file).to_string_lossy().replace('\\', "/")
}

/// Why the clean ROM cannot be built from, or `None` when it can: it is there, and it is New Super Mario Bros.
pub(crate) fn clean_rom_problem(directory: &Path, settings: &ProjectSettings) -> Option<String> {
    let Some(file) = clean_rom_file(directory, settings) else {
        return Some("This project has no clean ROM to build from yet.".to_owned());
    };
    let header = bigstar_core::read_file_prefix(&file, bigstar_nds::DEFAULT_HEADER_REGION_SIZE)
        .and_then(|bytes| bigstar_nds::parse_rom_header(&bytes))
        .map_err(|_| format!("The clean ROM {} is missing or is not a DS ROM.", settings.clean_rom));
    match header {
        Err(problem) => Some(problem),
        Ok(header) if bigstar_project::nsmb_graphics_profile(&header.game_code).is_none() => {
            Some(format!("The clean ROM {} is not New Super Mario Bros. (its game code is {}).", settings.clean_rom, header.game_code))
        }
        Ok(_) => None,
    }
}
