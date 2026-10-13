//! Big Star Editor `project` layer: the manifest, the project model, the ROM read where it lies, the level
//! catalog, the generated module data and the effective filesystem a plan and a ROM add up to. Host-only: it touches
//! the filesystem.

mod effective_filesystem;
mod filesystem_view;
mod level_catalog;
mod manifest;
mod module_data;
mod paths;
mod project;
mod rom_reader;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use effective_filesystem::{EffectiveFilesystem, RomAddress, split_rom_address};
pub use filesystem_view::{
    FilesystemEntry, FilesystemStatus, FilesystemView, FilesystemViewRequest, ModuleVariantMapping, build_filesystem_view, filesystem_status_name,
    module_file_target,
};
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
