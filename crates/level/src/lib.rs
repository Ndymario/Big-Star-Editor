//! Big Star Editor `level` layer: the editable level with its undo history, the module data catalogs, sprite
//! definitions, tilemap rasterisation and the level interchange files. Pure: no filesystem, safe for the browser.

mod commands;
mod editable_level;
mod exchange;
mod module_data;
mod sprite_definitions;
mod state;
mod tilemap;

pub use editable_level::{
    EditableLevel, IndexedLevelValue, LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, ReorderKind, SpriteInsertion,
};
pub use exchange::*;
pub use module_data::{
    DEFAULT_STAGE_OBJECT_ID_BASE, LevelDataDeclaration, LevelDataKind, ModuleDataCatalog, StageObjectDeclaration, parse_level_data_catalog,
    parse_stage_object_catalog,
};
pub use sprite_definitions::*;
pub use state::LevelTilesetReferences;
pub use tilemap::*;

#[cfg(test)]
mod tests;
