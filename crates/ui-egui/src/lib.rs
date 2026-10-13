//! Big Star Editor `ui-egui` layer: the frontend. It speaks to a host through `bigstar_client::Connection` and knows
//! nothing about files, ROMs or how the host is reached. Wasm-safe.

mod app;
mod builds;
mod canvas;
mod clipboard;
mod controls;
pub mod demo;
mod dialogs;
mod editors;
mod files;
mod graphics_editor;
mod i18n;
mod icons;
mod level_data;
mod level_view;
mod message_editor;
mod motion;
mod picker;
mod properties;
mod properties_ui;
mod resource_editor;
mod services;
mod session;
mod theme;
mod tileset_editor;
mod tools;
mod workspace;

pub use app::{BigStarApp, EMBEDDED_ROOT_KEY, LANGUAGE_KEY, RECENT_HOSTS_KEY, THEME_KEY};
pub use i18n::{KEYS as TRANSLATED_STRINGS, Language};
pub use level_view::{Item, Kind, LevelView};
pub use services::{FileShelf, MemoryServices, Services};
pub use session::{BytesFor, FileRead, Opening, Session};
pub use workspace::{DocTab, Memory};

#[cfg(test)]
mod tests;
