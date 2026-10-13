//! Big Star Editor `formats` layer: parsers and writers for the file formats inside an NSMB ROM.
//!
//! Port of `libs/nsmb-formats`, in the order of docs/rust-port-plan.md. Compression, NARC, glue blocks, courses,
//! format identification, the graphics codecs (tiles, palettes, screen maps, Nitro 2D, TEX0), BMG messages, tilesets and
//! UI resources live here.

mod compression;
mod g2d;
mod glue;
mod graphics;
mod identify;
mod level;
mod message;
mod narc;
mod tex0;
mod tile_overlay;
mod tileset;
mod ui_resources;

pub use compression::*;
pub use g2d::{
    FourCc, Nanr, NanrFrame, NanrSequence, Ncer, NcerBounds, NcerCell, NcerOam, NcerTransfer, Ncgr, NcgrPosition, Nclr, Nitro2dBlock, Nitro2dFile,
    Nscr, parse_nanr, parse_ncer, parse_ncgr, parse_nclr, parse_nitro_2d_file, parse_nscr, render_ncer_cell, write_nanr, write_ncer, write_ncgr,
    write_nclr, write_nitro_2d_file, write_nscr,
};
pub use glue::*;
pub use graphics::*;
pub use identify::*;
pub use level::*;
pub use message::*;
pub use narc::*;
pub use tex0::*;
pub use tile_overlay::*;
pub use tileset::*;
pub use ui_resources::*;

#[cfg(test)]
mod tests;
