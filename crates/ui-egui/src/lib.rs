//! Big Star Editor `ui-egui` layer: the frontend. It speaks to a host through `bigstar_client::Connection` and knows
//! nothing about files, ROMs or how the host is reached. Wasm-safe.

mod app;
mod canvas;
mod clipboard;
pub mod demo;
mod editors;
mod files;
mod graphics_editor;
mod level_data;
mod level_view;
mod message_editor;
mod picker;
mod properties;
mod properties_ui;
mod resource_editor;
mod services;
mod session;
mod tileset_editor;
mod tools;
mod workspace;

pub use app::{BigStarApp, EMBEDDED_ROOT_KEY, RECENT_HOSTS_KEY};
pub use level_view::{Item, Kind, LevelView};
pub use services::{FileShelf, MemoryServices, Services};
pub use session::{BytesFor, FileRead, Opening, Session};
pub use workspace::{DocTab, Memory};

#[cfg(test)]
mod tests;
