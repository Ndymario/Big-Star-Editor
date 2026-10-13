//! Big Star Editor `project` layer: the manifest, the project model, the ROM read where it lies, the level
//! catalog, the generated module data, the effective filesystem a plan and a ROM add up to, level graphics resolution
//! the transactional patch apply, and the Lua actor and tile scripts. Host-only: it touches the filesystem and runs Lua.

mod effective_filesystem;
mod filesystem_view;
mod graphics_resources;
mod level_catalog;
mod lua_actors;
mod lua_sandbox;
mod lua_tiles;
mod manifest;
mod module_data;
mod patch_apply;
mod paths;
mod project;
mod rom_reader;
mod tile_overlays;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub use effective_filesystem::{EffectiveFilesystem, RomAddress, split_rom_address};
pub use filesystem_view::{
    FilesystemEntry, FilesystemStatus, FilesystemView, FilesystemViewRequest, ModuleVariantMapping, build_filesystem_view, filesystem_status_name,
    module_file_target,
};
pub use graphics_resources::{
    ActorResourceFiles, BackgroundResourceFiles, LevelGraphicsResources, LevelTilesets, NsmbGraphicsProfile, ObjectBankOverlayTable, ResourceFile,
    TilesetResourceFiles, find_object_bank_overlays, load_background_graphics, load_level_tilesets, nsmb_graphics_profile, resolve_level_graphics,
};
pub use level_catalog::{LevelCatalogArea, LevelCatalogEntry, nsmb_level_catalog};
pub use lua_actors::{
    ActorImageDecoder, ActorResource, ActorResourceKind, ActorResourceResolver, LuaActorCatalog, LuaActorDefinition, LuaActorDrawCommand,
    LuaActorDrawKind, LuaActorLayer, LuaActorProgram, LuaActorRender, LuaActorRenderInput, StageObjectIdentity, StageObjectNameResolver,
    has_lua_renderer, load_lua_actor_layers, lua_actor_bounds, render_lua_actor,
};
pub use lua_tiles::{LuaTileLayer, load_lua_tile_layers};
pub use manifest::{
    BUILT_ROM_FILE_NAME, INPUT_ROM_FILE_NAME, MANIFEST_FILE_NAME, MANIFEST_VERSION, default_rom_path, index_rom_files, index_rom_files_from,
    load_manifest, make_project_relative, save_manifest,
};
pub use module_data::{DEFAULT_GENERATED_DIR, load_module_data_catalog};
pub use patch_apply::{
    DEFAULT_BACKUPS_KEPT, PatchApplyOutcome, PatchApplyRequest, PatchApplyResult, apply_patch_transactionally, built_rom_path, patch_backup_root,
    patch_staging_dir, prune_patch_backups,
};
pub use paths::lexically_normal;
pub use project::{GameFile, Project, Rom, create_empty_project};
pub use rom_reader::{ArchiveMember, RomReader};
pub use tile_overlays::{TileOverlay, TileOverlayCatalog, TileOverlayScript, overlay_map16_images, overlay_map16_sheet};

#[cfg(test)]
mod tests;
