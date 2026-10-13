//! Big Star Editor `host` layer: the service that owns the workspace, its projects, the ROM reader and the level
//! sessions with their undo history. Host only: it reads files, so it never builds for the browser.

mod host;
mod levels;
mod workspace;

pub use host::{Host, HostSession};
pub use workspace::Workspace;

#[cfg(test)]
mod tests;
