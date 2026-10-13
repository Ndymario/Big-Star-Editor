//! Big Star Editor `project` layer: the manifest, the project model, the ROM read where it lies, the level
//! catalog and the generated module data. Host-only: it touches the filesystem.

mod level_catalog;
mod manifest;
mod module_data;
mod paths;
mod project;
mod rom_reader;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use level_catalog::{LevelCatalogArea, LevelCatalogEntry, nsmb_level_catalog};
pub use manifest::{
    BUILT_ROM_FILE_NAME, INPUT_ROM_FILE_NAME, MANIFEST_FILE_NAME, MANIFEST_VERSION, default_rom_path, index_rom_files, index_rom_files_from,
    load_manifest, make_project_relative, save_manifest,
};
pub use module_data::{DEFAULT_GENERATED_DIR, load_module_data_catalog};
pub use paths::lexically_normal;
pub use project::{GameFile, Project, Rom, create_empty_project};
pub use rom_reader::{ArchiveMember, RomReader};

#[cfg(test)]
mod tests;
