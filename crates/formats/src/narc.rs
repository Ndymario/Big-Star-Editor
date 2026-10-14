use bigstar_core::{Error, ErrorCode, LittleEndianReader, Result};
use bigstar_nds::parse_fnt;

use crate::{compress_nitro_lz10, compress_nitro_lz11, decompress_nitro_lz};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NarcArchive {
    pub name_table: Vec<u8>,
    pub files: Vec<Vec<u8>>,
}

impl NarcArchive {
    /// Finds a member by its `/`-separated path in the name table.
    pub fn find(&self, path: &str) -> Result<usize> {
        parse_fnt(&self.name_table)?
            .into_iter()
            .find(|entry| entry.path == path)
            .map(|entry| usize::from(entry.id))
            .ok_or_else(|| Error::new(ErrorCode::NotFound, "NARC member was not found"))
    }
}

fn parse_error(message: &str) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidArgument, message)
}

pub fn is_narc(data: &[u8]) -> bool {
    data.starts_with(b"NARC")
}

/// What an archive was wrapped in when it was read. Part of the file's identity, not a size choice to be made again on
/// the way out: a game that reads an archive through its decompressor will not take a raw one in its place, so an
/// archive that arrived compressed goes back compressed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NarcWrapper {
    None,
    Lz10,
    Lz11,
}

/// The wrapper `data` carries and the archive inside it, or `None` when it is not an archive at all. Decided by what
/// the bytes decode to, never by the file's name: `.carc` is a naming habit, not a format.
pub fn unwrap_narc(data: &[u8]) -> Option<(NarcWrapper, Vec<u8>)> {
    if is_narc(data) {
        return Some((NarcWrapper::None, data.to_vec()));
    }
    let wrapper = match data.first() {
        Some(0x10) => NarcWrapper::Lz10,
        Some(0x11) => NarcWrapper::Lz11,
        _ => return None,
    };
    decompress_nitro_lz(data).ok().filter(|plain| is_narc(plain)).map(|plain| (wrapper, plain))
}

/// `archive` written and put back in the wrapper it was read with.
pub fn write_wrapped_narc(archive: &NarcArchive, wrapper: NarcWrapper) -> Result<Vec<u8>> {
    let plain = write_narc(archive)?;
    match wrapper {
        NarcWrapper::None => Ok(plain),
        NarcWrapper::Lz10 => compress_nitro_lz10(&plain),
        NarcWrapper::Lz11 => compress_nitro_lz11(&plain),
    }
}

pub fn parse_narc(data: &[u8]) -> Result<NarcArchive> {
    let r = LittleEndianReader::new(data);
    if data.len() < 16 || !is_narc(data) || r.u16(4)? != 0xfffe || r.u16(12)? != 16 || r.u32(8)? as usize != data.len() {
        return Err(parse_error("invalid NARC header"));
    }
    let (mut fat, mut fnt, mut img) = (None, None, None);
    let mut cursor = 16usize;
    let mut chunks = 0usize;
    while cursor < data.len() {
        if cursor + 8 > data.len() {
            return Err(parse_error("truncated NARC chunk header"));
        }
        let size = r.u32(cursor + 4)? as usize;
        if size < 8 || size > data.len() - cursor {
            return Err(parse_error("NARC chunk extends past the file"));
        }
        let magic = data.get(cursor..cursor + 4).unwrap_or_default();
        let (slot, what) = match magic {
            b"BTAF" => (&mut fat, "FAT"),
            b"BTNF" => (&mut fnt, "FNT"),
            b"GMIF" => (&mut img, "image"),
            _ => return Err(Error::new(ErrorCode::Unsupported, "unknown NARC chunk")),
        };
        if slot.is_some() {
            return Err(parse_error(&format!("duplicate NARC {what}")));
        }
        *slot = Some((cursor, size));
        cursor += size;
        chunks += 1;
    }
    let (Some((fat, _)), Some((fnt, fnt_size)), Some((img, img_size))) = (fat, fnt, img) else {
        return Err(parse_error("NARC is missing a required chunk"));
    };
    if cursor != data.len() || chunks != usize::from(r.u16(14)?) {
        return Err(parse_error("NARC is missing a required chunk"));
    }
    let count = usize::from(r.u16(fat + 8)?);
    if 12 + count * 8 > r.u32(fat + 4)? as usize {
        return Err(parse_error("NARC FAT is truncated"));
    }
    let name_table = data.get(fnt + 8..fnt + fnt_size).unwrap_or_default().to_vec();
    let names = parse_fnt(&name_table)?;
    if names.last().is_some_and(|n| usize::from(n.id) >= count) {
        return Err(parse_error("NARC name table refers past its FAT"));
    }
    let base = img + 8;
    let available = (img_size - 8) as u32;
    let mut files = Vec::with_capacity(count);
    for i in 0..count {
        let start = r.u32(fat + 12 + i * 8)?;
        let end = r.u32(fat + 16 + i * 8)?;
        if end < start || end > available {
            return Err(Error::new(ErrorCode::OutOfBounds, "NARC member lies outside its image"));
        }
        let member = data
            .get(base + start as usize..base + end as usize)
            .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "NARC member lies outside its image"))?;
        files.push(member.to_vec());
    }
    Ok(NarcArchive { name_table, files })
}

pub fn write_narc(archive: &NarcArchive) -> Result<Vec<u8>> {
    if archive.files.len() > 65535 {
        return Err(invalid("NARC has more than 65535 members"));
    }
    let names = parse_fnt(&archive.name_table)?;
    if names.last().is_some_and(|n| usize::from(n.id) >= archive.files.len()) {
        return Err(invalid("NARC name table refers past its members"));
    }
    let mut allocation: Vec<u8> = Vec::new();
    let mut image: Vec<u8> = Vec::new();
    allocation.extend_from_slice(&(archive.files.len() as u16).to_le_bytes());
    allocation.extend_from_slice(&0u16.to_le_bytes());
    for file in &archive.files {
        let fits = image.len().checked_add(file.len()).and_then(|n| n.checked_add(3)).is_some_and(|n| n <= u32::MAX as usize);
        if !fits {
            return Err(invalid("NARC image is too large"));
        }
        allocation.extend_from_slice(&(image.len() as u32).to_le_bytes());
        image.extend_from_slice(file);
        allocation.extend_from_slice(&(image.len() as u32).to_le_bytes());
        image.resize((image.len() + 3) & !3usize, 0xff);
    }
    let fat = 8 + allocation.len();
    let fnt = 8 + archive.name_table.len();
    let img = 8 + image.len();
    let total = 16 + fat + fnt + img;
    if total > u32::MAX as usize {
        return Err(invalid("NARC is too large"));
    }
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"NARC");
    out.extend_from_slice(&0xfffeu16.to_le_bytes());
    out.extend_from_slice(&0x0100u16.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(b"BTAF");
    out.extend_from_slice(&(fat as u32).to_le_bytes());
    out.extend_from_slice(&allocation);
    out.extend_from_slice(b"BTNF");
    out.extend_from_slice(&(fnt as u32).to_le_bytes());
    out.extend_from_slice(&archive.name_table);
    out.extend_from_slice(b"GMIF");
    out.extend_from_slice(&(img as u32).to_le_bytes());
    out.extend_from_slice(&image);
    Ok(out)
}
