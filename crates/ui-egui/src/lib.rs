//! Big Star Editor `ui-egui` layer: the frontend. It speaks to a host through `bigstar_client::Connection` and knows
//! nothing about files, ROMs or how the host is reached. Wasm-safe.

mod app;
mod canvas;
mod clipboard;
pub mod demo;
mod editors;
mod files;
mod level_data;
mod level_view;
mod picker;
mod properties;
mod properties_ui;
mod services;
mod session;
mod tools;
mod workspace;

pub use app::{BigStarApp, EMBEDDED_ROOT_KEY, RECENT_HOSTS_KEY};
pub use level_view::{Item, Kind, LevelView};
pub use services::{FileShelf, MemoryServices, Services};
pub use session::{BytesFor, Opening, Session};
pub use workspace::{DocTab, Memory};

#[cfg(test)]
mod tests;
