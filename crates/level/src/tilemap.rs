//! The tile array a level's objects rasterise into. Port of `libs/nsmb-project/src/level_tilemap.cpp`, plus
//! `render_tileset_object` from `graphics_resources.cpp`; pure, so the frontend composes the tilemap itself.
//!
//! A level is not a stack of objects. `InitBGDat @ 020b3924` walks the object list once and rasterises every object
//! into one array of Map16 blocks -- one block per 16x16 cell -- and that array is all the hardware ever sees. So two
//! objects cannot both show through a cell: the one written later wins outright, transparent pixels and all. See
//! `docs/tileset-memory.md`.

use std::collections::HashMap;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::{NsmbLevelObject, OBJECT_LAYOUT_EMPTY_TILE, OBJECT_LAYOUT_UNWRITTEN, ObjectLayout, RgbaImage, render_object_layout};
use serde::{Deserialize, Serialize};

/// The array is allocated in chunks of 16x16 cells (`processX_allocChunk @ 020b597c`), and so is this, which keeps an
/// object dropped far outside the level from asking for the rectangle between it and the origin.
pub const LEVEL_CHUNK_CELLS: usize = 16;

/// One tileset slot, decoded: a 16x16 image per Map16 block and the object layouts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecodedTileset {
    pub map16_tiles: Vec<RgbaImage>,
    pub objects: Vec<ObjectLayout>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTilemapCell {
    /// Index into that slot's Map16, or -1 for the empty block -- written like any other block, and drawing nothing.
    pub block: i32,
    pub slot: u8,
}

impl LevelTilemapCell {
    /// The slot no object writes: what an untouched cell holds.
    pub const NO_SLOT: u8 = 0xff;

    pub fn written(&self) -> bool {
        self.slot != Self::NO_SLOT
    }

    pub fn draws(&self) -> bool {
        self.written() && self.block >= 0
    }
}

impl Default for LevelTilemapCell {
    fn default() -> Self {
        Self { block: -1, slot: Self::NO_SLOT }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTilemapChunk {
    /// In chunks, so cell `(x * 16, y * 16)` is the corner.
    pub x: i32,
    pub y: i32,
    pub cells: Vec<LevelTilemapCell>,
}

impl LevelTilemapChunk {
    fn new(x: i32, y: i32) -> Self {
        Self { x, y, cells: vec![LevelTilemapCell::default(); LEVEL_CHUNK_CELLS * LEVEL_CHUNK_CELLS] }
    }

    pub fn cell(&self, x: usize, y: usize) -> LevelTilemapCell {
        self.cells.get(y * LEVEL_CHUNK_CELLS + x).copied().unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTilemap {
    pub chunks: Vec<LevelTilemapChunk>,
}

impl LevelTilemap {
    /// The cell at a level position in cells, or an unwritten one where no chunk was allocated.
    pub fn cell_at(&self, x: i32, y: i32) -> LevelTilemapCell {
        let cells = LEVEL_CHUNK_CELLS as i32;
        let (chunk_x, chunk_y) = (x.div_euclid(cells), y.div_euclid(cells));
        self.chunks
            .iter()
            .find(|chunk| chunk.x == chunk_x && chunk.y == chunk_y)
            .map(|chunk| chunk.cell((x - chunk_x * cells) as usize, (y - chunk_y * cells) as usize))
            .unwrap_or_default()
    }
}

/// Blocks 1 through 6 are never written. `processX_InsertIntoChunk @ 020b54f4` drops them before it reaches the
/// array, so an object that names one leaves the cell -- and whatever an earlier object put in it -- alone.
pub const LEVEL_TILEMAP_FIRST_SKIPPED_BLOCK: i32 = 1;
pub const LEVEL_TILEMAP_LAST_SKIPPED_BLOCK: i32 = 6;

/// Where each tileset slot's Map16 starts in the one address space the three share, in blocks: jyotyu at 0, the level
/// tileset at 256, nohara at 1024.
pub const LEVEL_TILESET_BLOCK_BASES: [i32; 3] = [0, 256, 1024];

/// What placing an object does to the cells it covers. Two of these draw nothing, so a picker that previews them
/// draws seven identical blanks unless it says which is which.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LevelObjectEffect {
    /// The blocks it names, the ordinary case.
    Draws,
    /// Every cell is the empty block: it clears what it is laid over.
    Erases,
    /// Every cell names a block 1-6, so it does nothing whatsoever.
    Inert,
}

fn skipped(block: i32) -> bool {
    (LEVEL_TILEMAP_FIRST_SKIPPED_BLOCK..=LEVEL_TILEMAP_LAST_SKIPPED_BLOCK).contains(&block)
}

pub fn level_object_effect(object: &ObjectLayout, tileset_slot: u8) -> LevelObjectEffect {
    let Some(&base) = LEVEL_TILESET_BLOCK_BASES.get(usize::from(tileset_slot)) else { return LevelObjectEffect::Draws };
    let (mut any, mut all_empty, mut all_skipped) = (false, true, true);
    for tile in object.rows.iter().chain(&object.slope_fill).chain(&object.slope_behind).flatten() {
        any = true;
        let block = if tile.tile == OBJECT_LAYOUT_EMPTY_TILE { 0 } else { tile.tile.wrapping_add(base) };
        if block != 0 {
            all_empty = false;
        }
        if !skipped(block) {
            all_skipped = false;
        }
    }
    if !any {
        LevelObjectEffect::Draws
    } else if all_empty {
        LevelObjectEffect::Erases
    } else if all_skipped {
        LevelObjectEffect::Inert
    } else {
        LevelObjectEffect::Draws
    }
}

/// The largest object the editor will rasterise. A level's objects are smaller than this by orders of magnitude; the
/// cap is here so that a width read out of a damaged file cannot ask for a billion cells.
const MAXIMUM_OBJECT_CELLS: usize = 1 << 20;

/// The tile array the game would build from these objects, in the order they are listed -- which is the order the
/// editor draws and reorders them in.
pub fn rasterize_level_objects(objects: &[NsmbLevelObject], tilesets: &[Option<&DecodedTileset>; 3]) -> LevelTilemap {
    let mut tilemap = LevelTilemap::default();
    let mut chunks: HashMap<(i32, i32), usize> = HashMap::new();
    let cells = LEVEL_CHUNK_CELLS as i32;
    let mut write = |tilemap: &mut LevelTilemap, x: i32, y: i32, value: LevelTilemapCell| {
        // Division that rounds towards minus infinity, because cells left of the origin belong to the chunk left of
        // it: -1 / 16 is chunk -1, cell 15.
        let (chunk_x, chunk_y) = (x.div_euclid(cells), y.div_euclid(cells));
        let index = *chunks.entry((chunk_x, chunk_y)).or_insert_with(|| {
            tilemap.chunks.push(LevelTilemapChunk::new(chunk_x, chunk_y));
            tilemap.chunks.len() - 1
        });
        let offset = ((y - chunk_y * cells) * cells + (x - chunk_x * cells)) as usize;
        if let Some(cell) = tilemap.chunks.get_mut(index).and_then(|chunk| chunk.cells.get_mut(offset)) {
            *cell = value;
        }
    };

    for object in objects {
        let slot = usize::from(object.tileset_slot);
        let Some(Some(tileset)) = tilesets.get(slot) else { continue };
        let Some(layout) = tileset.objects.get(usize::from(object.object_id)) else { continue };
        let width = usize::from(object.width.max(1));
        let height = usize::from(object.height.max(1));
        if width > MAXIMUM_OBJECT_CELLS / height {
            continue;
        }
        let Ok(rendered) = render_object_layout(layout, width, height) else { continue };
        let base = LEVEL_TILESET_BLOCK_BASES.get(slot).copied().unwrap_or(0);
        for (index, &value) in rendered.iter().enumerate() {
            if value == OBJECT_LAYOUT_UNWRITTEN {
                continue;
            }
            // The block the game would put in the cell, in the space the three tilesets share -- which is what its
            // skip list is in.
            let block = if value == OBJECT_LAYOUT_EMPTY_TILE { 0 } else { value.wrapping_add(base) };
            if skipped(block) {
                continue;
            }
            let (x, y) = ((index % width) as i32, (index / width) as i32);
            let cell = LevelTilemapCell { block: if value >= 0 { value } else { -1 }, slot: object.tileset_slot };
            write(&mut tilemap, i32::from(object.x) + x, i32::from(object.y) + y, cell);
        }
    }
    tilemap
}

fn copy_image(source: &RgbaImage, destination: &mut RgbaImage, destination_x: usize, destination_y: usize) {
    for y in 0..source.height {
        let from = y * source.width * 4;
        let to = ((destination_y + y) * destination.width + destination_x) * 4;
        let length = source.width * 4;
        if let (Some(row), Some(target)) = (source.pixels.get(from..from + length), destination.pixels.get_mut(to..to + length)) {
            target.copy_from_slice(row);
        }
    }
}

/// One object drawn from its tileset's Map16 images, `width` by `height` blocks.
pub fn render_tileset_object(tileset: &DecodedTileset, object_id: u16, width: usize, height: usize) -> Result<RgbaImage> {
    let object = tileset.objects.get(usize::from(object_id)).ok_or_else(|| Error::new(ErrorCode::NotFound, "tileset object ID is not defined"))?;
    let layout = render_object_layout(object, width, height)?;
    let mut image = RgbaImage { width: width * 16, height: height * 16, pixels: vec![0; width * 16 * height * 16 * 4] };
    for (index, &tile) in layout.iter().enumerate() {
        let Ok(tile) = usize::try_from(tile) else { continue };
        if let Some(block) = tileset.map16_tiles.get(tile) {
            copy_image(block, &mut image, (index % width) * 16, (index / width) * 16);
        }
    }
    Ok(image)
}
