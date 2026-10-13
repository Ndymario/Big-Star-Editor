//! What the game draws in place of a Map16 block it hands to the 3D engine, and the substitution itself. The data
//! types and image work of `libs/nsmb-project/src/lua_tiles.cpp`; the Lua loader that fills them arrives in phase 12.
//!
//! `StageLayout::renderTileRow` passes every block through `handle3DTile` before writing the tilemap, and for a good
//! many of them what reaches the screen is a textured quad instead: the pipes, fences, pillars, quicksand, tower and
//! water surfaces. See `docs/tileset-memory.md` and `docs/lua-tiles.md`.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use bigstar_formats::RgbaImage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TileOverlay {
    /// What the game draws in the block's place. `None` when it draws nothing, which is a real answer: a few blocks
    /// are blanked with no quad behind them.
    pub image: Option<Arc<RgbaImage>>,
    /// Set when the game rewrites the block to another block rather than to a texture, as it does for the thin fences.
    pub remap: Option<u16>,
    /// False when the block's own Map16 still draws and the texture goes over it, which is what the tower and water
    /// blocks do.
    pub replaces: bool,
}

impl Default for TileOverlay {
    fn default() -> Self {
        Self { image: None, remap: None, replaces: true }
    }
}

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

    /// Distinct blocks claimed for this tileset, which is what a log line wants.
    pub fn block_count(&self, tileset_id: u16) -> usize {
        self.scripts.iter().filter(|script| script.speaks_for(tileset_id)).flat_map(|script| script.blocks.keys()).collect::<HashSet<_>>().len()
    }
}

/// Scales `source` over a `width` x `height` rectangle of `target`, nearest neighbour, skipping clear pixels.
pub(crate) fn blit_over(target: &mut RgbaImage, left: usize, top: usize, width: usize, height: usize, source: &RgbaImage) {
    if width == 0 || height == 0 || source.width == 0 || source.height == 0 {
        return;
    }
    for y in 0..height {
        let sy = y * source.height / height;
        for x in 0..width {
            let sx = x * source.width / width;
            let from = (sy * source.width + sx) * 4;
            let Some(pixel) = source.pixels.get(from..from + 4) else { continue };
            if pixel.get(3) == Some(&0) {
                continue;
            }
            let to = ((top + y) * target.width + left + x) * 4;
            if let Some(slot) = target.pixels.get_mut(to..to + 4) {
                slot.copy_from_slice(pixel);
            }
        }
    }
}

fn clear_rect(target: &mut RgbaImage, left: usize, top: usize, width: usize, height: usize) {
    for y in 0..height {
        let start = ((top + y) * target.width + left) * 4;
        if let Some(row) = target.pixels.get_mut(start..start + width * 4) {
            row.fill(0);
        }
    }
}

fn blank(width: usize, height: usize) -> RgbaImage {
    RgbaImage { width, height, pixels: vec![0; width * height * 4] }
}

/// `page_offset` is the tileset's first absolute block: 0 for jyotyu, 256 for the level tileset, 1024 for nohara.
/// One block per image, which is what the level canvas draws from.
pub fn overlay_map16_images(blocks: &mut [RgbaImage], tileset_id: u16, page_offset: u16, overlays: &TileOverlayCatalog) {
    // Resolved against the Map16 as the files describe it, then applied in one pass, so a block the game rewrites to
    // another block picks up that block's own graphics rather than whatever replaced them.
    let mut replacements = Vec::new();
    for (i, block) in blocks.iter().enumerate() {
        let Some(overlay) = overlays.find(tileset_id, page_offset.wrapping_add(i as u16)) else { continue };
        if let Some(remap) = overlay.remap {
            let from = usize::from(remap);
            let page = usize::from(page_offset);
            let image = from.checked_sub(page).and_then(|index| blocks.get(index)).cloned().unwrap_or_else(|| blank(block.width, block.height));
            replacements.push((i, image));
        } else if overlay.replaces {
            let mut image = blank(block.width, block.height);
            if let Some(texture) = &overlay.image {
                let (width, height) = (image.width, image.height);
                blit_over(&mut image, 0, 0, width, height, texture);
            }
            replacements.push((i, image));
        } else if let Some(texture) = &overlay.image {
            let mut image = block.clone();
            let (width, height) = (image.width, image.height);
            blit_over(&mut image, 0, 0, width, height, texture);
            replacements.push((i, image));
        }
    }
    for (index, image) in replacements {
        if let Some(slot) = blocks.get_mut(index) {
            *slot = image;
        }
    }
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
    if blocks_wide == 0 || sheet.width == 0 || sheet.height == 0 {
        return;
    }
    let cell = sheet.width / blocks_wide;
    if cell == 0 {
        return;
    }
    // Remaps read the sheet before anything is drawn on it, for the same reason as above.
    let original = sheet.clone();
    for i in 0..block_count {
        let Some(overlay) = overlays.find(tileset_id, page_offset.wrapping_add(i as u16)) else { continue };
        let (left, top) = ((i % blocks_wide) * cell, (i / blocks_wide) * cell);
        if top + cell > sheet.height {
            break;
        }
        if let Some(remap) = overlay.remap {
            clear_rect(sheet, left, top, cell, cell);
            let Some(index) = usize::from(remap).checked_sub(usize::from(page_offset)).filter(|&index| index < block_count) else { continue };
            let (source_left, source_top) = ((index % blocks_wide) * cell, (index / blocks_wide) * cell);
            if source_top + cell > original.height {
                continue;
            }
            for y in 0..cell {
                let from = ((source_top + y) * original.width + source_left) * 4;
                let to = ((top + y) * sheet.width + left) * 4;
                if let (Some(row), Some(target)) = (original.pixels.get(from..from + cell * 4), sheet.pixels.get_mut(to..to + cell * 4)) {
                    target.copy_from_slice(row);
                }
            }
        } else if overlay.replaces {
            clear_rect(sheet, left, top, cell, cell);
            if let Some(texture) = &overlay.image {
                blit_over(sheet, left, top, cell, cell, texture);
            }
        } else if let Some(texture) = &overlay.image {
            blit_over(sheet, left, top, cell, cell, texture);
        }
    }
}
