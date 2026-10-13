//! What the game draws in place of a Map16 block it hands to the 3D engine, and the substitution itself. The data
//! types and image work of `libs/nsmb-project/src/lua_tiles.cpp`; the Lua loader that fills them arrives in phase 12.
//!
//! `StageLayout::renderTileRow` passes every block through `handle3DTile` before writing the tilemap, and for a good
//! many of them what reaches the screen is a textured quad instead: the pipes, fences, pillars, quicksand, tower and
//! water surfaces. See `docs/tileset-memory.md` and `docs/lua-tiles.md`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use bigstar_formats::RgbaImage;
pub use bigstar_formats::TileOverlay;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TileOverlayScript {
    pub name: String,
    pub source: PathBuf,
    /// Which tileset ids this script speaks for. Empty means every tileset.
    pub tilesets: Vec<u16>,
    pub blocks: HashMap<u16, TileOverlay>,
    /// Files on disk behind the textures, for the watcher.
    pub resources: Vec<PathBuf>,
    /// Blocks whose texture would not resolve, named so the log can say which. The script keeps the blocks that did
    /// resolve.
    pub errors: Vec<String>,
}

impl TileOverlayScript {
    fn speaks_for(&self, tileset_id: u16) -> bool {
        self.tilesets.is_empty() || self.tilesets.contains(&tileset_id)
    }
}

/// Scripts in layer order; the first one that speaks for a tileset and claims a block wins, so a module overrides
/// one block without restating the file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TileOverlayCatalog {
    pub scripts: Vec<TileOverlayScript>,
}

impl TileOverlayCatalog {
    pub fn find(&self, tileset_id: u16, block: u16) -> Option<&TileOverlay> {
        self.scripts.iter().filter(|script| script.speaks_for(tileset_id)).find_map(|script| script.blocks.get(&block))
    }

    pub fn is_empty(&self) -> bool {
        self.scripts.iter().all(|script| script.blocks.is_empty())
    }

    /// Every block claimed for this tileset with what replaces it, the first script to claim a block winning, in block
    /// order.
    pub fn blocks(&self, tileset_id: u16) -> Vec<(u16, &TileOverlay)> {
        let mut blocks: Vec<u16> =
            self.scripts.iter().filter(|script| script.speaks_for(tileset_id)).flat_map(|script| script.blocks.keys().copied()).collect();
        blocks.sort_unstable();
        blocks.dedup();
        blocks.into_iter().filter_map(|block| self.find(tileset_id, block).map(|overlay| (block, overlay))).collect()
    }

    /// Distinct blocks claimed for this tileset, which is what a log line wants.
    pub fn block_count(&self, tileset_id: u16) -> usize {
        self.scripts.iter().filter(|script| script.speaks_for(tileset_id)).flat_map(|script| script.blocks.keys()).collect::<HashSet<_>>().len()
    }
}

/// The substitution for one tileset's blocks, one image per block; see [`bigstar_formats::overlay_map16_images`].
pub fn overlay_map16_images(blocks: &mut [RgbaImage], tileset_id: u16, page_offset: u16, overlays: &TileOverlayCatalog) {
    bigstar_formats::overlay_map16_images(blocks, page_offset, |block| overlays.find(tileset_id, block));
}

/// The same substitution on one sheet of `blocks_wide` blocks per row, which is what the tileset editor draws from.
pub fn overlay_map16_sheet(
    sheet: &mut RgbaImage,
    blocks_wide: usize,
    block_count: usize,
    tileset_id: u16,
    page_offset: u16,
    overlays: &TileOverlayCatalog,
) {
    bigstar_formats::overlay_map16_sheet(sheet, blocks_wide, block_count, page_offset, |block| overlays.find(tileset_id, block));
}
