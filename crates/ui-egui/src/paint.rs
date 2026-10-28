//! What the graphics editors' tools do, on anything laid out as a grid of cells: a sheet's pixels, a screen map's
//! entries, a tileset's Map16 quadrants. Pure, so the tools are tested without drawing them. Every operation is
//! clipped to the grid, so a stroke or paste running off an edge just stops there.

use bigstar_formats::{IndexedTile8Bpp, Map16Tile, ScreenMapEntry};

pub trait Grid {
    type Cell: Copy + PartialEq;
    /// Width and height in cells.
    fn size(&self) -> (usize, usize);
    fn get(&self, x: usize, y: usize) -> Option<Self::Cell>;
    fn set(&mut self, x: usize, y: usize, cell: Self::Cell);
}

/// A cell position that may lie off the grid, as a drag past an edge gives.
pub type Point = (i64, i64);

fn cell<G: Grid + ?Sized>(grid: &G, (x, y): Point) -> Option<(usize, usize)> {
    let (width, height) = grid.size();
    let (x, y) = (usize::try_from(x).ok()?, usize::try_from(y).ok()?);
    (x < width && y < height).then_some((x, y))
}

pub fn get<G: Grid + ?Sized>(grid: &G, at: Point) -> Option<G::Cell> {
    let (x, y) = cell(grid, at)?;
    grid.get(x, y)
}

pub fn put<G: Grid + ?Sized>(grid: &mut G, at: Point, value: G::Cell) {
    if let Some((x, y)) = cell(grid, at) {
        grid.set(x, y, value);
    }
}

/// The cells from `from` to `to`, both included, as Bresenham steps them.
pub fn line_points(from: Point, to: Point) -> Vec<Point> {
    let (mut x, mut y) = from;
    let (dx, dy) = ((to.0 - x).abs(), -(to.1 - y).abs());
    let (sx, sy) = (if x < to.0 { 1 } else { -1 }, if y < to.1 { 1 } else { -1 });
    let mut error = dx + dy;
    let mut points = Vec::new();
    loop {
        points.push((x, y));
        if (x, y) == to {
            return points;
        }
        let doubled = 2 * error;
        if doubled >= dy {
            error += dy;
            x += sx;
        }
        if doubled <= dx {
            error += dx;
            y += sy;
        }
    }
}

pub fn line<G: Grid + ?Sized>(grid: &mut G, from: Point, to: Point, value: G::Cell) {
    for point in line_points(from, to) {
        put(grid, point, value);
    }
}

/// The rectangle two corners span, in either order, clipped to the grid: left, top, width, height.
pub fn region<G: Grid + ?Sized>(grid: &G, a: Point, b: Point) -> Option<(usize, usize, usize, usize)> {
    let (width, height) = grid.size();
    let clamp = |v: i64, limit: usize| v.clamp(0, limit as i64 - 1) as usize;
    if width == 0 || height == 0 {
        return None;
    }
    let (left, right) = (a.0.min(b.0), a.0.max(b.0));
    let (top, bottom) = (a.1.min(b.1), a.1.max(b.1));
    if right < 0 || bottom < 0 || left >= width as i64 || top >= height as i64 {
        return None;
    }
    let (left, top, right, bottom) = (clamp(left, width), clamp(top, height), clamp(right, width), clamp(bottom, height));
    Some((left, top, right - left + 1, bottom - top + 1))
}

pub fn rect<G: Grid + ?Sized>(grid: &mut G, a: Point, b: Point, value: G::Cell, filled: bool) {
    let (left, right) = (a.0.min(b.0), a.0.max(b.0));
    let (top, bottom) = (a.1.min(b.1), a.1.max(b.1));
    for y in top..=bottom {
        for x in left..=right {
            if filled || x == left || x == right || y == top || y == bottom {
                put(grid, (x, y), value);
            }
        }
    }
}

/// Fills the 4-connected area of cells equal to the one at `at`.
pub fn flood_fill<G: Grid + ?Sized>(grid: &mut G, at: Point, value: G::Cell) {
    let Some(target) = get(grid, at) else { return };
    if target == value {
        return;
    }
    let mut stack = vec![at];
    while let Some(point) = stack.pop() {
        if get(grid, point) != Some(target) {
            continue;
        }
        put(grid, point, value);
        let (x, y) = point;
        stack.extend([(x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)]);
    }
}

/// A rectangle of cells: a brush, a copied region, a picker selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pattern<C> {
    pub width: usize,
    pub height: usize,
    pub cells: Vec<C>,
}

impl<C: Copy> Pattern<C> {
    pub fn single(value: C) -> Self {
        Self { width: 1, height: 1, cells: vec![value] }
    }

    pub fn get(&self, x: usize, y: usize) -> Option<C> {
        if x >= self.width {
            return None;
        }
        self.cells.get(y * self.width + x).copied()
    }

    /// The pattern mirrored, each cell passed through `mirror` (which turns a tile's own flip bits).
    pub fn flipped(&self, horizontal: bool, vertical: bool, mirror: impl Fn(C) -> C) -> Self {
        let mut cells = Vec::with_capacity(self.cells.len());
        for y in 0..self.height {
            for x in 0..self.width {
                let sx = if horizontal { self.width - 1 - x } else { x };
                let sy = if vertical { self.height - 1 - y } else { y };
                if let Some(value) = self.get(sx, sy) {
                    cells.push(mirror(value));
                }
            }
        }
        Self { width: self.width, height: self.height, cells }
    }
}

/// The cells of the rectangle two corners span.
pub fn copy<G: Grid + ?Sized>(grid: &G, a: Point, b: Point) -> Option<Pattern<G::Cell>> {
    let (left, top, width, height) = region(grid, a, b)?;
    let mut cells = Vec::with_capacity(width * height);
    for y in top..top + height {
        for x in left..left + width {
            cells.push(grid.get(x, y)?);
        }
    }
    Some(Pattern { width, height, cells })
}

/// Places `pattern` with its top-left cell at `at`.
pub fn stamp<G: Grid + ?Sized>(grid: &mut G, pattern: &Pattern<G::Cell>, at: Point) {
    for y in 0..pattern.height {
        for x in 0..pattern.width {
            if let Some(value) = pattern.get(x, y) {
                put(grid, (at.0 + x as i64, at.1 + y as i64), value);
            }
        }
    }
}

/// Fills the rectangle with `pattern` repeated from its top-left corner, as NSMB Editor's Draw does.
pub fn fill_pattern<G: Grid + ?Sized>(grid: &mut G, a: Point, b: Point, pattern: &Pattern<G::Cell>) {
    if pattern.width == 0 || pattern.height == 0 {
        return;
    }
    let (left, top) = (a.0.min(b.0), a.1.min(b.1));
    let (right, bottom) = (a.0.max(b.0), a.1.max(b.1));
    for y in top..=bottom {
        for x in left..=right {
            let (px, py) = (((x - left) as usize) % pattern.width, ((y - top) as usize) % pattern.height);
            if let Some(value) = pattern.get(px, py) {
                put(grid, (x, y), value);
            }
        }
    }
}

/// Mirrors the rectangle in place; `mirror` turns each cell's own flip bits, for cells that have them.
pub fn flip_region<G: Grid + ?Sized>(grid: &mut G, a: Point, b: Point, horizontal: bool, vertical: bool, mirror: impl Fn(G::Cell) -> G::Cell) {
    let Some(copied) = copy(grid, a, b) else { return };
    let Some((left, top, _, _)) = region(grid, a, b) else { return };
    stamp(grid, &copied.flipped(horizontal, vertical, mirror), (left as i64, top as i64));
}

/// Applies `change` to every cell of the rectangle.
pub fn map_region<G: Grid + ?Sized>(grid: &mut G, a: Point, b: Point, change: impl Fn(G::Cell) -> G::Cell) {
    let Some((left, top, width, height)) = region(grid, a, b) else { return };
    for y in top..top + height {
        for x in left..left + width {
            if let Some(value) = grid.get(x, y) {
                grid.set(x, y, change(value));
            }
        }
    }
}

/// A screen entry turned over with the region it is in.
pub fn mirror_entry(horizontal: bool, vertical: bool) -> impl Fn(ScreenMapEntry) -> ScreenMapEntry {
    move |mut entry| {
        entry.horizontal_flip ^= horizontal;
        entry.vertical_flip ^= vertical;
        entry
    }
}

// --- Grids ---

/// A sheet of indexed pixels as a character file stores it: 8x8 tiles in reading order, or one row-major bitmap
/// when `linear`. A 4bpp pixel is a nibble, the low one first.
pub struct IndexedSheet<'a> {
    pub bytes: &'a mut [u8],
    pub width_tiles: usize,
    pub eight: bool,
    pub linear: bool,
}

impl IndexedSheet<'_> {
    fn tiles(&self) -> usize {
        self.bytes.len() / if self.eight { 64 } else { 32 }
    }

    fn index(&self, x: usize, y: usize) -> usize {
        let width_tiles = self.width_tiles.max(1);
        if self.linear { y * width_tiles * 8 + x } else { ((y / 8) * width_tiles + x / 8) * 64 + (y % 8) * 8 + x % 8 }
    }
}

impl Grid for IndexedSheet<'_> {
    type Cell = u8;

    fn size(&self) -> (usize, usize) {
        let width_tiles = self.width_tiles.max(1);
        (width_tiles * 8, self.tiles().div_ceil(width_tiles) * 8)
    }

    fn get(&self, x: usize, y: usize) -> Option<u8> {
        let index = self.index(x, y);
        if self.eight { self.bytes.get(index).copied() } else { self.bytes.get(index / 2).map(|byte| (byte >> ((index & 1) * 4)) & 15) }
    }

    fn set(&mut self, x: usize, y: usize, value: u8) {
        let index = self.index(x, y);
        if self.eight {
            if let Some(byte) = self.bytes.get_mut(index) {
                *byte = value;
            }
        } else if let Some(byte) = self.bytes.get_mut(index / 2) {
            let shift = (index & 1) * 4;
            *byte = (*byte & !(15 << shift)) | ((value & 15) << shift);
        }
    }
}

/// Decoded 8bpp tiles laid out `width_tiles` across, as a tileset's graphics sheet draws them.
pub struct TileSheet<'a> {
    pub tiles: &'a mut [IndexedTile8Bpp],
    pub width_tiles: usize,
}

impl Grid for TileSheet<'_> {
    type Cell = u8;

    fn size(&self) -> (usize, usize) {
        let width_tiles = self.width_tiles.max(1);
        (width_tiles * 8, self.tiles.len().div_ceil(width_tiles) * 8)
    }

    fn get(&self, x: usize, y: usize) -> Option<u8> {
        let tile = self.tiles.get((y / 8) * self.width_tiles.max(1) + x / 8)?;
        tile.pixels.get((y % 8) * 8 + x % 8).copied()
    }

    fn set(&mut self, x: usize, y: usize, value: u8) {
        let width_tiles = self.width_tiles.max(1);
        if let Some(pixel) = self.tiles.get_mut((y / 8) * width_tiles + x / 8).and_then(|t| t.pixels.get_mut((y % 8) * 8 + x % 8)) {
            *pixel = value;
        }
    }
}

/// A screen map's entries, `width` across.
pub struct Screen<'a> {
    pub entries: &'a mut [ScreenMapEntry],
    pub width: usize,
}

impl Grid for Screen<'_> {
    type Cell = ScreenMapEntry;

    fn size(&self) -> (usize, usize) {
        let width = self.width.max(1);
        (width, self.entries.len() / width)
    }

    fn get(&self, x: usize, y: usize) -> Option<ScreenMapEntry> {
        self.entries.get(y * self.width.max(1) + x).copied()
    }

    fn set(&mut self, x: usize, y: usize, value: ScreenMapEntry) {
        if let Some(entry) = self.entries.get_mut(y * self.width.max(1) + x) {
            *entry = value;
        }
    }
}

/// A screen map as the file stores it: little-endian halfwords, bits 0-9 the tile, 10 and 11 the flips, 12-15 the
/// palette row.
pub struct RawScreen<'a> {
    pub bytes: &'a mut [u8],
    pub width: usize,
}

pub fn decode_entry(value: u16) -> ScreenMapEntry {
    ScreenMapEntry::new(i32::from(value & 0x3ff), (value >> 12) as i16, value & 0x400 != 0, value & 0x800 != 0)
}

pub fn encode_entry(entry: ScreenMapEntry) -> u16 {
    (entry.tile.clamp(0, 0x3ff) as u16)
        | (u16::from(entry.horizontal_flip) << 10)
        | (u16::from(entry.vertical_flip) << 11)
        | ((entry.palette.clamp(0, 15) as u16) << 12)
}

impl Grid for RawScreen<'_> {
    type Cell = ScreenMapEntry;

    fn size(&self) -> (usize, usize) {
        let width = self.width.max(1);
        (width, self.bytes.len() / 2 / width)
    }

    fn get(&self, x: usize, y: usize) -> Option<ScreenMapEntry> {
        let at = (y * self.width.max(1) + x) * 2;
        let pair = self.bytes.get(at..at + 2)?;
        Some(decode_entry(u16::from_le_bytes([*pair.first()?, *pair.get(1)?])))
    }

    fn set(&mut self, x: usize, y: usize, value: ScreenMapEntry) {
        let at = (y * self.width.max(1) + x) * 2;
        if let Some(pair) = self.bytes.get_mut(at..at + 2) {
            pair.copy_from_slice(&encode_entry(value).to_le_bytes());
        }
    }
}

/// A tileset's Map16 blocks as 8x8 quadrants, `width_blocks` blocks across: the layout the Map16 sheet draws, so the
/// sheet can be painted like any tilemap.
pub struct Map16Sheet<'a> {
    pub blocks: &'a mut [Map16Tile],
    pub width_blocks: usize,
}

impl Map16Sheet<'_> {
    /// The block and quadrant under quadrant cell (`x`, `y`).
    pub fn locate(width_blocks: usize, x: usize, y: usize) -> (usize, usize) {
        ((y / 2) * width_blocks.max(1) + x / 2, (y % 2) * 2 + x % 2)
    }
}

impl Grid for Map16Sheet<'_> {
    type Cell = ScreenMapEntry;

    fn size(&self) -> (usize, usize) {
        let width = self.width_blocks.max(1);
        (width * 2, self.blocks.len().div_ceil(width) * 2)
    }

    fn get(&self, x: usize, y: usize) -> Option<ScreenMapEntry> {
        let (block, quadrant) = Self::locate(self.width_blocks, x, y);
        self.blocks.get(block)?.tiles.get(quadrant).copied()
    }

    fn set(&mut self, x: usize, y: usize, value: ScreenMapEntry) {
        let (block, quadrant) = Self::locate(self.width_blocks, x, y);
        if let Some(entry) = self.blocks.get_mut(block).and_then(|b| b.tiles.get_mut(quadrant)) {
            *entry = value;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bigstar_formats::{EditableTileset, render_map16_sheet};

    struct Plain {
        width: usize,
        cells: Vec<u8>,
    }

    impl Grid for Plain {
        type Cell = u8;
        fn size(&self) -> (usize, usize) {
            (self.width, self.cells.len() / self.width)
        }
        fn get(&self, x: usize, y: usize) -> Option<u8> {
            self.cells.get(y * self.width + x).copied()
        }
        fn set(&mut self, x: usize, y: usize, value: u8) {
            self.cells[y * self.width + x] = value;
        }
    }

    fn plain(width: usize, height: usize) -> Plain {
        Plain { width, cells: vec![0; width * height] }
    }

    #[test]
    fn lines_rectangles_and_fills_stay_on_the_grid() {
        let mut grid = plain(4, 4);
        line(&mut grid, (-2, -2), (5, 5), 1);
        assert_eq!([grid.cells[0], grid.cells[5], grid.cells[10], grid.cells[15]], [1; 4]);
        let mut grid = plain(4, 4);
        rect(&mut grid, (1, 1), (5, 5), 2, false);
        assert_eq!(grid.cells[5], 2);
        assert_eq!(grid.cells[15], 0, "the far corner is off the grid, so its edges are too");
        flood_fill(&mut grid, (0, 0), 3);
        assert_eq!(grid.cells.iter().filter(|&&c| c == 3).count(), 7, "the top row and left column around the outline");
        assert_eq!(grid.cells[10], 0, "inside the outline is not reached");
    }

    #[test]
    fn a_pattern_tiles_across_a_rectangle_and_a_paste_clips_at_the_edge() {
        let mut grid = plain(5, 2);
        let pattern = Pattern { width: 2, height: 1, cells: vec![7, 8] };
        fill_pattern(&mut grid, (4, 1), (0, 0), &pattern);
        assert_eq!(grid.cells, [7, 8, 7, 8, 7, 7, 8, 7, 8, 7]);
        let copied = copy(&grid, (0, 0), (1, 1)).unwrap();
        stamp(&mut grid, &copied, (4, 1));
        assert_eq!(grid.cells[9], 7);
        assert_eq!(copy(&grid, (9, 9), (12, 12)), None);
    }

    #[test]
    fn flipping_a_region_turns_each_entrys_own_bits_too() {
        let mut entries = vec![ScreenMapEntry::new(1, 0, false, false), ScreenMapEntry::new(2, 0, true, false)];
        let mut screen = Screen { entries: &mut entries, width: 2 };
        flip_region(&mut screen, (0, 0), (1, 0), true, false, mirror_entry(true, false));
        assert_eq!(entries, [ScreenMapEntry::new(2, 0, false, false), ScreenMapEntry::new(1, 0, true, false)]);
    }

    #[test]
    fn sheets_address_pixels_as_the_file_lays_them_out() {
        let mut bytes = vec![0u8; 64];
        let mut sheet = IndexedSheet { bytes: &mut bytes, width_tiles: 2, eight: false, linear: false };
        assert_eq!(sheet.size(), (16, 8));
        sheet.set(9, 0, 5);
        assert_eq!(sheet.get(9, 0), Some(5));
        assert_eq!(bytes[32], 0x50, "pixel 1 of tile 1, the high nibble");
        let mut bytes = vec![0u8; 128];
        IndexedSheet { bytes: &mut bytes, width_tiles: 2, eight: true, linear: true }.set(8, 0, 9);
        assert_eq!(bytes[8], 9);
    }

    #[test]
    fn raw_screen_entries_round_trip_their_bits() {
        let entry = ScreenMapEntry::new(0x123, 5, true, false);
        assert_eq!(decode_entry(encode_entry(entry)), entry);
    }

    #[test]
    fn the_map16_grid_matches_the_sheet_the_editor_draws() {
        // Block 1's bottom-right quadrant names tile 1, white; everything else names tile 0, clear.
        let mut tileset = EditableTileset {
            graphics: vec![IndexedTile8Bpp::default(), IndexedTile8Bpp { pixels: [1; 64] }],
            palette: vec![bigstar_formats::Bgr555Color::new(0, 0, 0, true), bigstar_formats::Bgr555Color::new(255, 255, 255, false)],
            map16: vec![Map16Tile::default(); 3],
            ..EditableTileset::default()
        };
        let mut sheet = Map16Sheet { blocks: &mut tileset.map16, width_blocks: 2 };
        assert_eq!(sheet.size(), (4, 4));
        sheet.set(3, 1, ScreenMapEntry::new(1, 0, false, false));
        assert_eq!(tileset.map16[1].tiles[3].tile, 1);
        let image = render_map16_sheet(&tileset, 2).unwrap();
        let at = |x: usize, y: usize| image.pixels[(y * image.width + x) * 4 + 3];
        assert_eq!((at(3 * 8, 8), at(2 * 8, 8)), (255, 0));
    }
}
