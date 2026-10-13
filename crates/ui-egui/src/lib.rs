//! Big Star Editor `ui-egui` layer: the frontend. It speaks to a host through `bigstar_client::Connection` and knows
//! nothing about files, ROMs or how the host is reached. Wasm-safe.

mod app;
mod canvas;
pub mod demo;
mod level_view;
mod services;
mod session;

pub use app::{BigStarApp, EMBEDDED_ROOT_KEY, RECENT_HOSTS_KEY};
pub use level_view::{Item, Kind, LevelView};
pub use services::{MemoryServices, Services};
pub use session::Session;

#[cfg(test)]
mod tests;
