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
}

fn path(directory: &Path) -> PathBuf {
    directory.join(SETTINGS_FILE_NAME)
}

/// A missing or unreadable file is the defaults: the choices are conveniences, never a reason to refuse a project.
pub(crate) fn load_settings(directory: &Path) -> ProjectSettings {
    let stored: Stored = std::fs::read_to_string(path(directory)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default();
    ProjectSettings {
        active_variant: stored.active_variant,
        active_module: stored.active_module,
        generated_path: stored.generated_path,
        ..Default::default()
    }
}

pub(crate) fn save_settings(directory: &Path, settings: &ProjectSettings) -> Result<()> {
    let stored = Stored {
        active_variant: settings.active_variant.clone(),
        active_module: settings.active_module.clone(),
        generated_path: settings.generated_path.clone(),
    };
    let text = serde_json::to_string_pretty(&stored).map_err(|e| Error::new(ErrorCode::InvalidArgument, e.to_string()))?;
    write_text_file_atomically(&path(directory), &format!("{text}\n"))
}
