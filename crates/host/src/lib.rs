//! Big Star Editor `host` layer: the service that owns the workspace, its projects, the ROM reader and the level
//! sessions with their undo history, the decoded graphics and actor scripts, and the watch on the project. Host only: it reads files, so it never builds for the browser.

mod filesystem;
mod graphics;
mod host;
mod levels;
mod settings;
mod watch;
mod workspace;

pub use host::{Host, HostOptions, HostSession, default_data_dir};
pub use workspace::Workspace;

#[cfg(test)]
mod tests;
