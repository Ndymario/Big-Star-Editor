use std::path::Path;

use bigstar_patching::{ProjectConfigKind, detect_project_config};

/// Whether `directory` is an NCPatcher project: it has an `ncpatcher.yaml`, or a legacy `ncpatcher.json` the editor
/// offers to migrate. That file is the whole of what makes a project; Big Star keeps no manifest of its own.
pub fn is_ncpatcher_project(directory: &Path) -> bool {
    detect_project_config(directory).kind != ProjectConfigKind::None
}

/// A project is named after its directory, as NCPatcher names it.
pub fn project_name(directory: &Path) -> String {
    directory.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}
