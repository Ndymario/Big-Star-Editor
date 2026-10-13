//! Big Star Editor `patching` layer: launching external tools without a shell, and the NCPatcher 2.x client.
//!
//! Port of `libs/nsmb-patching`. Host-only: it starts processes and reads their files.

mod ncpatcher;
mod process;

pub use ncpatcher::*;
pub use process::*;

#[cfg(test)]
mod tests;
