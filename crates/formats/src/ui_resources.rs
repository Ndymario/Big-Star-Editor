//! UI resources: BNBL touch boxes, BNCL cell layouts, BNCD cell data, ENPG images, the DS banner and the NSMBeUD ROM
//! user metadata text. Port of `libs/nsmb-formats/src/ui_resources.cpp`.

use std::collections::HashSet;

use bigstar_core::{Error, ErrorCode, LittleEndianReader, Result};
use bigstar_nds::crc16;

use crate::g2d::{put16, put32};

const ENPG_PIXELS_SIZE: usize = 256 * 256;
const ENPG_PALETTE_SIZE: usize = 256 * 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiResourceKind {
    Unknown,
    Bnbl,
    Bncl,
    Bncd,
    Enpg,
    Banner,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BnblEntry {
    pub x: u16,
    pub y: u16,
    pub x_alignment: u8,
    pub y_alignment: u8,
    pub x_reserved: u8,
    pub y_reserved: u8,
    pub width: u8,
    pub height: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bnbl {
    pub original: Vec<u8>,
    pub entries: Vec<BnblEntry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BnclEntry {
    pub x: u16,
    pub y: u16,
    pub x_alignment: u8,
    pub y_alignment: u8,
    pub x_reserved: u8,
    pub y_reserved: u8,
    pub graphic_id: u8,
    pub reserved: [u8; 3],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bncl {
    pub original: Vec<u8>,
    pub entries: Vec<BnclEntry>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BncdEntry {
    pub width: u8,
    pub height: u8,
    pub subentry_start: u16,
    pub subentry_count: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BncdSubentry {
    pub oam_attr0: u16,
    pub oam_attr1: u16,
    pub unknown: u32,
    pub tile_number: u16,
    pub tile_count: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bncd {
    pub original: Vec<u8>,
    pub entries_offset: u32,
    pub subentries_offset: u32,
    pub texture_offset: u32,
    pub texture_size: u32,
    pub entries: Vec<BncdEntry>,
    pub subentries: Vec<BncdSubentry>,
    pub texture: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enpg {
    pub pixels: Vec<u8>,
    pub palette: [u16; 256],
    pub trailing: Vec<u8>,
}

impl Default for Enpg {
    fn default() -> Self {
        Self { pixels: Vec::new(), palette: [0; 256], trailing: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DsBanner {
    pub original: Vec<u8>,
    pub version: u16,
    pub icon: [u8; 0x200],
    pub palette: [u16; 16],
    /// UTF-16 code units, zero-padded.
    pub title_slots: [[u16; 128]; 8],
    pub title_count: usize,
}

impl Default for DsBanner {
    fn default() -> Self {
        Self { original: Vec::new(), version: 0, icon: [0; 0x200], palette: [0; 16], title_slots: [[0; 128]; 8], title_count: 0 }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RomUserMetadataEntry {
    pub key: i32,
    pub value: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RomUserMetadataSection {
    pub name: String,
    pub entries: Vec<RomUserMetadataEntry>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RomUserMetadata {
    pub original: Vec<u8>,
    pub original_sections: Vec<RomUserMetadataSection>,
    pub sections: Vec<RomUserMetadataSection>,
}

fn err(code: ErrorCode, message: impl Into<String>) -> Error {
    Error::new(code, message)
}

fn byte(bytes: &[u8], offset: usize) -> u8 {
    bytes.get(offset).copied().unwrap_or(0)
}

fn set_byte(bytes: &mut [u8], offset: usize, value: u8) {
    if let Some(slot) = bytes.get_mut(offset) {
        *slot = value;
    }
}

fn read16(bytes: &[u8], offset: usize) -> Result<u16> {
    LittleEndianReader::new(bytes).u16(offset)
}

fn read32(bytes: &[u8], offset: usize) -> Result<u32> {
    LittleEndianReader::new(bytes).u32(offset)
}

fn lower_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase()
}

fn validate_position(coordinate: u16, alignment: u8, reserved: u8) -> Result<()> {
    if coordinate > 0x0fff || alignment > 3 || reserved > 3 {
        return Err(err(ErrorCode::InvalidArgument, "UI position or alignment is outside its packed field"));
    }
    Ok(())
}

fn packed_position(coordinate: u16, alignment: u8, reserved: u8) -> u16 {
    coordinate | (u16::from(alignment) << 12) | (u16::from(reserved) << 14)
}

fn unpack_position(value: u16) -> (u16, u8, u8) {
    (value & 0x0fff, ((value >> 12) & 3) as u8, (value >> 14) as u8)
}

fn banner_minimum_size(version: u16) -> usize {
    match version {
        1 => 0x840,
        2 => 0x940,
        3 | 0x0103 => 0xA40,
        _ => 0,
    }
}

fn banner_title_count(version: u16) -> usize {
    match version {
        1 => 6,
        2 => 7,
        _ => 8,
    }
}

fn update_banner_crc(bytes: &mut [u8], offset: usize, length: usize, field: usize) {
    let crc = crc16(bytes.get(offset..offset + length).unwrap_or_default(), 0xFFFF);
    put16(bytes, field, crc);
}

fn trim(value: &str) -> &str {
    value.trim_matches([' ', '\t', '\r'])
}

fn parse_key(text: &str) -> Result<i32> {
    let clean = trim(text);
    let digits = clean.strip_prefix('-').unwrap_or(clean);
    let parsed = if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) { clean.parse::<i32>().ok() } else { None };
    match parsed {
        Some(key) if key >= 0 => Ok(key),
        _ => Err(err(ErrorCode::ParseError, "metadata key must be a non-negative decimal integer")),
    }
}

fn printable(text: &str) -> bool {
    text.bytes().all(|c| (0x20..=0x7e).contains(&c))
}

fn validate_metadata_text(text: &str, label: &str) -> Result<()> {
    if text.is_empty() || text.contains(['\r', '\n', '[', ']']) {
        return Err(err(ErrorCode::InvalidArgument, format!("{label} is empty or contains a reserved character")));
    }
    if !printable(text) {
        return Err(err(ErrorCode::InvalidArgument, format!("{label} must contain printable ASCII only")));
    }
    Ok(())
}

pub fn identify_ui_resource(bytes: &[u8], path: &str) -> UiResourceKind {
    if bytes.starts_with(b"JNBL") {
        return UiResourceKind::Bnbl;
    }
    if bytes.starts_with(b"JNCL") {
        return UiResourceKind::Bncl;
    }
    if bytes.starts_with(b"JNCD") {
        return UiResourceKind::Bncd;
    }
    let name = lower_name(path);
    if name.ends_with(".enpg") && bytes.len() >= ENPG_PIXELS_SIZE + ENPG_PALETTE_SIZE {
        return UiResourceKind::Enpg;
    }
    if name == "banner.bin" && bytes.len() >= 2 && banner_minimum_size(u16::from_le_bytes([byte(bytes, 0), byte(bytes, 1)])) != 0 {
        return UiResourceKind::Banner;
    }
    UiResourceKind::Unknown
}

pub fn parse_bnbl(bytes: &[u8]) -> Result<Bnbl> {
    if bytes.len() < 8 || !bytes.starts_with(b"JNBL") {
        return Err(err(ErrorCode::ParseError, "BNBL header is missing or invalid"));
    }
    let count = usize::from(byte(bytes, 6));
    if count > (bytes.len() - 8) / 6 {
        return Err(err(ErrorCode::OutOfBounds, "BNBL entry table is truncated"));
    }
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 8 + i * 6;
        let (Ok(x), Ok(y)) = (read16(bytes, offset), read16(bytes, offset + 2)) else {
            return Err(err(ErrorCode::OutOfBounds, "BNBL entry is truncated"));
        };
        let (x, x_alignment, x_reserved) = unpack_position(x);
        let (y, y_alignment, y_reserved) = unpack_position(y);
        entries.push(BnblEntry {
            x,
            y,
            x_alignment,
            y_alignment,
            x_reserved,
            y_reserved,
            width: byte(bytes, offset + 4),
            height: byte(bytes, offset + 5),
        });
    }
    Ok(Bnbl { original: bytes.to_vec(), entries })
}

/// Rebuilds a JNBL/JNCL table of `stride`-byte entries: the original header with the new count, the entries `fill`
/// writes, then everything that followed the old table.
fn write_table(original: &[u8], count: usize, stride: usize, name: &str, mut fill: impl FnMut(&mut [u8], usize) -> Result<()>) -> Result<Vec<u8>> {
    if original.len() < 8 || count > 255 {
        return Err(err(ErrorCode::InvalidArgument, format!("{name} header is invalid or has too many entries")));
    }
    let old_end = 8 + usize::from(byte(original, 6)) * stride;
    if old_end > original.len() {
        return Err(err(ErrorCode::InvalidArgument, format!("original {name} table is truncated")));
    }
    let mut result = original.get(..8).unwrap_or_default().to_vec();
    set_byte(&mut result, 6, count as u8);
    result.resize(8 + count * stride, 0);
    for i in 0..count {
        fill(&mut result, 8 + i * stride)?;
    }
    result.extend_from_slice(original.get(old_end..).unwrap_or_default());
    Ok(result)
}

pub fn write_bnbl(value: &Bnbl) -> Result<Vec<u8>> {
    let mut entries = value.entries.iter();
    write_table(&value.original, value.entries.len(), 6, "BNBL", |result, offset| {
        let Some(entry) = entries.next() else { return Ok(()) };
        validate_position(entry.x, entry.x_alignment, entry.x_reserved)?;
        validate_position(entry.y, entry.y_alignment, entry.y_reserved)?;
        put16(result, offset, packed_position(entry.x, entry.x_alignment, entry.x_reserved));
        put16(result, offset + 2, packed_position(entry.y, entry.y_alignment, entry.y_reserved));
        set_byte(result, offset + 4, entry.width);
        set_byte(result, offset + 5, entry.height);
        Ok(())
    })
}

pub fn parse_bncl(bytes: &[u8]) -> Result<Bncl> {
    if bytes.len() < 8 || !bytes.starts_with(b"JNCL") {
        return Err(err(ErrorCode::ParseError, "BNCL header is missing or invalid"));
    }
    let count = usize::from(byte(bytes, 6));
    if count > (bytes.len() - 8) / 8 {
        return Err(err(ErrorCode::OutOfBounds, "BNCL entry table is truncated"));
    }
    let mut entries = Vec::with_capacity(count);
    for i in 0..count {
        let offset = 8 + i * 8;
        let (Ok(x), Ok(y)) = (read16(bytes, offset), read16(bytes, offset + 2)) else {
            return Err(err(ErrorCode::OutOfBounds, "BNCL entry is truncated"));
        };
        let (x, x_alignment, x_reserved) = unpack_position(x);
        let (y, y_alignment, y_reserved) = unpack_position(y);
        entries.push(BnclEntry {
            x,
            y,
            x_alignment,
            y_alignment,
            x_reserved,
            y_reserved,
            graphic_id: byte(bytes, offset + 4),
            reserved: [byte(bytes, offset + 5), byte(bytes, offset + 6), byte(bytes, offset + 7)],
        });
    }
    Ok(Bncl { original: bytes.to_vec(), entries })
}

pub fn write_bncl(value: &Bncl) -> Result<Vec<u8>> {
    let mut entries = value.entries.iter();
    write_table(&value.original, value.entries.len(), 8, "BNCL", |result, offset| {
        let Some(entry) = entries.next() else { return Ok(()) };
        validate_position(entry.x, entry.x_alignment, entry.x_reserved)?;
        validate_position(entry.y, entry.y_alignment, entry.y_reserved)?;
        put16(result, offset, packed_position(entry.x, entry.x_alignment, entry.x_reserved));
        put16(result, offset + 2, packed_position(entry.y, entry.y_alignment, entry.y_reserved));
        set_byte(result, offset + 4, entry.graphic_id);
        if let Some(slot) = result.get_mut(offset + 5..offset + 8) {
            slot.copy_from_slice(&entry.reserved);
        }
        Ok(())
    })
}

pub fn parse_bncd(bytes: &[u8]) -> Result<Bncd> {
    if bytes.len() < 0x1c || !bytes.starts_with(b"JNCD") {
        return Err(err(ErrorCode::ParseError, "BNCD header is missing or invalid"));
    }
    let header = (|| Ok::<_, Error>((read16(bytes, 6)?, read32(bytes, 8)?, read32(bytes, 12)?, read32(bytes, 16)?, read32(bytes, 20)?)))();
    let Ok((count, entries, subentries, texture, texture_size)) = header else {
        return Err(err(ErrorCode::OutOfBounds, "BNCD header is truncated"));
    };
    let entry_end = u64::from(entries) + u64::from(count) * 6;
    let texture_end = u64::from(texture) + u64::from(texture_size);
    if entries < 0x1c || entry_end > u64::from(subentries) || subentries > texture || texture_end > bytes.len() as u64 {
        return Err(err(ErrorCode::OutOfBounds, "BNCD tables or texture data overlap or leave the file"));
    }
    let subentry_count = ((texture - subentries) / 12) as usize;
    let mut result = Bncd {
        original: bytes.to_vec(),
        entries_offset: entries,
        subentries_offset: subentries,
        texture_offset: texture,
        texture_size,
        ..Bncd::default()
    };
    result.entries.reserve(usize::from(count));
    for i in 0..usize::from(count) {
        let offset = entries as usize + i * 6;
        let (Ok(start), Ok(size)) = (read16(bytes, offset + 2), read16(bytes, offset + 4)) else {
            return Err(err(ErrorCode::OutOfBounds, "BNCD entry is truncated"));
        };
        if usize::from(start) + usize::from(size) > subentry_count {
            return Err(err(ErrorCode::OutOfBounds, "BNCD entry references subentries outside the table"));
        }
        result.entries.push(BncdEntry { width: byte(bytes, offset), height: byte(bytes, offset + 1), subentry_start: start, subentry_count: size });
    }
    result.subentries.reserve(subentry_count);
    for i in 0..subentry_count {
        let offset = subentries as usize + i * 12;
        let subentry = (|| {
            Ok::<_, Error>(BncdSubentry {
                oam_attr0: read16(bytes, offset)?,
                oam_attr1: read16(bytes, offset + 2)?,
                unknown: read32(bytes, offset + 4)?,
                tile_number: read16(bytes, offset + 8)?,
                tile_count: read16(bytes, offset + 10)?,
            })
        })();
        let Ok(subentry) = subentry else {
            return Err(err(ErrorCode::OutOfBounds, "BNCD subentry is truncated"));
        };
        result.subentries.push(subentry);
    }
    result.texture = bytes.get(texture as usize..texture_end as usize).unwrap_or_default().to_vec();
    Ok(result)
}

pub fn write_bncd(value: &Bncd) -> Result<Vec<u8>> {
    if value.original.len() < 0x1c {
        return Err(err(ErrorCode::InvalidArgument, "BNCD original data is missing"));
    }
    if read16(&value.original, 6).ok().map(usize::from) != Some(value.entries.len()) {
        return Err(err(ErrorCode::InvalidArgument, "BNCD entry count cannot change"));
    }
    let entries_end = u64::from(value.entries_offset) + value.entries.len() as u64 * 6;
    let subentries_end = u64::from(value.subentries_offset) + value.subentries.len() as u64 * 12;
    let texture_end = u64::from(value.texture_offset) + u64::from(value.texture_size);
    if value.entries_offset < 0x1c
        || entries_end > u64::from(value.subentries_offset)
        || subentries_end > u64::from(value.texture_offset)
        || texture_end > value.original.len() as u64
    {
        return Err(err(ErrorCode::InvalidArgument, "BNCD table offsets leave the original file"));
    }
    if value.texture.len() as u64 != u64::from(value.texture_size) {
        return Err(err(ErrorCode::InvalidArgument, "BNCD texture size cannot change"));
    }
    let mut result = value.original.clone();
    for (i, entry) in value.entries.iter().enumerate() {
        if usize::from(entry.subentry_start) + usize::from(entry.subentry_count) > value.subentries.len() {
            return Err(err(ErrorCode::InvalidArgument, "BNCD entry references unavailable subentries"));
        }
        let offset = value.entries_offset as usize + i * 6;
        set_byte(&mut result, offset, entry.width);
        set_byte(&mut result, offset + 1, entry.height);
        put16(&mut result, offset + 2, entry.subentry_start);
        put16(&mut result, offset + 4, entry.subentry_count);
    }
    for (i, entry) in value.subentries.iter().enumerate() {
        let offset = value.subentries_offset as usize + i * 12;
        put16(&mut result, offset, entry.oam_attr0);
        put16(&mut result, offset + 2, entry.oam_attr1);
        put32(&mut result, offset + 4, entry.unknown);
        put16(&mut result, offset + 8, entry.tile_number);
        put16(&mut result, offset + 10, entry.tile_count);
    }
    let start = value.texture_offset as usize;
    if let Some(slot) = result.get_mut(start..start + value.texture.len()) {
        slot.copy_from_slice(&value.texture);
    }
    Ok(result)
}

pub fn parse_enpg(bytes: &[u8]) -> Result<Enpg> {
    if bytes.len() < ENPG_PIXELS_SIZE + ENPG_PALETTE_SIZE {
        return Err(err(ErrorCode::OutOfBounds, "ENPG requires 256x256 indexed pixels and a 256-colour palette"));
    }
    let mut result = Enpg { pixels: bytes.get(..ENPG_PIXELS_SIZE).unwrap_or_default().to_vec(), ..Enpg::default() };
    let palette = bytes.get(ENPG_PIXELS_SIZE..ENPG_PIXELS_SIZE + ENPG_PALETTE_SIZE).unwrap_or_default();
    for (slot, &pair) in result.palette.iter_mut().zip(palette.as_chunks::<2>().0) {
        *slot = u16::from_le_bytes(pair);
    }
    result.trailing = bytes.get(ENPG_PIXELS_SIZE + ENPG_PALETTE_SIZE..).unwrap_or_default().to_vec();
    Ok(result)
}

pub fn write_enpg(value: &Enpg) -> Result<Vec<u8>> {
    if value.pixels.len() != ENPG_PIXELS_SIZE {
        return Err(err(ErrorCode::InvalidArgument, "ENPG pixel plane must be exactly 256x256 bytes"));
    }
    let mut result = value.pixels.clone();
    result.extend(value.palette.iter().flat_map(|c| c.to_le_bytes()));
    result.extend_from_slice(&value.trailing);
    Ok(result)
}

pub fn parse_ds_banner(bytes: &[u8]) -> Result<DsBanner> {
    if bytes.len() < 2 {
        return Err(err(ErrorCode::OutOfBounds, "DS banner header is truncated"));
    }
    let version = u16::from_le_bytes([byte(bytes, 0), byte(bytes, 1)]);
    let minimum = banner_minimum_size(version);
    if minimum == 0 {
        return Err(err(ErrorCode::Unsupported, "DS banner version is unsupported"));
    }
    if bytes.len() < minimum {
        return Err(err(ErrorCode::OutOfBounds, "DS banner is truncated"));
    }
    let mut result = DsBanner { original: bytes.to_vec(), version, title_count: banner_title_count(version), ..DsBanner::default() };
    result.icon.copy_from_slice(bytes.get(0x20..0x220).unwrap_or(&[0; 0x200]));
    for (i, slot) in result.palette.iter_mut().enumerate() {
        *slot = crate::g2d::u16_at(bytes, 0x220 + i * 2);
    }
    for (language, slot) in result.title_slots.iter_mut().enumerate().take(result.title_count) {
        for (i, unit) in slot.iter_mut().enumerate() {
            *unit = crate::g2d::u16_at(bytes, 0x240 + language * 0x100 + i * 2);
        }
    }
    Ok(result)
}

pub fn write_ds_banner(value: &DsBanner) -> Result<Vec<u8>> {
    let minimum = banner_minimum_size(value.version);
    if minimum == 0 || value.original.len() < minimum || value.title_count != banner_title_count(value.version) {
        return Err(err(ErrorCode::InvalidArgument, "DS banner model does not match its version"));
    }
    let mut result = value.original.clone();
    if let Some(slot) = result.get_mut(0x20..0x220) {
        slot.copy_from_slice(&value.icon);
    }
    for (i, &color) in value.palette.iter().enumerate() {
        put16(&mut result, 0x220 + i * 2, color);
    }
    for (language, slot) in value.title_slots.iter().enumerate().take(value.title_count) {
        for (i, &unit) in slot.iter().enumerate() {
            put16(&mut result, 0x240 + language * 0x100 + i * 2, unit);
        }
    }
    update_banner_crc(&mut result, 0x20, 0x820, 2);
    if value.version >= 2 {
        update_banner_crc(&mut result, 0x20, 0x920, 4);
    }
    if value.version >= 3 {
        update_banner_crc(&mut result, 0x20, 0xA20, 6);
    }
    Ok(result)
}

/// The title in language slot `index`, up to its first zero unit, as UTF-16 code units.
pub fn banner_title(value: &DsBanner, index: usize) -> Vec<u16> {
    if index >= value.title_count {
        return Vec::new();
    }
    let Some(slot) = value.title_slots.get(index) else { return Vec::new() };
    slot.iter().copied().take_while(|&unit| unit != 0).collect()
}

pub fn set_banner_title(value: &mut DsBanner, index: usize, title: &[u16]) -> Result<()> {
    if index >= value.title_count {
        return Err(err(ErrorCode::OutOfBounds, "banner language index is unavailable"));
    }
    if title.len() > 128 {
        return Err(err(ErrorCode::InvalidArgument, "banner title exceeds 128 UTF-16 code units"));
    }
    let slot = value.title_slots.get_mut(index).ok_or_else(|| err(ErrorCode::OutOfBounds, "banner language index is unavailable"))?;
    *slot = [0; 128];
    for (unit, &value) in slot.iter_mut().zip(title) {
        *unit = value;
    }
    Ok(())
}

pub fn parse_rom_user_metadata(bytes: &[u8]) -> Result<RomUserMetadata> {
    if !bytes.iter().all(|&c| c == b'\n' || c == b'\r' || c == b'\t' || (0x20..=0x7e).contains(&c)) {
        return Err(err(ErrorCode::ParseError, "ROM user metadata must be ASCII text"));
    }
    let text = String::from_utf8_lossy(bytes);
    let mut result = RomUserMetadata { original: bytes.to_vec(), ..RomUserMetadata::default() };
    let mut section_names = HashSet::new();
    for (index, raw) in text.split('\n').enumerate() {
        let line_number = index + 1;
        let line = trim(raw);
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if line.len() < 3 || !line.ends_with(']') {
                return Err(err(ErrorCode::ParseError, format!("metadata section header is malformed at line {line_number}")));
            }
            let name = line.get(1..line.len() - 1).unwrap_or_default().to_owned();
            validate_metadata_text(&name, "metadata section name")?;
            if !section_names.insert(name.clone()) {
                return Err(err(ErrorCode::ParseError, "metadata section is duplicated"));
            }
            result.sections.push(RomUserMetadataSection { name, entries: Vec::new() });
        } else {
            let Some(current) = result.sections.last_mut() else {
                return Err(err(ErrorCode::ParseError, "metadata entry appears before a section"));
            };
            let Some((key, value)) = line.split_once('=') else {
                return Err(err(ErrorCode::ParseError, format!("metadata entry is missing '=' at line {line_number}")));
            };
            let key = parse_key(key)?;
            let value = trim(value).to_owned();
            if value.contains(['\r', '\n']) {
                return Err(err(ErrorCode::ParseError, "metadata value contains a line break"));
            }
            if current.entries.iter().any(|entry| entry.key == key) {
                return Err(err(ErrorCode::ParseError, "metadata key is duplicated within a section"));
            }
            current.entries.push(RomUserMetadataEntry { key, value });
        }
    }
    result.original_sections = result.sections.clone();
    Ok(result)
}

pub fn write_rom_user_metadata(value: &RomUserMetadata) -> Result<Vec<u8>> {
    if value.sections == value.original_sections {
        return Ok(value.original.clone());
    }
    let mut text = String::new();
    let mut names = HashSet::new();
    for section in &value.sections {
        validate_metadata_text(&section.name, "metadata section name")?;
        if !names.insert(section.name.as_str()) {
            return Err(err(ErrorCode::InvalidArgument, "metadata section is duplicated"));
        }
        text.push('[');
        text.push_str(&section.name);
        text.push_str("]\n");
        let mut keys = HashSet::new();
        for entry in &section.entries {
            if entry.key < 0 || !keys.insert(entry.key) {
                return Err(err(ErrorCode::InvalidArgument, "metadata key is negative or duplicated"));
            }
            if entry.value.contains(['\r', '\n']) || !printable(&entry.value) {
                return Err(err(ErrorCode::InvalidArgument, "metadata value must be printable ASCII on one line"));
            }
            text.push_str(&format!("{}={}\n", entry.key, entry.value));
        }
    }
    Ok(text.into_bytes())
}
