//! Graphics format identification, BGR555 palettes, 4/8bpp tiles, screen maps, Map16, object layouts and
//! quantisation. Port of the non-compression half of `libs/nsmb-formats/src/graphics.cpp`.

use std::collections::BTreeMap;

use bigstar_core::{Error, ErrorCode, LittleEndianReader, Result};
use serde::{Deserialize, Serialize};

use crate::FourCc;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Bgr555Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub transparent: bool,
}

impl Bgr555Color {
    pub const fn new(red: u8, green: u8, blue: u8, transparent: bool) -> Self {
        Self { red, green, blue, transparent }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedTile4Bpp {
    pub pixels: [u8; 64],
}

impl Default for IndexedTile4Bpp {
    fn default() -> Self {
        Self { pixels: [0; 64] }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexedTile8Bpp {
    pub pixels: [u8; 64],
}

impl Default for IndexedTile8Bpp {
    fn default() -> Self {
        Self { pixels: [0; 64] }
    }
}

/// Tile and palette are counted from the resource's own offsets, so an entry pointing below them lands negative.
/// Those entries name graphics a different tileset loads into shared VRAM, which this resource does not carry: the
/// game draws them, an editor holding one tileset cannot, so nothing is drawn for them. They keep their negative
/// value rather than being clamped, so writing the map back reproduces the byte it was read from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScreenMapEntry {
    pub tile: i32,
    pub palette: i16,
    pub horizontal_flip: bool,
    pub vertical_flip: bool,
}

impl ScreenMapEntry {
    pub const fn new(tile: i32, palette: i16, horizontal_flip: bool, vertical_flip: bool) -> Self {
        Self { tile, palette, horizontal_flip, vertical_flip }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Map16Tile {
    pub tiles: [ScreenMapEntry; 4],
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RgbaImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedImage {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub palette: Vec<Bgr555Color>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EncodedObjectLayouts {
    pub definitions: Vec<u8>,
    pub index: Vec<u8>,
}

/// A definition whose two tile bytes are both zero names Map16 block 0, which every tileset leaves blank. It is
/// still a block: the game writes it into the level's tile array like any other, so it hides whatever was already
/// there.
pub const OBJECT_LAYOUT_EMPTY_TILE: i32 = -1;

/// A cell no part of the layout reaches -- the empty half of a slope. Nothing is written there, so whatever was
/// already in the tile array stays.
pub const OBJECT_LAYOUT_UNWRITTEN: i32 = i32::MIN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectLayoutTile {
    pub tile: i32,
    pub control: u8,
}

impl ObjectLayoutTile {
    pub const fn new(tile: i32, control: u8) -> Self {
        Self { tile, control }
    }
}

impl Default for ObjectLayoutTile {
    fn default() -> Self {
        Self { tile: OBJECT_LAYOUT_EMPTY_TILE, control: 0 }
    }
}

/// The control bytes that open a slope's two side blocks. Any other byte with the top bit set opens its steps.
const OBJECT_LAYOUT_SLOPE_BEHIND: u8 = 0x84;
const OBJECT_LAYOUT_SLOPE_FILL: u8 = 0x85;

/// A fill tile with exactly this control is laid once, against the step; any other repeats along the row.
const OBJECT_LAYOUT_SLOPE_ONCE: u8 = 4;

/// A slope's `rows` are one step of it, laid corner to corner along the diagonal. Each step's rows are then carried
/// on to the edges of the object: `slope_fill` on the side the slope runs towards, which is the solid ground under
/// it or the ceiling over it, and `slope_behind` on the side it came from. No retail tileset has a `slope_behind`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectLayout {
    pub rows: Vec<Vec<ObjectLayoutTile>>,
    pub slope_fill: Vec<Vec<ObjectLayoutTile>>,
    pub slope_behind: Vec<Vec<ObjectLayoutTile>>,
    pub slope: bool,
    pub slope_left: bool,
    pub slope_down: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsFormatKind {
    Unknown,
    NitroCharacterGraphics,
    NitroPalette,
    NitroScreenMap,
    NitroCellLayout,
    NitroCellAnimation,
    NitroTexture,
    NitroModel,
    RawCharacterGraphicsCandidate,
    RawPaletteCandidate,
    RawScreenMapCandidate,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NitroHeader {
    pub magic: FourCc,
    pub byte_order_mark: u16,
    pub version: u16,
    pub file_size: u32,
    pub header_size: u16,
    pub block_count: u16,
}

fn err(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

pub fn identify_graphics_format(data: &[u8], path_hint: &str) -> GraphicsFormatKind {
    // BNCL (JNCL) and BNCD (JNCD) are not NitroSystem formats; they are UI resources and are not claimed here.
    for (magic, kind) in [
        (b"RGCN", GraphicsFormatKind::NitroCharacterGraphics),
        (b"RLCN", GraphicsFormatKind::NitroPalette),
        (b"RCSN", GraphicsFormatKind::NitroScreenMap),
        (b"RECN", GraphicsFormatKind::NitroCellLayout),
        (b"RNAN", GraphicsFormatKind::NitroCellAnimation),
        (b"BTX0", GraphicsFormatKind::NitroTexture),
        (b"BMD0", GraphicsFormatKind::NitroModel),
    ] {
        if data.starts_with(magic) {
            return kind;
        }
    }
    let hint = path_hint.to_ascii_lowercase();
    if hint.ends_with(".ncgr") || hint.ends_with("_ncg.bin") {
        GraphicsFormatKind::RawCharacterGraphicsCandidate
    } else if hint.ends_with(".nclr") || hint.ends_with("_ncl.bin") {
        GraphicsFormatKind::RawPaletteCandidate
    } else if hint.ends_with(".nscr") || hint.ends_with("_nsc.bin") {
        GraphicsFormatKind::RawScreenMapCandidate
    } else if hint.ends_with(".nsbtx") {
        GraphicsFormatKind::NitroTexture
    } else if hint.ends_with(".nsbmd") {
        GraphicsFormatKind::NitroModel
    } else {
        GraphicsFormatKind::Unknown
    }
}

pub fn graphics_format_name(kind: GraphicsFormatKind) -> &'static str {
    match kind {
        GraphicsFormatKind::NitroCharacterGraphics => "Nitro character graphics",
        GraphicsFormatKind::NitroPalette => "Nitro palette",
        GraphicsFormatKind::NitroScreenMap => "Nitro screen map",
        GraphicsFormatKind::NitroCellLayout => "Nitro cell layout",
        GraphicsFormatKind::NitroCellAnimation => "Nitro cell animation",
        GraphicsFormatKind::NitroTexture => "Nitro texture",
        GraphicsFormatKind::NitroModel => "Nitro model",
        GraphicsFormatKind::RawCharacterGraphicsCandidate => "raw character graphics candidate",
        GraphicsFormatKind::RawPaletteCandidate => "raw palette candidate",
        GraphicsFormatKind::RawScreenMapCandidate => "raw screen map candidate",
        GraphicsFormatKind::Unknown => "unknown",
    }
}

pub fn parse_nitro_header(data: &[u8]) -> Result<NitroHeader> {
    if data.len() < 16 {
        return Err(err(ErrorCode::OutOfBounds, "Nitro format header requires at least 16 bytes"));
    }
    let r = LittleEndianReader::new(data);
    let fields = (|| Ok::<_, Error>((r.u16(4)?, r.u16(6)?, r.u32(8)?, r.u16(12)?, r.u16(14)?)))();
    let Ok((byte_order_mark, version, file_size, header_size, block_count)) = fields else {
        return Err(err(ErrorCode::ParseError, "could not read Nitro header fields"));
    };
    if file_size as usize > data.len() {
        return Err(err(ErrorCode::ParseError, "Nitro header file size exceeds available data"));
    }
    let mut magic = [0u8; 4];
    magic.copy_from_slice(data.get(..4).unwrap_or(&[0; 4]));
    Ok(NitroHeader { magic, byte_order_mark, version, file_size, header_size, block_count })
}

pub(crate) fn expand_5_to_8(value: u16) -> u8 {
    ((value << 3) | (value >> 2)) as u8
}

pub(crate) fn shrink_8_to_5(value: u8) -> u16 {
    ((u32::from(value) * 31 + 127) / 255) as u16
}

pub(crate) fn unpack_bgr555(packed: u16, transparent: bool) -> Bgr555Color {
    Bgr555Color {
        red: expand_5_to_8(packed & 0x1F),
        green: expand_5_to_8((packed >> 5) & 0x1F),
        blue: expand_5_to_8((packed >> 10) & 0x1F),
        transparent,
    }
}

pub(crate) fn pack_bgr555(red: u8, green: u8, blue: u8) -> u16 {
    shrink_8_to_5(red) | (shrink_8_to_5(green) << 5) | (shrink_8_to_5(blue) << 10)
}

pub fn decode_bgr555_palette(data: &[u8], first_color_transparent: bool) -> Result<Vec<Bgr555Color>> {
    if !data.len().is_multiple_of(2) {
        return Err(err(ErrorCode::ParseError, "BGR555 palette data must contain an even number of bytes"));
    }
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .enumerate()
        .map(|(i, &pair)| unpack_bgr555(u16::from_le_bytes(pair), first_color_transparent && i == 0))
        .collect())
}

/// A tileset's palette file is several 256-colour palettes end to end, and the game gives each one its own
/// transparent first colour. Marks colour 0 of every block, which a flat decode leaves opaque from the second
/// block on.
pub fn mark_palette_blocks_transparent(palette: &mut [Bgr555Color], block: usize) {
    if block == 0 {
        return;
    }
    for color in palette.iter_mut().step_by(block) {
        color.transparent = true;
    }
}

pub fn encode_bgr555_palette(colors: &[Bgr555Color]) -> Result<Vec<u8>> {
    if colors.len() > usize::MAX / 2 {
        return Err(err(ErrorCode::OutOfBounds, "palette is too large"));
    }
    Ok(colors.iter().flat_map(|c| pack_bgr555(c.red, c.green, c.blue).to_le_bytes()).collect())
}

pub fn decode_4bpp_tiles(data: &[u8]) -> Result<Vec<IndexedTile4Bpp>> {
    if !data.len().is_multiple_of(32) {
        return Err(err(ErrorCode::ParseError, "4bpp tile data must be a multiple of 32 bytes"));
    }
    Ok(data
        .as_chunks::<32>()
        .0
        .iter()
        .map(|chunk| {
            let mut tile = IndexedTile4Bpp::default();
            for (pair, packed) in tile.pixels.as_chunks_mut::<2>().0.iter_mut().zip(chunk) {
                *pair = [packed & 0x0F, (packed >> 4) & 0x0F];
            }
            tile
        })
        .collect())
}

pub fn encode_4bpp_tiles(tiles: &[IndexedTile4Bpp]) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(tiles.len() * 32);
    for tile in tiles {
        for &[low, high] in tile.pixels.as_chunks::<2>().0 {
            if low > 15 || high > 15 {
                return Err(err(ErrorCode::InvalidArgument, "4bpp tile contains a palette index above 15"));
            }
            output.push(low | (high << 4));
        }
    }
    Ok(output)
}

pub fn decode_8bpp_tiles(data: &[u8]) -> Result<Vec<IndexedTile8Bpp>> {
    if !data.len().is_multiple_of(64) {
        return Err(err(ErrorCode::ParseError, "8bpp tile data must be a multiple of 64 bytes"));
    }
    Ok(data.as_chunks::<64>().0.iter().map(|&pixels| IndexedTile8Bpp { pixels }).collect())
}

pub fn encode_8bpp_tiles(tiles: &[IndexedTile8Bpp]) -> Result<Vec<u8>> {
    Ok(tiles.iter().flat_map(|tile| tile.pixels).collect())
}

pub fn decode_screen_map(data: &[u8], tile_offset: u16, palette_offset: u8) -> Result<Vec<ScreenMapEntry>> {
    if !data.len().is_multiple_of(2) {
        return Err(err(ErrorCode::ParseError, "screen map must contain 16-bit entries"));
    }
    Ok(data
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&pair| {
            let packed = u16::from_le_bytes(pair);
            ScreenMapEntry {
                tile: i32::from(packed & 0x03FF) - i32::from(tile_offset),
                palette: (i32::from((packed >> 12) & 0x0F) - i32::from(palette_offset)) as i16,
                horizontal_flip: packed & 0x0400 != 0,
                vertical_flip: packed & 0x0800 != 0,
            }
        })
        .collect())
}

pub fn decode_map16(data: &[u8], tile_offset: u16, palette_offset: u8) -> Result<Vec<Map16Tile>> {
    if !data.len().is_multiple_of(8) {
        return Err(err(ErrorCode::ParseError, "Map16 data must be a multiple of 8 bytes"));
    }
    let entries = decode_screen_map(data, tile_offset, palette_offset)?;
    Ok(entries.as_chunks::<4>().0.iter().map(|&tiles| Map16Tile { tiles }).collect())
}

pub fn encode_screen_map(entries: &[ScreenMapEntry], tile_offset: u16, palette_offset: u8) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(entries.len() * 2);
    for entry in entries {
        let tile = i64::from(entry.tile) + i64::from(tile_offset);
        let palette = i64::from(entry.palette) + i64::from(palette_offset);
        if !(0..=0x3ff).contains(&tile) || !(0..=15).contains(&palette) {
            return Err(err(ErrorCode::InvalidArgument, "screen-map tile or palette index is not representable"));
        }
        let packed =
            tile as u16 | if entry.horizontal_flip { 0x400 } else { 0 } | if entry.vertical_flip { 0x800 } else { 0 } | ((palette as u16) << 12);
        output.extend_from_slice(&packed.to_le_bytes());
    }
    Ok(output)
}

pub fn encode_map16(tiles: &[Map16Tile], tile_offset: u16, palette_offset: u8) -> Result<Vec<u8>> {
    let entries: Vec<ScreenMapEntry> = tiles.iter().flat_map(|tile| tile.tiles).collect();
    encode_screen_map(&entries, tile_offset, palette_offset)
}

pub fn render_screen_map(map: &[ScreenMapEntry], width_tiles: usize, tiles: &[IndexedTile8Bpp], palette: &[Bgr555Color]) -> Result<RgbaImage> {
    if width_tiles == 0 || !map.len().is_multiple_of(width_tiles) {
        return Err(err(ErrorCode::InvalidArgument, "screen map width does not divide its entries"));
    }
    let width = width_tiles * 8;
    let height = (map.len() / width_tiles) * 8;
    let mut image = RgbaImage { width, height, pixels: vec![0; width * height * 4] };
    for (entry_index, entry) in map.iter().enumerate() {
        // An entry below the resource's offsets, or past the tiles it carries, names graphics this resource does
        // not hold. Leave it clear rather than drawing whatever tile happens to sit at that index.
        if entry.tile < 0 || entry.palette < 0 {
            continue;
        }
        let Some(tile) = tiles.get(entry.tile as usize) else { continue };
        for y in 0..8usize {
            for x in 0..8usize {
                let source_x = if entry.horizontal_flip { 7 - x } else { x };
                let source_y = if entry.vertical_flip { 7 - y } else { y };
                let color_index = usize::from(tile.pixels.get(source_y * 8 + source_x).copied().unwrap_or(0)) + entry.palette as usize * 256;
                let Some(color) = palette.get(color_index) else { continue };
                let target_x = (entry_index % width_tiles) * 8 + x;
                let target_y = (entry_index / width_tiles) * 8 + y;
                let pixel = (target_y * width + target_x) * 4;
                if let Some(slot) = image.pixels.get_mut(pixel..pixel + 4) {
                    slot.copy_from_slice(&[color.red, color.green, color.blue, if color.transparent { 0 } else { 255 }]);
                }
            }
        }
    }
    Ok(image)
}

pub fn decode_object_layouts(definitions: &[u8], index: &[u8], tile_page_offset: u8) -> Result<Vec<ObjectLayout>> {
    if !index.len().is_multiple_of(4) {
        return Err(err(ErrorCode::ParseError, "object layout index must use four-byte entries"));
    }
    let mut objects = Vec::with_capacity(index.len() / 4);
    for &[low, high, _, _] in index.as_chunks::<4>().0 {
        let offset = usize::from(u16::from_le_bytes([low, high]));
        if offset >= definitions.len() {
            return Err(err(ErrorCode::OutOfBounds, "object layout offset is outside its definition file"));
        }
        // The steps, then the two side blocks of a slope: behind and fill.
        let mut sections: [Vec<Vec<ObjectLayoutTile>>; 3] = [vec![Vec::new()], Vec::new(), Vec::new()];
        let mut section = 0;
        let mut layout = ObjectLayout::default();
        let mut cursor = offset;
        let mut terminated = false;
        while let Some(&control) = definitions.get(cursor) {
            cursor += 1;
            if control == 0xFF {
                terminated = true;
                break;
            }
            let Some(rows) = sections.get_mut(section) else { break };
            if control == 0xFE {
                if rows.last().is_some_and(|row| !row.is_empty()) {
                    rows.push(Vec::new());
                }
                continue;
            }
            if control & 0x80 != 0 {
                // The object's first byte says it is a slope and which way it runs. In a slope every later one
                // starts a block over, as `InitBGDat_processSlope @ 020b3f8c` reads them.
                if cursor == offset + 1 {
                    layout.slope = true;
                    layout.slope_left = control & 1 != 0;
                    layout.slope_down = control & 2 != 0;
                } else if layout.slope {
                    section = match control {
                        OBJECT_LAYOUT_SLOPE_BEHIND => 1,
                        OBJECT_LAYOUT_SLOPE_FILL => 2,
                        _ => 0,
                    };
                    if let Some(rows) = sections.get_mut(section) {
                        *rows = vec![Vec::new()];
                    }
                }
                continue;
            }
            let (Some(&low), Some(&high)) = (definitions.get(cursor), definitions.get(cursor + 1)) else {
                return Err(err(ErrorCode::OutOfBounds, "object layout tile is truncated"));
            };
            cursor += 2;
            let mut tile =
                if low == 0 && high == 0 { OBJECT_LAYOUT_EMPTY_TILE } else { i32::from(low) + (i32::from(high) - i32::from(tile_page_offset)) * 256 };
            if control & 0x40 != 0 {
                tile += 768;
            }
            if let Some(row) = rows.last_mut() {
                row.push(ObjectLayoutTile { tile, control });
            }
        }
        if !terminated {
            return Err(err(ErrorCode::OutOfBounds, "object layout definition is missing its terminator"));
        }
        for rows in &mut sections {
            while rows.last().is_some_and(Vec::is_empty) {
                rows.pop();
            }
        }
        [layout.rows, layout.slope_behind, layout.slope_fill] = sections;
        objects.push(layout);
    }
    Ok(objects)
}

pub fn encode_object_layouts(objects: &[ObjectLayout], tile_page_offset: u8, index_template: &[u8]) -> Result<EncodedObjectLayouts> {
    if !index_template.is_empty() && index_template.len() != objects.len() * 4 {
        return Err(err(ErrorCode::InvalidArgument, "object index template has the wrong size"));
    }
    let mut output = EncodedObjectLayouts {
        definitions: Vec::new(),
        index: if index_template.is_empty() { vec![0; objects.len() * 4] } else { index_template.to_vec() },
    };
    let out_of_range = || err(ErrorCode::InvalidArgument, "object tile is outside the representable page range");
    for (object_index, object) in objects.iter().enumerate() {
        let Ok(offset) = u16::try_from(output.definitions.len()) else {
            return Err(err(ErrorCode::OutOfBounds, "object definitions exceed the 16-bit index range"));
        };
        if let Some(slot) = output.index.get_mut(object_index * 4..object_index * 4 + 2) {
            slot.copy_from_slice(&offset.to_le_bytes());
        }
        if object.rows.is_empty() {
            return Err(err(ErrorCode::InvalidArgument, "object layout has no rows"));
        }
        if object.slope {
            output.definitions.push(0x80 | u8::from(object.slope_left) | (u8::from(object.slope_down) << 1));
        }
        // Every row ends in a line break, the last one too: the game counts a slope's rows by them. A slope's side
        // blocks follow its steps, each behind its own marker.
        let sides: [(u8, &[Vec<ObjectLayoutTile>]); 2] = if object.slope {
            [(OBJECT_LAYOUT_SLOPE_BEHIND, &object.slope_behind), (OBJECT_LAYOUT_SLOPE_FILL, &object.slope_fill)]
        } else {
            [(0, &[]), (0, &[])]
        };
        let mut markers = sides.iter().flat_map(|&(marker, rows)| rows.iter().enumerate().map(move |(i, row)| ((i == 0).then_some(marker), row)));
        for (marker, row) in object.rows.iter().map(|row| (None, row)).chain(&mut markers) {
            if let Some(marker) = marker {
                output.definitions.push(marker);
            }
            for entry in row {
                if entry.control & 0x80 != 0 {
                    return Err(err(ErrorCode::InvalidArgument, "object tile control collides with a layout marker"));
                }
                output.definitions.push(entry.control);
                if entry.tile < 0 {
                    output.definitions.extend_from_slice(&[0, 0]);
                    continue;
                }
                let tile = i64::from(entry.tile) - if entry.control & 0x40 != 0 { 768 } else { 0 };
                if !(0..=0xffff).contains(&tile) {
                    return Err(out_of_range());
                }
                let page = (tile >> 8) + i64::from(tile_page_offset);
                if !(0..=255).contains(&page) || (tile & 0xff == 0 && page == 0) {
                    return Err(out_of_range());
                }
                output.definitions.push(tile as u8);
                output.definitions.push(page as u8);
            }
            output.definitions.push(0xfe);
        }
        output.definitions.push(0xff);
    }
    Ok(output)
}

/// The tiles one row of a slope's side block puts in a run of `count` cells, nearest the step first. A run may come
/// back short, and the cells past its end are not written. Follows `processRect_processRow @ 020b4d94`.
fn slope_run(row: &[ObjectLayoutTile], count: usize, towards_step: bool) -> Vec<i32> {
    let mut run = Vec::with_capacity(count);
    if row.iter().any(|tile| tile.control & 1 != 0) {
        // A row with a horizontal repeat is laid as a rectangle's would be, left to right whichever way the slope
        // runs.
        let first_repeat = row.iter().position(|tile| tile.control & 1 != 0).unwrap_or(0);
        let after_repeat = row.iter().skip(first_repeat).position(|tile| tile.control & 1 == 0).map_or(row.len(), |i| first_repeat + i);
        let (before, rest) = row.split_at(first_repeat);
        let (repeat, after) = rest.split_at(after_repeat - first_repeat);
        run.extend(before.iter().take(count).map(|tile| tile.tile));
        let mut repeating = repeat.iter().cycle();
        while run.len() + after.len() < count {
            let Some(tile) = repeating.next() else { break };
            run.push(tile.tile);
        }
        let mut trailing = after.iter().cycle();
        while run.len() < count {
            let Some(tile) = trailing.next() else { break };
            run.push(tile.tile);
        }
        if towards_step {
            run.reverse();
        }
        return run;
    }
    // Otherwise the row is read from its end nearest the step: tiles marked to be laid once, then the ones that
    // repeat, and from the next once-only tile on, whatever closes the run at the object's edge.
    let tiles: Vec<ObjectLayoutTile> = if towards_step { row.iter().rev().copied().collect() } else { row.to_vec() };
    let once = |tile: &ObjectLayoutTile| tile.control == OBJECT_LAYOUT_SLOPE_ONCE;
    let leading = tiles.iter().take_while(|tile| once(tile)).count();
    run.extend(tiles.iter().take(leading.min(count)).map(|tile| tile.tile));
    let Some(rest) = tiles.get(leading..).filter(|rest| !rest.is_empty()) else { return run };
    let closing = rest.iter().position(once).unwrap_or(rest.len());
    let (repeat, close) = rest.split_at(closing);
    let mut repeating = repeat.iter().cycle();
    while run.len() + close.len() < count {
        let Some(tile) = repeating.next() else { break };
        run.push(tile.tile);
    }
    run.extend(close.iter().take(count.saturating_sub(run.len())).map(|tile| tile.tile));
    run
}

/// A slope, as `InitBGDat_processSlope @ 020b3f8c` lays it: one step per call of `processSlope_omgstop @ 020b42f0`,
/// from the corner the slope starts in -- the bottom one, or the top one for a slope that hangs down, on the side it
/// runs away from -- until the steps leave the object's height.
fn render_slope(object: &ObjectLayout, width: usize, height: usize, output: &mut [i32]) {
    let (width, height) = (width as i64, height as i64);
    let mut write = |x: i64, y: i64, tile: i32| {
        if (0..width).contains(&x)
            && (0..height).contains(&y)
            && let Some(slot) = output.get_mut((y * width + x) as usize)
        {
            *slot = tile;
        }
    };
    // The game takes a block's width from its last row, and a row shorter than that leaves its cells alone.
    let step_width = object.rows.last().map_or(0, Vec::len) as i64;
    let step_height = object.rows.len() as i64;
    if step_width == 0 {
        return;
    }
    // The rows of a block, in the order a step's lines meet them: from the bottom up on a rising slope, and round
    // again when the step is taller than the block.
    fn line(block: &[Vec<ObjectLayoutTile>], index: i64, down: bool) -> Option<&Vec<ObjectLayoutTile>> {
        let wrapped = (index as usize).checked_rem(block.len())?;
        block.get(if down { wrapped } else { block.len() - 1 - wrapped })
    }
    let mut x = if object.slope_left { width - step_width } else { 0 };
    let mut y = if object.slope_down { 0 } else { height - 1 };
    while (0..height).contains(&y) {
        for index in 0..step_height {
            let row_y = if object.slope_down { y + index } else { y - index };
            if let Some(row) = line(&object.rows, index, object.slope_down) {
                for (column, tile) in row.iter().take(step_width as usize).enumerate() {
                    write(x + column as i64, row_y, tile.tile);
                }
            }
            // The cells left and right of the step, out to the object's edges.
            let left = x.clamp(0, width);
            let right = (x + step_width).clamp(0, width);
            let (fill, behind) = if object.slope_left { ((0, left), (right, width)) } else { ((right, width), (0, left)) };
            if let Some(row) = line(&object.slope_fill, index, object.slope_down) {
                let run = slope_run(row, (fill.1 - fill.0) as usize, object.slope_left);
                for (offset, &tile) in run.iter().enumerate() {
                    write(if object.slope_left { fill.1 - 1 - offset as i64 } else { fill.0 + offset as i64 }, row_y, tile);
                }
            }
            if let Some(row) = line(&object.slope_behind, index, object.slope_down) {
                // Laid left to right on either slope, so on one that runs right it starts at the object's edge.
                for (offset, &tile) in slope_run(row, (behind.1 - behind.0) as usize, false).iter().enumerate() {
                    write(behind.0 + offset as i64, row_y, tile);
                }
            }
        }
        x += if object.slope_left { -step_width } else { step_width };
        y += if object.slope_down { step_height } else { -step_height };
    }
}

/// The block each cell of an object at this size holds. A cell the layout does not reach comes back as
/// [`OBJECT_LAYOUT_UNWRITTEN`], which is not the same as a cell it fills with [`OBJECT_LAYOUT_EMPTY_TILE`].
pub fn render_object_layout(object: &ObjectLayout, width: usize, height: usize) -> Result<Vec<i32>> {
    if width == 0 || height == 0 || object.rows.is_empty() {
        return Err(err(ErrorCode::InvalidArgument, "object layout dimensions and rows must be nonzero"));
    }
    let cells = width.checked_mul(height).ok_or_else(|| err(ErrorCode::InvalidArgument, "object layout dimensions are too large"))?;
    let mut output = vec![OBJECT_LAYOUT_UNWRITTEN; cells];
    if object.slope {
        render_slope(object, width, height, &mut output);
        return Ok(output);
    }

    fn choose(values: &[ObjectLayoutTile], position: usize, extent: usize) -> i32 {
        let pick = |index: usize| values.get(index).map_or(OBJECT_LAYOUT_EMPTY_TILE, |tile| tile.tile);
        let Some(first_repeat) = values.iter().position(|tile| tile.control & 1 != 0) else {
            return pick(position % values.len());
        };
        let after_repeat = values.iter().skip(first_repeat).position(|tile| tile.control & 1 == 0).map_or(values.len(), |i| first_repeat + i);
        let before = first_repeat;
        let repeat = after_repeat - first_repeat;
        let after = values.len() - after_repeat;
        if position < before {
            pick(position)
        } else if position >= extent - extent.min(after) {
            pick(values.len() - (extent - position))
        } else {
            pick(first_repeat + (position - before) % repeat.max(1))
        }
    }

    let mut before_rows = Vec::new();
    let mut repeat_rows = Vec::new();
    let mut after_rows = Vec::new();
    let mut repeat_found = false;
    for row in &object.rows {
        let Some(first) = row.first() else { continue };
        if first.control & 2 != 0 {
            repeat_found = true;
            repeat_rows.push(row);
        } else if repeat_found {
            after_rows.push(row);
        } else {
            before_rows.push(row);
        }
    }
    if before_rows.is_empty() && repeat_rows.is_empty() {
        return Err(err(ErrorCode::ParseError, "object layout has no renderable rows"));
    }
    for y in 0..height {
        let row = if repeat_rows.is_empty() {
            before_rows.get(y % before_rows.len())
        } else if y < before_rows.len() {
            before_rows.get(y)
        } else if y >= height - height.min(after_rows.len()) {
            after_rows.get(after_rows.len() - (height - y))
        } else {
            repeat_rows.get((y - before_rows.len()) % repeat_rows.len())
        };
        let Some(row) = row else { continue };
        for x in 0..width {
            if let Some(slot) = output.get_mut(y * width + x) {
                *slot = choose(row, x, width);
            }
        }
    }
    Ok(output)
}

#[derive(Clone, Copy)]
struct Sample {
    rgb: [u8; 3],
    count: usize,
}

fn component(sample: Sample, channel: usize) -> u8 {
    sample.rgb.get(channel).copied().unwrap_or(0)
}

/// Median-cut quantisation to at most `maximum_colors` BGR555 colours, the first one transparent when any pixel's
/// alpha is below `alpha_threshold`.
///
/// Boxes are sorted with a stable sort; the C++ uses `std::sort`, so the two can split ties between equal channel
/// values differently.
pub fn quantize_bgr555(image: &RgbaImage, maximum_colors: usize, alpha_threshold: u8) -> Result<IndexedImage> {
    let complete = image.width != 0
        && image.height != 0
        && image.width.checked_mul(image.height).and_then(|n| n.checked_mul(4)).is_some_and(|n| n == image.pixels.len());
    if !complete {
        return Err(err(ErrorCode::InvalidArgument, "quantization needs a complete RGBA image"));
    }
    if !(2..=256).contains(&maximum_colors) {
        return Err(err(ErrorCode::InvalidArgument, "palette size must be 2 through 256"));
    }
    let mut frequency: BTreeMap<u16, usize> = BTreeMap::new();
    let mut transparent = false;
    for &[red, green, blue, alpha] in image.pixels.as_chunks::<4>().0 {
        if alpha < alpha_threshold {
            transparent = true;
            continue;
        }
        *frequency.entry(pack_bgr555(red, green, blue)).or_default() += 1;
    }
    let samples: Vec<Sample> = frequency
        .iter()
        .map(|(&packed, &count)| Sample {
            rgb: [expand_5_to_8(packed & 31), expand_5_to_8((packed >> 5) & 31), expand_5_to_8((packed >> 10) & 31)],
            count,
        })
        .collect();
    let sample = |index: usize| samples.get(index).copied().unwrap_or(Sample { rgb: [0; 3], count: 0 });
    let available = maximum_colors - usize::from(transparent);
    let mut boxes: Vec<Vec<usize>> = Vec::new();
    if !samples.is_empty() {
        boxes.push((0..samples.len()).collect());
    }
    let range = |values: &[usize], channel: usize| -> i32 {
        let channel_values = values.iter().map(|&i| i32::from(component(sample(i), channel)));
        let (low, high) = channel_values.fold((i32::MAX, i32::MIN), |(low, high), v| (low.min(v), high.max(v)));
        if values.is_empty() { 0 } else { high - low }
    };
    let widest = |values: &[usize]| [range(values, 0), range(values, 1), range(values, 2)];
    while boxes.len() < available {
        // First box with the largest spread, as std::max_element picks.
        let mut found: Option<(usize, i32)> = None;
        for (index, values) in boxes.iter().enumerate() {
            let spread = widest(values).into_iter().max().unwrap_or(0);
            if found.is_none_or(|(_, best)| spread > best) {
                found = Some((index, spread));
            }
        }
        let Some((found, _)) = found else { break };
        let Some(values) = boxes.get_mut(found) else { break };
        if values.len() < 2 {
            break;
        }
        let ranges = widest(values);
        let mut channel = 0;
        for (index, &value) in ranges.iter().enumerate() {
            if value > ranges.get(channel).copied().unwrap_or(0) {
                channel = index;
            }
        }
        values.sort_by_key(|&i| component(sample(i), channel));
        let total: usize = values.iter().map(|&i| sample(i).count).sum();
        let mut seen = 0;
        let mut split = 1;
        while split < values.len() {
            seen += values.get(split - 1).map_or(0, |&i| sample(i).count);
            if seen * 2 >= total {
                break;
            }
            split += 1;
        }
        let split = split.min(values.len() - 1);
        let right = values.split_off(split);
        boxes.push(right);
    }

    let mut output = IndexedImage { width: image.width, height: image.height, pixels: vec![0; image.width * image.height], palette: Vec::new() };
    if transparent {
        output.palette.push(Bgr555Color::new(0, 0, 0, true));
    }
    for values in &boxes {
        let (mut count, mut r, mut g, mut b) = (0usize, 0usize, 0usize, 0usize);
        for &i in values {
            let s = sample(i);
            count += s.count;
            r += usize::from(s.rgb[0]) * s.count;
            g += usize::from(s.rgb[1]) * s.count;
            b += usize::from(s.rgb[2]) * s.count;
        }
        if let (Some(r), Some(g), Some(b)) = (r.checked_div(count), g.checked_div(count), b.checked_div(count)) {
            output.palette.push(Bgr555Color::new(r as u8, g as u8, b as u8, false));
        }
    }
    if output.palette.is_empty() {
        output.palette.push(Bgr555Color::new(0, 0, 0, true));
    }
    let first = usize::from(transparent);
    for (target, &[red, green, blue, alpha]) in output.pixels.iter_mut().zip(image.pixels.as_chunks::<4>().0) {
        if transparent && alpha < alpha_threshold {
            *target = 0;
            continue;
        }
        let mut best = first;
        let mut best_error = u32::MAX;
        for (candidate, color) in output.palette.iter().enumerate().skip(first) {
            let dr = i32::from(red) - i32::from(color.red);
            let dg = i32::from(green) - i32::from(color.green);
            let db = i32::from(blue) - i32::from(color.blue);
            let error = (dr * dr + dg * dg + db * db) as u32;
            if error < best_error {
                best_error = error;
                best = candidate;
            }
        }
        *target = best as u8;
    }
    Ok(output)
}
