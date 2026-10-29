//! The tileset workspace: a level's three tilesets (graphics, Map16, objects, behaviours) and its background and
//! foreground, edited together and saved as only the files that changed. A port of `TilesetEditorDialog` and of
//! `MainWindow::openTilesetEditor`'s saving; the document is pure and `show` draws it as a tab.

use crate::controls::{self, Button, Choice as _, TextField, Width};
use crate::icons;
use crate::properties::TILESET_NAMES;
use std::collections::BTreeMap;
use std::sync::Arc;

use bigstar_formats::{
    BACKGROUND_PALETTE_OFFSET, BACKGROUND_TILE_OFFSET, Bgr555Color, EditableTilemap, EditableTileset, FOREGROUND_PALETTE_OFFSET,
    FOREGROUND_TILE_OFFSET, IndexedTile8Bpp, OBJECT_LAYOUT_EMPTY_TILE, ObjectLayout, ObjectLayoutTile, RgbaImage, ScreenMapEntry,
    TILE_BEHAVIOR_FLAG_NAMES, TILE_BEHAVIOR_PIPE_MAIN_NAMES, TILE_BEHAVIOR_SUBTYPE_NAMES, TileBehaviorParameterKind, TileBehaviorParts, TileOverlay,
    TilemapResourceData, TilesetResourceData, bgr555_to_rgb, compose_tileset_memory, decode_tile_behavior, encode_tile_behavior,
    import_tilemap_image, overlay_map16_sheet, pack_bgr555, parse_tilemap, parse_tileset, parse_tileset_bundle, render_composed_map16_sheet,
    render_map16_sheet, render_object_layout, render_screen_map, render_tilemap, tile_behavior_parameter_kind, tile_behavior_parameter_name,
    write_tilemap, write_tileset, write_tileset_bundle,
};
use bigstar_level::LevelTilesetReferences;
use bigstar_protocol::{FileWrite, LevelId, ResourceBytes, TilemapSlot, TilesetResources, TilesetSlot};
use egui::{Color32, Pos2, Rect, RichText, Stroke, StrokeKind, Ui, Vec2};

use crate::graphics_editor::entry_text;
use crate::history::{History, Snapshots, Undoable};
use crate::image_manager::{self, Bitmap, Images, Lists, ManagerState, decode_png, encode_png};
use crate::paint::{Grid, Map16Sheet, Pattern, Screen, TileSheet};
use crate::palette_panel;
use crate::pixel_editor::{self, Command, HistoryButton, PixelEditor, history_buttons};
use crate::services::Services;
use crate::tile_picker::{self, PickerState};
use crate::tilemap_editor::TilemapEditor;
use crate::zoom_canvas::{self, CanvasInput, upload};

/// Tiles across in the graphics sheets and pickers, and Map16 blocks across in the Map16 sheet.
pub const GRAPHICS_COLUMNS: usize = 32;
pub const MAP16_COLUMNS: usize = 16;

pub const SLOT_NAMES: [(u8, &str); 3] = [(0, "shared"), (1, "level"), (2, "sub-unit")];
/// A bitmap the Graphics tab lists: one of the tilesets' graphics, or the background's (0) or foreground's (1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Sheet {
    /// The tileset in the chosen slot.
    #[default]
    Current,
    Tileset(usize),
    Layer(usize),
}

/// A colour rounded to what the DS stores, as eight bits a channel.
fn rounded([r, g, b]: [u8; 3]) -> Bgr555Color {
    let [r, g, b] = bgr555_to_rgb(pack_bgr555(r, g, b));
    Bgr555Color::new(r, g, b, false)
}

pub const QUADRANTS: [&str; 4] = ["Top left", "Top right", "Bottom left", "Bottom right"];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Slots,
    Graphics,
    Map16,
    Objects,
    Behaviors,
    Background,
    Foreground,
}

pub const TABS: [(Tab, &str); 7] = [
    (Tab::Slots, "Slots"),
    (Tab::Graphics, "Graphics"),
    (Tab::Map16, "Map16"),
    (Tab::Objects, "Objects"),
    (Tab::Behaviors, "Behaviors"),
    (Tab::Background, "Background"),
    (Tab::Foreground, "Foreground"),
];

fn resource(slot: &TilesetSlot) -> TilesetResourceData {
    TilesetResourceData {
        graphics: slot.graphics.bytes.clone(),
        palette: slot.palette.bytes.clone(),
        map16: slot.map16.bytes.clone(),
        objects: slot.objects.bytes.clone(),
        object_index: slot.object_index.bytes.clone(),
        behaviors: slot.behaviors.bytes.clone(),
    }
}

fn map_resource(slot: &TilemapSlot) -> TilemapResourceData {
    TilemapResourceData { graphics: slot.graphics.bytes.clone(), palette: slot.palette.bytes.clone(), screen: slot.screen.bytes.clone() }
}

/// One level's tilesets and backgrounds being edited.
pub struct TilesetDoc {
    pub level: LevelId,
    pub title: String,
    pub tilesets: Vec<EditableTileset>,
    pub tilemaps: [EditableTilemap; 2],
    /// The files they came from, as read: what a save compares against and where it writes.
    sources: Vec<TilesetSlot>,
    map_sources: [TilemapSlot; 2],
    /// The references the level had, and the ones the Slots tab says now.
    initial: LevelTilesetReferences,
    pub references: LevelTilesetReferences,
    overlays: BTreeMap<u16, TileOverlay>,
    pub slot: usize,
    pub tab: Tab,
    /// Draw Map16 entries that name another tileset's graphics against all three, as the game loads them.
    pub composed: bool,
    /// Draw what the game hands to the 3D engine instead of the Map16 bytes.
    pub solid3d: bool,
    pub map16_entry: usize,
    pub quadrant: usize,
    pub object: usize,
    pub object_cell: Option<(usize, usize)>,
    pub behavior_entry: usize,
    pub map_entries: [usize; 2],
    /// Grows with every change, so drawn sheets are uploaded again.
    pub version: u64,
    history: History<Snapshot>,
}

/// What an edit can change, for undo: every tileset and both tilemaps.
type Snapshot = (Vec<EditableTileset>, [EditableTilemap; 2]);

impl Snapshots for TilesetDoc {
    type State = Snapshot;

    fn history(&self) -> &History<Snapshot> {
        &self.history
    }

    fn history_mut(&mut self) -> &mut History<Snapshot> {
        &mut self.history
    }

    fn snapshot(&self) -> Snapshot {
        (self.tilesets.clone(), self.tilemaps.clone())
    }

    fn restore(&mut self, (tilesets, tilemaps): Snapshot) {
        self.tilesets = tilesets;
        self.tilemaps = tilemaps;
        self.object = self.object.min(self.objects().len().saturating_sub(1));
        self.object_cell = None;
        self.touched();
    }
}

impl TilesetDoc {
    pub fn open(resources: TilesetResources, title: &str) -> Result<Self, String> {
        let mut tilesets = Vec::new();
        for slot in &resources.tilesets {
            let parsed = parse_tileset(&resource(slot), slot.tile_offset, slot.palette_offset, slot.tile_page_offset).map_err(|e| e.message)?;
            tilesets.push(parsed);
        }
        if tilesets.len() != 3 {
            return Err("the level does not name three tilesets".to_owned());
        }
        let background =
            parse_tilemap(&map_resource(&resources.background), 0, BACKGROUND_TILE_OFFSET, BACKGROUND_PALETTE_OFFSET).map_err(|e| e.message)?;
        let foreground =
            parse_tilemap(&map_resource(&resources.foreground), 0, FOREGROUND_TILE_OFFSET, FOREGROUND_PALETTE_OFFSET).map_err(|e| e.message)?;
        let overlays = resources
            .overlays
            .into_iter()
            .map(|o| (o.block, TileOverlay { image: o.image.map(Arc::new), remap: o.remap, replaces: o.replaces }))
            .collect();
        Ok(Self {
            level: resources.level,
            title: title.to_owned(),
            tilesets,
            tilemaps: [background, foreground],
            sources: resources.tilesets,
            map_sources: [resources.background, resources.foreground],
            initial: resources.references,
            references: resources.references,
            overlays,
            slot: 1,
            tab: Tab::default(),
            composed: true,
            solid3d: true,
            map16_entry: 0,
            quadrant: 0,
            object: 0,
            object_cell: None,
            behavior_entry: 0,
            map_entries: [0, 0],
            version: 0,
            history: History::default(),
        })
    }

    /// A side panel's id. Panels are named across the whole window, so each workspace's are told apart by its level,
    /// and two shown side by side keep their own sizes.
    fn panel_id(&self, name: &str) -> egui::Id {
        egui::Id::new((name, self.level))
    }

    fn touched(&mut self) {
        self.version += 1;
    }

    // --- Undo ---

    /// One edit, as one undo step.
    pub fn edit<R>(&mut self, name: &str, change: impl FnOnce(&mut Self) -> R) -> R {
        self.begin_edit(name);
        let result = change(self);
        self.end_edit();
        result
    }

    pub fn tileset(&self) -> Option<&EditableTileset> {
        self.tilesets.get(self.slot)
    }

    fn tileset_mut(&mut self) -> Option<&mut EditableTileset> {
        self.version += 1;
        self.tilesets.get_mut(self.slot)
    }

    pub fn set_slot(&mut self, slot: usize) {
        if slot < self.tilesets.len() {
            self.slot = slot;
            self.map16_entry = 0;
            self.object = 0;
            self.object_cell = None;
            self.behavior_entry = 0;
            self.touched();
        }
    }

    /// The level references the Slots tab edits; they take effect when the level is saved and reloaded.
    pub fn references_changed(&self) -> bool {
        self.references != self.initial
    }

    // --- Pictures ---

    /// The bitmaps the Graphics tab lists: every tileset that has graphics, then the background and foreground's.
    pub fn sheets(&self) -> Vec<(Sheet, String)> {
        let tilesets = self.tilesets.iter().enumerate().filter(|(_, t)| !t.graphics.is_empty()).map(|(i, _)| {
            (
                Sheet::Tileset(i),
                format!("Tileset {}", SLOT_NAMES.get(i).map(|(n, name)| controls::numbered(i64::from(*n), name, false)).unwrap_or_default()),
            )
        });
        let layers = self
            .tilemaps
            .iter()
            .enumerate()
            .filter(|(_, m)| !m.graphics.is_empty())
            .map(|(i, _)| (Sheet::Layer(i), if i == 0 { "Background".to_owned() } else { "Foreground".to_owned() }));
        tilesets.chain(layers).collect()
    }

    /// `sheet`, with the chosen slot's tileset named by its slot.
    pub fn resolve(&self, sheet: Sheet) -> Sheet {
        if sheet == Sheet::Current { Sheet::Tileset(self.slot) } else { sheet }
    }

    fn sheet_parts(&self, sheet: Sheet) -> Option<(&[IndexedTile8Bpp], &[Bgr555Color])> {
        match self.resolve(sheet) {
            Sheet::Current => None,
            Sheet::Tileset(i) => self.tilesets.get(i).map(|t| (t.graphics.as_slice(), t.palette.as_slice())),
            Sheet::Layer(i) => self.tilemaps.get(i).map(|m| (m.graphics.as_slice(), m.palette.as_slice())),
        }
    }

    fn sheet_parts_mut(&mut self, sheet: Sheet) -> Option<(&mut Vec<IndexedTile8Bpp>, &mut Vec<Bgr555Color>)> {
        self.version += 1;
        match self.resolve(sheet) {
            Sheet::Current => None,
            Sheet::Tileset(i) => self.tilesets.get_mut(i).map(|t| (&mut t.graphics, &mut t.palette)),
            Sheet::Layer(i) => self.tilemaps.get_mut(i).map(|m| (&mut m.graphics, &mut m.palette)),
        }
    }

    /// How many 256-colour palettes `sheet` has.
    pub fn sheet_palettes(&self, sheet: Sheet) -> usize {
        self.sheet_parts(sheet).map_or(0, |(_, palette)| palette.len().div_ceil(256))
    }

    /// Palette `palette` of `sheet`'s, its 256 colours as eight bits a channel.
    pub fn sheet_palette(&self, sheet: Sheet, palette: usize) -> Option<Vec<[u8; 3]>> {
        let (_, colours) = self.sheet_parts(sheet)?;
        let chosen = colours.get(palette * 256..)?;
        Some(chosen.iter().take(256).map(|c| [c.red, c.green, c.blue]).collect())
    }

    /// `sheet`'s graphics drawn in palette `palette`, `GRAPHICS_COLUMNS` tiles across.
    pub fn sheet_image(&self, sheet: Sheet, palette: usize) -> Result<RgbaImage, String> {
        let (tiles, colours) = self.sheet_parts(sheet).ok_or("no graphics")?;
        let mut tiles = tiles.to_vec();
        let bitmap = Bitmap::read(&TileSheet { width_tiles: GRAPHICS_COLUMNS.min(tiles.len().max(1)), tiles: &mut tiles });
        let palette: Vec<[u8; 3]> = colours.iter().skip(palette * 256).take(256).map(|c| [c.red, c.green, c.blue]).collect();
        Ok(image_manager::render(&bitmap, &palette))
    }

    /// The sheet the Map16 and Behaviours tabs both draw. Composed, an entry naming another tileset's graphics
    /// resolves instead of coming out clear; with 3D on, blocks the game draws as quads show those.
    pub fn map16_sheet(&self) -> Result<RgbaImage, String> {
        let tileset = self.tileset().ok_or("no tileset")?;
        let mut sheet = if self.composed {
            render_composed_map16_sheet(tileset, &compose_tileset_memory(&self.tilesets), MAP16_COLUMNS)
        } else {
            render_map16_sheet(tileset, MAP16_COLUMNS)
        }
        .map_err(|e| e.message)?;
        if self.solid3d && !self.overlays.is_empty() {
            let page = u16::from(tileset.tile_page_offset) * 256;
            overlay_map16_sheet(&mut sheet, MAP16_COLUMNS, tileset.map16.len(), page, |block| self.overlays.get(&block));
        }
        Ok(sheet)
    }

    pub fn tilemap_image(&self, which: usize) -> Result<RgbaImage, String> {
        let map = self.tilemaps.get(which).ok_or("no tilemap")?;
        render_tilemap(map).map_err(|e| e.message)
    }

    // --- Map16 ---

    pub fn map16_count(&self) -> usize {
        self.tileset().map_or(0, |t| t.map16.len())
    }

    pub fn map16_value(&self) -> Option<ScreenMapEntry> {
        self.tileset()?.map16.get(self.map16_entry)?.tiles.get(self.quadrant).copied()
    }

    pub fn set_map16_value(&mut self, value: ScreenMapEntry) {
        let (entry, quadrant) = (self.map16_entry, self.quadrant);
        if let Some(slot) = self.tileset_mut().and_then(|t| t.map16.get_mut(entry)).and_then(|m| m.tiles.get_mut(quadrant)) {
            *slot = value;
        }
    }

    /// Runs `edit` on the chosen tileset's Map16 blocks as a sheet of quadrants, `width` blocks across.
    pub fn with_map16<R>(&mut self, width: usize, edit: impl FnOnce(&mut dyn Grid<Cell = ScreenMapEntry>) -> R) -> Option<R> {
        let tileset = self.tileset_mut()?;
        Some(edit(&mut Map16Sheet { blocks: &mut tileset.map16, width_blocks: width }))
    }

    /// Runs `edit` on a tilemap's entries.
    pub fn with_tilemap<R>(&mut self, which: usize, edit: impl FnOnce(&mut dyn Grid<Cell = ScreenMapEntry>) -> R) -> Option<R> {
        self.touched();
        let map = self.tilemaps.get_mut(which)?;
        let width = map.width_tiles;
        Some(edit(&mut Screen { entries: &mut map.screen, width }))
    }

    /// Runs `edit` on `sheet`'s graphics as a sheet of pixels, as the Graphics tab draws them.
    pub fn with_sheet<R>(&mut self, sheet: Sheet, edit: impl FnOnce(&mut dyn Grid<Cell = u8>) -> R) -> Option<R> {
        let (tiles, _) = self.sheet_parts_mut(sheet)?;
        let width_tiles = GRAPHICS_COLUMNS.min(tiles.len().max(1));
        Some(edit(&mut TileSheet { tiles, width_tiles }))
    }

    /// Sets colour `index` of `sheet`'s palettes (counted across all of them), rounded to what the DS stores.
    pub fn set_sheet_colour(&mut self, sheet: Sheet, index: usize, rgb: [u8; 3]) {
        let changed = rounded(rgb);
        if self.sheet_parts(sheet).and_then(|(_, p)| p.get(index)).is_some_and(|c| Bgr555Color { transparent: false, ..*c } == changed) {
            return;
        }
        self.edit("Change colour", |doc| {
            if let Some(colour) = doc.sheet_parts_mut(sheet).and_then(|(_, p)| p.get_mut(index)) {
                *colour = Bgr555Color { transparent: colour.transparent, ..changed };
            }
        });
    }

    /// The level's tile memory with every tileset at its offsets, as the Map16 tab's picker shows it, drawn in
    /// palette block `palette` (counted from this tileset's own), `GRAPHICS_COLUMNS` tiles across.
    pub fn composed_tiles(&self, palette: i16) -> Result<(RgbaImage, usize), String> {
        let tileset = self.tileset().ok_or("no tileset")?;
        let memory = compose_tileset_memory(&self.tilesets);
        let count = memory.graphics.len();
        let block = palette + i16::from(tileset.palette_offset);
        let mut entries: Vec<ScreenMapEntry> = (0..count).map(|i| ScreenMapEntry::new(i as i32, block, false, false)).collect();
        entries.resize(count.div_ceil(GRAPHICS_COLUMNS).max(1) * GRAPHICS_COLUMNS, ScreenMapEntry::new(-1, 0, false, false));
        let image = render_screen_map(&entries, GRAPHICS_COLUMNS, &memory.graphics, &memory.palette).map_err(|e| e.message)?;
        Ok((image, count))
    }

    /// The palette blocks a Map16 entry can name, counted from this tileset's own.
    pub fn map16_palettes(&self) -> usize {
        let memory = compose_tileset_memory(&self.tilesets);
        let offset = self.tileset().map_or(0, |t| usize::from(t.palette_offset));
        (memory.palette.len() / 256).saturating_sub(offset).max(1)
    }

    /// A tilemap's own tiles, `GRAPHICS_COLUMNS` across in palette block `palette`, for its picker.
    pub fn tilemap_tiles(&self, which: usize, palette: i16) -> Result<(RgbaImage, usize), String> {
        let map = self.tilemaps.get(which).ok_or("no tilemap")?;
        let count = map.graphics.len();
        let mut entries: Vec<ScreenMapEntry> = (0..count).map(|i| ScreenMapEntry::new(i as i32, palette, false, false)).collect();
        entries.resize(count.div_ceil(GRAPHICS_COLUMNS).max(1) * GRAPHICS_COLUMNS, ScreenMapEntry::new(-1, 0, false, false));
        let image = render_screen_map(&entries, GRAPHICS_COLUMNS, &map.graphics, &map.palette).map_err(|e| e.message)?;
        Ok((image, count))
    }

    // --- Objects ---

    pub fn objects(&self) -> &[ObjectLayout] {
        self.tileset().map_or(&[], |t| t.objects.as_slice())
    }

    /// The block of the chosen object that table row `row` is in, and the row's place in it. A slope's table goes
    /// on past its steps into its two side blocks.
    fn block_mut(&mut self, row: usize) -> Option<(&mut Vec<Vec<ObjectLayoutTile>>, usize)> {
        let object = self.object;
        let layout = self.tileset_mut().and_then(|t| t.objects.get_mut(object))?;
        let mut row = row;
        if layout.slope {
            for block in [&mut layout.rows, &mut layout.slope_behind] {
                if row < block.len() {
                    return Some((block, row));
                }
                row -= block.len();
            }
            return Some((&mut layout.slope_fill, row));
        }
        Some((&mut layout.rows, row))
    }

    fn cell_mut(&mut self, row: usize, column: usize) -> Option<&mut ObjectLayoutTile> {
        let (block, row) = self.block_mut(row)?;
        block.get_mut(row)?.get_mut(column)
    }

    /// The chosen object's rows as its table shows them, each block's first row named when the object is a slope.
    pub fn object_table(&self) -> Vec<(&'static str, Vec<ObjectLayoutTile>)> {
        let Some(layout) = self.objects().get(self.object) else { return Vec::new() };
        let blocks: &[(&'static str, &[Vec<ObjectLayoutTile>])] = if layout.slope {
            &[("Step", &layout.rows), ("Behind", &layout.slope_behind), ("Fill", &layout.slope_fill)]
        } else {
            &[("", &layout.rows)]
        };
        let named = |&(name, rows): &(&'static str, &[Vec<ObjectLayoutTile>])| {
            rows.iter().enumerate().map(move |(i, row)| (if i == 0 { name } else { "" }, row.clone())).collect::<Vec<_>>()
        };
        blocks.iter().flat_map(named).collect()
    }

    pub fn add_object(&mut self) {
        if let Some(tileset) = self.tileset_mut() {
            tileset.objects.push(ObjectLayout { rows: vec![vec![ObjectLayoutTile::new(-1, 0)]], ..ObjectLayout::default() });
            self.object = tileset.objects.len() - 1;
            self.object_cell = None;
        }
    }

    /// Deletes the chosen object; a tileset keeps at least one.
    pub fn delete_object(&mut self) {
        let object = self.object;
        if let Some(tileset) = self.tileset_mut()
            && tileset.objects.len() > 1
            && object < tileset.objects.len()
        {
            tileset.objects.remove(object);
            self.object = object.min(tileset.objects.len() - 1);
            self.object_cell = None;
        }
    }

    /// Removes the chosen tile; an emptied row goes, and an emptied object keeps one empty tile. A slope's side
    /// blocks may be emptied altogether.
    pub fn remove_object_tile(&mut self) {
        let Some((row, column)) = self.object_cell else { return };
        if let Some((rows, row)) = self.block_mut(row)
            && let Some(cells) = rows.get_mut(row)
            && column < cells.len()
        {
            cells.remove(column);
            if cells.is_empty() {
                rows.remove(row);
            }
        }
        if let Some((rows, _)) = self.block_mut(0)
            && rows.is_empty()
        {
            rows.push(vec![ObjectLayoutTile::new(-1, 0)]);
        }
        self.object_cell = None;
    }

    /// Puts the picked Map16 blocks into the chosen object from the chosen cell: over the cells there, or (with
    /// `insert`) before them. A pick several rows tall carries on into the rows below, which are added as needed.
    /// The cell after the last one placed is chosen next, so clicking block after block builds a row.
    pub fn place_blocks(&mut self, blocks: &Pattern<Option<usize>>, insert: bool) {
        let (row, column) = self.object_cell.unwrap_or((0, 0));
        for y in 0..blocks.height {
            let Some((rows, at)) = self.block_mut(row + y) else { return };
            while rows.len() <= at {
                rows.push(Vec::new());
            }
            let Some(cells) = rows.get_mut(at) else { return };
            for x in 0..blocks.width {
                let tile = blocks.get(x, y).flatten().map_or(OBJECT_LAYOUT_EMPTY_TILE, |b| b as i32);
                let at = (column + x).min(cells.len());
                match cells.get_mut(at) {
                    Some(cell) if !insert => cell.tile = tile,
                    _ => cells.insert(at, ObjectLayoutTile::new(tile, 0)),
                }
            }
        }
        self.object_cell = Some((row + blocks.height.saturating_sub(1), column + blocks.width));
    }

    pub fn delete_object_row(&mut self, row: usize) {
        if let Some((rows, at)) = self.block_mut(row)
            && at < rows.len()
        {
            rows.remove(at);
        }
        if let Some((rows, _)) = self.block_mut(0)
            && rows.is_empty()
        {
            rows.push(vec![ObjectLayoutTile::new(-1, 0)]);
        }
        self.object_cell = None;
    }

    /// Inserts an empty row after table row `row`, in the same block.
    pub fn insert_object_row(&mut self, row: usize) {
        if let Some((rows, at)) = self.block_mut(row) {
            rows.insert((at + 1).min(rows.len()), vec![ObjectLayoutTile::new(-1, 0)]);
            self.object_cell = Some((row + 1, 0));
        }
    }

    pub fn duplicate_object(&mut self) {
        let Some(layout) = self.objects().get(self.object).cloned() else { return };
        let at = self.object + 1;
        if let Some(tileset) = self.tileset_mut() {
            tileset.objects.insert(at.min(tileset.objects.len()), layout);
            self.object = at;
            self.object_cell = None;
        }
    }

    /// Puts `layout` after the chosen object, as a paste does.
    pub fn paste_object(&mut self, layout: ObjectLayout) {
        let at = self.object + 1;
        if let Some(tileset) = self.tileset_mut() {
            tileset.objects.insert(at.min(tileset.objects.len()), layout);
            self.object = at.min(tileset.objects.len() - 1);
            self.object_cell = None;
        }
    }

    /// Moves the chosen object one place up (`-1`) or down (`1`); objects are placed in levels by number, so this
    /// renumbers them.
    pub fn move_object(&mut self, by: isize) {
        let from = self.object;
        let Some(to) = from.checked_add_signed(by) else { return };
        if let Some(tileset) = self.tileset_mut()
            && to < tileset.objects.len()
        {
            tileset.objects.swap(from, to);
            self.object = to;
        }
    }

    /// The chosen object, to change its slope settings.
    pub fn object_mut(&mut self) -> Option<&mut ObjectLayout> {
        let object = self.object;
        self.tileset_mut()?.objects.get_mut(object)
    }

    pub fn set_object_control(&mut self, control: u8) {
        let Some((row, column)) = self.object_cell else { return };
        if let Some(cell) = self.cell_mut(row, column) {
            cell.control = control;
        }
    }

    // --- Behaviours ---

    pub fn behavior(&self) -> u32 {
        self.tileset().and_then(|t| t.behaviors.get(self.behavior_entry)).copied().unwrap_or(0)
    }

    /// Sets the chosen entry's behaviour, giving the tileset a behaviour table first if it had none.
    pub fn set_behavior(&mut self, value: u32) {
        let entry = self.behavior_entry;
        if let Some(tileset) = self.tileset_mut() {
            if tileset.map16.is_empty() {
                return;
            }
            if tileset.behaviors.is_empty() {
                tileset.behaviors.resize(tileset.map16.len(), 0);
            }
            if let Some(slot) = tileset.behaviors.get_mut(entry) {
                *slot = value;
            }
        }
    }

    /// Sets every block in `entries` to `value`, as an edit with several blocks chosen does.
    pub fn set_behaviors(&mut self, entries: &[usize], value: u32) {
        let chosen = self.behavior_entry;
        for &entry in entries {
            self.behavior_entry = entry;
            self.set_behavior(value);
        }
        self.behavior_entry = chosen;
    }

    /// What the behaviour's parameter means, for its kind.
    pub fn behavior_meaning(parts: TileBehaviorParts) -> String {
        let kind = tile_behavior_parameter_kind(parts);
        let name = tile_behavior_parameter_name(kind, parts.parameter);
        let mut meaning = if name.is_empty() { format!("Unknown {}", parts.parameter) } else { name.to_owned() };
        if kind == TileBehaviorParameterKind::PipeDoor
            && let Some(main) = TILE_BEHAVIOR_PIPE_MAIN_NAMES.get(usize::from(parts.parameter >> 5))
        {
            meaning = format!("{main} — {meaning}");
        }
        meaning
    }

    // --- Backgrounds ---

    pub fn map_value(&self, which: usize) -> Option<ScreenMapEntry> {
        let entry = *self.map_entries.get(which)?;
        self.tilemaps.get(which)?.screen.get(entry).copied()
    }

    pub fn set_map_value(&mut self, which: usize, value: ScreenMapEntry) {
        let Some(&entry) = self.map_entries.get(which) else { return };
        if let Some(slot) = self.tilemaps.get_mut(which).and_then(|m| m.screen.get_mut(entry)) {
            *slot = value;
            self.touched();
        }
    }

    // --- Import and export ---

    pub fn import_tilemap(&mut self, which: usize, image: &RgbaImage) -> Result<(), String> {
        self.touched();
        let map = self.tilemaps.get_mut(which).ok_or("no tilemap")?;
        import_tilemap_image(map, image).map_err(|e| e.message)
    }

    /// The chosen tileset as a `.bstileset` bundle.
    pub fn export_bundle(&self) -> Result<Vec<u8>, String> {
        let tileset = self.tileset().ok_or("no tileset")?;
        let data = write_tileset(tileset).map_err(|e| e.message)?;
        write_tileset_bundle(&data).map_err(|e| e.message)
    }

    /// Replaces the chosen tileset with a bundle's, read at this slot's offsets.
    pub fn import_bundle(&mut self, bytes: &[u8]) -> Result<(), String> {
        let data = parse_tileset_bundle(bytes).map_err(|e| e.message)?;
        let tileset = self.tileset_mut().ok_or("no tileset")?;
        let value = parse_tileset(&data, tileset.tile_offset, tileset.palette_offset, tileset.tile_page_offset).map_err(|e| e.message)?;
        *tileset = value;
        self.map16_entry = 0;
        self.object = 0;
        self.object_cell = None;
        self.behavior_entry = 0;
        Ok(())
    }

    // --- Saving ---

    pub fn tileset_data(&self, slot: usize) -> Result<TilesetResourceData, String> {
        let tileset = self.tilesets.get(slot).ok_or("tileset slot is outside the editor")?;
        write_tileset(tileset).map_err(|e| e.message)
    }

    pub fn tilemap_data(&self, which: usize) -> Result<TilemapResourceData, String> {
        let map = self.tilemaps.get(which).ok_or("no tilemap")?;
        write_tilemap(map).map_err(|e| e.message)
    }

    /// Every file whose bytes the edits changed, with the bytes to write: only changed resources are written.
    pub fn writes(&self) -> Result<Vec<FileWrite>, String> {
        let mut writes = Vec::new();
        let mut queue = |file: &ResourceBytes, after: Vec<u8>| -> Result<(), String> {
            if file.bytes == after {
                return Ok(());
            }
            if file.rom_path.is_empty() {
                return Err("edited resource has no ROM file target".to_owned());
            }
            writes.push(FileWrite { path: file.rom_path.clone(), bytes: after });
            Ok(())
        };
        for (slot, source) in self.sources.iter().enumerate() {
            let after = self.tileset_data(slot)?;
            queue(&source.graphics, after.graphics)?;
            queue(&source.palette, after.palette)?;
            queue(&source.map16, after.map16)?;
            queue(&source.objects, after.objects)?;
            queue(&source.object_index, after.object_index)?;
            queue(&source.behaviors, after.behaviors)?;
        }
        for (which, source) in self.map_sources.iter().enumerate() {
            let after = self.tilemap_data(which)?;
            queue(&source.graphics, after.graphics)?;
            queue(&source.palette, after.palette)?;
            queue(&source.screen, after.screen)?;
        }
        Ok(writes)
    }
}

/// What the person asked the workspace to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilesetAction {
    Save,
    Close,
}

/// The tabs' tools, pickers and drawn sheets, each sheet with the document version it shows.
#[derive(Default)]
pub struct TilesetView {
    textures: BTreeMap<&'static str, (u64, egui::TextureHandle)>,
    /// Sheets drawn for a version, so they are not drawn again every frame.
    images: Sheets,
    notice: Option<String>,
    pixels: PixelEditor,
    map16: TilemapEditor,
    map16_tiles: PickerState,
    maps: [TilemapEditor; 2],
    map_tiles: [PickerState; 2],
    /// The brush has been made from each picker's selection once.
    brushes: [bool; 3],
    object_blocks: PickerState,
    behavior_blocks: PickerState,
    /// The object table's zoom, and the object preview's size in blocks.
    object_zoom: f32,
    preview: (usize, usize),
    object_clipboard: Option<ObjectLayout>,
    /// A whole object table row is chosen rather than one of its cells.
    row_chosen: bool,
    behavior_search: String,
    /// The Graphics tab's bitmap, and its choice of bitmap and palette.
    sheet: Sheet,
    graphics: ManagerState,
}

/// Drawn sheets by name, each with the version it was drawn for and how many cells it has.
type Sheets = BTreeMap<&'static str, (u64, Result<(Arc<RgbaImage>, usize), String>)>;

/// A sheet drawn once per `version`, under `key`.
fn cached(
    images: &mut Sheets,
    key: &'static str,
    version: u64,
    draw: impl FnOnce() -> Result<(Arc<RgbaImage>, usize), String>,
) -> Result<(Arc<RgbaImage>, usize), String> {
    let entry = images.entry(key).or_insert_with(|| (u64::MAX, Err(String::new())));
    if entry.0 != version {
        *entry = (version, draw());
    }
    entry.1.clone()
}

pub fn show(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, services: &dyn Services) -> Option<TilesetAction> {
    let mut action = None;
    // A header line as every document has, the title cut short before it reaches Save and Cancel; the tools under it.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if Button::new("Cancel").show(ui).clicked() {
                action = Some(TilesetAction::Close);
            }
            if Button::new("Save")
                .with_icon(icons::SAVE)
                .primary()
                .show(ui)
                .on_hover_text("Writes the files the edits changed into their modules")
                .clicked()
            {
                action = Some(TilesetAction::Save);
            }
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                controls::one_line(ui, RichText::new(format!("Tilesets and backgrounds — {}", doc.title)).strong(), 0.0);
            });
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.label("Tileset slot");
        let mut slot = doc.slot;
        if let Some(picked) = controls::numbered_choice("tileset-slot", &SLOT_NAMES).show(ui, i64::try_from(slot).ok()) {
            slot = usize::from(picked);
        }
        if slot != doc.slot {
            doc.set_slot(slot);
            view.brushes = [false; 3];
        }
        match history_buttons(ui, doc.can_undo(), doc.can_redo()) {
            Some(HistoryButton::Undo) => doc.undo(),
            Some(HistoryButton::Redo) => doc.redo(),
            None => {}
        }
        if Button::new("Import tileset…").show(ui).clicked() {
            match services.open_file("Import tileset", &["bstileset"]) {
                Some(Ok((_, bytes))) => view.notice = doc.edit("Import tileset", |doc| doc.import_bundle(&bytes)).err(),
                Some(Err(error)) => view.notice = Some(error),
                None => {}
            }
        }
        if Button::new("Export tileset…").show(ui).clicked() {
            let written = doc.export_bundle().and_then(|bytes| services.save_file("Export tileset", "tileset.bstileset", "bstileset", &bytes));
            view.notice = written.err();
        }
    });
    controls::tabs(ui, "tileset", &mut doc.tab, &TABS);
    controls::notice_line(ui, &mut view.notice, true);
    ui.add_space(2.0);
    match doc.tab {
        Tab::Slots => slots(ui, doc),
        Tab::Graphics => graphics(ui, doc, view, services),
        Tab::Map16 => map16(ui, doc, view),
        Tab::Objects => objects(ui, doc, view),
        Tab::Behaviors => behaviors(ui, doc, view),
        Tab::Background => tilemap(ui, doc, view, services, 0),
        Tab::Foreground => tilemap(ui, doc, view, services, 1),
    }
    action
}

fn slots(ui: &mut Ui, doc: &mut TilesetDoc) {
    let before = doc.references;
    egui::Grid::new("tileset-references").num_columns(2).show(ui, |ui| {
        let references = &mut doc.references;
        for (label, value) in [
            ("Background graphics", &mut references.background_graphics_id),
            ("Level tileset", &mut references.tileset_id),
            ("Foreground graphics", &mut references.foreground_graphics_id),
            ("Background screen", &mut references.background_screen_id),
            ("Background palette", &mut references.background_palette_id),
            ("Level tileset palette", &mut references.tileset_palette_id),
            ("Foreground screen", &mut references.foreground_screen_id),
            ("Foreground palette", &mut references.foreground_palette_id),
        ] {
            ui.label(label);
            if label.starts_with("Level tileset") {
                tileset_choice(ui, label, value);
            } else {
                ui.add(controls::number(value).range(0..=255));
            }
            ui.end_row();
        }
    });
    crate::properties::follow_tileset(&mut doc.references, before);
    ui.add(
        egui::Label::new(
            "These IDs choose the level's graphics. Edits below change the graphics loaded now; a changed ID takes effect once the level is saved and opened again.",
        )
        .wrap(),
    );
}

/// The level tileset, picked by name. A number the game's tables have no room for stays as it is until another is
/// picked.
fn tileset_choice(ui: &mut Ui, id: &str, value: &mut u16) {
    let choices: Vec<(u16, &str)> = (0u16..).zip(TILESET_NAMES).collect();
    if let Some(picked) = controls::numbered_choice(("tileset-choice", id), &choices).width(260.0).show(ui, Some(i64::from(*value))) {
        *value = picked;
    }
}

fn entry_fields(
    ui: &mut Ui,
    value: ScreenMapEntry,
    tiles: std::ops::RangeInclusive<i32>,
    palettes: std::ops::RangeInclusive<i16>,
) -> Option<ScreenMapEntry> {
    let mut edited = value;
    egui::Grid::new(ui.next_auto_id()).num_columns(2).show(ui, |ui| {
        ui.label("Tile");
        ui.add(controls::number(&mut edited.tile).range(tiles));
        ui.end_row();
        ui.label("Palette");
        ui.add(controls::number(&mut edited.palette).range(palettes));
        ui.end_row();
        ui.label("Horizontal flip");
        controls::checkbox(ui, &mut edited.horizontal_flip, "");
        ui.end_row();
        ui.label("Vertical flip");
        controls::checkbox(ui, &mut edited.vertical_flip, "");
        ui.end_row();
    });
    (edited != value).then_some(edited)
}

/// Uploads `image` under `key` when `version` moves on, and gives the texture to draw blocks from.
fn texture(
    ui: &Ui,
    textures: &mut BTreeMap<&'static str, (u64, egui::TextureHandle)>,
    key: &'static str,
    image: &RgbaImage,
    version: u64,
) -> egui::TextureId {
    let mut slot = textures.remove(key);
    upload(ui, key, image, version, &mut slot);
    let id = slot.as_ref().map_or(egui::TextureId::default(), |(_, handle)| handle.id());
    if let Some(slot) = slot {
        textures.insert(key, slot);
    }
    id
}

/// Draws Map16 block `block` from the Map16 sheet's texture into `rect`.
fn draw_block(painter: &egui::Painter, texture: egui::TextureId, sheet: &RgbaImage, rect: Rect, block: usize) {
    let (width, height) = (sheet.width.max(1) as f32, sheet.height.max(1) as f32);
    let (column, row) = ((block % MAP16_COLUMNS) as f32, (block / MAP16_COLUMNS) as f32);
    let uv = Rect::from_min_size(Pos2::new(column * 16.0 / width, row * 16.0 / height), Vec2::new(16.0 / width, 16.0 / height));
    painter.image(texture, rect, uv, Color32::WHITE);
}

/// The Map16 sheet as the Map16, Objects and Behaviors tabs draw it, and how many blocks it has.
fn map16_sheet(doc: &TilesetDoc, view: &mut TilesetView) -> (u64, Result<(Arc<RgbaImage>, usize), String>) {
    let version = doc.version * 4 + u64::from(doc.composed) * 2 + u64::from(doc.solid3d);
    let count = doc.map16_count();
    (version, cached(&mut view.images, "map16", version, || doc.map16_sheet().map(|image| (Arc::new(image), count))))
}

/// Whether the canvas's gesture or the commands edit, and so open an undo step; and whether they end one.
fn edits(input: &CanvasInput, commands: &[Command]) -> (bool, bool) {
    let command = commands.iter().any(Command::edits);
    (input.pressed().is_some() || command, input.released().is_some() || command)
}

/// One of the Graphics tab's bitmaps and its palettes, for the image manager's imports and exports.
struct SheetImages<'a> {
    doc: &'a mut TilesetDoc,
    sheet: Sheet,
}

impl Images for SheetImages<'_> {
    fn bitmap(&self, _: usize) -> Option<Bitmap> {
        let (tiles, _) = self.doc.sheet_parts(self.sheet)?;
        let mut tiles = tiles.to_vec();
        Some(Bitmap::read(&TileSheet { width_tiles: GRAPHICS_COLUMNS.min(tiles.len().max(1)), tiles: &mut tiles }))
    }

    fn palette(&self, index: usize) -> Option<image_manager::Palette> {
        self.doc.sheet_palette(self.sheet, index)
    }

    fn colours(&self) -> usize {
        256
    }

    fn set_bitmap(&mut self, _: usize, pixels: Vec<u8>) -> Result<(), String> {
        let read = self.bitmap(0).ok_or("no graphics")?;
        self.doc.with_sheet(self.sheet, |grid| Bitmap { pixels, ..read }.write(grid));
        Ok(())
    }

    fn set_palette(&mut self, index: usize, colours: &[[u8; 3]]) {
        let Some((_, palette)) = self.doc.sheet_parts_mut(self.sheet) else { return };
        for (i, &rgb) in colours.iter().enumerate().skip(1).take(255) {
            if let Some(colour) = palette.get_mut(index * 256 + i) {
                *colour = Bgr555Color { transparent: colour.transparent, ..rounded(rgb) };
            }
        }
    }
}

fn graphics(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, services: &dyn Services) {
    let sheets = doc.sheets();
    // The chosen slot's tileset is the one listed as chosen, so the slot above and the list agree.
    let shown = doc.resolve(view.sheet);
    view.graphics.bitmap = sheets.iter().position(|(s, _)| *s == shown).unwrap_or(0);
    let palettes = doc.sheet_palettes(shown);
    let palette_names: Vec<String> = (0..palettes).map(|i| format!("Palette {i}")).collect();
    controls::side_panel(ui, egui::Panel::left(doc.panel_id("tileset-graphics-images")), 240.0).show(ui, |ui| {
        // Scrolls sideways too: what does not fit a narrow panel is scrolled to, not drawn over the canvas beside it.
        egui::ScrollArea::both().show(ui, |ui| {
            let names: Vec<String> = sheets.iter().map(|(_, name)| name.clone()).collect();
            let lists = Lists { bitmaps: &names, palettes: &palette_names, groups: &[], shared: false, no_import: None };
            let (_, action) = image_manager::show(ui, "tileset-graphics", &mut view.graphics, &lists);
            if let Some(&(chosen, _)) = sheets.get(view.graphics.bitmap)
                && chosen != shown
            {
                view.sheet = chosen;
                if let Sheet::Tileset(slot) = chosen {
                    // A tileset is chosen by choosing its slot, which the other tabs follow.
                    view.sheet = Sheet::Current;
                    doc.set_slot(slot);
                    view.brushes = [false; 3];
                }
            }
            if let Some(action) = action {
                let sheet = doc.resolve(view.sheet);
                let name = sheets.iter().find(|(s, _)| *s == sheet).map_or("graphics".to_owned(), |(_, n)| n.clone());
                let palettes: Vec<usize> = (0..doc.sheet_palettes(sheet)).collect();
                doc.begin_edit("Import graphics");
                let result = image_manager::run(action, &view.graphics, &mut SheetImages { doc, sheet }, &[0], &palettes, services, &name);
                doc.end_edit();
                view.notice = result.err();
            }
        });
    });
    let sheet = doc.resolve(view.sheet);
    let palette = view.graphics.palette.min(doc.sheet_palettes(sheet).saturating_sub(1));
    let colours: Vec<[u8; 4]> = doc.sheet_palette(sheet, palette).unwrap_or_default().iter().map(|&[r, g, b]| [r, g, b, 255]).collect();
    controls::side_panel(ui, egui::Panel::right(doc.panel_id("tileset-graphics-palette")), 300.0).show(ui, |ui| {
        ui.strong(format!("Palette {palette}"));
        let edited = egui::ScrollArea::both().show(ui, |ui| palette_panel::show(ui, &colours, &mut view.pixels.colours, 256, true)).inner;
        if let Some((index, rgb)) = edited {
            doc.set_sheet_colour(sheet, palette * 256 + index, rgb);
        }
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let mut commands = Vec::new();
        view.pixels.toolbar(ui, None, &mut commands);
        // Drawn again when the graphics change, or another bitmap or palette is chosen.
        let code = match sheet {
            Sheet::Current => 0,
            Sheet::Tileset(i) => i,
            Sheet::Layer(i) => 4 + i,
        };
        let key = doc.version * 64 + code as u64 * 8 + palette.min(7) as u64;
        let image = match cached(&mut view.images, "graphics", key, || doc.sheet_image(sheet, palette).map(|image| (Arc::new(image), 0))) {
            Ok((image, _)) => image,
            Err(error) => {
                controls::error_text(ui, error);
                return;
            }
        };
        let input = view.pixels.canvas(ui, "tileset-graphics", &image, key, ui.available_height() - zoom_canvas::below(ui));
        view.pixels.keys(ui, &input, &mut commands);
        ui.weak(pixel_editor::position_text(view.pixels.hover(), &image));
        let (begins, ends) = edits(&input, &commands);
        if begins {
            doc.begin_edit(view.pixels.step_name(&commands));
        }
        if input.gesture.is_some() || !commands.is_empty() {
            let ctx = ui.ctx().clone();
            doc.with_sheet(sheet, |grid| view.pixels.apply(&ctx, &input, &commands, grid, 256));
        }
        if ends {
            doc.end_edit();
        }
    });
}

/// The brush a picker's selection makes: tile numbers from `first`, which is where the picker's tile 0 is in the
/// layer's own numbering.
fn brush_from(picker: &PickerState, count: usize, first: i32) -> Pattern<ScreenMapEntry> {
    let picked = picker.indices(GRAPHICS_COLUMNS, count);
    let cells = picked.cells.iter().map(|i| ScreenMapEntry::new(i.map_or(-1, |i| i as i32 - first), 0, false, false)).collect();
    Pattern { width: picked.width, height: picked.height, cells }
}

fn map16(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    view.map16.snap = 2;
    let palettes = doc.map16_palettes();
    view.map16.palette = view.map16.palette.clamp(0, palettes as i16 - 1);
    let (version, sheet) = map16_sheet(doc, view);
    controls::side_panel(ui, egui::Panel::right(doc.panel_id("tileset-map16-side")), 340.0).show(ui, |ui| {
        ui.strong("Tiles");
        ui.weak("Drag to choose the tiles to paint with. All three tilesets are here, at the place the level loads them.");
        let palette = view.map16.palette;
        let key = doc.version * 32 + palette.max(0) as u64;
        let first = doc.tileset().map_or(0, |t| i32::from(t.tile_offset));
        match cached(&mut view.images, "map16-tiles", key, || doc.composed_tiles(palette).map(|(image, count)| (Arc::new(image), count))) {
            Ok((tiles, count)) => {
                let picked =
                    tile_picker::show(ui, "map16-tiles", &tiles, key, &mut view.map16_tiles, 8, GRAPHICS_COLUMNS, ui.available_height() * 0.55);
                // The picker opens on this tileset's own first tile, not the first slot's.
                let opening = !view.brushes[0];
                if opening {
                    view.brushes[0] = true;
                    view.map16_tiles.select(usize::try_from(first).unwrap_or(0), GRAPHICS_COLUMNS);
                }
                if opening || picked.changed {
                    view.map16.set_brush(brush_from(&view.map16_tiles, count, first));
                }
            }
            Err(error) => {
                controls::error_text(ui, error);
            }
        }
        ui.separator();
        if let Ok((image, _)) = &sheet {
            egui::ScrollArea::vertical().id_salt("map16-inspector").show(ui, |ui| block_inspector(ui, doc, view, image, version));
        }
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let mut commands = Vec::new();
        view.map16.toolbar(ui, None, palettes, &mut commands);
        ui.horizontal_wrapped(|ui| {
            controls::checkbox(ui, &mut doc.composed, "Show the other tilesets’ graphics").on_hover_text(
                "A Map16 entry can use tiles from another tileset. When checked, those tiles are drawn the way the game loads them; when unchecked, only this tileset's own tiles are shown.",
            );
            controls::checkbox(ui, &mut doc.solid3d, "Show 3D tiles").on_hover_text(
                "The game draws some blocks with its 3D engine instead of from their Map16. When checked, they show the texture the game shows; when unchecked, they show what the file holds, which is what you want when editing them.",
            );
        });
        let image = match &sheet {
            Ok((image, _)) => image.clone(),
            Err(error) => {
                controls::error_text(ui, error);
                return;
            }
        };
        let input = view.map16.canvas(ui, "map16", &image, version, ui.available_height() - zoom_canvas::below(ui));
        view.map16.keys(ui, &input, &mut commands);
        let hovered = view.map16.hover().and_then(|(x, y)| {
            let (block, quadrant) = Map16Sheet::locate(MAP16_COLUMNS, usize::try_from(x).ok()?, usize::try_from(y).ok()?);
            if x >= (MAP16_COLUMNS * 2) as i64 {
                return None;
            }
            let entry = doc.tileset()?.map16.get(block)?.tiles.get(quadrant).copied()?;
            Some((block, entry))
        });
        ui.weak(match hovered {
            Some((block, entry)) => format!("Block {block} (0x{block:03x}) · {}", entry_text(view.map16.hover(), Some(entry))),
            None => String::new(),
        });
        let (begins, ends) = edits(&input, &commands);
        if begins {
            doc.begin_edit(view.map16.step_name(&commands));
        }
        if input.gesture.is_some() || !commands.is_empty() {
            let ctx = ui.ctx().clone();
            let outcome = doc.with_map16(MAP16_COLUMNS, |grid| view.map16.apply(&ctx, &input, &commands, grid)).unwrap_or_default();
            if let Some((x, y)) = outcome.clicked {
                let (block, quadrant) = Map16Sheet::locate(MAP16_COLUMNS, x, y);
                if block < doc.map16_count() {
                    doc.map16_entry = block;
                    doc.quadrant = quadrant;
                }
            }
        }
        if ends {
            doc.end_edit();
        }
    });
}

/// The chosen block, large: a click on a quadrant paints the brush's first tile there, a right-click picks it.
/// Below it, the chosen quadrant as numbers and the block's behaviour.
fn block_inspector(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, sheet: &RgbaImage, version: u64) {
    let entry = doc.map16_entry;
    ui.strong(format!("Block {entry} (0x{entry:03x})"));
    let id = texture(ui, &mut view.textures, "map16-block", sheet, version);
    let size = 128.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    draw_block(ui.painter(), id, sheet, rect, entry);
    for quadrant in 0..4 {
        let at = Rect::from_min_size(rect.min + Vec2::new((quadrant % 2) as f32, (quadrant / 2) as f32) * size / 2.0, Vec2::splat(size / 2.0));
        let response = ui.interact(at, ui.id().with(("quadrant", quadrant)), egui::Sense::click());
        let chosen = doc.quadrant == quadrant;
        let stroke = if chosen { Stroke::new(2.0, zoom_canvas::SELECTION) } else { Stroke::new(1.0, ui.visuals().weak_text_color()) };
        ui.painter().rect_stroke(at, 0.0, stroke, StrokeKind::Inside);
        let response = response.on_hover_text(format!(
            "{}: click paints the brush's tile here, right-click picks this tile",
            QUADRANTS.get(quadrant).copied().unwrap_or_default()
        ));
        if response.clicked() {
            doc.quadrant = quadrant;
            if let Some(&tile) = view.map16.effective_brush().cells.first() {
                doc.edit("Draw tiles", |doc| doc.set_map16_value(tile));
            }
        }
        if response.secondary_clicked() {
            doc.quadrant = quadrant;
            if let Some(value) = doc.map16_value() {
                view.map16.brush = Pattern::single(value);
                view.map16.picked = true;
            }
        }
    }
    ui.label(QUADRANTS.get(doc.quadrant).copied().unwrap_or_default());
    if let Some(value) = doc.map16_value()
        && let Some(edited) = entry_fields(ui, value, -1023..=1023, -15..=15)
    {
        doc.edit("Edit quadrant", |doc| doc.set_map16_value(edited));
    }
    let behavior = doc.tileset().and_then(|t| t.behaviors.get(entry)).copied().unwrap_or(0);
    let parts = decode_tile_behavior(behavior);
    let flags: Vec<&str> = TILE_BEHAVIOR_FLAG_NAMES.iter().enumerate().filter(|(i, _)| parts.flags & (1 << i) != 0).map(|(_, name)| *name).collect();
    ui.horizontal_wrapped(|ui| {
        ui.label("Behaviour");
        ui.weak(if flags.is_empty() { "none".to_owned() } else { flags.join(", ") });
        if Button::new("Edit…").small().show(ui).clicked() {
            doc.tab = Tab::Behaviors;
            doc.behavior_entry = entry;
            view.behavior_blocks.select(entry, MAP16_COLUMNS);
        }
    });
}

fn objects(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    if view.object_zoom == 0.0 {
        view.object_zoom = 2.0;
        view.preview = (4, 3);
    }
    let (version, sheet) = map16_sheet(doc, view);
    let (sheet, count) = match sheet {
        Ok(sheet) => sheet,
        Err(error) => {
            controls::error_text(ui, error);
            return;
        }
    };
    let id = texture(ui, &mut view.textures, "objects-map16", &sheet, version);

    controls::side_panel(ui, egui::Panel::left(doc.panel_id("tileset-object-list")), 200.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            if Button::new("Add").small().show(ui).clicked() {
                doc.edit("Add object", TilesetDoc::add_object);
            }
            if Button::new("Duplicate").small().show(ui).clicked() {
                doc.edit("Duplicate object", TilesetDoc::duplicate_object);
            }
            if Button::new("Delete").small().show(ui).clicked() {
                doc.edit("Delete object", TilesetDoc::delete_object);
            }
        });
        ui.horizontal(|ui| {
            if Button::new("Copy").small().show(ui).clicked() {
                view.object_clipboard = doc.objects().get(doc.object).cloned();
            }
            if Button::new("Paste").small().enabled(view.object_clipboard.is_some()).show(ui).clicked()
                && let Some(layout) = view.object_clipboard.clone()
            {
                doc.edit("Paste object", |doc| doc.paste_object(layout));
            }
            let renumbers = "Objects are placed in levels by number, so this renumbers them";
            if Button::new("Up").small().show(ui).on_hover_text(renumbers).clicked() {
                doc.edit("Move object", |doc| doc.move_object(-1));
            }
            if Button::new("Down").small().show(ui).on_hover_text(renumbers).clicked() {
                doc.edit("Move object", |doc| doc.move_object(1));
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for (index, layout) in doc.objects().to_vec().iter().enumerate() {
                let chosen = doc.object == index;
                let (rect, response) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 52.0), egui::Sense::click());
                if chosen {
                    ui.painter().rect_filled(rect, 3.0, ui.visuals().selection.bg_fill);
                }
                let icon = Rect::from_min_size(rect.min + Vec2::splat(2.0), Vec2::splat(48.0));
                let (width, height) = (layout.rows.iter().map(Vec::len).max().unwrap_or(1).clamp(1, 6), layout.rows.len().clamp(1, 6));
                draw_object(ui.painter(), id, &sheet, count, layout, icon, width, height);
                ui.painter().text(
                    Pos2::new(icon.right() + 8.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("Object {index}"),
                    egui::FontId::proportional(13.0),
                    ui.visuals().text_color(),
                );
                if response.clicked() {
                    doc.object = index;
                    doc.object_cell = None;
                }
            }
        });
    });

    controls::side_panel(ui, egui::Panel::right(doc.panel_id("tileset-object-blocks")), 300.0).show(ui, |ui| {
        ui.strong("Map16 blocks");
        ui.weak("Click a block to put it in the chosen cell; Shift+click inserts it instead. Drag to place several.");
        let picked = tile_picker::show(ui, "object-blocks", &sheet, version, &mut view.object_blocks, 16, MAP16_COLUMNS, ui.available_height());
        if picked.finished {
            let insert = ui.input(|i| i.modifiers.shift);
            let blocks = view.object_blocks.indices(MAP16_COLUMNS, count);
            doc.edit(if insert { "Insert blocks" } else { "Place blocks" }, |doc| doc.place_blocks(&blocks, insert));
            view.row_chosen = false;
        }
    });

    egui::CentralPanel::default().show(ui, |ui| {
        object_settings(ui, doc, view);
        let height = (ui.available_height() * 0.5).max(120.0);
        egui::ScrollArea::both()
            .id_salt("object-table")
            .max_height(height)
            .auto_shrink([false, true])
            .show(ui, |ui| object_table(ui, doc, view, id, &sheet, count));
        ui.separator();
        cell_settings(ui, doc, view, count);
        ui.separator();
        object_preview(ui, doc, view, id, &sheet, count);
    });
}

/// Draws `layout` laid out `width` by `height` blocks into `rect`, scaled to fit.
#[allow(clippy::too_many_arguments)]
fn draw_object(
    painter: &egui::Painter,
    texture: egui::TextureId,
    sheet: &RgbaImage,
    count: usize,
    layout: &ObjectLayout,
    rect: Rect,
    width: usize,
    height: usize,
) {
    let Ok(cells) = render_object_layout(layout, width, height) else { return };
    let cell = (rect.width() / width as f32).min(rect.height() / height as f32);
    for (index, &block) in cells.iter().enumerate() {
        if block < 0 || block as usize >= count {
            continue;
        }
        let at = Rect::from_min_size(rect.min + Vec2::new((index % width) as f32, (index / width) as f32) * cell, Vec2::splat(cell));
        draw_block(painter, texture, sheet, at, block as usize);
    }
}

/// The object's own settings: which it is, and whether it is a slope and which way that runs.
fn object_settings(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    ui.horizontal_wrapped(|ui| {
        ui.strong(format!("Object {} of {}", doc.object, doc.objects().len()));
        if let Some(layout) = doc.objects().get(doc.object).cloned() {
            let mut edited = layout.clone();
            controls::checkbox(ui, &mut edited.slope, "Slope").on_hover_text("Laid corner to corner along a diagonal instead of in rows");
            if edited.slope {
                controls::checkbox(ui, &mut edited.slope_left, "Runs left");
                controls::checkbox(ui, &mut edited.slope_down, "Runs down");
            }
            if edited != layout {
                doc.edit("Edit slope", |doc| {
                    if let Some(layout) = doc.object_mut() {
                        *layout = edited;
                    }
                });
            }
        }
        ui.separator();
        if Button::icon(icons::ZOOM_OUT).show(ui).clicked() {
            view.object_zoom = (view.object_zoom - 1.0).max(1.0);
        }
        if Button::icon(icons::ZOOM_IN).show(ui).clicked() {
            view.object_zoom = (view.object_zoom + 1.0).min(4.0);
        }
    });
}

/// The object's rows, each block drawn, with a slot after each row to add to it.
fn object_table(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, id: egui::TextureId, sheet: &RgbaImage, count: usize) {
    let size = 16.0 * view.object_zoom;
    let rows = doc.object_table();
    let slope = doc.objects().get(doc.object).is_some_and(|o| o.slope);
    let accent = zoom_canvas::SELECTION;
    let tip = "Click to choose · Ctrl+click deletes · Alt+click repeats across · Shift+click repeats down";
    ui.spacing_mut().item_spacing = Vec2::splat(1.0);
    for (y, (block, row)) in rows.iter().enumerate() {
        ui.horizontal(|ui| {
            let header = if slope && !block.is_empty() { format!("{block} {y}") } else { format!("{y}") };
            let (rect, response) = ui.allocate_exact_size(Vec2::new(if slope { 64.0 } else { 28.0 }, size), egui::Sense::click());
            let row_chosen = view.row_chosen && doc.object_cell.is_some_and(|(r, _)| r == y);
            if row_chosen {
                ui.painter().rect_filled(rect, 2.0, ui.visuals().selection.bg_fill);
            }
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, header, egui::FontId::proportional(11.0), ui.visuals().text_color());
            let response = response.on_hover_text("Click to choose the row · Ctrl+click deletes it");
            if response.clicked() {
                if ui.input(|i| i.modifiers.command) {
                    doc.edit("Delete row", |doc| doc.delete_object_row(y));
                } else {
                    doc.object_cell = Some((y, 0));
                    view.row_chosen = true;
                }
            }
            for x in 0..=row.len() {
                let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::click());
                let painter = ui.painter();
                let chosen = !view.row_chosen && doc.object_cell == Some((y, x));
                match row.get(x) {
                    Some(cell) => {
                        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
                        if cell.tile >= 0 && (cell.tile as usize) < count {
                            draw_block(painter, id, sheet, rect, cell.tile as usize);
                        } else {
                            painter.text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                "∅",
                                egui::FontId::proportional(12.0),
                                ui.visuals().weak_text_color(),
                            );
                        }
                        if cell.control & 1 != 0 {
                            painter.rect_stroke(rect, 0.0, Stroke::new(2.0, Color32::from_rgb(230, 60, 60)), StrokeKind::Inside);
                        }
                        if cell.control & 2 != 0 {
                            painter.rect_stroke(rect.shrink(3.0), 0.0, Stroke::new(2.0, Color32::from_rgb(60, 120, 240)), StrokeKind::Inside);
                        }
                    }
                    None => {
                        painter.rect_stroke(rect.shrink(2.0), 2.0, Stroke::new(1.0, ui.visuals().weak_text_color()), StrokeKind::Inside);
                        painter.text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "+",
                            egui::FontId::proportional(14.0),
                            ui.visuals().weak_text_color(),
                        );
                    }
                }
                if chosen {
                    painter.rect_stroke(rect, 0.0, Stroke::new(2.0, accent), StrokeKind::Outside);
                }
                let response = match row.get(x) {
                    Some(cell) => response.on_hover_text(format!("Block {} · control 0x{:02x}\n{tip}", cell.tile, cell.control)),
                    None => response.on_hover_text("Click, then a block on the right, to add to this row"),
                };
                if !response.clicked() {
                    continue;
                }
                let modifiers = ui.input(|i| i.modifiers);
                doc.object_cell = Some((y, x));
                view.row_chosen = false;
                let Some(cell) = row.get(x) else { continue };
                if modifiers.command {
                    doc.edit("Delete tile", TilesetDoc::remove_object_tile);
                } else if modifiers.alt || modifiers.shift {
                    let bit = if modifiers.alt { 1 } else { 2 };
                    doc.edit("Change repeat", |doc| doc.set_object_control(cell.control ^ bit));
                    doc.object_cell = Some((y, x));
                }
            }
        });
    }
}

/// The chosen cell's block and control byte, and the buttons that add and remove.
fn cell_settings(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, count: usize) {
    let chosen = doc.object_cell.and_then(|(y, x)| doc.object_table().get(y).and_then(|(_, row)| row.get(x)).copied());
    ui.horizontal_wrapped(|ui| {
        if let (Some(cell), Some((y, x))) = (chosen, doc.object_cell) {
            ui.label("Block");
            let mut tile = cell.tile;
            ui.add(controls::number(&mut tile).range(-1..=count as i32 - 1)).on_hover_text("-1 leaves the cell empty");
            let mut across = cell.control & 1 != 0;
            let mut down = cell.control & 2 != 0;
            controls::checkbox(ui, &mut across, "Repeat across").on_hover_text("The tiles marked repeat fill the object's width (Alt+click)");
            controls::checkbox(ui, &mut down, "Repeat down")
                .on_hover_text("Marked on a row's first tile, the row repeats to fill the object's height (Shift+click)");
            ui.label("Control");
            let mut control = (cell.control & !3) | u8::from(across) | (u8::from(down) << 1);
            ui.add(controls::number(&mut control).range(0..=127).hexadecimal(2, false, true));
            let edited = ObjectLayoutTile::new(tile, control);
            if edited != cell {
                doc.edit("Edit tile", |doc| {
                    if let Some(slot) = doc.cell_mut(y, x) {
                        *slot = edited;
                    }
                });
            }
            ui.separator();
        }
        if Button::new("Empty tile").small().show(ui).on_hover_text("Puts a cell that draws nothing in the chosen place").clicked() {
            doc.edit("Place blocks", |doc| doc.place_blocks(&Pattern::single(None), ui.input(|i| i.modifiers.shift)));
        }
        if Button::new("New row").small().show(ui).on_hover_text("Adds a row after the chosen one").clicked() {
            let row = doc.object_cell.map_or(doc.object_table().len().saturating_sub(1), |(row, _)| row);
            doc.edit("Add row", |doc| doc.insert_object_row(row));
            view.row_chosen = false;
        }
        let delete = Button::new("Delete")
            .small()
            .enabled(doc.object_cell.is_some())
            .show(ui)
            .on_hover_text("Removes the chosen tile, or row (Delete)")
            .clicked()
            || (!ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Delete)));
        if delete && let Some((row, _)) = doc.object_cell {
            if view.row_chosen {
                doc.edit("Delete row", |doc| doc.delete_object_row(row));
            } else {
                doc.edit("Delete tile", TilesetDoc::remove_object_tile);
            }
        }
    });
}

/// The object laid out at a size dragged out on it, as a level would show it.
fn object_preview(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, id: egui::TextureId, sheet: &RgbaImage, count: usize) {
    let (mut width, mut height) = view.preview;
    ui.horizontal_wrapped(|ui| {
        ui.label("Preview");
        ui.add(controls::number(&mut width).range(1..=64));
        ui.label("×");
        ui.add(controls::number(&mut height).range(1..=64));
        ui.weak("Drag on the preview to size it.");
    });
    let cell = 16.0 * view.object_zoom.min(2.0);
    let (area, response) = ui.allocate_exact_size(ui.available_size().max(Vec2::splat(32.0)), egui::Sense::click_and_drag());
    let painter = ui.painter_at(area);
    painter.rect_filled(area, 0.0, ui.visuals().extreme_bg_color);
    if (response.dragged() || response.clicked())
        && let Some(at) = response.interact_pointer_pos()
    {
        width = (((at.x - area.left()) / cell) as usize + 1).clamp(1, 64);
        height = (((at.y - area.top()) / cell) as usize + 1).clamp(1, 64);
    }
    view.preview = (width, height);
    let rect = Rect::from_min_size(area.min, Vec2::new(width as f32, height as f32) * cell);
    if let Some(layout) = doc.objects().get(doc.object) {
        draw_object(&painter, id, sheet, count, layout, rect, width, height);
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0, ui.visuals().weak_text_color()), StrokeKind::Outside);
}

fn behaviors(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    let (version, sheet) = map16_sheet(doc, view);
    let count = doc.map16_count();
    controls::side_panel(ui, egui::Panel::left(doc.panel_id("tileset-behavior-blocks")), 320.0).show(ui, |ui| {
        ui.strong("Map16 blocks");
        ui.weak("Drag to choose several blocks: an edit sets them all.");
        match &sheet {
            Ok((image, _)) => {
                if tile_picker::show(ui, "behavior-blocks", image, version, &mut view.behavior_blocks, 16, MAP16_COLUMNS, ui.available_height())
                    .changed
                {
                    let first = view.behavior_blocks.first(MAP16_COLUMNS);
                    if first < count {
                        doc.behavior_entry = first;
                    }
                }
            }
            Err(error) => {
                controls::error_text(ui, error);
            }
        }
    });
    let chosen: Vec<usize> = view.behavior_blocks.indices(MAP16_COLUMNS, count).cells.into_iter().flatten().collect();
    egui::CentralPanel::default().show(ui, |ui| {
        let raw = doc.behavior();
        let parts = decode_tile_behavior(raw);
        let mut edited = parts;
        ui.horizontal(|ui| {
            ui.label("Map16 block");
            let last = count.saturating_sub(1);
            let before = doc.behavior_entry;
            ui.add(controls::number(&mut doc.behavior_entry).range(0..=last));
            if doc.behavior_entry != before {
                view.behavior_blocks.select(doc.behavior_entry, MAP16_COLUMNS);
            }
            if chosen.len() > 1 {
                ui.weak(format!("and {} more chosen", chosen.len() - 1));
            }
        });
        ui.columns(2, |columns| {
            let [left, right] = columns else { return };
            left.label("Flags");
            egui::ScrollArea::vertical().id_salt("behavior-flags").show(left, |ui| {
                for (i, name) in TILE_BEHAVIOR_FLAG_NAMES.iter().enumerate() {
                    let bit = 1u32 << i;
                    let mut on = edited.flags & bit != 0;
                    if controls::checkbox(ui, &mut on, *name).changed() {
                        edited.flags ^= bit;
                    }
                }
            });
            let ui = right;
            ui.horizontal(|ui| {
                ui.label("Subtype");
                controls::drop_down("behavior-subtype")
                    .selected_text(TILE_BEHAVIOR_SUBTYPE_NAMES.get(usize::from(edited.subtype)).copied().unwrap_or_default())
                    .show_ui(ui, |ui| {
                        for (i, name) in TILE_BEHAVIOR_SUBTYPE_NAMES.iter().enumerate() {
                            ui.choice_value(&mut edited.subtype, u8::try_from(i).unwrap_or(0), *name);
                        }
                    });
            });
            ui.horizontal(|ui| {
                ui.label("Parameter");
                ui.add(controls::number(&mut edited.parameter));
                ui.weak(TilesetDoc::behavior_meaning(parts));
            });
            // Every value the parameter has a name for, for this kind of block.
            let kind = tile_behavior_parameter_kind(edited);
            let named: Vec<(u8, &str)> =
                (0..=255u8).map(|v| (v, tile_behavior_parameter_name(kind, v))).filter(|(_, name)| !name.is_empty()).collect();
            if !named.is_empty() {
                controls::search(ui, &mut view.behavior_search, "Find a value", Width::Fill);
                let filter = view.behavior_search.to_lowercase();
                egui::ScrollArea::vertical().id_salt("behavior-values").max_height(220.0).show(ui, |ui| {
                    for (value, name) in named.iter().filter(|(_, name)| name.to_lowercase().contains(&filter)) {
                        if ui.choice(edited.parameter == *value, controls::numbered(i64::from(*value), name, false)).clicked() {
                            edited.parameter = *value;
                        }
                    }
                });
            }
            ui.horizontal(|ui| {
                ui.label("Raw hex");
                let id = egui::Id::new(("behavior-raw", doc.slot, doc.behavior_entry));
                let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("{raw:08x}"));
                let response = ui.add(TextField::new(&mut text).width(Width::Px(90.0)).code());
                if response.lost_focus() {
                    if let Ok(value) = u32::from_str_radix(text.trim(), 16) {
                        edited = decode_tile_behavior(value);
                    }
                    ui.data_mut(|d| d.remove::<String>(id));
                } else if response.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, text));
                }
            });
        });
        if edited != parts && count > 0 {
            let value = encode_tile_behavior(edited);
            let entries = if chosen.contains(&doc.behavior_entry) { chosen.clone() } else { vec![doc.behavior_entry] };
            doc.edit("Edit behaviour", |doc| doc.set_behaviors(&entries, value));
        }
    });
}

fn tilemap(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, services: &dyn Services, which: usize) {
    let name = if which == 0 { "Background" } else { "Foreground" };
    let (key, tiles_key): (&'static str, &'static str) =
        if which == 0 { ("background", "background-tiles") } else { ("foreground", "foreground-tiles") };
    let palettes = doc.tilemaps.get(which).map_or(1, |m| (m.palette.len() / 256).max(1));
    let Some(editor) = view.maps.get_mut(which) else { return };
    editor.palette = editor.palette.clamp(0, palettes as i16 - 1);
    let palette = editor.palette;
    controls::side_panel(ui, egui::Panel::right(doc.panel_id("tilemap-side").with(which)), 320.0).show(ui, |ui| {
        ui.strong("Tiles");
        ui.weak("Drag to choose the tiles to paint with.");
        let version = doc.version * 32 + palette.max(0) as u64;
        match cached(&mut view.images, tiles_key, version, || doc.tilemap_tiles(which, palette).map(|(image, count)| (Arc::new(image), count))) {
            Ok((image, count)) => {
                if let (Some(picker), Some(editor), Some(made)) =
                    (view.map_tiles.get_mut(which), view.maps.get_mut(which), view.brushes.get_mut(which + 1))
                {
                    let picked = tile_picker::show(ui, tiles_key, &image, version, picker, 8, GRAPHICS_COLUMNS, ui.available_height() * 0.6);
                    if picked.changed || !*made {
                        *made = true;
                        editor.set_brush(brush_from(picker, count, 0));
                    }
                }
            }
            Err(error) => {
                controls::error_text(ui, error);
            }
        }
        ui.separator();
        egui::ScrollArea::vertical().id_salt("tilemap-inspector").show(ui, |ui| {
            let entry = doc.map_entries.get(which).copied().unwrap_or(0);
            ui.strong(format!("Entry {entry}"));
            if let Some(value) = doc.map_value(which)
                && let Some(edited) = entry_fields(ui, value, 0..=1023, 0..=15)
            {
                doc.edit("Edit entry", |doc| doc.set_map_value(which, edited));
            }
        });
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let Some(editor) = view.maps.get_mut(which) else { return };
        let mut commands = Vec::new();
        editor.toolbar(ui, None, palettes, &mut commands);
        ui.horizontal(|ui| {
            if Button::new("Import PNG…").show(ui).clicked() {
                match services.open_file(&format!("Import {name}"), &["png"]) {
                    Some(Ok((_, bytes))) => {
                        view.notice = decode_png(&bytes).and_then(|image| doc.edit("Import image", |doc| doc.import_tilemap(which, &image))).err();
                    }
                    Some(Err(error)) => view.notice = Some(error),
                    None => {}
                }
            }
            if Button::new("Export PNG…").show(ui).clicked() {
                let written = doc
                    .tilemap_image(which)
                    .and_then(|image| encode_png(&image))
                    .and_then(|bytes| services.save_file(&format!("Export {name}"), &format!("{}.png", name.to_lowercase()), "png", &bytes));
                view.notice = written.err();
            }
        });
        let image = match cached(&mut view.images, key, doc.version, || doc.tilemap_image(which).map(|image| (Arc::new(image), 0))) {
            Ok((image, _)) => image,
            Err(error) => {
                controls::error_text(ui, error);
                return;
            }
        };
        let Some(editor) = view.maps.get_mut(which) else { return };
        let input = editor.canvas(ui, key, &image, doc.version, ui.available_height() - zoom_canvas::below(ui));
        editor.keys(ui, &input, &mut commands);
        let hovered = editor.hover().and_then(|(x, y)| {
            let map = doc.tilemaps.get(which)?;
            if x < 0 || y < 0 || x as usize >= map.width_tiles {
                return None;
            }
            map.screen.get(y as usize * map.width_tiles + x as usize).copied()
        });
        ui.weak(entry_text(editor.hover(), hovered));
        let (begins, ends) = edits(&input, &commands);
        if begins {
            doc.begin_edit(editor.step_name(&commands));
        }
        if input.gesture.is_some() || !commands.is_empty() {
            let ctx = ui.ctx().clone();
            let outcome = doc.with_tilemap(which, |grid| editor.apply(&ctx, &input, &commands, grid)).unwrap_or_default();
            if let Some((x, y)) = outcome.clicked
                && let Some(width) = doc.tilemaps.get(which).map(|m| m.width_tiles)
                && let Some(slot) = doc.map_entries.get_mut(which)
            {
                *slot = y * width + x;
            }
            if outcome.picked
                && editor.brush.width * editor.brush.height == 1
                && let (Some(entry), Some(picker)) = (editor.brush.cells.first(), view.map_tiles.get_mut(which))
            {
                picker.select(entry.tile.max(0) as usize, GRAPHICS_COLUMNS);
                editor.palette = entry.palette;
            }
        }
        if ends {
            doc.end_edit();
        }
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bigstar_formats::{Bgr555Color, Map16Tile, encode_8bpp_tiles, encode_bgr555_palette, encode_map16};

    fn bytes(rom_path: &str, bytes: Vec<u8>) -> ResourceBytes {
        ResourceBytes { rom_path: rom_path.to_owned(), bytes }
    }

    /// One tile, two colours, one Map16 entry and one empty object per slot, and a one-row 64-entry tilemap; with a texture for
    /// block 256 (slot 1's first), as a tile script would claim.
    pub(crate) fn resources() -> TilesetResources {
        let graphics = encode_8bpp_tiles(&[IndexedTile8Bpp::default()]).unwrap();
        let palette = encode_bgr555_palette(&[Bgr555Color::new(0, 0, 0, true), Bgr555Color::new(255, 255, 255, false)]).unwrap();
        let map16 = encode_map16(&[Map16Tile::default()], 0, 0).unwrap();
        let slot = |n: usize| TilesetSlot {
            graphics: bytes(&format!("BG_ncg/t{n}_ncg.bin"), graphics.clone()),
            palette: bytes(&format!("BG_ncl/t{n}_ncl.bin"), palette.clone()),
            map16: bytes(&format!("BG_pnl/t{n}_pnl.bin"), map16.clone()),
            objects: bytes(&format!("BG_unt/t{n}.bin"), vec![0, 0, 0, 0xff]),
            object_index: bytes(&format!("BG_unt/t{n}_hd.bin"), vec![0, 0, 0, 0]),
            behaviors: bytes(&format!("BG_chk/t{n}_chk.bin"), vec![0, 0, 0, 0]),
            tile_page_offset: if n == 1 { 1 } else { 0 },
            ..TilesetSlot::default()
        };
        let map = TilemapSlot {
            graphics: bytes("BG_ncg/bg_ncg.bin", graphics.clone()),
            palette: bytes("BG_ncl/bg_ncl.bin", palette.clone()),
            screen: bytes("BG_nsc/bg_nsc.bin", vec![0; 128]),
        };
        let texture = RgbaImage { width: 16, height: 16, pixels: vec![0x7f; 16 * 16 * 4] };
        TilesetResources {
            level: 1,
            tilesets: vec![slot(0), slot(1), slot(2)],
            background: map.clone(),
            foreground: TilemapSlot { screen: bytes("BG_nsc/fg_nsc.bin", vec![0; 128]), ..map },
            references: LevelTilesetReferences {
                tileset_id: 1,
                background_graphics_id: 2,
                background_screen_id: 3,
                foreground_graphics_id: 4,
                ..Default::default()
            },
            overlays: vec![bigstar_protocol::Map16Overlay { block: 256, image: Some(texture), remap: None, replaces: true }],
        }
    }

    #[test]
    fn the_tileset_workspace_exposes_every_surface_and_preserves_resources() {
        let mut doc = TilesetDoc::open(resources(), "1-1").unwrap();
        assert_eq!(TABS.len(), 7);
        assert_eq!(doc.slot, 1, "the level's own tileset first");
        doc.references.tileset_id = 42;
        assert!(doc.references_changed());
        let unchanged = doc.tileset_data(1).unwrap();
        assert_eq!(unchanged.graphics, resources().tilesets[1].graphics.bytes);
        assert_eq!(unchanged.objects, resources().tilesets[1].objects.bytes);
        assert!(doc.writes().unwrap().is_empty(), "nothing edited, nothing written");

        // Drawing a block as the game draws it must not edit the block.
        assert!(doc.solid3d);
        let with = doc.map16_sheet().unwrap();
        assert_eq!(with.pixels[..4], [0x7f; 4], "block 256 shows its texture");
        doc.solid3d = false;
        assert_ne!(doc.map16_sheet().unwrap().pixels[..4], [0x7f; 4]);
        doc.solid3d = true;
        assert_eq!(doc.tileset_data(1).unwrap().map16, resources().tilesets[1].map16.bytes);

        let mut value = doc.map16_value().unwrap();
        value.tile = 1;
        doc.set_map16_value(value);
        doc.object_cell = Some((0, 0));
        doc.place_blocks(&Pattern::single(Some(1)), false);
        doc.set_behavior(0x89ab_cdef);
        let changed = doc.tileset_data(1).unwrap();
        let reparsed = parse_tileset(&changed, 0, 0, 1).unwrap();
        assert_eq!(reparsed.map16[0].tiles[0].tile, 1);
        assert_eq!(reparsed.objects[0].rows[0][0].tile, 1);
        assert_eq!(reparsed.behaviors[0], 0x89ab_cdef);

        // Only the files that changed are written, to their own paths: an object the same size leaves the index be.
        let paths: Vec<String> = doc.writes().unwrap().into_iter().map(|w| w.path).collect();
        assert_eq!(paths, ["BG_pnl/t1_pnl.bin", "BG_unt/t1.bin", "BG_chk/t1_chk.bin"].map(str::to_owned));
    }

    #[test]
    fn objects_grow_and_shrink_but_never_vanish() {
        let mut doc = TilesetDoc::open(resources(), "1-1").unwrap();
        doc.add_object();
        assert_eq!((doc.objects().len(), doc.object), (2, 1));
        doc.insert_object_row(0);
        assert_eq!(doc.object_cell, Some((1, 0)));
        // Clicking blocks replaces the chosen cell and moves on, so a row builds block by block.
        doc.place_blocks(&Pattern { width: 2, height: 1, cells: vec![Some(3), Some(4)] }, false);
        assert_eq!(doc.objects()[1].rows[1], [ObjectLayoutTile::new(3, 0), ObjectLayoutTile::new(4, 0)]);
        assert_eq!(doc.object_cell, Some((1, 2)), "the slot after the row, to add to it");
        // Shift inserts instead.
        doc.object_cell = Some((1, 0));
        doc.place_blocks(&Pattern::single(Some(5)), true);
        assert_eq!(doc.objects()[1].rows[1].iter().map(|t| t.tile).collect::<Vec<_>>(), [5, 3, 4]);
        doc.set_object_control(1);
        assert_eq!(doc.objects()[1].rows[1][1], ObjectLayoutTile::new(3, 1));
        // A pick two rows tall carries on into a row it adds.
        doc.object_cell = Some((1, 3));
        doc.place_blocks(&Pattern { width: 1, height: 2, cells: vec![Some(6), Some(7)] }, false);
        assert_eq!(doc.objects()[1].rows.len(), 3);
        assert_eq!(doc.objects()[1].rows[2], [ObjectLayoutTile::new(7, 0)]);
        doc.delete_object_row(2);
        doc.object_cell = Some((0, 0));
        doc.remove_object_tile();
        assert_eq!(doc.objects()[1].rows.len(), 1, "the emptied row goes");
        for _ in 0..4 {
            doc.object_cell = Some((0, 0));
            doc.remove_object_tile();
        }
        assert_eq!(doc.objects()[1].rows, [vec![ObjectLayoutTile::new(-1, 0)]], "an emptied object keeps one empty tile");
        doc.duplicate_object();
        doc.move_object(-1);
        assert_eq!((doc.objects().len(), doc.object), (3, 1));
        doc.delete_object();
        doc.delete_object();
        doc.delete_object();
        assert_eq!(doc.objects().len(), 1, "a tileset keeps one object");
    }

    #[test]
    fn a_slopes_table_reaches_its_fill() {
        let mut doc = TilesetDoc::open(resources(), "1-1").unwrap();
        let tile = |tile| vec![ObjectLayoutTile::new(tile, 4)];
        doc.tileset_mut().unwrap().objects[0] =
            ObjectLayout { rows: vec![tile(1)], slope_fill: vec![tile(2)], slope: true, ..ObjectLayout::default() };
        assert_eq!(doc.object_table(), [("Step", tile(1)), ("Fill", tile(2))]);
        doc.object_cell = Some((1, 0));
        doc.place_blocks(&Pattern::single(Some(9)), false);
        assert_eq!(doc.object_table()[1].1, tile(9), "a replaced block keeps its control");
        doc.insert_object_row(1);
        assert_eq!(doc.objects()[0].slope_fill, [tile(9), vec![ObjectLayoutTile::new(-1, 0)]]);
        doc.remove_object_tile();
        doc.object_cell = Some((1, 0));
        doc.remove_object_tile();
        assert!(doc.objects()[0].slope_fill.is_empty(), "a slope can lose its fill");
        assert_eq!(doc.objects()[0].rows, [tile(1)]);
    }

    #[test]
    fn a_tileset_without_behaviours_gets_a_table_when_one_is_set() {
        let mut resources = resources();
        resources.tilesets[1].behaviors.bytes.clear();
        let mut doc = TilesetDoc::open(resources, "1-1").unwrap();
        assert_eq!(doc.behavior(), 0);
        doc.set_behavior(encode_tile_behavior(TileBehaviorParts { flags: 1, subtype: 0, parameter: 0 }));
        assert_eq!(doc.tilesets[1].behaviors.len(), doc.map16_count());
        assert_eq!(decode_tile_behavior(doc.behavior()).flags, 1);
    }

    #[test]
    fn backgrounds_edit_their_entries_and_bundles_round_trip() {
        let mut doc = TilesetDoc::open(resources(), "1-1").unwrap();
        let mut value = doc.map_value(1).unwrap();
        value.palette = 3;
        doc.set_map_value(1, value);
        let paths: Vec<String> = doc.writes().unwrap().into_iter().map(|w| w.path).collect();
        assert_eq!(paths, ["BG_nsc/fg_nsc.bin"]);

        let bundle = doc.export_bundle().unwrap();
        doc.object_cell = Some((0, 0));
        doc.place_blocks(&Pattern::single(Some(7)), false);
        doc.import_bundle(&bundle).unwrap();
        assert_eq!(doc.objects()[0].rows[0][0].tile, -1, "the bundle's tileset replaces the edited one");
        assert!(doc.import_bundle(b"nope").is_err());
    }

    #[test]
    fn an_edit_with_nowhere_to_go_is_refused() {
        let mut resources = resources();
        resources.tilesets[0].map16.rom_path.clear();
        let mut doc = TilesetDoc::open(resources, "1-1").unwrap();
        doc.set_slot(0);
        let mut value = doc.map16_value().unwrap();
        value.tile = 2;
        doc.set_map16_value(value);
        assert_eq!(doc.writes().unwrap_err(), "edited resource has no ROM file target");
    }

    #[test]
    fn the_graphics_tab_draws_and_edits_each_palette_and_each_layer() {
        let mut doc = TilesetDoc::open(resources(), "1-1").unwrap();
        // A second block of 256 colours, as retail tilesets have.
        doc.tilesets[1].palette.resize(512, Bgr555Color::new(0, 0, 0, false));
        assert_eq!(doc.sheet_palettes(Sheet::Current), 2);
        doc.with_sheet(Sheet::Current, |grid| grid.set(0, 0, 3));
        doc.set_sheet_colour(Sheet::Current, 256 + 3, [255, 0, 0]);
        assert_eq!(doc.tilesets[1].palette[259], Bgr555Color::new(255, 0, 0, false));
        assert_eq!(doc.sheet_image(Sheet::Current, 1).unwrap().pixels[..4], [255, 0, 0, 255], "drawn in palette 1");
        assert_ne!(doc.sheet_image(Sheet::Current, 0).unwrap().pixels[..4], [255, 0, 0, 255]);
        // An import of every palette writes from colour 1 on, in the block it is for.
        SheetImages { doc: &mut doc, sheet: Sheet::Current }.set_palette(1, &[[0, 0, 255], [0, 255, 0]]);
        assert_eq!(doc.tilesets[1].palette[257], Bgr555Color::new(0, 255, 0, false));
        assert_eq!(doc.tilesets[1].palette[256], Bgr555Color::new(0, 0, 0, false), "colour 0 is left alone");

        // The background is a bitmap of its own, saved as its own graphics.
        let names: Vec<String> = doc.sheets().into_iter().map(|(_, name)| name).collect();
        assert!(names.contains(&"Background".to_owned()), "{names:?}");
        let before = doc.writes().unwrap().len();
        doc.with_sheet(Sheet::Layer(0), |grid| grid.set(0, 0, 1));
        assert_eq!(doc.writes().unwrap().len(), before + 1, "the background's graphics are written too");
    }

    #[test]
    fn every_tab_undoes_into_one_history() {
        let mut doc = TilesetDoc::open(resources(), "1-1").unwrap();
        doc.edit("Edit quadrant", |doc| {
            let mut value = doc.map16_value().unwrap();
            value.tile = 1;
            doc.set_map16_value(value);
        });
        doc.begin_edit("Brush");
        doc.with_sheet(Sheet::Tileset(1), |grid| grid.set(0, 0, 1));
        doc.with_sheet(Sheet::Tileset(1), |grid| grid.set(1, 0, 1));
        doc.end_edit();
        doc.edit("Draw tiles", |doc| doc.with_tilemap(0, |grid| grid.set(0, 0, ScreenMapEntry::new(5, 0, false, false))));
        assert_eq!(doc.history_names().0, ["Draw tiles", "Brush", "Edit quadrant"]);
        doc.undo();
        doc.undo();
        assert_eq!(doc.tilesets[1].graphics[0].pixels[..2], [0, 0], "the stroke was one step");
        assert_eq!(doc.map16_value().unwrap().tile, 1);
        doc.undo();
        assert!(doc.writes().unwrap().is_empty(), "undone back to what was read");
        doc.redo();
        assert_eq!(doc.writes().unwrap().len(), 1);
    }

    #[test]
    fn the_map16_picker_paints_tiles_from_every_slot_in_this_tilesets_numbering() {
        let doc = TilesetDoc::open(resources(), "1-1").unwrap();
        let (image, count) = doc.composed_tiles(0).unwrap();
        assert_eq!((image.width, count), (GRAPHICS_COLUMNS * 8, 1), "the fixture's slots all sit at offset 0");
        let mut picker = PickerState::default();
        picker.select(0, GRAPHICS_COLUMNS);
        assert_eq!(brush_from(&picker, count, 192).cells, [ScreenMapEntry::new(-192, 0, false, false)], "a tile below this tileset's own");
    }
}
