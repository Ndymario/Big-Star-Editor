//! Editable tilesets (graphics, palette, Map16, objects, behaviours) and background/foreground tilemaps, the
//! `.bstileset` bundle, composed tileset memory and tile behaviour naming. Port of
//! `libs/nsmb-formats/src/tileset.cpp`.

use bigstar_core::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};

use crate::g2d::{put16, put32, u16_at, u32_at};
use crate::{
    Bgr555Color, IndexedTile8Bpp, Map16Tile, ObjectLayout, RgbaImage, ScreenMapEntry, compress_nitro_lz10, compress_nitro_lz11, decode_8bpp_tiles,
    decode_bgr555_palette, decode_map16, decode_object_layouts, decode_screen_map, decompress_nitro_lz, encode_8bpp_tiles, encode_bgr555_palette,
    encode_map16, encode_object_layouts, encode_screen_map, mark_palette_blocks_transparent, quantize_bgr555, render_screen_map,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilesetResourceData {
    pub graphics: Vec<u8>,
    pub palette: Vec<u8>,
    pub map16: Vec<u8>,
    pub objects: Vec<u8>,
    pub object_index: Vec<u8>,
    pub behaviors: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilemapResourceData {
    pub graphics: Vec<u8>,
    pub palette: Vec<u8>,
    pub screen: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditableTileset {
    pub tile_offset: u16,
    pub palette_offset: u8,
    pub tile_page_offset: u8,
    pub graphics: Vec<IndexedTile8Bpp>,
    pub palette: Vec<Bgr555Color>,
    pub map16: Vec<Map16Tile>,
    pub objects: Vec<ObjectLayout>,
    pub behaviors: Vec<u32>,
    /// Writer state used to keep untouched resources byte-identical, including their original LZ stream and any
    /// object-index metadata bytes.
    pub original: TilesetResourceData,
    pub original_objects: Vec<ObjectLayout>,
}

/// Where the game loads a level's background (`back`) layer: its screens name tiles from 576 and palette rows from 10,
/// the slots the layer's 320 8bpp tiles and two 256-colour palettes occupy. Measured on every retail level.
pub const BACKGROUND_TILE_OFFSET: u16 = 576;
pub const BACKGROUND_PALETTE_OFFSET: u8 = 10;
/// Where the game loads a level's foreground (`free`) layer: tiles from 256 and palette rows from 8.
pub const FOREGROUND_TILE_OFFSET: u16 = 256;
pub const FOREGROUND_PALETTE_OFFSET: u8 = 8;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditableTilemap {
    pub width_tiles: usize,
    /// The first tile and palette row the layer's screen names: the editor's tile 0 and palette 0 are these.
    pub tile_offset: u16,
    pub palette_offset: u8,
    pub graphics: Vec<IndexedTile8Bpp>,
    pub palette: Vec<Bgr555Color>,
    pub screen: Vec<ScreenMapEntry>,
    pub original: TilemapResourceData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileBehaviorParameterKind {
    Generic,
    Coin,
    Question,
    Explodable,
    Brick,
    Slope,
    PipeDoor,
    Climbable,
    PartialSolid,
    Harmful,
    Donut,
    Conveyor,
    Conflict,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TileBehaviorParts {
    pub flags: u32,
    pub subtype: u8,
    pub parameter: u8,
}

pub const TILE_BEHAVIOR_FLAG_NAMES: [&str; 20] = [
    "Solid",
    "Coin",
    "Question block",
    "Explodable block",
    "Brick block",
    "Slope",
    "Flipped slope",
    "Unknown 0x80",
    "Door or pipe",
    "Water",
    "Climbable",
    "Partial solid",
    "Harmful",
    "Invisible block",
    "Push player down",
    "Solid on top",
    "Unknown 0x10000",
    "Donut lift",
    "Unknown 0x40000",
    "Unknown 0x80000",
];

pub const TILE_BEHAVIOR_SUBTYPE_NAMES: [&str; 16] = [
    "Normal",
    "Ice",
    "Snow",
    "Quicksand",
    "Conveyor right",
    "Conveyor left",
    "Horizontal rope",
    "Sideways spikes / collapsible stairs",
    "Unknown 8",
    "Ledge",
    "Vertical pole",
    "No sliding on slopes",
    "Unknown 12",
    "Slow movement",
    "Sandy",
    "Unknown 15",
];

pub const TILE_BEHAVIOR_PIPE_MAIN_NAMES: [&str; 8] = [
    "Door / breakable green pipe",
    "Unbreakable green pipe",
    "Breakable red pipe",
    "Unbreakable red pipe",
    "Breakable yellow pipe",
    "Unbreakable yellow pipe",
    "Unknown pipe type 6",
    "Unknown pipe type 7",
];

/// The character and palette memory a level actually has, with every tileset laid out at its own offsets in one
/// space -- what `loadJyotyuAndSubNohara` and `loadCharacterGraphics` build in VRAM. A Map16 entry naming a tile below
/// its own tileset's offsets is drawable against this, because the tileset that supplies that tile is present too.
/// See `docs/tileset-memory.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ComposedTilesetMemory {
    /// Indexed by absolute tile number.
    pub graphics: Vec<IndexedTile8Bpp>,
    /// Indexed by absolute colour index.
    pub palette: Vec<Bgr555Color>,
}

fn err(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

struct Resource {
    decoded: Vec<u8>,
    compression: u8,
}

fn unwrap_resource(bytes: &[u8]) -> Result<Resource> {
    match bytes.first() {
        Some(&compression @ (0x10 | 0x11)) => Ok(Resource { decoded: decompress_nitro_lz(bytes)?, compression }),
        _ => Ok(Resource { decoded: bytes.to_vec(), compression: 0 }),
    }
}

/// Encodes `decoded` the way `original` was stored: unchanged bytes come back as the original stream, changed ones
/// are recompressed with the same LZ variant.
fn rewrap(decoded: &[u8], original: &[u8]) -> Result<Vec<u8>> {
    let old = unwrap_resource(original)?;
    if decoded == old.decoded.as_slice() {
        return Ok(original.to_vec());
    }
    match old.compression {
        0x10 => compress_nitro_lz10(decoded),
        0x11 => compress_nitro_lz11(decoded),
        _ => Ok(decoded.to_vec()),
    }
}

fn named(value: u8, names: &[(u8, &'static str)]) -> &'static str {
    names.iter().find(|(key, _)| *key == value).map_or("", |(_, name)| name)
}

fn render_tiles(tiles: &[IndexedTile8Bpp], palette: &[Bgr555Color], width_tiles: usize) -> Result<RgbaImage> {
    if width_tiles == 0 {
        return Err(err(ErrorCode::InvalidArgument, "tile sheet width must be nonzero"));
    }
    if tiles.len() > 0x400 {
        return Err(err(ErrorCode::OutOfBounds, "tile sheet exceeds the DS tile index range"));
    }
    let mut screen: Vec<ScreenMapEntry> = (0..tiles.len()).map(|i| ScreenMapEntry::new(i as i32, 0, false, false)).collect();
    while !screen.len().is_multiple_of(width_tiles) {
        screen.push(ScreenMapEntry::new(-1, 0, false, false));
    }
    render_screen_map(&screen, width_tiles, tiles, palette)
}

fn import_indexed(
    tiles: &mut Vec<IndexedTile8Bpp>,
    palette: &mut Vec<Bgr555Color>,
    image: &RgbaImage,
    mut screen: Option<&mut Vec<ScreenMapEntry>>,
    reserve_transparent: bool,
) -> Result<()> {
    if image.width == 0 || image.height == 0 || !image.width.is_multiple_of(8) || !image.height.is_multiple_of(8) {
        return Err(err(ErrorCode::InvalidArgument, "PNG dimensions must be nonzero multiples of eight"));
    }
    let mut indexed = quantize_bgr555(image, if reserve_transparent { 255 } else { 256 }, 128)?;
    if reserve_transparent && !indexed.palette.first().is_some_and(|c| c.transparent) {
        indexed.palette.insert(0, Bgr555Color::new(0, 0, 0, true));
        for pixel in &mut indexed.pixels {
            *pixel = pixel.wrapping_add(1);
        }
    }
    let (tiles_wide, tiles_high) = (image.width / 8, image.height / 8);
    if tiles_wide * tiles_high > 1024 {
        return Err(err(ErrorCode::OutOfBounds, "PNG needs more than 1024 DS tiles"));
    }
    tiles.clear();
    if let Some(screen) = screen.as_deref_mut() {
        screen.clear();
    }
    *palette = indexed.palette;
    for ty in 0..tiles_high {
        for tx in 0..tiles_wide {
            let mut tile = IndexedTile8Bpp::default();
            for (i, pixel) in tile.pixels.iter_mut().enumerate() {
                let (x, y) = (i % 8, i / 8);
                *pixel = indexed.pixels.get((ty * 8 + y) * image.width + tx * 8 + x).copied().unwrap_or(0);
            }
            match screen.as_deref_mut() {
                Some(screen) => {
                    let index = match tiles.iter().position(|t| *t == tile) {
                        Some(found) => found,
                        None => {
                            tiles.push(tile);
                            tiles.len() - 1
                        }
                    };
                    screen.push(ScreenMapEntry::new(index as i32, 0, false, false));
                }
                None => tiles.push(tile),
            }
        }
    }
    Ok(())
}

pub fn parse_tileset(resources: &TilesetResourceData, tile_offset: u16, palette_offset: u8, tile_page_offset: u8) -> Result<EditableTileset> {
    let graphics = unwrap_resource(&resources.graphics)?;
    let palette = unwrap_resource(&resources.palette)?;
    let map16 = unwrap_resource(&resources.map16)?;
    let objects = unwrap_resource(&resources.objects)?;
    let index = unwrap_resource(&resources.object_index)?;
    let behaviors = unwrap_resource(&resources.behaviors)?;
    let decoded_graphics = decode_8bpp_tiles(&graphics.decoded);
    let decoded_palette = decode_bgr555_palette(&palette.decoded, true).map(|mut p| {
        mark_palette_blocks_transparent(&mut p, 256);
        p
    });
    let decoded_map16 = decode_map16(&map16.decoded, tile_offset, palette_offset);
    let decoded_objects = decode_object_layouts(&objects.decoded, &index.decoded, tile_page_offset);
    let (graphics, palette, map16, objects) = (decoded_graphics?, decoded_palette?, decoded_map16?, decoded_objects?);
    if !behaviors.decoded.len().is_multiple_of(4) {
        return Err(err(ErrorCode::ParseError, "tile behaviors must use four-byte records"));
    }
    let behaviors: Vec<u32> = behaviors.decoded.as_chunks::<4>().0.iter().map(|&b| u32::from_le_bytes(b)).collect();
    if !behaviors.is_empty() && behaviors.len() != map16.len() {
        return Err(err(ErrorCode::ParseError, "tile behavior count does not match Map16 tile count"));
    }
    Ok(EditableTileset {
        tile_offset,
        palette_offset,
        tile_page_offset,
        graphics,
        palette,
        map16,
        original_objects: objects.clone(),
        objects,
        behaviors,
        original: resources.clone(),
    })
}

pub fn write_tileset(tileset: &EditableTileset) -> Result<TilesetResourceData> {
    let graphics = encode_8bpp_tiles(&tileset.graphics)?;
    let palette = encode_bgr555_palette(&tileset.palette)?;
    let map16 = encode_map16(&tileset.map16, tileset.tile_offset, tileset.palette_offset)?;
    if !tileset.behaviors.is_empty() && tileset.behaviors.len() != tileset.map16.len() {
        return Err(err(ErrorCode::InvalidArgument, "tile behavior count does not match Map16 tile count"));
    }
    let behavior_bytes: Vec<u8> = tileset.behaviors.iter().flat_map(|b| b.to_le_bytes()).collect();
    let (object_bytes, index_bytes) = if tileset.objects == tileset.original_objects {
        (unwrap_resource(&tileset.original.objects)?.decoded, unwrap_resource(&tileset.original.object_index)?.decoded)
    } else {
        let old_index = unwrap_resource(&tileset.original.object_index)?;
        let index_template: &[u8] = if old_index.decoded.len() == tileset.objects.len() * 4 { &old_index.decoded } else { &[] };
        let encoded = encode_object_layouts(&tileset.objects, tileset.tile_page_offset, index_template)?;
        (encoded.definitions, encoded.index)
    };
    let original = &tileset.original;
    Ok(TilesetResourceData {
        graphics: rewrap(&graphics, &original.graphics)?,
        palette: rewrap(&palette, &original.palette)?,
        map16: rewrap(&map16, &original.map16)?,
        objects: rewrap(&object_bytes, &original.objects)?,
        object_index: rewrap(&index_bytes, &original.object_index)?,
        behaviors: rewrap(&behavior_bytes, &original.behaviors)?,
    })
}

const BUNDLE_PARTS: usize = 6;
const BUNDLE_HEADER_SIZE: usize = 8 + BUNDLE_PARTS * 12;

pub fn write_tileset_bundle(resources: &TilesetResourceData) -> Result<Vec<u8>> {
    let parts: [&[u8]; BUNDLE_PARTS] =
        [&resources.graphics, &resources.palette, &resources.map16, &resources.objects, &resources.object_index, &resources.behaviors];
    let mut total = BUNDLE_HEADER_SIZE;
    for part in parts {
        total = total
            .checked_add(part.len())
            .filter(|&t| t <= u32::MAX as usize)
            .ok_or_else(|| err(ErrorCode::OutOfBounds, "tileset bundle exceeds 32-bit offsets"))?;
    }
    let mut output = vec![0u8; BUNDLE_HEADER_SIZE];
    output.reserve(total - BUNDLE_HEADER_SIZE);
    if let Some(slot) = output.get_mut(..4) {
        slot.copy_from_slice(b"BSTS");
    }
    put16(&mut output, 4, 1);
    put16(&mut output, 6, BUNDLE_PARTS as u16);
    for (i, part) in parts.iter().enumerate() {
        let entry = 8 + i * 12;
        let cursor = output.len() as u32;
        if let Some(kind) = output.get_mut(entry) {
            *kind = i as u8;
        }
        put32(&mut output, entry + 4, cursor);
        put32(&mut output, entry + 8, part.len() as u32);
        output.extend_from_slice(part);
    }
    Ok(output)
}

pub fn parse_tileset_bundle(bytes: &[u8]) -> Result<TilesetResourceData> {
    if bytes.len() < BUNDLE_HEADER_SIZE || !bytes.starts_with(b"BSTS") || u16_at(bytes, 4) != 1 || usize::from(u16_at(bytes, 6)) != BUNDLE_PARTS {
        return Err(err(ErrorCode::ParseError, "tileset bundle header is invalid"));
    }
    let mut parts: [Vec<u8>; BUNDLE_PARTS] = Default::default();
    let mut seen = [false; BUNDLE_PARTS];
    let mut ranges = Vec::new();
    for i in 0..BUNDLE_PARTS {
        let entry = 8 + i * 12;
        let kind = usize::from(bytes.get(entry).copied().unwrap_or(u8::MAX));
        match seen.get_mut(kind) {
            Some(slot) if !*slot => *slot = true,
            _ => return Err(err(ErrorCode::ParseError, "tileset bundle resource kind is invalid or duplicated")),
        }
        let offset = u32_at(bytes, entry + 4) as usize;
        let size = u32_at(bytes, entry + 8) as usize;
        if offset < BUNDLE_HEADER_SIZE || offset > bytes.len() || size > bytes.len() - offset {
            return Err(err(ErrorCode::OutOfBounds, "tileset bundle resource is outside the file"));
        }
        if size != 0 {
            ranges.push((offset, offset + size));
        }
        if let Some(part) = parts.get_mut(kind) {
            *part = bytes.get(offset..offset + size).unwrap_or_default().to_vec();
        }
    }
    ranges.sort_unstable();
    if ranges.windows(2).any(|pair| matches!(pair, [a, b] if b.0 < a.1)) {
        return Err(err(ErrorCode::ParseError, "tileset bundle resources overlap"));
    }
    let [graphics, palette, map16, objects, object_index, behaviors] = parts;
    Ok(TilesetResourceData { graphics, palette, map16, objects, object_index, behaviors })
}

/// A background or foreground layer, its screen read relative to where the game loads the layer (`tile_offset`,
/// `palette_offset`; see [`BACKGROUND_TILE_OFFSET`] and [`FOREGROUND_TILE_OFFSET`]). Colour 0 of each 256-colour
/// palette is see-through, as the hardware draws it.
pub fn parse_tilemap(resources: &TilemapResourceData, width_tiles: usize, tile_offset: u16, palette_offset: u8) -> Result<EditableTilemap> {
    let graphics = unwrap_resource(&resources.graphics)?;
    let palette = unwrap_resource(&resources.palette)?;
    let screen = unwrap_resource(&resources.screen)?;
    let decoded_graphics = decode_8bpp_tiles(&graphics.decoded);
    let decoded_palette = decode_bgr555_palette(&palette.decoded, false).map(|mut p| {
        mark_palette_blocks_transparent(&mut p, 256);
        p
    });
    let decoded_screen = decode_screen_map(&screen.decoded, tile_offset, palette_offset);
    let (graphics, palette, screen) = (decoded_graphics?, decoded_palette?, decoded_screen?);
    let width_tiles = if width_tiles != 0 {
        width_tiles
    } else if screen.len().is_multiple_of(64) {
        64
    } else {
        32
    };
    if !screen.len().is_multiple_of(width_tiles) {
        return Err(err(ErrorCode::InvalidArgument, "tilemap width does not divide its screen entries"));
    }
    Ok(EditableTilemap { width_tiles, tile_offset, palette_offset, graphics, palette, screen, original: resources.clone() })
}

pub fn write_tilemap(tilemap: &EditableTilemap) -> Result<TilemapResourceData> {
    if tilemap.width_tiles == 0 || !tilemap.screen.len().is_multiple_of(tilemap.width_tiles) {
        return Err(err(ErrorCode::InvalidArgument, "tilemap width does not divide its screen entries"));
    }
    let graphics = encode_8bpp_tiles(&tilemap.graphics)?;
    let palette = encode_bgr555_palette(&tilemap.palette)?;
    let screen = encode_screen_map(&tilemap.screen, tilemap.tile_offset, tilemap.palette_offset)?;
    Ok(TilemapResourceData {
        graphics: rewrap(&graphics, &tilemap.original.graphics)?,
        palette: rewrap(&palette, &tilemap.original.palette)?,
        screen: rewrap(&screen, &tilemap.original.screen)?,
    })
}

pub fn render_tilemap(tilemap: &EditableTilemap) -> Result<RgbaImage> {
    render_screen_map(&tilemap.screen, tilemap.width_tiles, &tilemap.graphics, &tilemap.palette)
}

pub fn import_tilemap_image(tilemap: &mut EditableTilemap, image: &RgbaImage) -> Result<()> {
    import_indexed(&mut tilemap.graphics, &mut tilemap.palette, image, Some(&mut tilemap.screen), false)?;
    tilemap.width_tiles = image.width / 8;
    Ok(())
}

pub fn render_tileset_graphics(tileset: &EditableTileset, width_tiles: usize) -> Result<RgbaImage> {
    let width_tiles = width_tiles.min(tileset.graphics.len().max(1));
    render_tiles(&tileset.graphics, &tileset.palette, width_tiles)
}

/// Both sheets arrange the same way; they differ only in which space the entries are read against, and so in how
/// much has to be added back to each index.
fn map16_sheet(
    tileset: &EditableTileset,
    width_tiles: usize,
    graphics: &[IndexedTile8Bpp],
    palette: &[Bgr555Color],
    absolute: bool,
) -> Result<RgbaImage> {
    if width_tiles == 0 {
        return Err(err(ErrorCode::InvalidArgument, "Map16 sheet width must be nonzero"));
    }
    let map16_rows = tileset.map16.len().div_ceil(width_tiles);
    // Padding past the last Map16 tile is nothing, not tile zero.
    let mut arranged = vec![ScreenMapEntry::new(-1, 0, false, false); map16_rows * width_tiles * 4];
    for (i, tile) in tileset.map16.iter().enumerate() {
        for (q, &entry) in tile.tiles.iter().enumerate() {
            let x = (i % width_tiles) * 2 + q % 2;
            let y = (i / width_tiles) * 2 + q / 2;
            let mut entry = entry;
            if absolute {
                entry.tile += i32::from(tileset.tile_offset);
                entry.palette = entry.palette.wrapping_add(i16::from(tileset.palette_offset));
            }
            if let Some(slot) = arranged.get_mut(y * width_tiles * 2 + x) {
                *slot = entry;
            }
        }
    }
    render_screen_map(&arranged, width_tiles * 2, graphics, palette)
}

pub fn render_map16_sheet(tileset: &EditableTileset, width_tiles: usize) -> Result<RgbaImage> {
    map16_sheet(tileset, width_tiles, &tileset.graphics, &tileset.palette, false)
}

pub fn compose_tileset_memory(tilesets: &[EditableTileset]) -> ComposedTilesetMemory {
    let mut memory = ComposedTilesetMemory::default();
    for tileset in tilesets {
        let graphics_end = usize::from(tileset.tile_offset) + tileset.graphics.len();
        memory.graphics.resize(memory.graphics.len().max(graphics_end), IndexedTile8Bpp::default());
        let palette_end = usize::from(tileset.palette_offset) * 256 + tileset.palette.len();
        // A palette block no tileset fills is clear, not black: the game never draws through one, and a level's
        // tilesets rarely fill all the blocks their offsets reserve.
        memory.palette.resize(memory.palette.len().max(palette_end), Bgr555Color::new(0, 0, 0, true));
    }
    for tileset in tilesets {
        let start = usize::from(tileset.tile_offset);
        if let Some(slot) = memory.graphics.get_mut(start..start + tileset.graphics.len()) {
            slot.copy_from_slice(&tileset.graphics);
        }
        let start = usize::from(tileset.palette_offset) * 256;
        if let Some(slot) = memory.palette.get_mut(start..start + tileset.palette.len()) {
            slot.copy_from_slice(&tileset.palette);
        }
    }
    memory
}

/// The same sheet as [`render_map16_sheet`], drawn against the composed space, so entries this tileset cannot
/// resolve alone come out as the tiles the game shows.
pub fn render_composed_map16_sheet(tileset: &EditableTileset, memory: &ComposedTilesetMemory, width_tiles: usize) -> Result<RgbaImage> {
    map16_sheet(tileset, width_tiles, &memory.graphics, &memory.palette, true)
}

pub fn import_tileset_graphics(tileset: &mut EditableTileset, image: &RgbaImage) -> Result<()> {
    import_indexed(&mut tileset.graphics, &mut tileset.palette, image, None, true)
}

pub fn decode_tile_behavior(value: u32) -> TileBehaviorParts {
    TileBehaviorParts { flags: ((value >> 16) & 0xffff) | ((value & 0xf00) << 8), subtype: ((value >> 12) & 15) as u8, parameter: value as u8 }
}

pub fn encode_tile_behavior(value: TileBehaviorParts) -> u32 {
    ((value.flags & 0xffff) << 16) | ((value.flags & 0xf0000) >> 8) | (u32::from(value.subtype & 15) << 12) | u32::from(value.parameter)
}

pub fn tile_behavior_parameter_kind(value: TileBehaviorParts) -> TileBehaviorParameterKind {
    use TileBehaviorParameterKind as Kind;
    let flagged = [
        (2, Kind::Coin),
        (4, Kind::Question),
        (8, Kind::Explodable),
        (0x10, Kind::Brick),
        (0x20 | 0x40, Kind::Slope),
        (0x100, Kind::PipeDoor),
        (0x400, Kind::Climbable),
        (0x800, Kind::PartialSolid),
        (0x1000, Kind::Harmful),
        (0x20000, Kind::Donut),
    ];
    let mut kinds: Vec<Kind> = flagged.iter().filter(|(flag, _)| value.flags & flag != 0).map(|&(_, kind)| kind).collect();
    if value.subtype == 4 || value.subtype == 5 {
        kinds.push(Kind::Conveyor);
    }
    match kinds.as_slice() {
        [] => Kind::Generic,
        [only] => *only,
        _ => Kind::Conflict,
    }
}

/// What a question block or a brick gives up when it is hit, as far as an editor need tell them apart at a glance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BlockContents {
    /// A mushroom, or a fire flower for a player who is already big.
    PowerUp,
    Mushroom,
    FireFlower,
    Star,
    OneUp,
    MiniMushroom,
    MegaMushroom,
    Coin,
    /// More than one coin: a brick's run of them, or five floating ones.
    Coins,
    Vine,
    Springboard,
    Boo,
    BlueShell,
}

/// What a block with behaviour `value` holds, read off the parameter its question or brick flag gives it. `None` for
/// any other block, and for an empty or unknown parameter. Invisible and thrown variants hold what their visible
/// ones do.
pub fn block_contents(value: u32) -> Option<BlockContents> {
    use BlockContents as C;
    let parts = decode_tile_behavior(value);
    if parts.flags & 4 != 0 {
        return match parts.parameter {
            0 | 0x10 | 0x30 => Some(C::PowerUp),
            1 | 2 | 0x31 => Some(C::Star),
            3 | 0x32 => Some(C::OneUp),
            4 => Some(C::Springboard),
            5 => Some(C::MiniMushroom),
            6 => Some(C::Coins),
            7 | 0x11 | 0x33 => Some(C::Coin),
            8 | 0x34 => Some(C::Vine),
            9 | 10 | 0x35 => Some(C::Mushroom),
            12 => Some(C::Boo),
            13 => Some(C::MegaMushroom),
            _ => None,
        };
    }
    if parts.flags & 0x10 != 0 {
        return match parts.parameter {
            1 => Some(C::PowerUp),
            2 => Some(C::Star),
            3 => Some(C::OneUp),
            4 => Some(C::Coin),
            5 | 0x45 => Some(C::Coins),
            6 => Some(C::Vine),
            0x34 => Some(C::FireFlower),
            0x35 => Some(C::BlueShell),
            0x95 => Some(C::MiniMushroom),
            0x97 => Some(C::MegaMushroom),
            _ => None,
        };
    }
    None
}

const PIPE_DOOR_NAMES: [&str; 32] = [
    "Up left / door bottom-left",
    "Up right / door bottom-right",
    "Down left / mini door",
    "Down right / boss door left",
    "Vertical left / boss door centre",
    "Vertical right / boss door right",
    "Vertical-over-horizontal left",
    "Vertical-over-horizontal right",
    "Left-facing top",
    "Left-facing bottom",
    "Right-facing top",
    "Right-facing bottom",
    "Horizontal top",
    "Horizontal bottom",
    "Horizontal-over-vertical top",
    "Horizontal-over-vertical bottom",
    "Mini pipe top",
    "Cracked vertical top-left",
    "Mini pipe bottom",
    "Cracked vertical top-right",
    "Unknown 20",
    "Cracked vertical bottom-left",
    "Mini vertical-over-horizontal",
    "Cracked vertical bottom-right",
    "Mini pipe left",
    "Cracked mini pipe bottom",
    "Mini pipe right",
    "Cracked mini pipe top",
    "Mini vertical pipe",
    "Mini horizontal pipe",
    "Mini horizontal-over-vertical",
    "Pipe joint",
];

pub fn tile_behavior_parameter_name(kind: TileBehaviorParameterKind, value: u8) -> &'static str {
    use TileBehaviorParameterKind as Kind;
    match kind {
        Kind::Coin => named(value, &[(0, "Normal"), (2, "Unknown 2"), (4, "Unknown 4")]),
        Kind::Question => named(
            value,
            &[
                (0, "Mushroom or flower"),
                (1, "Starman"),
                (2, "Continuous Starman"),
                (3, "1-Up Mushroom"),
                (4, "Springboard"),
                (5, "Mini Mushroom"),
                (6, "Five floating coins"),
                (7, "Coin"),
                (8, "Vine"),
                (9, "Mushroom (parameter 9)"),
                (10, "Mushroom (parameter 10)"),
                (11, "Unknown 11"),
                (12, "Boo"),
                (13, "Mega Mushroom"),
                (14, "Unknown 14"),
                (15, "Unknown 15"),
                (0x10, "Thrown power-up"),
                (0x11, "Thrown coin"),
                (0x30, "Invisible power-up"),
                (0x31, "Invisible Starman"),
                (0x32, "Invisible 1-Up"),
                (0x33, "Invisible coin"),
                (0x34, "Invisible vine"),
                (0x35, "Invisible mushroom"),
                (0x36, "Invisible unknown 0x36"),
                (0x37, "Invisible unknown 0x37"),
            ],
        ),
        Kind::Brick => named(
            value,
            &[
                (0, "Empty"),
                (1, "Mushroom or flower"),
                (2, "Starman"),
                (3, "1-Up Mushroom"),
                (4, "Coin"),
                (5, "Multiple coins"),
                (6, "Vine"),
                (7, "Empty (parameter 7)"),
                (8, "Empty (parameter 8)"),
                (0x34, "Flower"),
                (0x35, "Blue shell"),
                (0x45, "Two coins"),
                (0x95, "Mini Mushroom"),
                (0x97, "Mega Mushroom"),
            ],
        ),
        Kind::Explodable => named(value, &[(0, "Used block shards"), (1, "Stone block shards"), (2, "Wooden block shards"), (3, "Red block shards")]),
        Kind::Harmful => named(
            value,
            &[
                (0, "Spikes left"),
                (1, "Spikes right"),
                (2, "Spikes up"),
                (3, "Spikes down"),
                (4, "Lava"),
                (5, "Instant death"),
                (7, "Harmful on all sides"),
            ],
        ),
        Kind::Conveyor => named(value, &[(0, "Slow"), (1, "Fast")]),
        Kind::Donut => named(value, &[(0, "Donut lift"), (1, "Beach platform left"), (2, "Beach platform right")]),
        Kind::Climbable => named(
            value,
            &[
                (0, "Vine"),
                (2, "Fence top-left"),
                (3, "Fence top"),
                (4, "Fence top-right"),
                (5, "Fence left"),
                (6, "Fence centre"),
                (7, "Fence right"),
                (8, "Fence bottom-left"),
                (9, "Fence bottom"),
                (10, "Fence bottom-right"),
                (11, "Flip fence top-left"),
                (12, "Flip fence top"),
                (13, "Flip fence top-right"),
                (14, "Flip fence left"),
                (15, "Flip fence centre"),
                (16, "Flip fence right"),
                (17, "Flip fence bottom-left"),
                (18, "Flip fence bottom"),
                (19, "Flip fence bottom-right"),
            ],
        ),
        Kind::Slope => named(
            value,
            &[
                (0, "1x1 up-right"),
                (1, "1x1 down-right"),
                (2, "2x1 up-right left"),
                (3, "2x1 up-right right"),
                (4, "2x1 down-right left"),
                (5, "2x1 down-right right"),
                (6, "1x2 up-right bottom"),
                (7, "1x2 up-right top"),
                (8, "1x2 down-right bottom"),
                (9, "1x2 down-right top"),
                (10, "Slope edge"),
                (11, "4x1 up-right part 1"),
                (12, "4x1 up-right part 2"),
                (13, "4x1 up-right part 3"),
                (14, "4x1 up-right part 4"),
                (15, "4x1 down-right part 1"),
                (16, "4x1 down-right part 2"),
                (17, "4x1 down-right part 3"),
                (18, "4x1 down-right part 4"),
            ],
        ),
        Kind::PipeDoor => PIPE_DOOR_NAMES.get(usize::from(value & 31)).copied().unwrap_or(""),
        Kind::PartialSolid => "Quadrant mask",
        Kind::Generic => named(
            value,
            &[
                (0, "Normal"),
                (1, "Path bottom-left to top-right"),
                (2, "Path top-left to bottom-right"),
                (3, "Path corner top-left"),
                (4, "Path corner bottom-right"),
                (5, "Horizontal path"),
                (6, "Vertical path"),
                (8, "2x1 up-right left"),
                (9, "2x1 up-right right"),
                (0xa, "2x1 down-right left"),
                (0xb, "2x1 down-right right"),
                (0xc, "1x2 up-right top"),
                (0xd, "1x2 up-right bottom"),
                (0xe, "1x2 down-right bottom"),
                (0xf, "1x2 down-right top"),
                (0x10, "1x1 circle path"),
                (0x11, "2x2 circle top-left"),
                (0x12, "2x2 circle top-right"),
                (0x13, "2x2 circle bottom-left"),
                (0x14, "2x2 circle bottom-right"),
                (0x15, "4x4 circle top-left"),
                (0x16, "4x4 circle top-middle left"),
                (0x17, "4x4 circle top-middle right"),
                (0x18, "4x4 circle top-right"),
                (0x19, "4x4 circle left-middle top"),
                (0x1a, "4x4 circle right-middle top"),
                (0x1b, "4x4 circle left-middle bottom"),
                (0x1c, "4x4 circle right-middle bottom"),
                (0x1d, "4x4 circle bottom-left"),
                (0x1e, "4x4 circle bottom-middle left"),
                (0x1f, "4x4 circle bottom-middle right"),
                (0x20, "4x4 circle bottom-right"),
                (0x22, "Path stopper"),
                (0x28, "Coin outline 1"),
                (0x29, "Coin outline 2"),
            ],
        ),
        Kind::Conflict => "Conflicting parameter flags",
    }
}
