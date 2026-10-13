use std::path::Path;

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_level::{ModuleDataCatalog, parse_level_data_catalog, parse_stage_object_catalog};

/// Where a build leaves its generated module data, relative to the project.
pub const DEFAULT_GENERATED_DIR: &str = "build/generated";

fn read_text(path: &Path) -> Result<String> {
    String::from_utf8(read_file(path)?).map_err(|_| Error::new(ErrorCode::ParseError, format!("{} is not UTF-8", path.display())))
}

fn exists(path: &Path) -> Result<bool> {
    path.try_exists().map_err(|_| Error::new(ErrorCode::IoError, format!("could not inspect {}", path.display())))
}

/// The declarations a project's last build generated. A project that has not been built has none, and that is an
/// empty catalog rather than an error.
pub fn load_module_data_catalog(project_dir: &Path, generated_dir: &Path) -> Result<ModuleDataCatalog> {
    let mut catalog = ModuleDataCatalog::default();
    let root = if generated_dir.is_absolute() { generated_dir.to_path_buf() } else { project_dir.join(generated_dir) };
    let level = root.join("level_data.json");
    let objects = root.join("stageobjects.json");
    if exists(&level)? {
        catalog.level_data = parse_level_data_catalog(&read_text(&level)?)?;
    }
    if exists(&objects)? {
        let (base, stage_objects) = parse_stage_object_catalog(&read_text(&objects)?)?;
        catalog.stage_object_id_base = base;
        catalog.stage_objects = stage_objects;
    }
    Ok(catalog)
}
