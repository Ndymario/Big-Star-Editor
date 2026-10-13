//! Big Star Editor `core` layer: errors, byte access, atomic file I/O and command history.
//!
//! Port of `libs/nsmb-core`. JSON is `serde_json` and logging is the `log` crate, so neither has a module here.

mod byte_buffer;
mod command;
mod endian;
mod error;
#[cfg(not(target_arch = "wasm32"))]
mod file_io;
mod fx;

pub use byte_buffer::ByteBuffer;
pub use command::{Command, CommandHistory};
pub use endian::{LittleEndianReader, LittleEndianWriter};
pub use error::{Error, ErrorCode, Result};
#[cfg(not(target_arch = "wasm32"))]
pub use file_io::{read_file, read_file_prefix, write_file_atomically, write_text_file_atomically};
pub use fx::Fx32;

#[cfg(test)]
mod tests;
