//! The uiStudio resources, the ROM banner and the ROM user metadata (NSMBeUD), as pages of the one graphics editor:
//! JNBL touch rectangles and JNCL graphic placements as tables beside a preview of the 256×192 screen, JNCD cells, OAM
//! and texture, ENPG 256×256 images, the banner's icon and titles, and NitroFS file 131's sections. A port of
//! `UiResourcePages`; the document is pure and keeps the tables as the text the person typed, read back on save, as
//! the Qt tables were.

use crate::controls::{self, Button, Choice as _, Row, TextField, Width};
use bigstar_formats::{
    Bnbl, BnblEntry, Bncd, BncdEntry, BncdSubentry, Bncl, BnclEntry, DsBanner, Enpg, RgbaImage, RomUserMetadata, RomUserMetadataEntry,
    RomUserMetadataSection, UiResourceKind, banner_title, identify_ui_resource, parse_bnbl, parse_bncd, parse_bncl, parse_ds_banner, parse_enpg,
    parse_rom_user_metadata, set_banner_title, write_bnbl, write_bncd, write_bncl, write_ds_banner, write_enpg, write_rom_user_metadata,
};
use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Ui, Vec2};

use crate::graphics_editor::expand;
use crate::history::History;
use crate::paint::{Grid, IndexedSheet};
use crate::palette_panel;
use crate::pixel_editor::{self, Command, HistoryButton, PixelEditor};
use crate::zoom_canvas;

/// The parsed resource.
pub enum Page {
    Bnbl(Bnbl),
    Bncl(Bncl),
    Bncd(Bncd),
    Enpg(Box<Enpg>),
    Banner(Box<DsBanner>),
    Metadata(RomUserMetadata),
}

/// A metadata section as the tree shows it: its name, and its entries' keys and values as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Section {
    pub name: String,
    pub entries: Vec<(String, String)>,
}

/// Which of the BNCD tabs is in front.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BncdTab {
    #[default]
    Cells,
    Oam,
    Texture,
}

pub struct ResourceDoc {
    pub path: String,
    pub page: Option<Page>,
    pub error: Option<String>,
    pub dirty: bool,
    /// Grows with every change to pixels or colours, so the drawn picture is uploaded again.
    pub version: u64,
    /// The JNBL/JNCL table, or JNCD's cells, as text.
    pub rows: Vec<Vec<String>>,
    /// JNCD's OAM entries, as text.
    pub subentries: Vec<Vec<String>>,
    pub titles: Vec<String>,
    pub sections: Vec<Section>,
    /// JNCD's texture read as 8bpp rather than 4bpp.
    pub eight: bool,
    /// Undo for the picture's pixels and colours.
    history: History<(Vec<u8>, Vec<u16>)>,
    pub selected_row: Option<usize>,
    /// The metadata section, and the entry in it, chosen.
    pub selected_item: Option<(usize, Option<usize>)>,
}

pub const BNBL_COLUMNS: [&str; 6] = ["X", "Y", "X align", "Y align", "Width", "Height"];
const BNBL_LIMITS: [u64; 6] = [4095, 4095, 3, 3, 255, 255];
pub const BNCL_COLUMNS: [&str; 5] = ["X", "Y", "X align", "Y align", "Graphic ID"];
const BNCL_LIMITS: [u64; 5] = [4095, 4095, 3, 3, 255];
pub const CELL_COLUMNS: [&str; 4] = ["Width", "Height", "First subentry", "Subentry count"];
const CELL_LIMITS: [u64; 4] = [255, 255, 65535, 65535];
pub const OAM_COLUMNS: [&str; 5] = ["OAM attr 0", "OAM attr 1", "Unknown", "Tile", "Tile count"];
const OAM_LIMITS: [u64; 5] = [65535, 65535, u32::MAX as u64, 65535, 65535];
const LANGUAGES: [&str; 8] = ["Japanese", "English", "French", "German", "Italian", "Spanish", "Chinese", "Korean"];

/// A table cell as a number no bigger than `maximum`: decimal, `0x` hexadecimal or `0`-led octal, as Qt reads it.
pub fn number(text: &str, maximum: u64) -> Result<u64, String> {
    let text = text.trim();
    let parsed = if let Some(hex) = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).ok()
    } else if text.len() > 1 && text.starts_with('0') {
        text.get(1..).and_then(|octal| u64::from_str_radix(octal, 8).ok())
    } else {
        text.parse().ok()
    };
    parsed.filter(|value| *value <= maximum).ok_or_else(|| "table contains an invalid numeric value".to_owned())
}

fn numbers<const N: usize>(row: &[String], limits: [u64; N]) -> Result<[u64; N], String> {
    let mut values = [0; N];
    for (slot, (cell, limit)) in values.iter_mut().zip(row.iter().zip(limits)) {
        *slot = number(cell, limit)?;
    }
    if row.len() < N {
        return Err("table contains an invalid numeric value".to_owned());
    }
    Ok(values)
}

fn texts<const N: usize>(values: [u64; N]) -> Vec<String> {
    values.iter().map(u64::to_string).collect()
}

/// A tile sheet's width in tiles: the square root of its tile count, rounded up.
fn sheet_width(tiles: usize) -> usize {
    ((tiles as f64).sqrt().ceil() as usize).max(1)
}

fn colour(value: u16) -> [u8; 4] {
    [expand(value & 31), expand((value >> 5) & 31), expand((value >> 10) & 31), 255]
}

fn bgr555([r, g, b]: [u8; 3]) -> u16 {
    let five = |channel: u8| (u16::from(channel) * 31 + 127) / 255;
    five(r) | (five(g) << 5) | (five(b) << 10)
}

/// A grey ramp of `colours` BGR555 entries standing in for "index N".
fn grey(colours: usize) -> Vec<u16> {
    (0..colours)
        .map(|i| {
            let channel = u16::try_from(i * 31 / colours.saturating_sub(1).max(1)).unwrap_or(31);
            channel | (channel << 5) | (channel << 10)
        })
        .collect()
}

/// Tiled pixels as a grid, laid out as [`tiled`] draws them.
fn tiled_sheet(bytes: &mut [u8], eight: bool) -> IndexedSheet<'_> {
    let width_tiles = sheet_width(bytes.len() / if eight { 64 } else { 32 });
    IndexedSheet { bytes, width_tiles, eight, linear: false }
}

/// 8×8 tiles laid out in a square-ish sheet.
fn tiled(data: &[u8], eight: bool, colours: &[u16]) -> RgbaImage {
    let tile_size = if eight { 64 } else { 32 };
    let tiles = data.len() / tile_size;
    let width = sheet_width(tiles);
    let height = tiles.div_ceil(width);
    let mut image = RgbaImage { width: width * 8, height: height * 8, pixels: vec![0; width * 8 * height * 8 * 4] };
    for tile in 0..tiles {
        for pixel in 0..64 {
            let index = if eight {
                data.get(tile * 64 + pixel).copied().unwrap_or(0)
            } else {
                (data.get(tile * 32 + pixel / 2).copied().unwrap_or(0) >> ((pixel & 1) * 4)) & 15
            };
            let value = colours.get(usize::from(index).min(colours.len().saturating_sub(1))).copied().unwrap_or(0);
            let (x, y) = ((tile % width) * 8 + pixel % 8, (tile / width) * 8 + pixel / 8);
            let at = (y * image.width + x) * 4;
            if let Some(slot) = image.pixels.get_mut(at..at + 4) {
                slot.copy_from_slice(&colour(value));
            }
        }
    }
    image
}

impl ResourceDoc {
    /// Reads plain bytes as the resource its magic or name says, or as ROM user metadata when the file is NitroFS
    /// file 131.
    pub fn open(path: &str, bytes: &[u8], metadata: bool) -> Self {
        let mut doc = Self {
            path: path.to_owned(),
            page: None,
            error: None,
            dirty: false,
            version: 0,
            rows: Vec::new(),
            subentries: Vec::new(),
            titles: Vec::new(),
            sections: Vec::new(),
            eight: false,
            history: History::default(),
            selected_row: None,
            selected_item: None,
        };
        let parsed = if metadata {
            parse_rom_user_metadata(bytes).map(Page::Metadata)
        } else {
            match identify_ui_resource(bytes, path) {
                UiResourceKind::Bnbl => parse_bnbl(bytes).map(Page::Bnbl),
                UiResourceKind::Bncl => parse_bncl(bytes).map(Page::Bncl),
                UiResourceKind::Bncd => parse_bncd(bytes).map(Page::Bncd),
                UiResourceKind::Enpg => parse_enpg(bytes).map(|e| Page::Enpg(Box::new(e))),
                UiResourceKind::Banner => parse_ds_banner(bytes).map(|b| Page::Banner(Box::new(b))),
                UiResourceKind::Unknown => {
                    Err(bigstar_core::Error::new(bigstar_core::ErrorCode::Unsupported, "file is not a supported M13 UI resource"))
                }
            }
        };
        match parsed {
            Ok(page) => {
                doc.fill(&page);
                doc.page = Some(page);
            }
            Err(error) => doc.error = Some(error.message),
        }
        doc
    }

    /// Whether these pages read a file.
    pub fn supports(bytes: &[u8], path: &str, metadata: bool) -> bool {
        Self::open(path, bytes, metadata).error.is_none()
    }

    fn fill(&mut self, page: &Page) {
        match page {
            Page::Bnbl(bnbl) => {
                self.rows = bnbl
                    .entries
                    .iter()
                    .map(|e| {
                        texts([e.x, e.y, u16::from(e.x_alignment), u16::from(e.y_alignment), u16::from(e.width), u16::from(e.height)].map(u64::from))
                    })
                    .collect();
            }
            Page::Bncl(bncl) => {
                self.rows = bncl
                    .entries
                    .iter()
                    .map(|e| texts([e.x, e.y, u16::from(e.x_alignment), u16::from(e.y_alignment), u16::from(e.graphic_id)].map(u64::from)))
                    .collect();
            }
            Page::Bncd(bncd) => {
                self.rows = bncd
                    .entries
                    .iter()
                    .map(|e| texts([u64::from(e.width), u64::from(e.height), u64::from(e.subentry_start), u64::from(e.subentry_count)]))
                    .collect();
                self.subentries = bncd
                    .subentries
                    .iter()
                    .map(|e| {
                        texts([
                            u64::from(e.oam_attr0),
                            u64::from(e.oam_attr1),
                            u64::from(e.unknown),
                            u64::from(e.tile_number),
                            u64::from(e.tile_count),
                        ])
                    })
                    .collect();
            }
            Page::Banner(banner) => {
                self.titles = (0..banner.title_count).map(|i| String::from_utf16_lossy(&banner_title(banner, i))).collect();
            }
            Page::Metadata(metadata) => {
                self.sections = metadata
                    .sections
                    .iter()
                    .map(|s| Section { name: s.name.clone(), entries: s.entries.iter().map(|e| (e.key.to_string(), e.value.clone())).collect() })
                    .collect();
            }
            Page::Enpg(_) => {}
        }
    }

    /// The name the document goes by.
    pub fn title(&self) -> &'static str {
        "UI Resource"
    }

    /// The columns of the main table, for the pages that have one.
    pub fn columns(&self) -> &'static [&'static str] {
        match &self.page {
            Some(Page::Bnbl(_)) => &BNBL_COLUMNS,
            Some(Page::Bncl(_)) => &BNCL_COLUMNS,
            Some(Page::Bncd(_)) => &CELL_COLUMNS,
            _ => &[],
        }
    }

    /// Adds a JNBL/JNCL row, 16×16 for a touch rectangle and all zero for a placement, and selects it.
    pub fn add_row(&mut self) {
        let row = match &self.page {
            Some(Page::Bnbl(_)) => texts([0, 0, 0, 0, 16, 16]),
            Some(Page::Bncl(_)) => texts([0, 0, 0, 0, 0]),
            _ => return,
        };
        self.rows.push(row);
        self.selected_row = Some(self.rows.len() - 1);
        self.dirty = true;
    }

    pub fn delete_row(&mut self) {
        if !matches!(self.page, Some(Page::Bnbl(_) | Page::Bncl(_))) {
            return;
        }
        if let Some(row) = self.selected_row.filter(|row| *row < self.rows.len()) {
            self.rows.remove(row);
            self.selected_row = None;
            self.dirty = true;
        }
    }

    /// What the screen preview draws: each readable row's rectangle on the 256×192 screen, its label and colour.
    pub fn preview(&self) -> Vec<(Rect, String, [u8; 3])> {
        match &self.page {
            Some(Page::Bnbl(_)) => self
                .rows
                .iter()
                .enumerate()
                .filter_map(|(i, row)| {
                    let [x, y, _, _, w, h] = numbers(row, BNBL_LIMITS).ok()?;
                    let rect = Rect::from_min_size(Pos2::new(x as f32, y as f32), Vec2::new(w as f32, h as f32));
                    Some((rect, i.to_string(), [35, 125, 220]))
                })
                .collect(),
            Some(Page::Bncl(_)) => self
                .rows
                .iter()
                .filter_map(|row| {
                    let [x, y, _, _, id] = numbers(row, BNCL_LIMITS).ok()?;
                    Some((Rect::from_min_size(Pos2::new(x as f32, y as f32), Vec2::splat(16.0)), id.to_string(), [160, 70, 190]))
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// The picture of a page that has one, its palette, and whether that palette is the file's own to edit.
    pub fn picture(&self) -> Option<(RgbaImage, Vec<u16>, bool)> {
        match &self.page {
            Some(Page::Bncd(bncd)) => {
                let colours = grey(if self.eight { 256 } else { 16 });
                Some((tiled(&bncd.texture, self.eight, &colours), colours, false))
            }
            Some(Page::Enpg(enpg)) => {
                let mut image = RgbaImage { width: 256, height: 256, pixels: Vec::with_capacity(256 * 256 * 4) };
                for &index in enpg.pixels.iter().take(256 * 256) {
                    image.pixels.extend(colour(enpg.palette.get(usize::from(index)).copied().unwrap_or(0)));
                }
                image.pixels.resize(256 * 256 * 4, 0);
                Some((image, enpg.palette.to_vec(), true))
            }
            Some(Page::Banner(banner)) => Some((tiled(&banner.icon, false, &banner.palette), banner.palette.to_vec(), true)),
            _ => None,
        }
    }

    /// Writes palette index `value` at a pixel of the picture.
    #[cfg(test)]
    pub fn paint(&mut self, x: usize, y: usize, value: u8) {
        if self.with_sheet(|grid| grid.set(x, y, value)).is_some() {
            self.version += 1;
            self.dirty = true;
        }
    }

    /// Runs `edit` on the picture's pixels as a grid of indices, laid out as the picture draws them.
    pub fn with_sheet<R>(&mut self, edit: impl FnOnce(&mut dyn Grid<Cell = u8>) -> R) -> Option<R> {
        let eight = self.eight;
        Some(match &mut self.page {
            Some(Page::Bncd(bncd)) => edit(&mut tiled_sheet(&mut bncd.texture, eight)),
            Some(Page::Enpg(enpg)) => edit(&mut IndexedSheet { bytes: &mut enpg.pixels, width_tiles: 32, eight: true, linear: true }),
            Some(Page::Banner(banner)) => edit(&mut tiled_sheet(&mut banner.icon, false)),
            _ => return None,
        })
    }

    /// The picture's pixels and colours, as an undo step keeps them.
    fn image_state(&self) -> (Vec<u8>, Vec<u16>) {
        match &self.page {
            Some(Page::Bncd(bncd)) => (bncd.texture.clone(), Vec::new()),
            Some(Page::Enpg(enpg)) => (enpg.pixels.clone(), enpg.palette.to_vec()),
            Some(Page::Banner(banner)) => (banner.icon.to_vec(), banner.palette.to_vec()),
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn set_image_state(&mut self, (pixels, palette): (Vec<u8>, Vec<u16>)) {
        match &mut self.page {
            Some(Page::Bncd(bncd)) => bncd.texture = pixels,
            Some(Page::Enpg(enpg)) => {
                enpg.pixels = pixels;
                enpg.palette.iter_mut().zip(palette).for_each(|(slot, value)| *slot = value);
            }
            Some(Page::Banner(banner)) => {
                banner.icon.iter_mut().zip(pixels).for_each(|(slot, value)| *slot = value);
                banner.palette.iter_mut().zip(palette).for_each(|(slot, value)| *slot = value);
            }
            _ => return,
        }
        self.version += 1;
        self.dirty = true;
    }

    pub fn begin_edit(&mut self, name: &str) {
        if !self.history.is_open() {
            let state = self.image_state();
            self.history.begin(name, &state);
        }
    }

    pub fn end_edit(&mut self) {
        if self.history.is_open() {
            let state = self.image_state();
            self.history.commit(&state);
        }
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn history_names(&self) -> (Vec<String>, Vec<String>) {
        self.history.names()
    }

    pub fn undo(&mut self) {
        let mut state = self.image_state();
        if self.history.undo(&mut state) {
            self.set_image_state(state);
        }
    }

    pub fn redo(&mut self) {
        let mut state = self.image_state();
        if self.history.redo(&mut state) {
            self.set_image_state(state);
        }
    }

    /// Changes a colour of the file's own palette, rounded to BGR555.
    pub fn set_colour(&mut self, index: usize, rgb: [u8; 3]) {
        let before = self.image_state();
        let slot = match &mut self.page {
            Some(Page::Enpg(enpg)) => enpg.palette.get_mut(index),
            Some(Page::Banner(banner)) => banner.palette.get_mut(index),
            _ => None,
        };
        if let Some(slot) = slot
            && *slot != bgr555(rgb)
        {
            *slot = bgr555(rgb);
            self.version += 1;
            self.dirty = true;
            let after = self.image_state();
            self.history.record("Change colour", before, &after);
        }
    }

    /// Adds a metadata section and selects it.
    pub fn add_section(&mut self) {
        self.sections.push(Section { name: "Names".to_owned(), entries: Vec::new() });
        self.selected_item = Some((self.sections.len() - 1, None));
        self.dirty = true;
    }

    /// Adds an entry to the section the selection is in.
    pub fn add_entry(&mut self) {
        let Some((section, _)) = self.selected_item else { return };
        let Some(entries) = self.sections.get_mut(section).map(|s| &mut s.entries) else { return };
        entries.push(("0".to_owned(), "Name".to_owned()));
        self.selected_item = Some((section, Some(entries.len() - 1)));
        self.dirty = true;
    }

    /// Deletes the selected entry, or the selected section with its entries.
    pub fn delete_item(&mut self) {
        match self.selected_item.take() {
            Some((section, Some(entry))) => {
                if let Some(entries) = self.sections.get_mut(section).map(|s| &mut s.entries)
                    && entry < entries.len()
                {
                    entries.remove(entry);
                    self.dirty = true;
                }
            }
            Some((section, None)) if section < self.sections.len() => {
                self.sections.remove(section);
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// Reads what was typed back into the resource.
    fn collect(&mut self) -> Result<(), String> {
        match &mut self.page {
            Some(Page::Bnbl(bnbl)) => {
                let mut entries = Vec::new();
                for (r, row) in self.rows.iter().enumerate() {
                    let [x, y, xa, ya, w, h] = numbers(row, BNBL_LIMITS)?;
                    let old = bnbl.entries.get(r).cloned().unwrap_or_default();
                    entries.push(BnblEntry {
                        x: x as u16,
                        y: y as u16,
                        x_alignment: xa as u8,
                        y_alignment: ya as u8,
                        x_reserved: old.x_reserved,
                        y_reserved: old.y_reserved,
                        width: w as u8,
                        height: h as u8,
                    });
                }
                bnbl.entries = entries;
            }
            Some(Page::Bncl(bncl)) => {
                let mut entries = Vec::new();
                for (r, row) in self.rows.iter().enumerate() {
                    let [x, y, xa, ya, id] = numbers(row, BNCL_LIMITS)?;
                    let old = bncl.entries.get(r).cloned().unwrap_or_default();
                    entries.push(BnclEntry {
                        x: x as u16,
                        y: y as u16,
                        x_alignment: xa as u8,
                        y_alignment: ya as u8,
                        x_reserved: old.x_reserved,
                        y_reserved: old.y_reserved,
                        graphic_id: id as u8,
                        reserved: old.reserved,
                    });
                }
                bncl.entries = entries;
            }
            Some(Page::Bncd(bncd)) => {
                for (entry, row) in bncd.entries.iter_mut().zip(&self.rows) {
                    let [w, h, start, count] = numbers(row, CELL_LIMITS)?;
                    *entry = BncdEntry { width: w as u8, height: h as u8, subentry_start: start as u16, subentry_count: count as u16 };
                }
                for (entry, row) in bncd.subentries.iter_mut().zip(&self.subentries) {
                    let [a0, a1, unknown, tile, count] = numbers(row, OAM_LIMITS)?;
                    *entry = BncdSubentry {
                        oam_attr0: a0 as u16,
                        oam_attr1: a1 as u16,
                        unknown: unknown as u32,
                        tile_number: tile as u16,
                        tile_count: count as u16,
                    };
                }
            }
            Some(Page::Banner(banner)) => {
                for (i, title) in self.titles.iter().enumerate() {
                    let units: Vec<u16> = title.encode_utf16().collect();
                    set_banner_title(banner, i, &units).map_err(|e| e.message)?;
                }
            }
            Some(Page::Metadata(metadata)) => {
                let mut sections = Vec::new();
                for section in &self.sections {
                    let mut entries = Vec::new();
                    for (key, value) in &section.entries {
                        let key = key.trim().parse::<i32>().ok().filter(|k| *k >= 0).ok_or("metadata key must be a non-negative integer")?;
                        entries.push(RomUserMetadataEntry { key, value: value.clone() });
                    }
                    sections.push(RomUserMetadataSection { name: section.name.clone(), entries });
                }
                metadata.sections = sections;
            }
            Some(Page::Enpg(_)) | None => {}
        }
        Ok(())
    }

    /// The edited file, plain; the workspace wraps it the way it came.
    pub fn output(&mut self) -> Result<Vec<u8>, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.collect()?;
        let written = match &self.page {
            Some(Page::Bnbl(bnbl)) => write_bnbl(bnbl),
            Some(Page::Bncl(bncl)) => write_bncl(bncl),
            Some(Page::Bncd(bncd)) => write_bncd(bncd),
            Some(Page::Enpg(enpg)) => write_enpg(enpg),
            Some(Page::Banner(banner)) => write_ds_banner(banner),
            Some(Page::Metadata(metadata)) => write_rom_user_metadata(metadata),
            None => return Err("nothing to write".to_owned()),
        };
        written.map_err(|e| e.message)
    }
}

/// The view's own state: the painting tools and the JNCD tab in front.
#[derive(Default)]
pub struct ResourceView {
    pixels: PixelEditor,
    shown_eight: bool,
    tab: BncdTab,
    banner_titles: bool,
}

/// Whether the person asked to save.
pub fn show(ui: &mut Ui, doc: &mut ResourceDoc, view: &mut ResourceView) -> bool {
    let save = controls::document_header(ui, &doc.path, "", doc.error.is_none() && doc.dirty);
    if let Some(error) = &doc.error {
        controls::error_text(ui, error);
        return save;
    }
    match &doc.page {
        Some(Page::Bnbl(_)) => {
            ui.label("JNBL touch rectangles. Alignment: 0=start, 1=center, 2=end, 3=reserved.");
            layout_page(ui, doc);
        }
        Some(Page::Bncl(_)) => {
            ui.label("JNCL graphic placements. Alignment: 0=start, 1=center, 2=end, 3=reserved.");
            layout_page(ui, doc);
        }
        Some(Page::Bncd(_)) => {
            controls::tabs(ui, "bncd", &mut view.tab, &[(BncdTab::Cells, "Cells"), (BncdTab::Oam, "OAM"), (BncdTab::Texture, "Texture")]);
            match view.tab {
                BncdTab::Cells => {
                    let mut rows = std::mem::take(&mut doc.rows);
                    doc.dirty |= table(ui, "bncd-cells", &CELL_COLUMNS, &mut rows, &mut None);
                    doc.rows = rows;
                }
                BncdTab::Oam => {
                    let mut rows = std::mem::take(&mut doc.subentries);
                    doc.dirty |= table(ui, "bncd-oam", &OAM_COLUMNS, &mut rows, &mut None);
                    doc.subentries = rows;
                }
                BncdTab::Texture => {
                    controls::drop_down("bncd-bpp").selected_text(if doc.eight { "8bpp" } else { "4bpp" }).show_ui(ui, |ui| {
                        ui.choice_value(&mut doc.eight, false, "4bpp");
                        ui.choice_value(&mut doc.eight, true, "8bpp");
                    });
                    image_page(ui, doc, view);
                }
            }
        }
        Some(Page::Enpg(_)) => {
            ui.label("ENPG 256x256 indexed image. Select a palette entry and click pixels to paint, or change the chosen colour below.");
            image_page(ui, doc, view);
        }
        Some(Page::Banner(_)) => {
            controls::tabs(ui, "banner", &mut view.banner_titles, &[(false, "Icon"), (true, "Titles")]);
            if view.banner_titles {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    egui::Grid::new("banner-titles").num_columns(2).show(ui, |ui| {
                        for (i, title) in doc.titles.iter_mut().enumerate() {
                            ui.label(LANGUAGES.get(i).copied().unwrap_or_default());
                            if ui.add(TextField::multiline(title).rows(3).width(Width::Px(420.0))).changed() {
                                doc.dirty = true;
                            }
                            ui.end_row();
                        }
                    });
                });
            } else {
                image_page(ui, doc, view);
            }
        }
        Some(Page::Metadata(_)) => {
            ui.label(
                "Names and object notes for this ROM, kept in file 131. A section named with a number holds the notes for that tileset's objects.",
            );
            metadata_page(ui, doc);
        }
        None => {}
    }
    save
}

/// A table of text cells with a row header to select by; true when a cell changed.
fn table(ui: &mut Ui, id: &str, columns: &[&str], rows: &mut [Vec<String>], selected: &mut Option<usize>) -> bool {
    let mut changed = false;
    egui::ScrollArea::both().id_salt(id).max_height((ui.available_height() - 36.0).max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new(id).num_columns(columns.len() + 1).striped(true).show(ui, |ui| {
            ui.label("");
            for column in columns {
                ui.strong(*column);
            }
            ui.end_row();
            for (r, row) in rows.iter_mut().enumerate() {
                if Row::new(r.to_string()).selected(*selected == Some(r)).fit().show(ui).clicked() {
                    *selected = Some(r);
                }
                for cell in row.iter_mut() {
                    let response = ui.add(TextField::new(cell).width(Width::Px(72.0)));
                    changed |= response.changed();
                    if response.gained_focus() {
                        *selected = Some(r);
                    }
                }
                ui.end_row();
            }
        });
    });
    changed
}

fn layout_page(ui: &mut Ui, doc: &mut ResourceDoc) {
    ui.horizontal_top(|ui| {
        let half = (ui.available_width() / 2.0).max(240.0);
        ui.allocate_ui(Vec2::new(half, ui.available_height()), |ui| {
            ui.vertical(|ui| {
                let mut rows = std::mem::take(&mut doc.rows);
                let mut selected = doc.selected_row;
                doc.dirty |= table(ui, "layout-table", doc.columns(), &mut rows, &mut selected);
                doc.rows = rows;
                doc.selected_row = selected;
                ui.horizontal(|ui| {
                    if Button::new("Add").show(ui).clicked() {
                        doc.add_row();
                    }
                    if Button::new("Delete").enabled(doc.selected_row.is_some()).show(ui).clicked() {
                        doc.delete_row();
                    }
                });
            });
        });
        // The 256×192 screen, scaled up to the room there is.
        let width = ui.available_width().clamp(256.0, 512.0);
        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, width * 0.75), egui::Sense::hover());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "LayoutPreview"));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let scale = rect.width() / 256.0;
        for (area, label, [r, g, b]) in doc.preview() {
            let shown = Rect::from_min_size(rect.min + area.min.to_vec2() * scale, area.size() * scale);
            painter.rect(shown, 0.0, Color32::from_rgba_unmultiplied(r, g, b, 55), Stroke::new(1.0, Color32::from_rgb(r, g, b)), StrokeKind::Inside);
            painter.text(shown.center(), egui::Align2::CENTER_CENTER, label, egui::FontId::proportional(11.0), ui.visuals().strong_text_color());
        }
    });
}

fn image_page(ui: &mut Ui, doc: &mut ResourceDoc, view: &mut ResourceView) {
    if view.shown_eight != doc.eight {
        // The same bytes read at another depth are another picture.
        view.shown_eight = doc.eight;
        doc.version += 1;
    }
    let Some((image, palette, editable)) = doc.picture() else { return };
    let colours: Vec<[u8; 4]> = palette.iter().map(|&c| colour(c)).collect();
    let row = if colours.len() > 16 && doc.eight || matches!(doc.page, Some(Page::Enpg(_))) { 256 } else { 16 };
    egui::Panel::right(egui::Id::new(("resource-palette", &doc.path))).resizable(true).default_size(300.0).show(ui, |ui| {
        ui.strong("Palette");
        let edited = egui::ScrollArea::vertical().show(ui, |ui| palette_panel::show(ui, &colours, &mut view.pixels.colours, row, editable)).inner;
        if let Some((index, rgb)) = edited {
            doc.set_colour(index, rgb);
        }
    });
    egui::CentralPanel::default().show(ui, |ui| {
        let mut commands = Vec::new();
        match view.pixels.toolbar(ui, Some((doc.can_undo(), doc.can_redo())), &mut commands) {
            Some(HistoryButton::Undo) => doc.undo(),
            Some(HistoryButton::Redo) => doc.redo(),
            None => {}
        }
        let input = view.pixels.canvas(ui, &doc.path, &image, doc.version, ui.available_height() - zoom_canvas::below(ui));
        view.pixels.keys(ui, &input, &mut commands);
        ui.weak(pixel_editor::position_text(view.pixels.hover(), &image));
        if input.pressed().is_some() || commands.iter().any(Command::edits) {
            doc.begin_edit(view.pixels.step_name(&commands));
        }
        let ctx = ui.ctx().clone();
        if doc.with_sheet(|grid| view.pixels.apply(&ctx, &input, &commands, grid, row)) == Some(true) {
            doc.version += 1;
            doc.dirty = true;
        }
        if input.released().is_some() || commands.iter().any(Command::edits) {
            doc.end_edit();
        }
    });
}

fn metadata_page(ui: &mut Ui, doc: &mut ResourceDoc) {
    let mut changed = false;
    let mut chosen = None;
    egui::ScrollArea::vertical().max_height((ui.available_height() - 36.0).max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("metadata-tree").num_columns(3).striped(true).show(ui, |ui| {
            ui.strong("Section");
            ui.strong("Key");
            ui.strong("Value");
            ui.end_row();
            for (s, section) in doc.sections.iter_mut().enumerate() {
                let response = ui.add(TextField::new(&mut section.name).width(Width::Px(140.0)));
                changed |= response.changed();
                if response.gained_focus() || response.clicked() {
                    chosen = Some((s, None));
                }
                if Row::new("section").selected(doc.selected_item == Some((s, None))).fit().show(ui).clicked() {
                    chosen = Some((s, None));
                }
                ui.label("");
                ui.end_row();
                for (e, (key, value)) in section.entries.iter_mut().enumerate() {
                    if Row::new("•").selected(doc.selected_item == Some((s, Some(e)))).fit().show(ui).clicked() {
                        chosen = Some((s, Some(e)));
                    }
                    let key = ui.add(TextField::new(key).width(Width::Px(60.0)));
                    let value = ui.add(TextField::new(value).width(Width::Px(320.0)));
                    changed |= key.changed() || value.changed();
                    if key.gained_focus() || value.gained_focus() {
                        chosen = Some((s, Some(e)));
                    }
                    ui.end_row();
                }
            }
        });
    });
    if let Some(item) = chosen {
        doc.selected_item = Some(item);
    }
    doc.dirty |= changed;
    ui.horizontal(|ui| {
        if Button::new("Add section").show(ui).clicked() {
            doc.add_section();
        }
        if Button::new("Add entry").enabled(doc.selected_item.is_some()).show(ui).clicked() {
            doc.add_entry();
        }
        if Button::new("Delete").enabled(doc.selected_item.is_some()).show(ui).clicked() {
            doc.delete_item();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const BNBL: [u8; 14] = [b'J', b'N', b'B', b'L', 0, 0, 1, 0, 1, 0, 2, 0, 16, 12];

    #[test]
    fn ui_resource_editors_preserve_untouched_files() {
        assert!(ResourceDoc::supports(&BNBL, "touch.bnbl", false));
        let mut layout = ResourceDoc::open("touch.bnbl", &BNBL, false);
        assert_eq!(layout.rows.len(), 1);
        assert_eq!(layout.rows[0], ["1", "2", "0", "0", "16", "12"]);
        assert_eq!(layout.output().unwrap(), BNBL);

        let enpg = vec![0u8; 0x10200];
        assert!(ResourceDoc::supports(&enpg, "loading.enpg", false));
        let mut image = ResourceDoc::open("loading.enpg", &enpg, false);
        assert!(image.picture().is_some(), "an ENPG is a picture");
        assert_eq!(image.output().unwrap(), enpg);

        let metadata = b"[Music]\n0=Overworld\n".to_vec();
        let mut notes = ResourceDoc::open("NSMBeUD", &metadata, true);
        assert_eq!(notes.sections, [Section { name: "Music".into(), entries: vec![("0".into(), "Overworld".into())] }]);
        assert_eq!(notes.output().unwrap(), metadata);
        // The same text is not metadata unless it is file 131.
        assert!(!ResourceDoc::supports(&metadata, "NSMBeUD", false));
    }

    #[test]
    fn layout_rows_are_edited_added_and_checked() {
        let mut layout = ResourceDoc::open("touch.bnbl", &BNBL, false);
        layout.rows[0][0] = "0x20".into();
        layout.add_row();
        assert_eq!(layout.selected_row, Some(1));
        let written = layout.output().unwrap();
        let parsed = parse_bnbl(&written).unwrap();
        assert_eq!(parsed.entries.len(), 2);
        assert_eq!((parsed.entries[0].x, parsed.entries[1].width, parsed.entries[1].height), (0x20, 16, 16));
        let preview = layout.preview();
        assert_eq!(preview.len(), 2);
        assert_eq!(preview[0].0, Rect::from_min_size(Pos2::new(32.0, 2.0), Vec2::new(16.0, 12.0)));

        layout.rows[1][2] = "4".into();
        assert_eq!(layout.output().unwrap_err(), "table contains an invalid numeric value", "alignment is two bits");
        assert_eq!(layout.preview().len(), 1, "an unreadable row is not drawn");
        layout.delete_row();
        assert_eq!(layout.rows.len(), 1);
        assert!(layout.output().is_ok());
    }

    #[test]
    fn numbers_read_the_way_qt_reads_them() {
        assert_eq!(number(" 42 ", 255), Ok(42));
        assert_eq!(number("0x1F", 255), Ok(31));
        assert_eq!(number("010", 255), Ok(8));
        assert!(number("256", 255).is_err());
        assert!(number("-1", 255).is_err());
        assert!(number("", 255).is_err());
    }

    #[test]
    fn an_enpg_paints_and_recolours() {
        let mut doc = ResourceDoc::open("loading.enpg", &vec![0u8; 0x10200], false);
        doc.paint(5, 7, 3);
        doc.set_colour(3, [255, 0, 0]);
        let written = doc.output().unwrap();
        let parsed = parse_enpg(&written).unwrap();
        assert_eq!(parsed.pixels[7 * 256 + 5], 3);
        assert_eq!(parsed.palette[3], 31);
        let (image, _, editable) = doc.picture().unwrap();
        assert!(editable);
        let at = (7 * 256 + 5) * 4;
        assert_eq!(image.pixels[at..at + 4], [255, 0, 0, 255]);
    }

    #[test]
    fn a_tiled_sheet_paints_in_both_depths() {
        let mut data = vec![0u8; 64];
        tiled_sheet(&mut data, false).set(1, 0, 0x0A);
        assert_eq!(data[0], 0xA0, "4bpp keeps the low nibble and the odd pixel's high one");
        // Two 4bpp tiles sit side by side.
        tiled_sheet(&mut data, false).set(8, 0, 2);
        assert_eq!(data[32], 2);
        let mut eight = vec![0u8; 128];
        tiled_sheet(&mut eight, true).set(9, 1, 200);
        assert_eq!(eight[64 + 9], 200);
        assert_eq!(grey(16)[15], 0x7fff);
        assert_eq!(tiled(&data, false, &grey(16)).width, 16);
    }

    fn banner() -> Vec<u8> {
        let mut bytes = vec![0u8; 0x840];
        bytes[0] = 1;
        for (i, unit) in "Mario".encode_utf16().enumerate() {
            bytes[0x340 + i * 2..0x342 + i * 2].copy_from_slice(&unit.to_le_bytes());
        }
        bytes
    }

    #[test]
    fn the_banner_edits_its_titles_and_icon() {
        let bytes = banner();
        let mut doc = ResourceDoc::open("banner.bin", &bytes, false);
        assert_eq!(doc.titles.len(), 6);
        assert_eq!(doc.titles[1], "Mario");
        doc.titles[1] = "Luigi's ✓".into();
        doc.paint(0, 0, 0);
        let written = doc.output().unwrap();
        let parsed = parse_ds_banner(&written).unwrap();
        assert_eq!(String::from_utf16_lossy(&banner_title(&parsed, 1)), "Luigi's ✓");
        assert_eq!(parsed.icon[0] & 15, 0);
        assert_eq!(doc.picture().unwrap().0.width, 32, "a 4×4 tile icon");

        doc.titles[0] = "x".repeat(129);
        assert!(doc.output().is_err(), "a title has room for 128 units");
    }

    #[test]
    fn metadata_sections_and_entries_are_added_and_checked() {
        let mut notes = ResourceDoc::open("NSMBeUD", b"[Music]\n0=Overworld\n", true);
        notes.selected_item = Some((0, None));
        notes.add_entry();
        assert_eq!(notes.selected_item, Some((0, Some(1))));
        notes.sections[0].entries[1] = ("3".into(), "Castle".into());
        notes.add_section();
        notes.sections[1].name = "12".into();
        notes.add_entry();
        let written = String::from_utf8(notes.output().unwrap()).unwrap();
        assert!(written.contains("[Music]\n0=Overworld\n3=Castle\n"), "{written}");
        assert!(written.contains("[12]\n0=Name\n"), "{written}");

        notes.sections[1].entries[0].0 = "-1".into();
        assert_eq!(notes.output().unwrap_err(), "metadata key must be a non-negative integer");
        notes.selected_item = Some((1, None));
        notes.delete_item();
        assert_eq!(notes.sections.len(), 1);
        assert!(notes.output().is_ok());
    }

    #[test]
    fn a_bncd_edits_its_tables_and_paints_its_texture() {
        // One cell at 0x1c using the one OAM entry at 0x24, and one 4bpp tile at 0x30.
        let mut bytes = b"JNCD".to_vec();
        bytes.resize(0x50, 0);
        bytes[6] = 1;
        for (at, value) in [(8usize, 0x1cu32), (12, 0x24), (16, 0x30), (20, 0x20)] {
            bytes[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes[0x1c..0x22].copy_from_slice(&[2, 1, 0, 0, 1, 0]);
        bytes[0x2c] = 7;
        let mut doc = ResourceDoc::open("a.bncd", &bytes, false);
        assert_eq!(doc.rows, [["2", "1", "0", "1"]]);
        assert_eq!(doc.subentries, [["0", "0", "0", "7", "0"]]);
        assert_eq!(doc.output().unwrap(), bytes);

        doc.subentries[0][3] = "9".into();
        doc.paint(0, 0, 5);
        let parsed = parse_bncd(&doc.output().unwrap()).unwrap();
        assert_eq!(parsed.subentries[0].tile_number, 9);
        assert_eq!(parsed.texture[0], 5);
        let (_, palette, editable) = doc.picture().unwrap();
        assert_eq!((palette.len(), editable), (16, false), "a grey ramp, not the file's");
        doc.eight = true;
        assert_eq!(doc.picture().unwrap().1.len(), 256);
    }
}
