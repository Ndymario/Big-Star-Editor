//! What a question block or a brick holds, shown with NSMB Editor's icons for it (its `tileoverrides2.png`): a
//! 16-pixel cell for each, laid over the whole block as NSMB Editor lays them.

use bigstar_formats::decode_tile_behavior;
use egui::{Color32, Mesh, Pos2, Rect, Vec2};

pub(crate) const SHEET: &[u8] = include_bytes!("../assets/level/block-contents.png");
const SHEET_CELLS: u32 = 28;

/// The sheet's cell for a block with behaviour `value`, read off the parameter its question or brick flag gives it.
/// The sheet runs through a question block's parameters 0 to 13 (11 is blank: nothing is known to come out), the
/// two thrown ones, a brick's 1 to 6, and the invisible blocks' 0x30 to 0x35, drawn dashed. Bricks the sheet has no
/// cell of their own for borrow the question block's. `None` for any other block and for an unknown parameter.
pub fn contents_cell(value: u32) -> Option<u32> {
    let parts = decode_tile_behavior(value);
    let parameter = u32::from(parts.parameter);
    if parts.flags & 4 != 0 {
        return match parameter {
            0..=10 | 12 | 13 => Some(parameter),
            0x10 => Some(14),
            0x11 => Some(15),
            0x30..=0x35 => Some(parameter - 0x30 + 22),
            _ => None,
        };
    }
    if parts.flags & 0x10 != 0 {
        return match parameter {
            1..=6 => Some(parameter + 15),
            0x34 => Some(16),
            0x45 => Some(19),
            0x95 => Some(5),
            0x97 => Some(13),
            _ => None,
        };
    }
    None
}

/// Adds sheet cell `cell` over `block` (screen points) to a mesh textured with the sheet.
pub fn add(mesh: &mut Mesh, block: Rect, cell: u32) {
    let width = 1.0 / SHEET_CELLS as f32;
    let uv = Rect::from_min_size(Pos2::new(cell.min(SHEET_CELLS - 1) as f32 * width, 0.0), Vec2::new(width, 1.0));
    mesh.add_rect_with_uv(block, uv, Color32::WHITE);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigstar_formats::{TileBehaviorParts, encode_tile_behavior};

    fn block(flags: u32, parameter: u8) -> u32 {
        encode_tile_behavior(TileBehaviorParts { flags, subtype: 0, parameter })
    }

    #[test]
    fn question_blocks_and_bricks_find_their_cell() {
        // Solid and question, solid and brick, as the game's own blocks are flagged.
        assert_eq!(contents_cell(block(1 | 4, 0)), Some(0), "mushroom or flower");
        assert_eq!(contents_cell(block(1 | 4, 7)), Some(7), "coin");
        assert_eq!(contents_cell(block(1 | 4, 0x11)), Some(15), "thrown coin");
        assert_eq!(contents_cell(block(1 | 4, 0x33)), Some(25), "invisible coin, dashed");
        assert_eq!(contents_cell(block(1 | 4, 11)), None, "nothing known comes out");
        assert_eq!(contents_cell(block(1 | 0x10, 5)), Some(20), "a brick's run of coins");
        assert_eq!(contents_cell(block(1 | 0x10, 0x95)), Some(5), "a brick's mini mushroom borrows the question block's");
        assert_eq!(contents_cell(block(1 | 0x10, 0)), None, "an empty brick");
        assert_eq!(contents_cell(block(2, 0)), None, "a coin is not a block holding one");
        assert_eq!(contents_cell(block(1, 7)), None);
        // The sheet has a cell for every one of them.
        let sheet = image::load_from_memory(SHEET).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (SHEET_CELLS * 16, 16));
    }
}
