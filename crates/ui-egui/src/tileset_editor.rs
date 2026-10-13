//! The tileset workspace: a level's three tilesets (graphics, Map16, objects, behaviours) and its background and
//! foreground, edited together and saved as only the files that changed. A port of `TilesetEditorDialog` and of
//! `MainWindow::openTilesetEditor`'s saving; the document is pure and `show` draws it as a tab.

use crate::controls::{self, Button, Choice as _, TextField, Width};
use crate::icons;
use std::collections::BTreeMap;
use std::sync::Arc;

use bigstar_formats::{
    EditableTilemap, EditableTileset, ObjectLayout, ObjectLayoutTile, RgbaImage, ScreenMapEntry, TILE_BEHAVIOR_FLAG_NAMES,
    TILE_BEHAVIOR_PIPE_MAIN_NAMES, TILE_BEHAVIOR_SUBTYPE_NAMES, TileBehaviorParameterKind, TileBehaviorParts, TileOverlay, TilemapResourceData,
    TilesetResourceData, compose_tileset_memory, decode_tile_behavior, encode_tile_behavior, import_tilemap_image, import_tileset_graphics,
    overlay_map16_sheet, parse_tilemap, parse_tileset, parse_tileset_bundle, render_composed_map16_sheet, render_map16_sheet, render_tilemap,
    render_tileset_graphics, tile_behavior_parameter_kind, tile_behavior_parameter_name, write_tilemap, write_tileset, write_tileset_bundle,
};
use bigstar_level::LevelTilesetReferences;
use bigstar_protocol::{FileWrite, LevelId, ResourceBytes, TilemapSlot, TilesetResources, TilesetSlot};
use egui::{RichText, Ui};

use crate::graphics_editor::picture;
use crate::services::Services;

pub const SLOT_NAMES: [&str; 3] = ["0 — shared", "1 — level", "2 — sub-unit"];
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

/// A PNG's pixels.
pub fn decode_png(bytes: &[u8]) -> Result<RgbaImage, String> {
    let decoded = image::load_from_memory_with_format(bytes, image::ImageFormat::Png).map_err(|_| "Could not read the PNG.".to_owned())?.to_rgba8();
    Ok(RgbaImage { width: decoded.width() as usize, height: decoded.height() as usize, pixels: decoded.into_raw() })
}

pub fn encode_png(value: &RgbaImage) -> Result<Vec<u8>, String> {
    let (width, height) = (u32::try_from(value.width).unwrap_or(0), u32::try_from(value.height).unwrap_or(0));
    let buffer = image::RgbaImage::from_raw(width, height, value.pixels.clone()).ok_or_else(|| "Could not write the PNG.".to_owned())?;
    let mut bytes = Vec::new();
    buffer.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).map_err(|_| "Could not write the PNG.".to_owned())?;
    Ok(bytes)
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
        let background = parse_tilemap(&map_resource(&resources.background), 0).map_err(|e| e.message)?;
        let foreground = parse_tilemap(&map_resource(&resources.foreground), 0).map_err(|e| e.message)?;
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
        })
    }

    fn touched(&mut self) {
        self.version += 1;
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

    pub fn graphics_sheet(&self) -> Result<RgbaImage, String> {
        let tileset = self.tileset().ok_or("no tileset")?;
        render_tileset_graphics(tileset, 32).map_err(|e| e.message)
    }

    /// The sheet the Map16 and Behaviours tabs both draw. Composed, an entry naming another tileset's graphics
    /// resolves instead of coming out clear; with 3D on, blocks the game draws as quads show those.
    pub fn map16_sheet(&self) -> Result<RgbaImage, String> {
        let tileset = self.tileset().ok_or("no tileset")?;
        let mut sheet = if self.composed {
            render_composed_map16_sheet(tileset, &compose_tileset_memory(&self.tilesets), 16)
        } else {
            render_map16_sheet(tileset, 16)
        }
        .map_err(|e| e.message)?;
        if self.solid3d && !self.overlays.is_empty() {
            let page = u16::from(tileset.tile_page_offset) * 256;
            overlay_map16_sheet(&mut sheet, 16, tileset.map16.len(), page, |block| self.overlays.get(&block));
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

    // --- Objects ---

    pub fn objects(&self) -> &[ObjectLayout] {
        self.tileset().map_or(&[], |t| t.objects.as_slice())
    }

    fn rows_mut(&mut self) -> Option<&mut Vec<Vec<ObjectLayoutTile>>> {
        let object = self.object;
        self.tileset_mut().and_then(|t| t.objects.get_mut(object)).map(|o| &mut o.rows)
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

    pub fn add_object_row(&mut self) {
        if let Some(rows) = self.rows_mut() {
            rows.push(vec![ObjectLayoutTile::new(-1, 0)]);
        }
    }

    /// Adds a tile to the end of the chosen row, or of the first.
    pub fn add_object_tile(&mut self) {
        let row = self.object_cell.map_or(0, |(row, _)| row);
        if let Some(rows) = self.rows_mut() {
            let last = rows.len().saturating_sub(1);
            if let Some(row) = rows.get_mut(row.min(last)) {
                row.push(ObjectLayoutTile::new(-1, 0));
            }
        }
    }

    /// Removes the chosen tile; an emptied row goes, and an emptied object keeps one empty tile.
    pub fn remove_object_tile(&mut self) {
        let Some((row, column)) = self.object_cell else { return };
        if let Some(rows) = self.rows_mut()
            && let Some(cells) = rows.get_mut(row)
            && column < cells.len()
        {
            cells.remove(column);
            if cells.is_empty() {
                rows.remove(row);
            }
            if rows.is_empty() {
                rows.push(vec![ObjectLayoutTile::new(-1, 0)]);
            }
        }
        self.object_cell = None;
    }

    /// A cell as typed: a number is the Map16 tile, anything else leaves the cell empty.
    pub fn set_object_tile(&mut self, row: usize, column: usize, text: &str) {
        let tile = text.trim().parse::<i32>().unwrap_or(-1);
        if let Some(cell) = self.rows_mut().and_then(|rows| rows.get_mut(row)).and_then(|cells| cells.get_mut(column)) {
            cell.tile = tile;
        }
    }

    pub fn object_control(&self) -> u8 {
        let Some((row, column)) = self.object_cell else { return 0 };
        self.objects().get(self.object).and_then(|o| o.rows.get(row)).and_then(|r| r.get(column)).map_or(0, |cell| cell.control)
    }

    pub fn set_object_control(&mut self, control: u8) {
        let Some((row, column)) = self.object_cell else { return };
        if let Some(cell) = self.rows_mut().and_then(|rows| rows.get_mut(row)).and_then(|cells| cells.get_mut(column)) {
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

    pub fn import_graphics(&mut self, image: &RgbaImage) -> Result<(), String> {
        let tileset = self.tileset_mut().ok_or("no tileset")?;
        import_tileset_graphics(tileset, image).map_err(|e| e.message)
    }

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

/// The drawn sheets, each with the document version it shows.
#[derive(Default)]
pub struct TilesetView {
    textures: BTreeMap<&'static str, (u64, egui::TextureHandle)>,
    notice: Option<String>,
    /// Object table cells as typed, by (object, row, column), until they read back.
    typing: Option<((usize, usize, usize), String)>,
}

pub fn show(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, services: &dyn Services) -> Option<TilesetAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("Tilesets and backgrounds — {}", doc.title)).strong());
        ui.label("Tileset slot");
        let mut slot = doc.slot;
        controls::drop_down("tileset-slot").selected_text(SLOT_NAMES.get(slot).copied().unwrap_or_default()).show_ui(ui, |ui| {
            for (i, name) in SLOT_NAMES.iter().enumerate() {
                ui.choice_value(&mut slot, i, *name);
            }
        });
        if slot != doc.slot {
            doc.set_slot(slot);
        }
        if Button::new("Import tileset…").show(ui).clicked() {
            match services.open_file("Import tileset", Some("bstileset")) {
                Some(Ok((_, bytes))) => view.notice = doc.import_bundle(&bytes).err(),
                Some(Err(error)) => view.notice = Some(error),
                None => {}
            }
        }
        if Button::new("Export tileset…").show(ui).clicked() {
            let written = doc.export_bundle().and_then(|bytes| services.save_file("Export tileset", "tileset.bstileset", "bstileset", &bytes));
            view.notice = written.err();
        }
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
        });
    });
    controls::tabs(ui, "tileset", &mut doc.tab, &TABS);
    if let Some(notice) = &view.notice {
        controls::warning_text(ui, notice);
    }
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

fn sheet(
    ui: &mut Ui,
    view: &mut TilesetView,
    name: &'static str,
    version: u64,
    image: Result<RgbaImage, String>,
    height: f32,
) -> Option<(usize, usize)> {
    match image {
        Ok(image) => {
            let mut slot = view.textures.remove(name);
            let clicked = picture(ui, name, &image, 0, version, &mut slot, height);
            if let Some(texture) = slot {
                view.textures.insert(name, texture);
            }
            clicked
        }
        Err(error) => {
            controls::error_text(ui, error);
            None
        }
    }
}

fn slots(ui: &mut Ui, doc: &mut TilesetDoc) {
    egui::Grid::new("tileset-references").num_columns(2).show(ui, |ui| {
        let references = &mut doc.references;
        for (label, value) in [
            ("Level tileset", &mut references.tileset_id),
            ("Background graphics", &mut references.background_graphics_id),
            ("Background screen", &mut references.background_screen_id),
            ("Foreground graphics", &mut references.foreground_graphics_id),
            ("Foreground screen", &mut references.foreground_screen_id),
        ] {
            ui.label(label);
            ui.add(controls::number(value).range(0..=255));
            ui.end_row();
        }
    });
    ui.add(
        egui::Label::new(
            "These IDs select the level's graphics slots. Resource edits below apply to the resources currently loaded; changed IDs take effect when the level is saved and reloaded.",
        )
        .wrap(),
    );
}

fn graphics(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, services: &dyn Services) {
    let height = (ui.available_height() - 40.0).max(120.0);
    sheet(ui, view, "tileset-graphics", doc.version, doc.graphics_sheet(), height);
    ui.horizontal(|ui| {
        if Button::new("Import PNG…").show(ui).clicked() {
            match services.open_file("Import tileset graphics", Some("png")) {
                Some(Ok((_, bytes))) => view.notice = decode_png(&bytes).and_then(|image| doc.import_graphics(&image)).err(),
                Some(Err(error)) => view.notice = Some(error),
                None => {}
            }
        }
        if Button::new("Export PNG…").show(ui).clicked() {
            let written = doc
                .graphics_sheet()
                .and_then(|image| encode_png(&image))
                .and_then(|bytes| services.save_file("Export tileset graphics", "tileset.png", "png", &bytes));
            view.notice = written.err();
        }
    });
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

fn map16(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    let height = (ui.available_height() - 210.0).max(120.0);
    if let Some((x, y)) = sheet(ui, view, "map16", doc.version, doc.map16_sheet(), height) {
        let index = (y / 16) * 16 + x / 16;
        if index < doc.map16_count() {
            doc.map16_entry = index;
        }
    }
    egui::Grid::new("map16-choice").num_columns(2).show(ui, |ui| {
        ui.label("Map16 tile");
        let last = doc.map16_count().saturating_sub(1);
        ui.add(controls::number(&mut doc.map16_entry).range(0..=last));
        ui.end_row();
        ui.label("Quadrant");
        controls::drop_down("map16-quadrant").selected_text(QUADRANTS.get(doc.quadrant).copied().unwrap_or_default()).show_ui(ui, |ui| {
            for (i, name) in QUADRANTS.iter().enumerate() {
                ui.choice_value(&mut doc.quadrant, i, *name);
            }
        });
        ui.end_row();
    });
    if let Some(value) = doc.map16_value()
        && let Some(edited) = entry_fields(ui, value, -1023..=1023, -15..=15)
    {
        doc.set_map16_value(edited);
    }
    let before = (doc.composed, doc.solid3d);
    controls::checkbox(ui, &mut doc.composed, "Show the other tilesets’ graphics").on_hover_text(
        "A Map16 entry can name a tile another tileset supplies. Checked, the sheet draws those against all three tilesets, the way the game loads them; unchecked, it shows only what this tileset carries.",
    );
    controls::checkbox(ui, &mut doc.solid3d, "Show 3D tiles").on_hover_text(
        "The game hands some blocks to the 3D engine and draws nothing from their Map16. Checked, the sheet shows the texture the game shows; unchecked, it shows the bytes the file holds, which is what you want when editing them.",
    );
    if before != (doc.composed, doc.solid3d) {
        doc.version += 1;
    }
}

fn objects(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    ui.horizontal(|ui| {
        ui.label("Object");
        let count = doc.objects().len();
        controls::drop_down("tileset-object").selected_text(format!("Object {}", doc.object)).show_ui(ui, |ui| {
            for i in 0..count {
                if ui.choice(doc.object == i, format!("Object {i}")).clicked() {
                    doc.object = i;
                    doc.object_cell = None;
                }
            }
        });
        if Button::new("Add").show(ui).clicked() {
            doc.add_object();
        }
        if Button::new("Delete").show(ui).clicked() {
            doc.delete_object();
        }
    });
    let rows: Vec<Vec<ObjectLayoutTile>> = doc.objects().get(doc.object).map(|o| o.rows.clone()).unwrap_or_default();
    let object = doc.object;
    egui::ScrollArea::both().max_height((ui.available_height() - 40.0).max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("tileset-object-table").striped(true).show(ui, |ui| {
            for (y, row) in rows.iter().enumerate() {
                for (x, cell) in row.iter().enumerate() {
                    let key = (object, y, x);
                    let mut text = match &view.typing {
                        Some((typing, text)) if *typing == key => text.clone(),
                        _ if cell.tile < 0 => String::new(),
                        _ => cell.tile.to_string(),
                    };
                    let selected = doc.object_cell == Some((y, x));
                    let response = ui.add(TextField::new(&mut text).width(Width::Px(44.0)).fill(if selected {
                        ui.visuals().selection.bg_fill
                    } else {
                        ui.visuals().extreme_bg_color
                    }));
                    let response = response.on_hover_text(format!("Control 0x{:02x}", cell.control));
                    if response.gained_focus() || response.clicked() {
                        doc.object_cell = Some((y, x));
                    }
                    if response.changed() {
                        doc.set_object_tile(y, x, &text);
                        view.typing = Some((key, text));
                    } else if response.lost_focus() {
                        view.typing = None;
                    }
                }
                ui.end_row();
            }
        });
    });
    ui.horizontal(|ui| {
        if Button::new("Add row").show(ui).clicked() {
            doc.add_object_row();
        }
        if Button::new("Add tile").show(ui).clicked() {
            doc.add_object_tile();
        }
        if Button::new("Remove tile").show(ui).clicked() {
            doc.remove_object_tile();
        }
        ui.label("Selected control");
        let mut control = doc.object_control();
        if ui.add_enabled(doc.object_cell.is_some(), controls::number(&mut control).range(0..=127)).changed() {
            doc.set_object_control(control);
        }
    });
}

fn behaviors(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView) {
    ui.horizontal_top(|ui| {
        let width = (ui.available_width() - 360.0).max(260.0);
        ui.allocate_ui(egui::Vec2::new(width, ui.available_height()), |ui| {
            let height = ui.available_height().max(120.0);
            if let Some((x, y)) = sheet(ui, view, "behavior-map16", doc.version, doc.map16_sheet(), height) {
                let index = (y / 16) * 16 + x / 16;
                if index < doc.map16_count() {
                    doc.behavior_entry = index;
                }
            }
        });
        ui.vertical(|ui| {
            let raw = doc.behavior();
            let parts = decode_tile_behavior(raw);
            let mut edited = parts;
            ui.horizontal(|ui| {
                ui.label("Map16 tile");
                let last = doc.map16_count().saturating_sub(1);
                ui.add(controls::number(&mut doc.behavior_entry).range(0..=last));
            });
            ui.label("Flags");
            egui::ScrollArea::vertical().id_salt("behavior-flags").max_height(260.0).show(ui, |ui| {
                for (i, name) in TILE_BEHAVIOR_FLAG_NAMES.iter().enumerate() {
                    let bit = 1u32 << i;
                    let mut on = edited.flags & bit != 0;
                    if controls::checkbox(ui, &mut on, *name).changed() {
                        edited.flags ^= bit;
                    }
                }
            });
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
            });
            ui.horizontal(|ui| {
                ui.label("Meaning");
                ui.weak(TilesetDoc::behavior_meaning(parts));
            });
            if edited != parts && doc.map16_count() > 0 {
                doc.set_behavior(encode_tile_behavior(edited));
            }
            ui.horizontal(|ui| {
                ui.label("Raw hex");
                let id = egui::Id::new(("behavior-raw", doc.slot, doc.behavior_entry));
                let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("{raw:08x}"));
                let response = ui.add(TextField::new(&mut text).width(Width::Px(90.0)).code());
                if response.lost_focus() {
                    if let Ok(value) = u32::from_str_radix(text.trim(), 16) {
                        doc.set_behavior(value);
                    }
                    ui.data_mut(|d| d.remove::<String>(id));
                } else if response.has_focus() {
                    ui.data_mut(|d| d.insert_temp(id, text));
                }
            });
        });
    });
}

fn tilemap(ui: &mut Ui, doc: &mut TilesetDoc, view: &mut TilesetView, services: &dyn Services, which: usize) {
    let name = if which == 0 { "Background" } else { "Foreground" };
    let texture = if which == 0 { "background" } else { "foreground" };
    let height = (ui.available_height() - 190.0).max(120.0);
    if let Some((x, y)) = sheet(ui, view, texture, doc.version, doc.tilemap_image(which), height)
        && let Some(map) = doc.tilemaps.get(which)
    {
        let entry = (y / 8) * map.width_tiles + x / 8;
        if entry < map.screen.len()
            && let Some(slot) = doc.map_entries.get_mut(which)
        {
            *slot = entry;
        }
    }
    let count = doc.tilemaps.get(which).map_or(0, |m| m.screen.len());
    if let Some(slot) = doc.map_entries.get_mut(which) {
        ui.horizontal(|ui| {
            ui.label("Screen entry");
            ui.add(controls::number(slot).range(0..=count.saturating_sub(1)));
        });
    }
    if let Some(value) = doc.map_value(which)
        && let Some(edited) = entry_fields(ui, value, 0..=1023, 0..=15)
    {
        doc.set_map_value(which, edited);
    }
    ui.horizontal(|ui| {
        if Button::new("Import PNG…").show(ui).clicked() {
            match services.open_file(&format!("Import {name}"), Some("png")) {
                Some(Ok((_, bytes))) => view.notice = decode_png(&bytes).and_then(|image| doc.import_tilemap(which, &image)).err(),
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
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bigstar_formats::{Bgr555Color, IndexedTile8Bpp, Map16Tile, encode_8bpp_tiles, encode_bgr555_palette, encode_map16};

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
        doc.set_object_tile(0, 0, "1");
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
        doc.add_object_row();
        doc.object_cell = Some((1, 0));
        doc.add_object_tile();
        assert_eq!(doc.objects()[1].rows[1].len(), 2);
        doc.set_object_control(5);
        assert_eq!(doc.object_control(), 5);
        doc.object_cell = Some((0, 0));
        doc.remove_object_tile();
        assert_eq!(doc.objects()[1].rows.len(), 1, "the emptied row goes");
        doc.object_cell = Some((0, 0));
        doc.remove_object_tile();
        doc.object_cell = Some((0, 0));
        doc.remove_object_tile();
        assert_eq!(doc.objects()[1].rows, [vec![ObjectLayoutTile::new(-1, 0)]], "an emptied object keeps one empty tile");
        doc.delete_object();
        doc.delete_object();
        assert_eq!(doc.objects().len(), 1, "a tileset keeps one object");
        doc.set_object_tile(0, 0, "x");
        assert_eq!(doc.objects()[0].rows[0][0].tile, -1);
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
        doc.set_object_tile(0, 0, "7");
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
    fn pngs_round_trip() {
        let image = RgbaImage { width: 2, height: 1, pixels: vec![1, 2, 3, 255, 4, 5, 6, 0] };
        assert_eq!(decode_png(&encode_png(&image).unwrap()).unwrap(), image);
        assert!(decode_png(b"not a png").is_err());
    }
}
