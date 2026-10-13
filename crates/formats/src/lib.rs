//! Big Star Editor `formats` layer: parsers and writers for the file formats inside an NSMB ROM.
//!
//! Port of `libs/nsmb-formats`, in the order of docs/rust-port-plan.md. Compression, NARC, glue blocks, courses and
//! format identification live here so far.

mod compression;
mod glue;
mod graphics;
mod identify;
mod level;
mod message;
mod narc;

pub use compression::*;
pub use glue::*;
pub use graphics::{GraphicsFormatKind, graphics_format_name, identify_graphics_format};
pub use identify::*;
pub use level::*;
pub use message::is_bmg;
pub use narc::*;

#[cfg(test)]
mod tests;
