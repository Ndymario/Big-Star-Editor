//! Big Star Editor `nds` layer: parsers for the structures of a Nintendo DS cartridge image.
//!
//! Port of `libs/nsmb-nds`. Paths from the FNT are `/`-separated strings rather than platform paths, so they mean
//! the same thing on every host and cross the protocol unchanged.

mod rom;

pub use rom::*;

#[cfg(test)]
mod tests;
