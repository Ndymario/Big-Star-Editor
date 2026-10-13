//! Big Star Editor `level` layer: the editable level with its undo history, and the module data catalogs.
//! Pure: no filesystem, safe for the browser.

mod commands;
mod editable_level;
mod module_data;
mod state;

pub use editable_level::{
    EditableLevel, IndexedLevelValue, LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, ReorderKind, SpriteInsertion,
};
pub use module_data::{
    DEFAULT_STAGE_OBJECT_ID_BASE, LevelDataDeclaration, LevelDataKind, ModuleDataCatalog, StageObjectDeclaration, parse_level_data_catalog,
    parse_stage_object_catalog,
};
pub use state::LevelTilesetReferences;

#[cfg(test)]
mod tests;
