//! Nitro 2D containers: NCLR, NCGR, NSCR, NCER and NANR, plus NCER cell rendering. Port of
//! `libs/nsmb-formats/src/g2d.cpp`. Blocks the codecs do not understand are kept and written back in order.

use bigstar_core::{Error, ErrorCode, Result};

use crate::{Bgr555Color, RgbaImage};

pub type FourCc = [u8; 4];

const PLTT: FourCc = *b"TTLP";
const PCMP: FourCc = *b"PMCP";
const CHAR: FourCc = *b"RAHC";
const CPOS: FourCc = *b"SOPC";
const SCRN: FourCc = *b"NRCS";
const CEBK: FourCc = *b"KBEC";
const ABNK: FourCc = *b"KNBA";

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nitro2dBlock {
    pub magic: FourCc,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nitro2dFile {
    pub magic: FourCc,
    pub byte_order: u16,
    pub version: u16,
    pub header_extra: Vec<u8>,
    pub blocks: Vec<Nitro2dBlock>,
}

impl Default for Nitro2dFile {
    fn default() -> Self {
        Self { magic: [0; 4], byte_order: 0xfeff, version: 0x0100, header_extra: Vec::new(), blocks: Vec::new() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nclr {
    pub file: Nitro2dFile,
    pub format: u32,
    pub extended_palette: bool,
    pub palette: Vec<u8>,
    pub compressed_palette_indices: Option<Vec<u16>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NcgrPosition {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ncgr {
    pub file: Nitro2dFile,
    pub height: u16,
    pub width: u16,
    pub pixel_format: u32,
    pub mapping_type: u32,
    pub character_format: u32,
    pub characters: Vec<u8>,
    pub position: Option<NcgrPosition>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nscr {
    pub file: Nitro2dFile,
    pub width: u16,
    pub height: u16,
    pub color_mode: u16,
    pub screen_format: u16,
    pub screen: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NcerOam {
    pub attr0: u16,
    pub attr1: u16,
    pub attr2: u16,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NcerBounds {
    pub max_x: i16,
    pub max_y: i16,
    pub min_x: i16,
    pub min_y: i16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NcerCell {
    pub attributes: u16,
    pub oam: Vec<NcerOam>,
    pub bounds: Option<NcerBounds>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NcerTransfer {
    pub source_offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ncer {
    pub file: Nitro2dFile,
    pub bank_attributes: u16,
    pub mapping_mode: u32,
    pub cells: Vec<NcerCell>,
    pub maximum_transfer_size: u32,
    pub transfers: Vec<NcerTransfer>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NanrFrame {
    pub duration: u16,
    pub padding: u16,
    pub content: Vec<u8>,
}

impl Default for NanrFrame {
    fn default() -> Self {
        Self { duration: 0, padding: 0xbeef, content: Vec::new() }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NanrSequence {
    pub loop_start: u16,
    pub animation_type: u32,
    pub play_mode: u32,
    pub frames: Vec<NanrFrame>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Nanr {
    pub file: Nitro2dFile,
    pub sequences: Vec<NanrSequence>,
}

fn err(code: ErrorCode, message: impl Into<String>) -> Error {
    Error::new(code, message)
}

pub(crate) fn fits(data: &[u8], offset: usize, size: usize) -> bool {
    offset <= data.len() && size <= data.len() - offset
}

// Readers return zero past the end. Every read below sits behind a `fits` check, as in the C++.
pub(crate) fn u16_at(data: &[u8], offset: usize) -> u16 {
    offset.checked_add(2).and_then(|end| data.get(offset..end)).and_then(|b| b.try_into().ok()).map_or(0, u16::from_le_bytes)
}

fn s16_at(data: &[u8], offset: usize) -> i16 {
    u16_at(data, offset) as i16
}

pub(crate) fn u32_at(data: &[u8], offset: usize) -> u32 {
    offset.checked_add(4).and_then(|end| data.get(offset..end)).and_then(|b| b.try_into().ok()).map_or(0, u32::from_le_bytes)
}

pub(crate) fn put16(data: &mut [u8], offset: usize, value: u16) {
    if let Some(slot) = data.get_mut(offset..offset + 2) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
}

pub(crate) fn put32(data: &mut [u8], offset: usize, value: u32) {
    if let Some(slot) = data.get_mut(offset..offset + 4) {
        slot.copy_from_slice(&value.to_le_bytes());
    }
}

fn read_cc(data: &[u8], offset: usize) -> FourCc {
    let mut magic = [0; 4];
    if let Some(bytes) = data.get(offset..offset + 4) {
        magic.copy_from_slice(bytes);
    }
    magic
}

fn show(magic: &FourCc) -> String {
    magic.iter().map(|&b| char::from(b)).collect()
}

fn block<'a>(file: &'a Nitro2dFile, magic: &FourCc) -> Option<&'a Nitro2dBlock> {
    file.blocks.iter().find(|b| &b.magic == magic)
}

fn set_block(file: &mut Nitro2dFile, magic: FourCc, data: Vec<u8>) {
    match file.blocks.iter_mut().find(|b| b.magic == magic) {
        Some(found) => found.data = data,
        None => file.blocks.insert(0, Nitro2dBlock { magic, data }),
    }
}

fn erase_block(file: &mut Nitro2dFile, magic: &FourCc) {
    file.blocks.retain(|b| &b.magic != magic);
}

fn expect(file: &Nitro2dFile, magic: &FourCc, name: &str) -> Result<()> {
    if &file.magic != magic {
        return Err(err(ErrorCode::ParseError, format!("{name} has file magic '{}'", show(&file.magic))));
    }
    Ok(())
}

fn slice(data: &[u8], offset: usize, size: usize, what: &str) -> Result<Vec<u8>> {
    if !fits(data, offset, size) {
        return Err(err(ErrorCode::OutOfBounds, format!("{what} points outside its block")));
    }
    Ok(data.get(offset..offset + size).unwrap_or_default().to_vec())
}

fn count16(count: usize, what: &str) -> Result<u16> {
    u16::try_from(count).map_err(|_| err(ErrorCode::InvalidArgument, format!("{what} exceeds 65535")))
}

fn size32(size: usize, what: &str) -> Result<u32> {
    u32::try_from(size).map_err(|_| err(ErrorCode::InvalidArgument, format!("{what} exceeds the format's 32-bit size")))
}

pub fn parse_nitro_2d_file(data: &[u8]) -> Result<Nitro2dFile> {
    if !fits(data, 0, 16) {
        return Err(err(ErrorCode::OutOfBounds, "Nitro 2D file header is truncated"));
    }
    let file_size = u32_at(data, 8) as usize;
    let header_size = usize::from(u16_at(data, 12));
    let block_count = usize::from(u16_at(data, 14));
    if u16_at(data, 4) != 0xfeff {
        return Err(err(ErrorCode::Unsupported, "only little-endian Nitro 2D files are supported"));
    }
    if header_size < 16 || header_size > file_size || file_size != data.len() {
        return Err(err(ErrorCode::ParseError, "Nitro 2D file size or header size is invalid"));
    }
    let mut file = Nitro2dFile {
        magic: read_cc(data, 0),
        byte_order: u16_at(data, 4),
        version: u16_at(data, 6),
        header_extra: data.get(16..header_size).unwrap_or_default().to_vec(),
        blocks: Vec::new(),
    };
    let mut offset = header_size;
    for _ in 0..block_count {
        if !fits(data, offset, 8) {
            return Err(err(ErrorCode::OutOfBounds, "Nitro 2D block header is truncated"));
        }
        let size = u32_at(data, offset + 4) as usize;
        if size < 8 || !fits(data, offset, size) {
            return Err(err(ErrorCode::ParseError, "Nitro 2D block size is invalid"));
        }
        file.blocks.push(Nitro2dBlock { magic: read_cc(data, offset), data: data.get(offset + 8..offset + size).unwrap_or_default().to_vec() });
        offset += size;
    }
    if offset != file_size {
        return Err(err(ErrorCode::ParseError, "Nitro 2D block table does not cover the declared file size"));
    }
    Ok(file)
}

pub fn write_nitro_2d_file(file: &Nitro2dFile) -> Result<Vec<u8>> {
    let block_count = count16(file.blocks.len(), "Nitro 2D block count")?;
    let header_size = count16(16 + file.header_extra.len(), "Nitro 2D header size")?;
    let mut total = usize::from(header_size);
    for entry in &file.blocks {
        size32(entry.data.len() + 8, "Nitro 2D block")?;
        total = total
            .checked_add(entry.data.len() + 8)
            .filter(|&t| t <= u32::MAX as usize)
            .ok_or_else(|| err(ErrorCode::InvalidArgument, "Nitro 2D file exceeds its 32-bit size"))?;
    }
    let mut output = Vec::with_capacity(total);
    output.extend_from_slice(&file.magic);
    output.extend_from_slice(&0xfeffu16.to_le_bytes());
    output.extend_from_slice(&file.version.to_le_bytes());
    output.extend_from_slice(&(total as u32).to_le_bytes());
    output.extend_from_slice(&header_size.to_le_bytes());
    output.extend_from_slice(&block_count.to_le_bytes());
    output.extend_from_slice(&file.header_extra);
    for entry in &file.blocks {
        output.extend_from_slice(&entry.magic);
        output.extend_from_slice(&((entry.data.len() + 8) as u32).to_le_bytes());
        output.extend_from_slice(&entry.data);
    }
    Ok(output)
}

pub fn parse_nclr(data: &[u8]) -> Result<Nclr> {
    let file = parse_nitro_2d_file(data)?;
    expect(&file, b"RLCN", "NCLR")?;
    let pltt = block(&file, &PLTT).filter(|b| b.data.len() >= 16).ok_or_else(|| err(ErrorCode::ParseError, "NCLR has no complete PLTT block"))?;
    let raw_size = u32_at(&pltt.data, 8) as usize;
    let raw_offset = u32_at(&pltt.data, 12) as usize;
    let palette = slice(&pltt.data, raw_offset, raw_size, "NCLR palette")?;
    let format = u32_at(&pltt.data, 0);
    let extended_palette = u32_at(&pltt.data, 4) != 0;
    let mut compressed_palette_indices = None;
    if let Some(pcmp) = block(&file, &PCMP) {
        if pcmp.data.len() < 8 {
            return Err(err(ErrorCode::ParseError, "NCLR PCMP block is truncated"));
        }
        let count = usize::from(u16_at(&pcmp.data, 0));
        let offset = u32_at(&pcmp.data, 4) as usize;
        if !fits(&pcmp.data, offset, count * 2) {
            return Err(err(ErrorCode::OutOfBounds, "NCLR PCMP index table is outside its block"));
        }
        compressed_palette_indices = Some((0..count).map(|i| u16_at(&pcmp.data, offset + i * 2)).collect());
    }
    Ok(Nclr { file, format, extended_palette, palette, compressed_palette_indices })
}

pub fn write_nclr(value: &Nclr) -> Result<Vec<u8>> {
    let palette_size = size32(value.palette.len(), "NCLR palette")?;
    let mut file = value.file.clone();
    file.magic = *b"RLCN";
    file.version = 0x0100;
    let mut pltt = vec![0; 16];
    put32(&mut pltt, 0, value.format);
    put32(&mut pltt, 4, u32::from(value.extended_palette));
    put32(&mut pltt, 8, palette_size);
    put32(&mut pltt, 12, 16);
    pltt.extend_from_slice(&value.palette);
    set_block(&mut file, PLTT, pltt);
    if let Some(indices) = &value.compressed_palette_indices {
        let count = count16(indices.len(), "NCLR compressed palette count")?;
        let mut pcmp = vec![0; 8];
        put16(&mut pcmp, 0, count);
        put16(&mut pcmp, 2, 0xbeef);
        put32(&mut pcmp, 4, 8);
        pcmp.extend(indices.iter().flat_map(|i| i.to_le_bytes()));
        set_block(&mut file, PCMP, pcmp);
    } else {
        erase_block(&mut file, &PCMP);
    }
    write_nitro_2d_file(&file)
}

pub fn parse_ncgr(data: &[u8]) -> Result<Ncgr> {
    let file = parse_nitro_2d_file(data)?;
    expect(&file, b"RGCN", "NCGR")?;
    let character =
        block(&file, &CHAR).filter(|b| b.data.len() >= 24).ok_or_else(|| err(ErrorCode::ParseError, "NCGR has no complete CHAR block"))?;
    let c = &character.data;
    let characters = slice(c, u32_at(c, 20) as usize, u32_at(c, 16) as usize, "NCGR character data")?;
    let (height, width) = (u16_at(c, 0), u16_at(c, 2));
    let (pixel_format, mapping_type, character_format) = (u32_at(c, 4), u32_at(c, 8), u32_at(c, 12));
    let mut position = None;
    if let Some(cpos) = block(&file, &CPOS) {
        if cpos.data.len() < 8 {
            return Err(err(ErrorCode::ParseError, "NCGR CPOS block is truncated"));
        }
        let p = &cpos.data;
        position = Some(NcgrPosition { x: u16_at(p, 0), y: u16_at(p, 2), width: u16_at(p, 4), height: u16_at(p, 6) });
    }
    Ok(Ncgr { file, height, width, pixel_format, mapping_type, character_format, characters, position })
}

pub fn write_ncgr(value: &Ncgr) -> Result<Vec<u8>> {
    let size = size32(value.characters.len(), "NCGR character data")?;
    let mut file = value.file.clone();
    file.magic = *b"RGCN";
    file.version = 0x0101;
    let mut character = vec![0; 24];
    put16(&mut character, 0, value.height);
    put16(&mut character, 2, value.width);
    put32(&mut character, 4, value.pixel_format);
    put32(&mut character, 8, value.mapping_type);
    put32(&mut character, 12, value.character_format);
    put32(&mut character, 16, size);
    put32(&mut character, 20, 24);
    character.extend_from_slice(&value.characters);
    set_block(&mut file, CHAR, character);
    if let Some(p) = value.position {
        let mut position = vec![0; 8];
        put16(&mut position, 0, p.x);
        put16(&mut position, 2, p.y);
        put16(&mut position, 4, p.width);
        put16(&mut position, 6, p.height);
        set_block(&mut file, CPOS, position);
    } else {
        erase_block(&mut file, &CPOS);
    }
    write_nitro_2d_file(&file)
}

pub fn parse_nscr(data: &[u8]) -> Result<Nscr> {
    let file = parse_nitro_2d_file(data)?;
    expect(&file, b"RCSN", "NSCR")?;
    let screen = block(&file, &SCRN).filter(|b| b.data.len() >= 12).ok_or_else(|| err(ErrorCode::ParseError, "NSCR has no complete SCRN block"))?;
    let s = &screen.data;
    let raw = slice(s, 12, u32_at(s, 8) as usize, "NSCR screen data")?;
    let (width, height, color_mode, screen_format) = (u16_at(s, 0), u16_at(s, 2), u16_at(s, 4), u16_at(s, 6));
    Ok(Nscr { file, width, height, color_mode, screen_format, screen: raw })
}

pub fn write_nscr(value: &Nscr) -> Result<Vec<u8>> {
    let size = size32(value.screen.len(), "NSCR screen data")?;
    let mut file = value.file.clone();
    file.magic = *b"RCSN";
    file.version = 0x0100;
    let mut screen = vec![0; 12];
    put16(&mut screen, 0, value.width);
    put16(&mut screen, 2, value.height);
    put16(&mut screen, 4, value.color_mode);
    put16(&mut screen, 6, value.screen_format);
    put32(&mut screen, 8, size);
    screen.extend_from_slice(&value.screen);
    set_block(&mut file, SCRN, screen);
    write_nitro_2d_file(&file)
}

pub fn parse_ncer(data: &[u8]) -> Result<Ncer> {
    let file = parse_nitro_2d_file(data)?;
    expect(&file, b"RECN", "NCER")?;
    let bank = block(&file, &CEBK).filter(|b| b.data.len() >= 24).ok_or_else(|| err(ErrorCode::ParseError, "NCER has no complete CEBK block"))?;
    let b = &bank.data;
    let count = usize::from(u16_at(b, 0));
    let attributes = u16_at(b, 2);
    let cells_offset = u32_at(b, 4) as usize;
    let stride = if attributes & 1 != 0 { 16 } else { 8 };
    if !fits(b, cells_offset, count * stride) {
        return Err(err(ErrorCode::OutOfBounds, "NCER cell table is outside its block"));
    }
    let mut cells = Vec::with_capacity(count);
    for index in 0..count {
        let offset = cells_offset + index * stride;
        let oam_count = usize::from(u16_at(b, offset));
        let oam_offset = u32_at(b, offset + 4) as usize;
        if !fits(b, oam_offset, oam_count * 6) {
            return Err(err(ErrorCode::OutOfBounds, "NCER OAM table is outside its block"));
        }
        let oam = (0..oam_count)
            .map(|i| {
                let at = oam_offset + i * 6;
                NcerOam { attr0: u16_at(b, at), attr1: u16_at(b, at + 2), attr2: u16_at(b, at + 4) }
            })
            .collect();
        let bounds = (stride == 16).then(|| NcerBounds {
            max_x: s16_at(b, offset + 8),
            max_y: s16_at(b, offset + 10),
            min_x: s16_at(b, offset + 12),
            min_y: s16_at(b, offset + 14),
        });
        cells.push(NcerCell { attributes: u16_at(b, offset + 2), oam, bounds });
    }
    let mut value = Ncer {
        file: Nitro2dFile::default(),
        bank_attributes: attributes,
        mapping_mode: u32_at(b, 8),
        cells,
        maximum_transfer_size: 0,
        transfers: Vec::new(),
    };
    let transfer_offset = u32_at(b, 12) as usize;
    if transfer_offset != 0 {
        if !fits(b, transfer_offset, 8) {
            return Err(err(ErrorCode::OutOfBounds, "NCER transfer header is outside its block"));
        }
        value.maximum_transfer_size = u32_at(b, transfer_offset);
        let entries = u32_at(b, transfer_offset + 4) as usize;
        if !fits(b, entries, count * 8) {
            return Err(err(ErrorCode::OutOfBounds, "NCER transfer table is outside its block"));
        }
        value.transfers =
            (0..count).map(|i| NcerTransfer { source_offset: u32_at(b, entries + i * 8), size: u32_at(b, entries + i * 8 + 4) }).collect();
    }
    value.file = file;
    Ok(value)
}

pub fn write_ncer(value: &Ncer) -> Result<Vec<u8>> {
    let cell_count = count16(value.cells.len(), "NCER cell count")?;
    let bounds = value.cells.iter().any(|cell| cell.bounds.is_some());
    let stride = if bounds { 16 } else { 8 };
    let mut bank = vec![0; 24 + value.cells.len() * stride];
    put16(&mut bank, 0, cell_count);
    put16(&mut bank, 2, (value.bank_attributes & !1) | u16::from(bounds));
    put32(&mut bank, 4, 24);
    put32(&mut bank, 8, value.mapping_mode);
    for (index, cell) in value.cells.iter().enumerate() {
        let oam_count = count16(cell.oam.len(), "NCER OAM count")?;
        let cell_offset = 24 + index * stride;
        let oam_offset = size32(bank.len(), "NCER bank")?;
        put16(&mut bank, cell_offset, oam_count);
        put16(&mut bank, cell_offset + 2, cell.attributes);
        put32(&mut bank, cell_offset + 4, oam_offset);
        if bounds {
            let b = cell.bounds.unwrap_or_default();
            put16(&mut bank, cell_offset + 8, b.max_x as u16);
            put16(&mut bank, cell_offset + 10, b.max_y as u16);
            put16(&mut bank, cell_offset + 12, b.min_x as u16);
            put16(&mut bank, cell_offset + 14, b.min_y as u16);
        }
        for oam in &cell.oam {
            bank.extend_from_slice(&oam.attr0.to_le_bytes());
            bank.extend_from_slice(&oam.attr1.to_le_bytes());
            bank.extend_from_slice(&oam.attr2.to_le_bytes());
        }
    }
    if !value.transfers.is_empty() {
        if value.transfers.len() != value.cells.len() {
            return Err(err(ErrorCode::InvalidArgument, "NCER transfer count must match its cell count"));
        }
        let header = size32(bank.len(), "NCER bank")?;
        put32(&mut bank, 12, header);
        bank.extend_from_slice(&value.maximum_transfer_size.to_le_bytes());
        bank.extend_from_slice(&(header + 8).to_le_bytes());
        for transfer in &value.transfers {
            bank.extend_from_slice(&transfer.source_offset.to_le_bytes());
            bank.extend_from_slice(&transfer.size.to_le_bytes());
        }
    }
    let mut file = value.file.clone();
    file.magic = *b"RECN";
    file.version = 0x0100;
    set_block(&mut file, CEBK, bank);
    write_nitro_2d_file(&file)
}

fn nanr_content_size(animation_type: u32) -> Option<usize> {
    match animation_type & 0xFF {
        0 => Some(2),
        1 => Some(16),
        2 => Some(8),
        _ => None,
    }
}

pub fn parse_nanr(data: &[u8]) -> Result<Nanr> {
    let file = parse_nitro_2d_file(data)?;
    expect(&file, b"RNAN", "NANR")?;
    let bank = block(&file, &ABNK).filter(|b| b.data.len() >= 24).ok_or_else(|| err(ErrorCode::ParseError, "NANR has no complete ABNK block"))?;
    let b = &bank.data;
    let sequence_count = usize::from(u16_at(b, 0));
    let total_frames = usize::from(u16_at(b, 2));
    let sequence_offset = u32_at(b, 4) as usize;
    let frame_offset = u32_at(b, 8) as usize;
    let content_offset = u32_at(b, 12) as usize;
    if !fits(b, sequence_offset, sequence_count * 16) {
        return Err(err(ErrorCode::OutOfBounds, "NANR sequence table is outside its block"));
    }
    let mut sequences = Vec::with_capacity(sequence_count);
    let mut counted_frames = 0;
    for index in 0..sequence_count {
        let offset = sequence_offset + index * 16;
        let frame_count = usize::from(u16_at(b, offset));
        let mut sequence = NanrSequence {
            loop_start: u16_at(b, offset + 2),
            animation_type: u32_at(b, offset + 4),
            play_mode: u32_at(b, offset + 8),
            frames: Vec::with_capacity(frame_count),
        };
        let frames = frame_offset + u32_at(b, offset + 12) as usize;
        if !fits(b, frames, frame_count * 8) {
            return Err(err(ErrorCode::OutOfBounds, "NANR frame table is outside its block"));
        }
        let content_size =
            nanr_content_size(sequence.animation_type).ok_or_else(|| err(ErrorCode::Unsupported, "NANR animation element type is unsupported"))?;
        for i in 0..frame_count {
            let frame = frames + i * 8;
            let content = content_offset + u32_at(b, frame) as usize;
            sequence.frames.push(NanrFrame {
                duration: u16_at(b, frame + 4),
                padding: u16_at(b, frame + 6),
                content: slice(b, content, content_size, "NANR frame content")?,
            });
        }
        counted_frames += frame_count;
        sequences.push(sequence);
    }
    if counted_frames != total_frames {
        return Err(err(ErrorCode::ParseError, "NANR total frame count disagrees with its sequences"));
    }
    Ok(Nanr { file, sequences })
}

pub fn write_nanr(value: &Nanr) -> Result<Vec<u8>> {
    let sequence_count = count16(value.sequences.len(), "NANR sequence count")?;
    let total: usize = value.sequences.iter().map(|s| s.frames.len()).sum();
    let total_frames = count16(total, "NANR frame count")?;
    let sequence_offset = 24usize;
    let frame_offset = sequence_offset + value.sequences.len() * 16;
    let content_offset = frame_offset + total * 8;
    let mut bank = vec![0; content_offset];
    put16(&mut bank, 0, sequence_count);
    put16(&mut bank, 2, total_frames);
    put32(&mut bank, 4, sequence_offset as u32);
    put32(&mut bank, 8, frame_offset as u32);
    put32(&mut bank, 12, content_offset as u32);
    let mut frame_index = 0usize;
    for (index, sequence) in value.sequences.iter().enumerate() {
        let frame_count = count16(sequence.frames.len(), "NANR sequence frame count")?;
        let entry = sequence_offset + index * 16;
        put16(&mut bank, entry, frame_count);
        put16(&mut bank, entry + 2, sequence.loop_start);
        put32(&mut bank, entry + 4, sequence.animation_type);
        put32(&mut bank, entry + 8, sequence.play_mode);
        put32(&mut bank, entry + 12, (frame_index * 8) as u32);
        let expected =
            nanr_content_size(sequence.animation_type).ok_or_else(|| err(ErrorCode::Unsupported, "NANR animation element type is unsupported"))?;
        for frame in &sequence.frames {
            if frame.content.len() != expected {
                return Err(err(ErrorCode::InvalidArgument, "NANR frame content has the wrong size for its animation element"));
            }
            let frame_entry = frame_offset + frame_index * 8;
            let content = size32(bank.len() - content_offset, "NANR content")?;
            put32(&mut bank, frame_entry, content);
            put16(&mut bank, frame_entry + 4, frame.duration);
            put16(&mut bank, frame_entry + 6, frame.padding);
            bank.extend_from_slice(&frame.content);
            frame_index += 1;
        }
    }
    let mut file = value.file.clone();
    file.magic = *b"RNAN";
    file.version = 0x0100;
    set_block(&mut file, ABNK, bank);
    write_nitro_2d_file(&file)
}

/// OAM object sizes in pixels, indexed by shape then size.
const OAM_DIMENSIONS: [[(i32, i32); 4]; 3] =
    [[(8, 8), (16, 16), (32, 32), (64, 64)], [(16, 8), (32, 8), (32, 16), (64, 32)], [(8, 16), (8, 32), (16, 32), (32, 64)]];

struct OamPlacement {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

fn place_oam(oam: &NcerOam) -> Result<OamPlacement> {
    let shape = usize::from((oam.attr0 >> 14) & 3);
    let size = usize::from((oam.attr1 >> 14) & 3);
    let (width, height) = OAM_DIMENSIONS
        .get(shape)
        .and_then(|sizes| sizes.get(size))
        .copied()
        .ok_or_else(|| err(ErrorCode::Unsupported, "NCER OAM shape is prohibited"))?;
    let mut x = i32::from(oam.attr1 & 0x1ff);
    let mut y = i32::from(oam.attr0 & 0xff);
    if x >= 256 {
        x -= 512;
    }
    if y >= 128 {
        y -= 256;
    }
    Ok(OamPlacement { x, y, width, height })
}

pub fn render_ncer_cell(cells: &Ncer, cell_index: usize, characters: &[u8], palette: &[Bgr555Color]) -> Result<RgbaImage> {
    let cell = cells.cells.get(cell_index).ok_or_else(|| err(ErrorCode::NotFound, "NCER cell index is outside the bank"))?;
    if cell.oam.is_empty() {
        return Ok(RgbaImage::default());
    }
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    for oam in &cell.oam {
        let p = place_oam(oam)?;
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x + p.width);
        max_y = max_y.max(p.y + p.height);
    }
    if max_x <= min_x || max_y <= min_y || max_x - min_x > 1024 || max_y - min_y > 1024 {
        return Err(err(ErrorCode::InvalidArgument, "NCER cell bounds are invalid"));
    }
    let (image_width, image_height) = ((max_x - min_x) as usize, (max_y - min_y) as usize);
    let mut image = RgbaImage { width: image_width, height: image_height, pixels: vec![0; image_width * image_height * 4] };
    let mapping_mode = cells.mapping_mode;
    for oam in &cell.oam {
        let p = place_oam(oam)?;
        let eight = oam.attr0 & 0x2000 != 0;
        let affine = oam.attr0 & 0x0100 != 0;
        let hflip = !affine && oam.attr1 & 0x1000 != 0;
        let vflip = !affine && oam.attr1 & 0x2000 != 0;
        let palette_base = if eight { 0 } else { usize::from(oam.attr2 >> 12) * 16 };
        let character = usize::from(oam.attr2 & 0x3ff);
        let bytes_per_tile = if eight { 64 } else { 32 };
        let base_byte = character * 32 * if mapping_mode < 4 { 1usize << mapping_mode } else { 1 };
        for y in 0..p.height {
            for x in 0..p.width {
                let sx = if hflip { p.width - 1 - x } else { x };
                let sy = if vflip { p.height - 1 - y } else { y };
                let (tile_x, tile_y) = ((sx / 8) as usize, (sy / 8) as usize);
                let tile_byte = if mapping_mode < 4 {
                    base_byte + (tile_y * (p.width as usize / 8) + tile_x) * bytes_per_tile
                } else {
                    (character + if eight { tile_y * 32 + tile_x * 2 } else { tile_y * 32 + tile_x }) * 32
                };
                let within = (sy % 8) as usize * 8 + (sx % 8) as usize;
                let color_index = if eight {
                    let Some(&value) = characters.get(tile_byte + within) else { continue };
                    usize::from(value)
                } else {
                    let Some(&value) = characters.get(tile_byte + within / 2) else { continue };
                    usize::from((value >> ((within % 2) * 4)) & 15)
                };
                if color_index == 0 {
                    continue;
                }
                let Some(color) = palette.get(palette_base + color_index) else { continue };
                let (dx, dy) = (p.x - min_x + x, p.y - min_y + y);
                if dx < 0 || dy < 0 || dx as usize >= image.width || dy as usize >= image.height {
                    continue;
                }
                let pixel = (dy as usize * image.width + dx as usize) * 4;
                if let Some(slot) = image.pixels.get_mut(pixel..pixel + 4) {
                    slot.copy_from_slice(&[color.red, color.green, color.blue, 255]);
                }
            }
        }
    }
    Ok(image)
}
