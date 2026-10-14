//! Saving a ROM edited directly: the edits an [`crate::EffectiveFilesystem`] holds in memory, written into the ROM
//! they were made against. Member edits are packed back into their archive, which goes back in the wrapper it came
//! in; whole files go straight to [`NdsRomWriter`], which lays them out the way NCPatcher would.

use std::collections::BTreeMap;
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::{parse_narc, unwrap_narc, write_wrapped_narc};
use bigstar_nds::NdsRomWriter;

use crate::{RomReader, split_rom_address};

/// `original` with every edit and, when given, the banner written into it. With nothing to write, `original` itself.
pub fn write_edited_rom(original: &Arc<[u8]>, edits: &BTreeMap<String, Vec<u8>>, banner: Option<&[u8]>) -> Result<Vec<u8>> {
    let mut reader = RomReader::from_bytes("edited.nds", Arc::clone(original))?;

    // Whole files first, then each container with its members replaced on top of whichever version of it is current.
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    let mut members: BTreeMap<String, Vec<(String, Vec<u8>)>> = BTreeMap::new();
    for (address, bytes) in edits {
        let split = split_rom_address(address);
        if split.inside_archive() {
            members.entry(split.container).or_default().push((split.member, bytes.clone()));
        } else {
            files.insert(split.container, bytes.clone());
        }
    }
    for (container, replaced) in members {
        let current = match files.get(&container) {
            Some(bytes) => bytes.clone(),
            None => reader.read(&container)?,
        };
        let (wrapper, plain) =
            unwrap_narc(&current).ok_or_else(|| Error::new(ErrorCode::Unsupported, format!("{container} is not a Nitro archive")))?;
        let mut archive = parse_narc(&plain)?;
        for (member, bytes) in replaced {
            let index = archive.find(&member).map_err(|_| Error::new(ErrorCode::NotFound, format!("{container} has no member {member}")))?;
            if let Some(slot) = archive.files.get_mut(index) {
                *slot = bytes;
            }
        }
        files.insert(container, write_wrapped_narc(&archive, wrapper)?);
    }

    let mut writer = NdsRomWriter::new(original.to_vec())?;
    for (path, bytes) in files {
        let id = reader.file_id(&path).ok_or_else(|| Error::new(ErrorCode::NotFound, format!("the ROM does not hold {path}")))?;
        writer.set_file(id, bytes)?;
    }
    if let Some(banner) = banner {
        writer.set_banner(banner.to_vec())?;
    }
    writer.commit()
}
