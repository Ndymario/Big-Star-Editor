//! Big Star Editor `ui-egui` layer: the frontend. It speaks to a host through `bigstar_client::Connection` and knows
//! nothing about files, ROMs or how the host is reached. Wasm-safe.

mod app;
mod block_icons;
mod builds;
mod canvas;
mod clipboard;
mod controls;
pub mod demo;
mod dialogs;
mod editors;
mod files;
mod graphics_editor;
mod hex_editor;
mod history;
mod i18n;
mod icons;
mod image_manager;
mod level_data;
mod level_list;
mod level_view;
mod message_editor;
mod module_viewer;
mod motion;
mod paint;
mod palette_panel;
mod picker;
mod pixel_editor;
mod properties;
mod properties_ui;
mod resource_editor;
mod services;
mod session;
mod theme;
mod tile_picker;
mod tilemap_editor;
mod tileset_editor;
mod tools;
mod workspace;
mod zoom_canvas;

pub use app::{BigStarApp, CANVAS_OPTIONS_KEY, EMBEDDED_ROOT_KEY, LANGUAGE_KEY, RECENT_HOSTS_KEY, THEME_KEY};
pub use i18n::{KEYS as TRANSLATED_STRINGS, Language};
pub use level_view::{DsScreens, Item, Kind, LevelView};
pub use services::{FileShelf, MemoryServices, Services};
pub use session::{BytesFor, FileRead, Opening, Session};
pub use workspace::{DocTab, Memory};

#[cfg(test)]
mod tests;
