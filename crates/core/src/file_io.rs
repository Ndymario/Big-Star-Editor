use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::{Error, ErrorCode, Result};

fn io_error(what: &str, path: &Path) -> Error {
    Error::new(ErrorCode::IoError, format!("{what} {}", path.display()))
}

pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    let mut file = File::open(path).map_err(|_| io_error("could not open", path))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data).map_err(|_| io_error("could not finish reading", path))?;
    Ok(data)
}

/// Reads at most `maximum_size` bytes; a shorter file yields what it has.
pub fn read_file_prefix(path: &Path, maximum_size: usize) -> Result<Vec<u8>> {
    let file = File::open(path).map_err(|_| io_error("could not open", path))?;
    let mut data = Vec::new();
    let limit = u64::try_from(maximum_size).map_err(|_| Error::new(ErrorCode::InvalidArgument, "requested file prefix is too large"))?;
    file.take(limit).read_to_end(&mut data).map_err(|_| io_error("could not finish reading", path))?;
    Ok(data)
}

fn temporary_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_owned();
    name.push(".bse-tmp");
    PathBuf::from(name)
}

/// Writes next to `path` and renames over it, so a failed save never leaves a half-written file behind.
pub fn write_file_atomically(path: &Path, data: &[u8]) -> Result<()> {
    if path.as_os_str().is_empty() {
        return Err(Error::new(ErrorCode::InvalidArgument, "output path is empty or file is too large"));
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|_| io_error("could not create directory for", path))?;
    }

    // ponytail: the desktop editor is the sole writer; use unique temp names if multi-process saves are added.
    let temporary = temporary_path(path);
    let written = File::create(&temporary)
        .map_err(|_| io_error("could not write", &temporary))
        .and_then(|mut out| out.write_all(data).and_then(|()| out.flush()).map_err(|_| io_error("could not finish writing", &temporary)));
    if let Err(error) = written {
        let _ = std::fs::remove_file(&temporary);
        return Err(error);
    }
    std::fs::rename(&temporary, path).map_err(|_| {
        let _ = std::fs::remove_file(&temporary);
        io_error("could not replace", path)
    })
}

pub fn write_text_file_atomically(path: &Path, text: &str) -> Result<()> {
    write_file_atomically(path, text.as_bytes())
}
