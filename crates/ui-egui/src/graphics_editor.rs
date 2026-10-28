//! The graphics editor: every NNS G2D format and TEX0, as a document. A palette lives in a different file from the
//! graphics it colours and a screen map in a third, and any of them can go with any other, so the person attaches the
//! palette and graphics a file is shown with, and the pairing is remembered per file. A port of `GraphicsEditorPanel` and `GraphicsCanvas`; the model here is pure, `show` draws it.

use crate::controls::{self, Choice as _};
use bigstar_formats::{
    GraphicsFormatKind, Nanr, Ncer, Ncgr, Nclr, Nscr, RgbaImage, ScreenMapEntry, Tex0, Tex0Palette, Tex0Texture, TextureFormat, bgr555_to_rgb,
    decode_bgr555_palette, identify_graphics_format, pack_bgr555, parse_nanr, parse_ncer, parse_ncgr, parse_nclr, parse_nscr, parse_tex0,
    render_ncer_cell, render_tex0_texture, write_nanr, write_ncer, write_ncgr, write_nclr, write_nscr, write_tex0,
};
use egui::Ui;
use serde::{Deserialize, Serialize};

use crate::editors::{rewrap, unwrap_with};
use crate::files::DraggedFile;
use crate::history::{History, Snapshots, Undoable};
use crate::image_manager::{self, Bitmap, Group, Images, Lists, ManagerAction, ManagerState};
use crate::paint::{Grid, IndexedSheet, Pattern, RawScreen};
use crate::palette_panel;
use crate::pixel_editor::{self, Command, HistoryButton, PixelEditor};
use crate::services::Services;
use crate::theme::Tokens;
use crate::tile_picker::{self, PickerState};
use crate::tilemap_editor::TilemapEditor;
use crate::zoom_canvas::{self, CanvasState, Steps};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Unsupported,
    Nclr,
    Ncgr,
    Nscr,
    Ncer,
    Nanr,
    Tex0,
    RawPalette,
    RawCharacters,
    RawScreen,
}

/// What the person paired a file with, kept per file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pairing {
    /// The palette files attached, in the order they are listed: graphics are drawn in any number of them.
    #[serde(default)]
    pub palettes: Vec<String>,
    /// The one of them the graphics are drawn in.
    #[serde(default)]
    pub shown: Option<String>,
    /// The one palette file a pairing used to hold, read into `palettes`.
    #[serde(default, skip_serializing)]
    pub palette: Option<String>,
    pub characters: Option<String>,
    pub width: usize,
}

impl Pairing {
    /// The palette files to read back, with what an older pairing held.
    pub fn palette_files(&self) -> Vec<String> {
        if self.palettes.is_empty() { self.palette.iter().cloned().collect() } else { self.palettes.clone() }
    }
}

/// A palette file attached to the graphics: as read, and its colours as edited here.
#[derive(Clone)]
struct AttachedPalette {
    path: String,
    /// The file as read, and its NCLR when it is one: what edited colours are written back into.
    original: Vec<u8>,
    nclr: Option<Nclr>,
    colours: Vec<u8>,
    /// Its colours were changed here, so a save writes it too.
    edited: bool,
}

impl AttachedPalette {
    fn name(&self) -> &str {
        self.path.rsplit(['/', '!']).next().unwrap_or(&self.path)
    }
}

pub fn pairing_key(path: &str) -> String {
    format!("graphicsEditor/{path}")
}

/// The palette file named as `path` is, beside it: `x_ncl.bin` for `x_ncg.bin`, `x.nclr` for `x.ncgr`, and so on for
/// screens and cells. What a file is paired with before the person says otherwise; any other palette can be added.
pub fn namesake_palette<'a>(path: &str, files: impl IntoIterator<Item = &'a str>) -> Option<String> {
    const SUFFIXES: [&str; 8] = ["_ncg.bin", "_nsc.bin", "_nce.bin", "_nan.bin", ".ncgr", ".nscr", ".ncer", ".nanr"];
    namesake(path, &SUFFIXES, ["_ncl.bin", ".nclr"], files)
}

/// The graphics file named as the screen or cells `path` is, beside it: `x_ncg.bin` for `x_nsc.bin`, `x.ncgr` for
/// `x.nscr`. Like [`namesake_palette`], only where pairing starts.
pub fn namesake_graphics<'a>(path: &str, files: impl IntoIterator<Item = &'a str>) -> Option<String> {
    const SUFFIXES: [&str; 6] = ["_nsc.bin", "_nce.bin", "_nan.bin", ".nscr", ".ncer", ".nanr"];
    namesake(path, &SUFFIXES, ["_ncg.bin", ".ncgr"], files)
}

/// The file in `files` that is `path` with one of `suffixes` swapped for one of `wanted`, whatever the case.
fn namesake<'a>(path: &str, suffixes: &[&str], wanted: [&str; 2], files: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let lower = path.to_ascii_lowercase();
    let stem = suffixes.iter().find_map(|suffix| lower.strip_suffix(suffix))?;
    let wanted = wanted.map(|suffix| format!("{stem}{suffix}"));
    files.into_iter().find(|file| wanted.iter().any(|w| file.eq_ignore_ascii_case(w))).map(str::to_owned)
}

/// Colour `index` of BGR555 palette bytes, opaque, as 8-bit channels.
fn unpack(bytes: &[u8], index: usize) -> [u8; 4] {
    let (Some(&low), Some(&high)) = (bytes.get(index * 2), bytes.get(index * 2 + 1)) else { return [0, 0, 0, 255] };
    let [r, g, b] = bgr555_to_rgb(u16::from_le_bytes([low, high]));
    [r, g, b, 255]
}

/// Writes 8-bit channels into colour `index` of BGR555 palette bytes, rounding to the nearest step.
pub fn pack(bytes: &mut [u8], index: usize, [r, g, b]: [u8; 3]) {
    let packed = pack_bgr555(r, g, b);
    if let Some(target) = bytes.get_mut(index * 2..index * 2 + 2) {
        target.copy_from_slice(&packed.to_le_bytes());
    }
}

/// A screen map's width in entries: the declared one in pixels when there is one, otherwise the square root.
fn screen_width(entries: usize, declared_pixels: usize) -> usize {
    if declared_pixels >= 8 { declared_pixels / 8 } else { sheet_width(entries, 0) }.max(1)
}

/// A sheet's width in tiles: the declared one when it is real, otherwise the square root, rounded up.
fn sheet_width(tiles: usize, declared: usize) -> usize {
    if declared != 0 && declared != 0xffff && declared <= tiles {
        return declared;
    }
    ((tiles as f64).sqrt().ceil() as usize).max(1)
}

fn blank(width: usize, height: usize) -> RgbaImage {
    RgbaImage { width, height, pixels: vec![0; width * height * 4] }
}

fn put(image: &mut RgbaImage, x: usize, y: usize, rgba: [u8; 4]) {
    if x < image.width && y < image.height {
        let at = (y * image.width + x) * 4;
        if let Some(pixel) = image.pixels.get_mut(at..at + 4) {
            pixel.copy_from_slice(&rgba);
        }
    }
}

/// HSV with the hue in degrees and saturation and value out of 255, as `QColor::fromHsv`.
fn hsv(hue: u32, saturation: u8, value: u8) -> [u8; 4] {
    let (s, v) = (f32::from(saturation) / 255.0, f32::from(value) / 255.0);
    let h = (hue % 360) as f32 / 60.0;
    let c = v * s;
    let x = c * (1.0 - (h % 2.0 - 1.0).abs());
    let (r, g, b) = match h as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let channel = |f: f32| ((f + m) * 255.0).round() as u8;
    [channel(r), channel(g), channel(b), 255]
}

/// What the editor shows for the state it is in.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    pub image: Option<RgbaImage>,
    /// A grid every this many pixels, when they are big enough on screen to tell apart.
    pub grid: usize,
    pub info: String,
    pub swatches: Vec<[u8; 4]>,
    /// Whether a swatch's colour can be changed here: only when the palette is this file's own.
    pub swatches_editable: bool,
}

/// What an edit can change, for undo.
#[derive(Clone, PartialEq)]
pub struct Contents {
    body: Vec<u8>,
    nclr: Option<Nclr>,
    ncgr: Option<Ncgr>,
    nscr: Option<Nscr>,
    tex0: Option<Tex0>,
    /// The attached palettes' colours, with the files they are: undoing into one only while it is still attached.
    companion: Vec<(String, Vec<u8>)>,
}

pub struct GraphicsDoc {
    pub path: String,
    original: Vec<u8>,
    pub body: Vec<u8>,
    pub mode: Mode,
    pub error: Option<String>,
    nclr: Option<Nclr>,
    ncgr: Option<Ncgr>,
    nscr: Option<Nscr>,
    ncer: Option<Ncer>,
    nanr: Option<Nanr>,
    tex0: Option<Tex0>,
    /// The cell, sequence, texture or (for a raw sheet) bit depth shown.
    pub primary: usize,
    /// The TEX0 palette.
    pub secondary: usize,
    pub pairing: Pairing,
    /// The palette files attached, in the pairing's order.
    palettes: Vec<AttachedPalette>,
    companion_characters: Vec<u8>,
    companion_8bpp: bool,
    pub companion_error: Option<String>,
    /// The palette a sheet is drawn and painted with: a row of sixteen colours at 4bpp, a block of 256 at 8bpp, of
    /// the attached file shown.
    pub palette_row: usize,
    history: History<Contents>,
    /// Grows with every change, so a drawn texture knows when it is stale.
    pub version: u64,
    pub dirty: bool,
}

impl Snapshots for GraphicsDoc {
    type State = Contents;

    fn history(&self) -> &History<Contents> {
        &self.history
    }

    fn history_mut(&mut self) -> &mut History<Contents> {
        &mut self.history
    }

    fn snapshot(&self) -> Contents {
        self.contents()
    }

    fn restore(&mut self, contents: Contents) {
        let companion;
        Contents { body: self.body, nclr: self.nclr, ncgr: self.ncgr, nscr: self.nscr, tex0: self.tex0, companion } = contents;
        for (path, colours) in companion {
            if let Some(palette) = self.palettes.iter_mut().find(|p| p.path == path && p.colours != colours) {
                palette.colours = colours;
                palette.edited = true;
            }
        }
        self.touched(true);
    }
}

impl GraphicsDoc {
    pub fn open(path: &str, bytes: &[u8]) -> Self {
        let mut doc = Self {
            path: path.to_owned(),
            original: bytes.to_vec(),
            body: unwrap_with(bytes).1,
            mode: Mode::Unsupported,
            error: None,
            nclr: None,
            ncgr: None,
            nscr: None,
            ncer: None,
            nanr: None,
            tex0: None,
            primary: 0,
            secondary: 0,
            pairing: Pairing::default(),
            palettes: Vec::new(),
            companion_characters: Vec::new(),
            companion_8bpp: false,
            companion_error: None,
            palette_row: 0,
            history: History::default(),
            version: 0,
            dirty: false,
        };
        doc.parse();
        doc
    }

    /// Whether this editor understands a file at all.
    pub fn supports(bytes: &[u8], path: &str) -> bool {
        Self::open(path, bytes).error.is_none()
    }

    fn parse(&mut self) {
        let body = &self.body;
        let fail = |message: String| (Mode::Unsupported, Some(message));
        let (mode, error) = match identify_graphics_format(body, &self.path) {
            GraphicsFormatKind::NitroPalette => match parse_nclr(body) {
                Ok(value) => {
                    self.nclr = Some(value);
                    (Mode::Nclr, None)
                }
                Err(e) => fail(e.message),
            },
            GraphicsFormatKind::NitroCharacterGraphics => match parse_ncgr(body) {
                Ok(value) => {
                    self.ncgr = Some(value);
                    (Mode::Ncgr, None)
                }
                Err(e) => fail(e.message),
            },
            GraphicsFormatKind::NitroScreenMap => match parse_nscr(body) {
                Ok(value) => {
                    self.nscr = Some(value);
                    (Mode::Nscr, None)
                }
                Err(e) => fail(e.message),
            },
            GraphicsFormatKind::NitroCellLayout => match parse_ncer(body) {
                Ok(value) => {
                    self.ncer = Some(value);
                    (Mode::Ncer, None)
                }
                Err(e) => fail(e.message),
            },
            GraphicsFormatKind::NitroCellAnimation => match parse_nanr(body) {
                Ok(value) => {
                    self.nanr = Some(value);
                    (Mode::Nanr, None)
                }
                Err(e) => fail(e.message),
            },
            GraphicsFormatKind::NitroTexture | GraphicsFormatKind::NitroModel => match parse_tex0(body) {
                Ok(value) => {
                    self.tex0 = Some(value);
                    (Mode::Tex0, None)
                }
                Err(e) => fail(e.message),
            },
            GraphicsFormatKind::RawPaletteCandidate if body.is_empty() || !body.len().is_multiple_of(2) => {
                fail("raw palette size must be even".to_owned())
            }
            GraphicsFormatKind::RawPaletteCandidate => (Mode::RawPalette, None),
            GraphicsFormatKind::RawCharacterGraphicsCandidate if body.is_empty() || !body.len().is_multiple_of(32) => {
                fail("raw 4bpp graphics must contain complete tiles".to_owned())
            }
            GraphicsFormatKind::RawCharacterGraphicsCandidate => (Mode::RawCharacters, None),
            GraphicsFormatKind::RawScreenMapCandidate if body.is_empty() || !body.len().is_multiple_of(2) => {
                fail("raw screen map size must be even".to_owned())
            }
            GraphicsFormatKind::RawScreenMapCandidate => (Mode::RawScreen, None),
            GraphicsFormatKind::Unknown => fail("this file is not a supported Nintendo DS graphics resource".to_owned()),
        };
        self.mode = mode;
        self.error = error;
    }

    /// Bumps the version and marks the file changed.
    pub(crate) fn touched(&mut self, edited: bool) {
        self.version += 1;
        self.dirty |= edited;
    }

    pub fn pairs_palette(&self) -> bool {
        matches!(self.mode, Mode::Ncgr | Mode::RawCharacters | Mode::Nscr | Mode::RawScreen | Mode::Ncer | Mode::Nanr)
    }

    pub fn pairs_characters(&self) -> bool {
        matches!(self.mode, Mode::Nscr | Mode::RawScreen | Mode::Ncer | Mode::Nanr)
    }

    /// A headerless sheet carries no width, so it is asked for.
    pub fn uses_width(&self) -> bool {
        matches!(self.mode, Mode::RawCharacters | Mode::RawScreen)
    }

    pub fn primary_choices(&self) -> Vec<String> {
        match self.mode {
            Mode::Ncer => (0..self.ncer.as_ref().map_or(0, |n| n.cells.len())).map(|i| format!("Cell {i}")).collect(),
            Mode::Nanr => (0..self.nanr.as_ref().map_or(0, |n| n.sequences.len())).map(|i| format!("Sequence {i}")).collect(),
            Mode::RawCharacters => vec!["4bpp".to_owned(), "8bpp".to_owned()],
            Mode::Tex0 => self.tex0.as_ref().map(|t| t.textures.iter().map(|x| x.name.clone()).collect()).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn secondary_choices(&self) -> Vec<String> {
        match self.mode {
            Mode::Tex0 => self.tex0.as_ref().map(|t| t.palettes.iter().map(|p| p.name.clone()).collect()).unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    pub fn set_primary(&mut self, index: usize) {
        self.primary = index;
        self.touched(false);
    }

    pub fn set_secondary(&mut self, index: usize) {
        self.secondary = index;
        self.touched(false);
    }

    pub fn set_width(&mut self, tiles: usize) {
        self.pairing.width = tiles;
        self.touched(false);
    }

    /// Whether a file attached here, without saying as what, is taken as the palette (`true`) or the graphics
    /// (`false`): by what it is, and a file of no known kind as whichever of the two this file pairs. `None` when it
    /// could be either, or neither is wanted.
    pub fn attaches_as(&self, path: &str, bytes: &[u8]) -> Option<bool> {
        let (palette, characters) = (self.pairs_palette(), self.pairs_characters());
        match identify_graphics_format(&unwrap_with(bytes).1, path) {
            GraphicsFormatKind::NitroPalette | GraphicsFormatKind::RawPaletteCandidate => palette.then_some(true),
            GraphicsFormatKind::NitroCharacterGraphics | GraphicsFormatKind::RawCharacterGraphicsCandidate => characters.then_some(false),
            GraphicsFormatKind::Unknown if palette && !characters => Some(true),
            _ => None,
        }
    }

    /// Attaches a file as the palette or the graphics, as [`GraphicsDoc::attaches_as`] decides; says why not otherwise.
    pub fn attach(&mut self, path: &str, bytes: &[u8]) {
        match self.attaches_as(path, bytes) {
            Some(true) => self.attach_palette(path, bytes),
            Some(false) => self.set_companion_characters(Some(path.to_owned()), Some(bytes)),
            None => {
                let name = path.rsplit(['/', '!']).next().unwrap_or(path);
                self.companion_error = Some(if self.pairs_characters() {
                    format!("{name} was not attached: drop it on the Graphics file row or the palette list to say which it is.")
                } else if self.pairs_palette() {
                    format!("{name} is not a palette: drop it on the palette list to read it as one.")
                } else {
                    format!("{name} was not attached: this file is drawn without a palette or graphics file.")
                });
                self.touched(false);
            }
        }
    }

    /// Takes back what the file was paired with last time: the palette files arrive later, in [`Self::attach_palette`].
    pub fn restore_pairing(&mut self, pairing: &Pairing) {
        self.pairing.palettes = pairing.palette_files();
        self.pairing.shown = pairing.shown.clone();
        self.set_width(pairing.width);
    }

    /// Adds a palette file to the ones the graphics can be drawn in; an NCLR gives its palette, anything else is taken
    /// as raw BGR555. One attached again is read again in its place. A new one is shown; one the pairing remembered
    /// arrives without changing which is.
    pub fn attach_palette(&mut self, path: &str, bytes: &[u8]) {
        let plain = unwrap_with(bytes).1;
        let nclr = parse_nclr(&plain).ok();
        let colours = nclr.as_ref().map_or(plain, |nclr| nclr.palette.clone());
        let attached = AttachedPalette { path: path.to_owned(), original: bytes.to_vec(), nclr, colours, edited: false };
        let loaded = self.palettes.iter().any(|p| p.path == path);
        let remembered = self.pairing.palettes.iter().any(|p| p == path);
        if !remembered {
            self.pairing.palettes.push(path.to_owned());
        }
        if !remembered || loaded {
            self.pairing.shown = Some(path.to_owned());
            self.palette_row = 0;
        }
        self.palettes.retain(|p| p.path != path);
        self.palettes.push(attached);
        let order = |path: &str| self.pairing.palettes.iter().position(|p| p == path);
        let mut palettes = std::mem::take(&mut self.palettes);
        palettes.sort_by_key(|p| order(&p.path));
        self.palettes = palettes;
        self.companion_error = None;
        self.touched(false);
    }

    /// Takes palette file `index` off the list; the graphics are drawn in the first one left, or in greys.
    pub fn detach_palette(&mut self, index: usize) {
        if index >= self.palettes.len() {
            return;
        }
        let removed = self.palettes.remove(index);
        self.pairing.palettes.retain(|p| *p != removed.path);
        if self.pairing.shown.as_deref() == Some(removed.path.as_str()) {
            self.pairing.shown = None;
            self.palette_row = 0;
        }
        self.touched(false);
    }

    /// The palette files attached, by name.
    pub fn palette_files(&self) -> Vec<&str> {
        self.palettes.iter().map(|p| p.path.as_str()).collect()
    }

    /// Which attached palette file the graphics are drawn in.
    pub fn shown_palette(&self) -> usize {
        self.pairing.shown.as_ref().and_then(|shown| self.palettes.iter().position(|p| &p.path == shown)).unwrap_or(0)
    }

    /// Draws the graphics in attached palette file `index`, in its row `row`.
    pub fn show_palette(&mut self, index: usize, row: usize) {
        let Some(path) = self.palettes.get(index).map(|p| p.path.clone()) else { return };
        if index != self.shown_palette() || row != self.palette_row {
            self.pairing.shown = Some(path);
            self.palette_row = row;
            self.touched(false);
        }
    }

    /// The colours of the palette file shown, empty when none is attached.
    fn shown_colours(&self) -> &[u8] {
        self.palettes.get(self.shown_palette()).map_or(&[], |p| p.colours.as_slice())
    }

    /// The attached palette files edited here, as they are to be written: their paths and bytes.
    pub fn companion_outputs(&self) -> Result<Vec<(String, Vec<u8>)>, String> {
        let mut outputs = Vec::new();
        for palette in self.palettes.iter().filter(|p| p.edited) {
            let body = match &palette.nclr {
                Some(nclr) => write_nclr(&Nclr { palette: palette.colours.clone(), ..nclr.clone() }).map_err(|e| e.message)?,
                None => palette.colours.clone(),
            };
            outputs.push((palette.path.clone(), rewrap(&palette.original, body)?));
        }
        Ok(outputs)
    }

    /// The file, and the palette files edited here, were written.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
        for palette in &mut self.palettes {
            palette.edited = false;
        }
    }

    /// How many rows (or blocks) of the palette size attached file `index` holds.
    fn rows_in(&self, index: usize) -> usize {
        self.palettes.get(index).map_or(1, |p| (p.colours.len() / 2).div_ceil(self.palette_size()).max(1))
    }

    /// Every palette the graphics can be drawn in, as the Palettes list lists them: each row of each attached file.
    fn palette_slots(&self) -> Vec<(usize, usize)> {
        (0..self.palettes.len()).flat_map(|file| (0..self.rows_in(file)).map(move |row| (file, row))).collect()
    }

    /// How many colours one palette of the list holds: a 4bpp row of sixteen, an 8bpp block of 256, or a whole
    /// palette file or TEX0 palette.
    pub fn palette_size(&self) -> usize {
        match self.mode {
            Mode::Ncgr | Mode::RawCharacters => {
                if self.eight_bits() {
                    256
                } else {
                    16
                }
            }
            Mode::Nscr | Mode::RawScreen | Mode::Ncer | Mode::Nanr => {
                if self.companion_8bpp {
                    256
                } else {
                    16
                }
            }
            Mode::Tex0 => self.tex0.as_ref().and_then(|t| t.palettes.get(self.secondary)).map_or(0, |p| p.colors.len() / 2),
            Mode::Nclr => self.nclr.as_ref().map_or(0, |n| n.palette.len() / 2),
            _ => self.body.len() / 2,
        }
        .max(1)
    }

    /// How many palettes a sheet can be drawn in: the rows (or blocks) of the palette file shown.
    pub fn palette_count(&self) -> usize {
        match self.mode {
            Mode::Tex0 => self.tex0.as_ref().map_or(0, |t| t.palettes.len()),
            _ => (self.active_palette(self.palette_size()).len() / 2).div_ceil(self.palette_size()).max(1),
        }
    }

    /// Takes companion graphics: an NCGR's characters (and bit depth), or raw 4bpp tiles.
    pub fn set_companion_characters(&mut self, path: Option<String>, bytes: Option<&[u8]>) {
        self.pairing.characters = path;
        self.companion_error = None;
        let plain = bytes.map(|b| unwrap_with(b).1).unwrap_or_default();
        (self.companion_characters, self.companion_8bpp) = match parse_ncgr(&plain) {
            Ok(ncgr) => (ncgr.characters, ncgr.pixel_format == 4),
            Err(_) => (plain, false),
        };
        self.touched(false);
    }

    /// The palette a picture is drawn with: the attached file shown, else a grey ramp standing in for "index N".
    fn active_palette(&self, colours: usize) -> Vec<u8> {
        if !self.shown_colours().is_empty() {
            return self.shown_colours().to_vec();
        }
        let mut gray = vec![0; colours * 2];
        for i in 0..colours {
            let shade = (i * 255 / colours.saturating_sub(1).max(1)) as u8;
            pack(&mut gray, i, [shade, shade, shade]);
        }
        gray
    }

    fn chosen_texture(&self) -> Option<&Tex0Texture> {
        self.tex0.as_ref()?.textures.get(self.primary)
    }

    /// Whether the image manager lists this file's bitmaps and palettes: a sheet, or a TEX0's textures.
    pub fn manages_images(&self) -> bool {
        matches!(self.mode, Mode::Ncgr | Mode::RawCharacters | Mode::Tex0)
    }

    /// The sheet being painted: its bytes, width in tiles, bit depth and whether it is a linear bitmap.
    pub(crate) fn sheet(&self) -> Option<(&[u8], usize, bool, bool)> {
        match self.mode {
            Mode::Ncgr => {
                let ncgr = self.ncgr.as_ref()?;
                // Character format 1 is a BMP-arranged sheet (NCBR), which shares the NCGR signature.
                let linear = ncgr.character_format & 0xff == 1;
                // The CPOS block records the real dimensions; it beats guessing a square root.
                let declared = ncgr.position.filter(|p| p.width >= 8).map_or(usize::from(ncgr.width), |p| usize::from(p.width) / 8);
                let eight = ncgr.pixel_format == 4;
                let tiles = ncgr.characters.len() / if eight { 64 } else { 32 };
                Some((&ncgr.characters, sheet_width(tiles, declared), eight, linear))
            }
            Mode::RawCharacters => {
                let eight = self.primary == 1;
                let tiles = self.body.len() / if eight { 64 } else { 32 };
                Some((&self.body, sheet_width(tiles, self.pairing.width), eight, false))
            }
            _ => None,
        }
    }

    pub fn render(&self) -> Rendered {
        if let Some(error) = &self.error {
            return Rendered { info: error.clone(), ..Rendered::default() };
        }
        match self.mode {
            Mode::Nclr => {
                let palette = self.nclr.as_ref().map(|n| n.palette.clone()).unwrap_or_default();
                self.palette_view(&palette, format!("NCLR palette — {} colours", palette.len() / 2))
            }
            Mode::RawPalette => self.palette_view(&self.body, format!("Raw BGR555 palette — {} colours", self.body.len() / 2)),
            Mode::Ncgr | Mode::RawCharacters => self.characters_view(),
            Mode::Nscr => {
                let nscr = self.nscr.as_ref();
                let (screen, width, height) = nscr.map_or((Vec::new(), 0, 0), |n| (n.screen.clone(), usize::from(n.width), usize::from(n.height)));
                self.screen_view(&screen, width, height, "NSCR")
            }
            Mode::RawScreen => self.screen_view(&self.body, self.pairing.width * 8, 0, "Raw screen map"),
            Mode::Ncer => self.cells_view(),
            Mode::Nanr => self.animation_view(),
            Mode::Tex0 => self.texture_view(),
            Mode::Unsupported => Rendered::default(),
        }
    }

    fn palette_view(&self, bytes: &[u8], info: String) -> Rendered {
        let count = bytes.len() / 2;
        let width = count.clamp(1, 16);
        let mut image = blank(width, count.div_ceil(width));
        let swatches: Vec<[u8; 4]> = (0..count).map(|i| unpack(bytes, i)).collect();
        for (i, rgba) in swatches.iter().enumerate() {
            put(&mut image, i % width, i / width, *rgba);
        }
        Rendered { image: Some(image), grid: 0, info, swatches, swatches_editable: true }
    }

    fn characters_view(&self) -> Rendered {
        let Some((bytes, width_tiles, eight, linear)) = self.sheet() else { return Rendered::default() };
        let tiles = bytes.len() / if eight { 64 } else { 32 };
        let rows = tiles.div_ceil(width_tiles);
        let width_pixels = width_tiles * 8;
        let mut image = blank(width_pixels, rows * 8);
        let palette = self.active_palette(if eight { 256 } else { 16 });
        let row = self.palette_row * if eight { 256 } else { 16 };
        for index in 0..tiles * 64 {
            let value =
                if eight { bytes.get(index).copied().unwrap_or(0) } else { (bytes.get(index / 2).copied().unwrap_or(0) >> ((index & 1) * 4)) & 15 };
            let (x, y) = if linear {
                (index % width_pixels, index / width_pixels)
            } else {
                let (tile, pixel) = (index / 64, index % 64);
                ((tile % width_tiles) * 8 + pixel % 8, (tile / width_tiles) * 8 + pixel / 8)
            };
            if y >= image.height {
                break;
            }
            put(&mut image, x, y, unpack(&palette, row + usize::from(value)));
        }
        let colours = palette.len() / 2;
        let info = format!(
            "{} — {tiles} tiles, {}bpp, {}.{}",
            if self.mode == Mode::Ncgr { "NCGR" } else { "Raw graphics" },
            if eight { 8 } else { 4 },
            if linear { "bitmap arrangement (NCBR)" } else { "8x8 tiled" },
            if self.palettes.is_empty() { " Indices are shown as greys until a palette file is attached." } else { "" }
        );
        let swatches = (0..colours).map(|i| unpack(&palette, i)).collect();
        Rendered { image: Some(image), grid: 1, info, swatches, swatches_editable: !self.palettes.is_empty() }
    }

    fn screen_view(&self, bytes: &[u8], declared_width: usize, declared_height: usize, name: &str) -> Rendered {
        let entries = bytes.len() / 2;
        let width = screen_width(entries, declared_width);
        let height = if declared_height >= 8 { declared_height / 8 } else { entries.div_ceil(width) };
        let mut image = blank(width * 8, height * 8);
        let composed = !self.companion_characters.is_empty();
        let eight = self.companion_8bpp;
        let palette = self.active_palette(if eight { 256 } else { 16 });
        for (i, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
            let value = u16::from_le_bytes(*pair);
            let (left, top) = ((i % width) * 8, (i / width) * 8);
            if !composed {
                // Nothing to compose with yet: each entry coloured by its tile id, so the arrangement is legible.
                let rgba = hsv(u32::from(value & 0x3ff) * 47 % 360, 150, 220);
                for p in 0..64 {
                    put(&mut image, left + p % 8, top + p / 8, rgba);
                }
                continue;
            }
            // Bits 0-9 character, 10 H-flip, 11 V-flip, 12-15 palette row.
            let tile = usize::from(value & 0x3ff);
            let (hflip, vflip) = (value & 0x400 != 0, value & 0x800 != 0);
            // An 8bpp map names a block of 256 only when there are extended palettes to name.
            let row = match eight {
                false => usize::from(value >> 12) * 16,
                true if palette.len() > 512 => usize::from(value >> 12) * 256,
                true => 0,
            };
            let tile_size = if eight { 64 } else { 32 };
            for pixel in 0..64 {
                let sx = if hflip { 7 - pixel % 8 } else { pixel % 8 };
                let sy = if vflip { 7 - pixel / 8 } else { pixel / 8 };
                let within = sy * 8 + sx;
                let offset = tile * tile_size + if eight { within } else { within / 2 };
                let Some(&byte) = self.companion_characters.get(offset) else { continue };
                let index = if eight { byte } else { (byte >> ((within & 1) * 4)) & 15 };
                if index != 0 {
                    put(&mut image, left + pixel % 8, top + pixel / 8, unpack(&palette, row + usize::from(index)));
                }
            }
        }
        let info = format!(
            "{name} — {entries} map entries.{}",
            if composed { "" } else { " Tile colours are identifiers; choose a graphics file to see the picture and paint it." }
        );
        let swatches = if composed { (0..palette.len() / 2).map(|i| unpack(&palette, i)).collect() } else { Vec::new() };
        Rendered { image: Some(image), grid: 0, info, swatches, swatches_editable: false }
    }

    fn cells_view(&self) -> Rendered {
        let Some(ncer) = &self.ncer else { return Rendered::default() };
        let index = self.primary;
        if !self.companion_characters.is_empty() && index < ncer.cells.len() {
            let packed = self.active_palette(if self.companion_8bpp { 256 } else { 16 });
            if let Ok(decoded) = decode_bgr555_palette(&packed, true) {
                match render_ncer_cell(ncer, index, &self.companion_characters, &decoded) {
                    Ok(image) => {
                        let oam = ncer.cells.get(index).map_or(0, |c| c.oam.len());
                        let from = self.pairing.characters.clone().unwrap_or_default();
                        return Rendered {
                            image: Some(image),
                            grid: 0,
                            info: format!("NCER cell {index} — {oam} OAM objects, composed with {from}."),
                            swatches: (0..packed.len() / 2).map(|i| unpack(&packed, i)).collect(),
                            swatches_editable: false,
                        };
                    }
                    Err(error) => return Rendered { info: error.message, ..Rendered::default() },
                }
            }
        }
        // Outlines show where each object sits until there are graphics to draw them with.
        let mut image = blank(256, 256);
        let mut info = String::new();
        if let Some(cell) = ncer.cells.get(index) {
            const DIMENSIONS: [[(i32, i32); 4]; 3] =
                [[(8, 8), (16, 16), (32, 32), (64, 64)], [(16, 8), (32, 8), (32, 16), (64, 32)], [(8, 16), (8, 32), (16, 32), (32, 64)]];
            let blue = [35, 125, 220, 255];
            for oam in &cell.oam {
                let mut x = i32::from(oam.attr1 & 0x1ff);
                if x >= 256 {
                    x -= 512;
                }
                let mut y = i32::from(oam.attr0 & 0xff);
                if y >= 128 {
                    y -= 256;
                }
                let shape = usize::from((oam.attr0 >> 14) & 3).min(2);
                let size = usize::from((oam.attr1 >> 14) & 3);
                let (w, h) = DIMENSIONS.get(shape).and_then(|s| s.get(size)).copied().unwrap_or((8, 8));
                let (x, y) = (x + 128, y + 128);
                for t in 0..2 {
                    for dx in 0..w {
                        for (px, py) in [(x + dx, y + t), (x + dx, y + h - 1 - t)] {
                            put(&mut image, px.max(0) as usize, py.max(0) as usize, blue);
                        }
                    }
                    for dy in 0..h {
                        for (px, py) in [(x + t, y + dy), (x + w - 1 - t, y + dy)] {
                            put(&mut image, px.max(0) as usize, py.max(0) as usize, blue);
                        }
                    }
                }
            }
            info = format!("NCER cell {index} — {} OAM objects. Outlines show placement; choose a graphics file to draw the cell.", cell.oam.len());
        }
        Rendered { image: Some(image), grid: 0, info, swatches: Vec::new(), swatches_editable: false }
    }

    fn animation_view(&self) -> Rendered {
        let Some(nanr) = &self.nanr else { return Rendered::default() };
        let Some(sequence) = nanr.sequences.get(self.primary) else {
            return Rendered { info: "NANR contains no animation sequences.".to_owned(), ..Rendered::default() };
        };
        let duration: u32 = sequence.frames.iter().map(|f| u32::from(f.duration)).sum();
        let info = format!(
            "NANR sequence {} — {} frames, {duration} ticks, loop starts at frame {}, play mode {}.",
            self.primary,
            sequence.frames.len(),
            sequence.loop_start,
            sequence.play_mode
        );
        Rendered { info, ..Rendered::default() }
    }

    fn texture_view(&self) -> Rendered {
        let Some(tex0) = &self.tex0 else { return Rendered::default() };
        let Some(texture) = tex0.textures.get(self.primary) else {
            return Rendered { info: "TEX0 contains no textures.".to_owned(), ..Rendered::default() };
        };
        let empty = Tex0Palette::default();
        let palette = tex0.palettes.get(self.secondary.min(tex0.palettes.len().saturating_sub(1))).unwrap_or(&empty);
        match render_tex0_texture(texture, palette) {
            Ok(image) => Rendered {
                image: Some(image),
                grid: 0,
                info: format!(
                    "TEX0 texture {} — {}×{}, format {}. Pick a palette colour to edit it.",
                    texture.name, texture.original_width, texture.original_height, texture.format as u8
                ),
                swatches: (0..palette.colors.len() / 2).map(|i| unpack(&palette.colors, i)).collect(),
                swatches_editable: !tex0.palettes.is_empty(),
            },
            Err(error) => Rendered { info: error.message, ..Rendered::default() },
        }
    }

    /// Runs `edit` on the sheet being painted, as a grid of indices. `None` when this file is not a sheet.
    pub fn with_sheet<R>(&mut self, edit: impl FnOnce(&mut dyn Grid<Cell = u8>) -> R) -> Option<R> {
        let (_, width_tiles, eight, linear) = self.sheet()?;
        let bytes = match self.mode {
            Mode::Ncgr => &mut self.ncgr.as_mut()?.characters,
            _ => &mut self.body,
        };
        Some(edit(&mut IndexedSheet { bytes, width_tiles, eight, linear }))
    }

    /// Writes palette index `value` at pixel (`x`, `y`) of the sheet.
    #[cfg(test)]
    pub fn paint(&mut self, x: usize, y: usize, value: u8) {
        if self.with_sheet(|grid| grid.set(x, y, value)).is_some() {
            self.touched(true);
        }
    }

    /// Whether the sheet stores eight bits a pixel, and so paints from the whole palette rather than a row of it.
    pub fn eight_bits(&self) -> bool {
        self.sheet().is_some_and(|(_, _, eight, _)| eight)
    }

    /// Whether a screen map can be painted here: it needs graphics to paint with.
    pub fn paints_screen(&self) -> bool {
        matches!(self.mode, Mode::Nscr | Mode::RawScreen) && !self.companion_characters.is_empty()
    }

    /// Runs `edit` on the screen map's entries. `None` when this file is not one.
    pub fn with_screen<R>(&mut self, edit: impl FnOnce(&mut dyn Grid<Cell = ScreenMapEntry>) -> R) -> Option<R> {
        let (bytes, declared) = match self.mode {
            Mode::Nscr => {
                let nscr = self.nscr.as_mut()?;
                let declared = usize::from(nscr.width);
                (&mut nscr.screen, declared)
            }
            Mode::RawScreen => (&mut self.body, self.pairing.width * 8),
            _ => return None,
        };
        let width = screen_width(bytes.len() / 2, declared);
        Some(edit(&mut RawScreen { bytes, width }))
    }

    /// The paired graphics as a sheet sixteen tiles across, in palette row `row`: what the tile picker shows.
    pub fn companion_sheet(&self, row: usize) -> Option<RgbaImage> {
        let eight = self.companion_8bpp;
        let tile_size = if eight { 64 } else { 32 };
        let tiles = self.companion_characters.len() / tile_size;
        if tiles == 0 {
            return None;
        }
        let palette = self.active_palette(if eight { 256 } else { 16 });
        let offset = row * if eight { 256 } else { 16 };
        let mut image = blank(16 * 8, tiles.div_ceil(16) * 8);
        for tile in 0..tiles {
            for pixel in 0..64 {
                let at = tile * tile_size + if eight { pixel } else { pixel / 2 };
                let Some(&byte) = self.companion_characters.get(at) else { continue };
                let index = if eight { byte } else { (byte >> ((pixel & 1) * 4)) & 15 };
                if index != 0 {
                    put(&mut image, (tile % 16) * 8 + pixel % 8, (tile / 16) * 8 + pixel / 8, unpack(&palette, offset + usize::from(index)));
                }
            }
        }
        Some(image)
    }

    /// How many tiles the paired graphics hold, and how many palette rows a screen entry can name.
    pub fn companion_counts(&self) -> (usize, usize) {
        let tiles = self.companion_characters.len() / if self.companion_8bpp { 64 } else { 32 };
        let colours = self.shown_colours().len();
        let rows = if self.companion_8bpp { colours / 512 } else { (colours / 32).min(16) }.max(1);
        (tiles, rows)
    }

    // --- Undo ---

    fn contents(&self) -> Contents {
        Contents {
            body: self.body.clone(),
            nclr: self.nclr.clone(),
            ncgr: self.ncgr.clone(),
            nscr: self.nscr.clone(),
            tex0: self.tex0.clone(),
            companion: self.palettes.iter().map(|p| (p.path.clone(), p.colours.clone())).collect(),
        }
    }

    /// Whether colour edits go to an attached palette file rather than this one.
    fn edits_companion(&self) -> bool {
        self.pairs_palette() && !self.palettes.is_empty()
    }

    /// The palette a colour edit changes: this file's own, or the paired palette file's.
    fn palette_bytes(&mut self) -> Option<&mut Vec<u8>> {
        match self.mode {
            _ if self.edits_companion() => {
                let shown = self.shown_palette();
                self.palettes.get_mut(shown).map(|p| &mut p.colours)
            }
            Mode::Nclr => self.nclr.as_mut().map(|n| &mut n.palette),
            Mode::RawPalette => Some(&mut self.body),
            Mode::Tex0 => {
                let secondary = self.secondary;
                self.tex0.as_mut().and_then(|t| t.palettes.get_mut(secondary)).map(|p| &mut p.colors)
            }
            _ => None,
        }
    }

    /// Sets palette colour `index` of this file's own palette.
    pub fn set_color(&mut self, index: usize, rgb: [u8; 3]) {
        let Some(bytes) = self.palette_bytes() else { return };
        let mut changed = bytes.clone();
        pack(&mut changed, index, rgb);
        if changed == *bytes {
            return;
        }
        let before = self.contents();
        if let Some(bytes) = self.palette_bytes() {
            *bytes = changed;
        }
        if self.edits_companion() {
            let shown = self.shown_palette();
            if let Some(palette) = self.palettes.get_mut(shown) {
                palette.edited = true;
            }
        }
        let after = self.contents();
        self.history.record("Change colour", before, &after);
        self.touched(true);
    }

    /// The file as edited, wrapped the way it was read.
    pub fn output(&self) -> Result<Vec<u8>, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let encoded = match self.mode {
            Mode::Nclr => self.nclr.as_ref().map(write_nclr),
            Mode::Ncgr => self.ncgr.as_ref().map(write_ncgr),
            Mode::Nscr => self.nscr.as_ref().map(write_nscr),
            Mode::Ncer => self.ncer.as_ref().map(write_ncer),
            Mode::Nanr => self.nanr.as_ref().map(write_nanr),
            Mode::Tex0 => self.tex0.as_ref().map(write_tex0),
            _ => None,
        };
        let body = match encoded {
            Some(result) => result.map_err(|e| e.message)?,
            None => self.body.clone(),
        };
        rewrap(&self.original, body)
    }
}

/// Bits a texel of a paletted TEX0 format stores; the other formats aren't indices into a palette alone.
fn tex0_bits(format: TextureFormat) -> Option<usize> {
    match format {
        TextureFormat::Palette4 => Some(2),
        TextureFormat::Palette16 => Some(4),
        TextureFormat::Palette256 => Some(8),
        _ => None,
    }
}

/// Where texel (`x`, `y`) of a paletted texture is: its byte, its shift and its mask.
fn tex0_texel(texture: &Tex0Texture, bits: usize, x: usize, y: usize) -> (usize, usize, u8) {
    let source = y * texture.width + x;
    let per_byte = 8 / bits;
    (source / per_byte, (source % per_byte) * bits, ((1u16 << bits) - 1) as u8)
}

fn tex0_bitmap(texture: &Tex0Texture) -> Option<Bitmap> {
    let bits = tex0_bits(texture.format)?;
    let (width, height) = (texture.original_width, texture.original_height);
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let (at, shift, mask) = tex0_texel(texture, bits, x, y);
            pixels.push((texture.texels.get(at).copied().unwrap_or(0) >> shift) & mask);
        }
    }
    Some(Bitmap { width, height, pixels })
}

/// The bitmaps and palettes [`image_manager`] moves in and out of PNGs: the sheet in each row of its palette, or a
/// TEX0's textures in each of its palettes.
struct DocImages<'a>(&'a mut GraphicsDoc);

impl Images for DocImages<'_> {
    fn bitmap(&self, index: usize) -> Option<Bitmap> {
        let doc = &*self.0;
        if doc.mode == Mode::Tex0 {
            return tex0_bitmap(doc.tex0.as_ref()?.textures.get(index)?);
        }
        let (bytes, width_tiles, eight, linear) = doc.sheet()?;
        let mut bytes = bytes.to_vec();
        Some(Bitmap::read(&IndexedSheet { bytes: &mut bytes, width_tiles, eight, linear }))
    }

    fn palette(&self, index: usize) -> Option<image_manager::Palette> {
        let doc = &*self.0;
        let (bytes, start, count) = if doc.mode == Mode::Tex0 {
            let colours = &doc.tex0.as_ref()?.palettes.get(index)?.colors;
            (colours.clone(), 0, colours.len() / 2)
        } else {
            // Each row of each attached file in turn; without one, the rows of the grey ramp.
            let size = doc.palette_size();
            match doc.palette_slots().get(index) {
                Some(&(file, row)) => (doc.palettes.get(file)?.colours.clone(), row * size, size),
                None if doc.palettes.is_empty() => (doc.active_palette(size), index * size, size),
                None => return None,
            }
        };
        Some((start..start + count).map(|i| unpack(&bytes, i)).map(|[r, g, b, _]| [r, g, b]).collect())
    }

    fn colours(&self) -> usize {
        match self.0.mode {
            Mode::Tex0 => self.0.chosen_texture().and_then(|t| tex0_bits(t.format)).map_or(256, |bits| 1 << bits),
            _ => self.0.palette_size(),
        }
    }

    fn set_bitmap(&mut self, index: usize, pixels: Vec<u8>) -> Result<(), String> {
        let doc = &mut *self.0;
        if doc.mode == Mode::Tex0 {
            let texture = doc.tex0.as_mut().and_then(|t| t.textures.get_mut(index)).ok_or("no texture")?;
            let bits = tex0_bits(texture.format).ok_or("Only 4-, 16- and 256-colour textures can be imported.")?;
            for (i, &value) in pixels.iter().enumerate() {
                let (x, y) = (i % texture.original_width.max(1), i / texture.original_width.max(1));
                let (at, shift, mask) = tex0_texel(texture, bits, x, y);
                if let Some(byte) = texture.texels.get_mut(at) {
                    *byte = (*byte & !(mask << shift)) | ((value & mask) << shift);
                }
            }
        } else {
            let read = DocImages(doc).bitmap(index).ok_or("no graphics")?;
            doc.with_sheet(|grid| Bitmap { pixels, ..read }.write(grid));
        }
        doc.touched(true);
        Ok(())
    }

    fn set_palette(&mut self, index: usize, colours: &[[u8; 3]]) {
        let doc = &mut *self.0;
        let (bytes, start) = if doc.mode == Mode::Tex0 {
            let Some(palette) = doc.tex0.as_mut().and_then(|t| t.palettes.get_mut(index)) else { return };
            (&mut palette.colors, 0)
        } else if let Some((file, row)) = doc.palette_slots().get(index).copied() {
            let start = row * doc.palette_size();
            let Some(palette) = doc.palettes.get_mut(file) else { return };
            palette.edited = true;
            (&mut palette.colours, start)
        } else {
            return;
        };
        for (i, &rgb) in colours.iter().enumerate().skip(1) {
            if (start + i) * 2 < bytes.len() {
                pack(bytes, start + i, rgb);
            }
        }
        doc.touched(true);
    }

    fn draw(&self, bitmap: usize, palette: usize) -> Option<RgbaImage> {
        let doc = &*self.0;
        if doc.mode == Mode::Tex0 {
            let tex0 = doc.tex0.as_ref()?;
            return render_tex0_texture(tex0.textures.get(bitmap)?, tex0.palettes.get(palette)?).ok();
        }
        Some(image_manager::render(&self.bitmap(bitmap)?, &self.palette(palette)?))
    }
}

/// Draws an image to look at without painting it, on the same canvas the editors paint on.
pub(crate) fn picture(
    ui: &mut Ui,
    name: &str,
    image: &RgbaImage,
    version: u64,
    texture: &mut Option<(u64, egui::TextureHandle)>,
    state: &mut CanvasState,
    max_height: f32,
) {
    ui.horizontal(|ui| zoom_canvas::zoom_controls(ui, state, Steps { fine: 0, coarse: 8 }));
    zoom_canvas::show(ui, name, image, version, texture, state, Steps { fine: 0, coarse: 8 }, &[], max_height);
}

/// What the person did in the editor that the workspace carries out.
pub enum GraphicsAction {
    /// Read this file as the palette (`Some(true)`) or the graphics (`Some(false)`), or as whichever it turns out to
    /// be (`None`).
    Load {
        path: String,
        palette: Option<bool>,
    },
    /// The pairing changed and should be remembered.
    Remember,
    Save,
}

/// The editor's view state: its tools, and the drawn picture with the version of the document it shows.
#[derive(Default)]
pub struct GraphicsView {
    pixels: PixelEditor,
    tiles: TilemapEditor,
    picker: PickerState,
    picker_version: u64,
    still: CanvasState,
    texture: Option<(u64, egui::TextureHandle)>,
    rendered: Option<(u64, Rendered)>,
    /// The image manager's choice of bitmap and palette, and what its last import or export said went wrong.
    manager: ManagerState,
    notice: Option<String>,
}

pub fn show(ui: &mut Ui, doc: &mut GraphicsDoc, view: &mut GraphicsView, services: &dyn Services) -> Vec<GraphicsAction> {
    let mut actions = Vec::new();
    let whole = ui.max_rect();
    if view.rendered.as_ref().is_none_or(|(version, _)| *version != doc.version) {
        view.rendered = Some((doc.version, doc.render()));
    }
    let Some((_, rendered)) = view.rendered.clone() else { return actions };
    if controls::document_header(ui, &doc.path, "", doc.dirty && doc.error.is_none()) {
        actions.push(GraphicsAction::Save);
    }
    ui.add(egui::Label::new(&rendered.info).wrap());
    if let Some(error) = &doc.companion_error {
        controls::error_text(ui, error);
    }
    controls::notice_line(ui, &mut view.notice, true);
    if doc.error.is_some() {
        return actions;
    }
    controls::side_panel(ui, egui::Panel::left(egui::Id::new(("graphics-options", &doc.path))), 240.0).show(ui, |ui| {
        // Scrolls sideways too: what does not fit a narrow panel is scrolled to, not drawn over the canvas beside it.
        egui::ScrollArea::both().show(ui, |ui| {
            options(ui, doc, &mut actions);
            if doc.pairs_palette() {
                ui.separator();
                // Palette files dropped here are added to the ones the graphics can be drawn in.
                let listed = ui.scope(|ui| {
                    if doc.manages_images() {
                        manager(ui, doc, view, services, &mut actions);
                    } else {
                        palette_files(ui, doc, &mut actions);
                    }
                    ui.weak("Drag palette files here from the Filesystem dock, or open one, to draw the graphics in them.");
                });
                if let Some(path) = dropped_on(ui, listed.response.rect) {
                    actions.push(GraphicsAction::Load { path, palette: Some(true) });
                }
            } else if doc.manages_images() {
                ui.separator();
                manager(ui, doc, view, services, &mut actions);
            }
        });
    });
    let palette_only = matches!(doc.mode, Mode::Nclr | Mode::RawPalette);
    let sheet = doc.sheet().is_some();
    let screen = doc.paints_screen();
    if !palette_only && (screen || !rendered.swatches.is_empty()) {
        controls::side_panel(ui, egui::Panel::right(egui::Id::new(("graphics-side", &doc.path))), 300.0).show(ui, |ui| {
            if screen {
                tile_side(ui, doc, view);
            } else {
                // Only the palette the picture is drawn in, as the list beside it chose it.
                let (size, chosen) = if doc.mode == Mode::Tex0 { (rendered.swatches.len(), 0) } else { (doc.palette_size(), doc.palette_row) };
                let start = (chosen * size).min(rendered.swatches.len());
                let shown = rendered.swatches.get(start..(start + size).min(rendered.swatches.len())).unwrap_or_default();
                let file = doc.palettes.get(doc.shown_palette()).filter(|_| doc.mode != Mode::Tex0).map(|p| p.name().to_owned());
                ui.strong(match (file, doc.palette_count() > 1) {
                    (Some(file), true) => format!("{file} · Palette {chosen}"),
                    (Some(file), false) => file,
                    (None, true) => format!("Palette {chosen}"),
                    (None, false) => "Palette".to_owned(),
                });
                let edited = egui::ScrollArea::both()
                    .show(ui, |ui| palette_panel::show(ui, shown, &mut view.pixels.colours, size.max(1), rendered.swatches_editable))
                    .inner;
                if let Some((index, rgb)) = edited {
                    doc.set_color(start + index, rgb);
                }
            }
        });
    }
    egui::CentralPanel::default().show(ui, |ui| {
        if palette_only {
            if let Some((index, rgb)) = palette_panel::show(ui, &rendered.swatches, &mut view.pixels.colours, 256, true) {
                doc.set_color(index, rgb);
            }
            return;
        }
        let Some(image) = &rendered.image else { return };
        if sheet {
            sheet_canvas(ui, doc, view, image);
        } else if screen {
            screen_canvas(ui, doc, view, image);
        } else {
            picture(ui, &doc.path, image, doc.version, &mut view.texture, &mut view.still, ui.available_height());
        }
    });
    // A file dropped anywhere else on the editor is attached as whatever it is; the lists beside take it as they say.
    if doc.pairs_palette()
        && let Some(path) = dropped_on(ui, whole.shrink(1.0))
    {
        actions.push(GraphicsAction::Load { path, palette: None });
    }
    actions
}

/// A file from the tree let go over `rect`, which is outlined while one is dragged over it.
fn dropped_on(ui: &Ui, rect: egui::Rect) -> Option<String> {
    if !egui::DragAndDrop::has_payload_of_type::<DraggedFile>(ui.ctx()) || !ui.rect_contains_pointer(rect) {
        return None;
    }
    let t = Tokens::get(ui.ctx());
    ui.painter().rect_stroke(rect, 4.0, egui::Stroke::new(2.0, t.primary), egui::StrokeKind::Inside);
    if !ui.input(|i| i.pointer.any_released()) {
        return None;
    }
    egui::DragAndDrop::take_payload::<DraggedFile>(ui.ctx()).map(|file| file.0.clone())
}

/// The palette files attached to a screen or cells, which have no image manager: the one drawn with, chosen, and
/// each with a button to take it off.
fn palette_files(ui: &mut Ui, doc: &mut GraphicsDoc, actions: &mut Vec<GraphicsAction>) {
    ui.strong("Palette files");
    let files: Vec<String> = doc.palette_files().into_iter().map(str::to_owned).collect();
    if files.is_empty() {
        ui.weak("(none)");
    }
    let shown = doc.shown_palette();
    for (i, path) in files.iter().enumerate() {
        let name = path.rsplit(['/', '!']).next().unwrap_or(path);
        ui.horizontal(|ui| {
            if controls::icon_button(ui, crate::icons::CLOSE, &format!("Detach {name}")).clicked() {
                doc.detach_palette(i);
                actions.push(GraphicsAction::Remember);
            }
            if ui.choice(i == shown, name).on_hover_text(path).clicked() {
                doc.show_palette(i, 0);
            }
        });
    }
}

/// A row naming the graphics file attached, that a file from the tree is dropped on to attach it as the graphics.
/// Returns the file dropped, and whether the attached one was taken off.
fn attached_slot(ui: &mut Ui, current: Option<&str>) -> (Option<String>, bool) {
    let t = Tokens::get(ui.ctx());
    let dragging = egui::DragAndDrop::has_payload_of_type::<DraggedFile>(ui.ctx());
    let mut detached = false;
    let inner = ui.horizontal(|ui| {
        let name = current.map_or("(none)", |path| path.rsplit(['/', '!']).next().unwrap_or(path));
        let label = ui.add(egui::Label::new(egui::RichText::new(name).color(if current.is_some() { t.foreground } else { t.muted })).truncate());
        if let Some(path) = current {
            label.on_hover_text(path);
            detached = controls::icon_button(ui, crate::icons::CLOSE, "Detach").clicked();
        }
    });
    let rect = inner.response.rect.expand(2.0);
    let over = dragging && ui.rect_contains_pointer(rect);
    if dragging {
        ui.painter().rect_stroke(
            rect,
            4.0,
            egui::Stroke::new(if over { 2.0 } else { 1.0 }, if over { t.primary } else { t.border }),
            egui::StrokeKind::Inside,
        );
    }
    let dropped = (over && ui.input(|i| i.pointer.any_released()))
        .then(|| egui::DragAndDrop::take_payload::<DraggedFile>(ui.ctx()))
        .flatten()
        .map(|file| file.0.clone());
    (dropped, detached)
}

/// The pairing, the cell or texture shown, and the width a headerless file is read at.
fn options(ui: &mut Ui, doc: &mut GraphicsDoc, actions: &mut Vec<GraphicsAction>) {
    egui::Grid::new(("graphics-form", &doc.path)).num_columns(2).show(ui, |ui| {
        // A TEX0's textures and palettes are the image manager's lists instead.
        let listed = doc.mode == Mode::Tex0;
        let primary = if listed { Vec::new() } else { doc.primary_choices() };
        if !primary.is_empty() {
            ui.label(if doc.mode == Mode::RawCharacters { "Bit depth" } else { "Image/cell" });
            let mut chosen = doc.primary.min(primary.len() - 1);
            controls::drop_down("graphics-primary").selected_text(primary.get(chosen).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                for (i, name) in primary.iter().enumerate() {
                    ui.choice_value(&mut chosen, i, name);
                }
            });
            if chosen != doc.primary {
                doc.set_primary(chosen);
            }
            ui.end_row();
        }
        let secondary = if listed { Vec::new() } else { doc.secondary_choices() };
        if !secondary.is_empty() {
            ui.label("Palette");
            let mut chosen = doc.secondary.min(secondary.len() - 1);
            controls::drop_down("graphics-secondary").selected_text(secondary.get(chosen).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                for (i, name) in secondary.iter().enumerate() {
                    ui.choice_value(&mut chosen, i, name);
                }
            });
            if chosen != doc.secondary {
                doc.set_secondary(chosen);
            }
            ui.end_row();
        }
        // A screen map's graphics live in another file, and nothing ties one to the other but the game's code: as in
        // NSMB Editor, the person says which go together, by dragging a file from the tree onto this row, or by
        // opening it and choosing this editor. Palettes are listed below, any number of them.
        if doc.pairs_characters() {
            ui.label("Graphics file");
            let (dropped, detached) = attached_slot(ui, doc.pairing.characters.clone().as_deref());
            if detached {
                doc.set_companion_characters(None, None);
                actions.push(GraphicsAction::Remember);
            }
            if let Some(path) = dropped {
                actions.push(GraphicsAction::Load { path, palette: Some(false) });
            }
            ui.end_row();
        }
        if doc.uses_width() {
            // A headerless sheet carries no width or pixel format, so they are asked for instead of guessed.
            ui.label("Width");
            let mut width = doc.pairing.width;
            ui.add(
                controls::number(&mut width).range(0..=256).custom_formatter(|n, _| if n == 0.0 { "auto".to_owned() } else { format!("{n} tiles") }),
            );
            if width != doc.pairing.width {
                doc.set_width(width);
                actions.push(GraphicsAction::Remember);
            }
            ui.end_row();
        }
    });
}

/// NSMB Editor's bitmaps and palettes: the sheet listed with each row of its palette, or a TEX0's textures with its
/// palettes, and the PNG imports and exports between them.
fn manager(ui: &mut Ui, doc: &mut GraphicsDoc, view: &mut GraphicsView, services: &dyn Services, actions: &mut Vec<GraphicsAction>) {
    let tex0 = doc.mode == Mode::Tex0;
    let name = doc.path.rsplit('/').next().unwrap_or_default().to_owned();
    let (bitmaps, palettes) = match &doc.tex0 {
        Some(tex0) if doc.mode == Mode::Tex0 => {
            (tex0.textures.iter().map(|t| t.name.clone()).collect(), tex0.palettes.iter().map(|p| p.name.clone()).collect())
        }
        _ if doc.palettes.is_empty() => (vec![name.clone()], vec!["Greys (no palette file)".to_owned()]),
        _ => (vec![name.clone()], doc.palette_slots().iter().map(|&(_, row)| format!("Palette {row}")).collect::<Vec<_>>()),
    };
    // Each attached file heads its rows, as NSMB Editor lists every palette opened for the graphics.
    let groups: Vec<Group> = if tex0 {
        Vec::new()
    } else {
        let slots = doc.palette_slots();
        doc.palettes
            .iter()
            .enumerate()
            .filter_map(|(file, p)| slots.iter().position(|&(f, _)| f == file).map(|start| Group { name: p.name().to_owned(), start }))
            .collect()
    };
    let shown = (doc.shown_palette(), doc.palette_row);
    view.manager.bitmap = if tex0 { doc.primary } else { 0 };
    view.manager.palette = if tex0 { doc.secondary } else { doc.palette_slots().iter().position(|&slot| slot == shown).unwrap_or(0) };
    let no_import = if tex0 {
        doc.chosen_texture().filter(|t| tex0_bits(t.format).is_none()).map(|_| "Only 4-, 16- and 256-colour textures can be imported.")
    } else if doc.palettes.is_empty() {
        Some("Attach a palette file first: an import's colours are matched to it, or written into it.")
    } else {
        None
    };
    let lists = Lists { bitmaps: &bitmaps, palettes: &palettes, groups: &groups, shared: tex0, no_import };
    let (_, action) = image_manager::show(ui, &doc.path.clone(), &mut view.manager, &lists);
    if tex0 {
        if view.manager.bitmap != doc.primary {
            doc.set_primary(view.manager.bitmap);
        }
        if view.manager.palette != doc.secondary {
            doc.set_secondary(view.manager.palette);
        }
    } else if let Some(&(file, row)) = doc.palette_slots().get(view.manager.palette) {
        doc.show_palette(file, row);
    }
    let Some(action) = action else { return };
    if let ManagerAction::Detach(file) = action {
        doc.detach_palette(file);
        actions.push(GraphicsAction::Remember);
        return;
    }
    // "All" is every texture stored the way the chosen one is, so they read back from one PNG with one palette size.
    let all: Vec<usize> = match doc.tex0.as_ref().filter(|_| tex0) {
        Some(t) => {
            let format = doc.chosen_texture().map(|c| c.format);
            t.textures.iter().enumerate().filter(|(_, x)| Some(x.format) == format).map(|(i, _)| i).collect()
        }
        None => vec![0],
    };
    let palettes: Vec<usize> = (0..palettes.len()).collect();
    let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem).to_owned();
    doc.begin_edit("Import bitmap");
    let result = image_manager::run(action, &view.manager, &mut DocImages(doc), &all, &palettes, services, &stem);
    doc.end_edit();
    view.notice = result.err();
}

/// The toolbar's undo or redo, carried out on the document.
fn take_step(doc: &mut GraphicsDoc, asked: Option<HistoryButton>) {
    match asked {
        Some(HistoryButton::Undo) => doc.undo(),
        Some(HistoryButton::Redo) => doc.redo(),
        None => {}
    }
}

fn sheet_canvas(ui: &mut Ui, doc: &mut GraphicsDoc, view: &mut GraphicsView, image: &RgbaImage) {
    let mut commands = Vec::new();
    let asked = view.pixels.toolbar(ui, Some((doc.can_undo(), doc.can_redo())), &mut commands);
    take_step(doc, asked);
    let input = view.pixels.canvas(ui, &doc.path, image, doc.version, ui.available_height() - zoom_canvas::below(ui));
    view.pixels.keys(ui, &input, &mut commands);
    ui.weak(pixel_editor::position_text(view.pixels.hover(), image));
    let edits = input.pressed().is_some() || commands.iter().any(Command::edits);
    if edits {
        doc.begin_edit(view.pixels.step_name(&commands));
    }
    let row = if doc.eight_bits() { 256 } else { 16 };
    let ctx = ui.ctx().clone();
    if doc.with_sheet(|grid| view.pixels.apply(&ctx, &input, &commands, grid, row)) == Some(true) {
        doc.touched(true);
    }
    if input.released().is_some() || commands.iter().any(Command::edits) {
        doc.end_edit();
    }
}

/// The paired graphics to paint the screen with, and the brush they make.
fn tile_side(ui: &mut Ui, doc: &mut GraphicsDoc, view: &mut GraphicsView) {
    let (tiles, rows) = doc.companion_counts();
    view.tiles.palette = view.tiles.palette.clamp(0, rows as i16 - 1);
    ui.strong("Tiles");
    ui.weak("Drag to choose the tiles to paint with.");
    let row = view.tiles.palette.max(0) as usize;
    let Some(sheet) = doc.companion_sheet(row) else { return };
    let version = doc.version * 16 + row as u64;
    let picked = tile_picker::show(ui, "graphics-tiles", &sheet, version, &mut view.picker, 8, 16, ui.available_height());
    if picked.changed || view.picker_version == 0 {
        view.picker_version = 1;
        let picked = view.picker.indices(16, tiles);
        let cells = picked.cells.iter().map(|i| ScreenMapEntry::new(i.unwrap_or(0) as i32, view.tiles.palette, false, false)).collect();
        view.tiles.set_brush(Pattern { width: picked.width, height: picked.height, cells });
    }
}

fn screen_canvas(ui: &mut Ui, doc: &mut GraphicsDoc, view: &mut GraphicsView, image: &RgbaImage) {
    let (_, rows) = doc.companion_counts();
    let mut commands = Vec::new();
    let asked = view.tiles.toolbar(ui, Some((doc.can_undo(), doc.can_redo())), rows, &mut commands);
    take_step(doc, asked);
    let input = view.tiles.canvas(ui, &doc.path, image, doc.version, ui.available_height() - zoom_canvas::below(ui));
    view.tiles.keys(ui, &input, &mut commands);
    let hovered = view.tiles.hover().and_then(|at| doc.with_screen(|grid| crate::paint::get(grid, at)).flatten());
    ui.weak(entry_text(view.tiles.hover(), hovered));
    let edits = input.pressed().is_some() || commands.iter().any(Command::edits);
    if edits {
        doc.begin_edit(view.tiles.step_name(&commands));
    }
    let ctx = ui.ctx().clone();
    if let Some(outcome) = doc.with_screen(|grid| view.tiles.apply(&ctx, &input, &commands, grid)) {
        if outcome.changed {
            doc.touched(true);
        }
        if outcome.picked && view.tiles.brush.width * view.tiles.brush.height == 1 {
            // A single tile picked from the map is shown chosen in the picker too.
            if let Some(entry) = view.tiles.brush.cells.first() {
                view.picker.select(entry.tile.max(0) as usize, 16);
                view.tiles.palette = entry.palette;
            }
        }
    }
    if input.released().is_some() || commands.iter().any(Command::edits) {
        doc.end_edit();
    }
}

/// The entry under the pointer, for a status line.
pub(crate) fn entry_text(at: Option<crate::paint::Point>, entry: Option<ScreenMapEntry>) -> String {
    match (at, entry) {
        (Some((x, y)), Some(entry)) => {
            let flips = match (entry.horizontal_flip, entry.vertical_flip) {
                (false, false) => "",
                (true, false) => " · flipped X",
                (false, true) => " · flipped Y",
                (true, true) => " · flipped X and Y",
            };
            format!("{x}, {y} · tile {} · palette {}{flips}", entry.tile, entry.palette)
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use bigstar_formats::{compress_nitro_lz10, decompress_nitro_lz};

    use super::*;

    fn ncgr(characters: Vec<u8>, width_tiles: u16, character_format: u32) -> Vec<u8> {
        let ncgr = Ncgr {
            width: width_tiles,
            height: (characters.len() / 32 / usize::from(width_tiles.max(1))) as u16,
            pixel_format: 3,
            character_format,
            characters,
            ..Ncgr::default()
        };
        write_ncgr(&ncgr).unwrap()
    }

    fn pixel(image: &RgbaImage, x: usize, y: usize) -> [u8; 4] {
        let at = (y * image.width + x) * 4;
        image.pixels[at..at + 4].try_into().unwrap()
    }

    #[test]
    fn a_raw_sheet_paints_the_chosen_index() {
        let tile = vec![0u8; 32];
        assert!(GraphicsDoc::supports(&tile, "root/enemy/test_ncg.bin"));
        let mut doc = GraphicsDoc::open("root/enemy/test_ncg.bin", &tile);
        assert_eq!(doc.render().swatches.len(), 16);
        doc.paint(0, 0, 3);
        let edited = doc.output().unwrap();
        assert_eq!((edited.len(), edited[0] & 0x0f), (32, 3));
        assert!(doc.dirty);
    }

    #[test]
    fn a_paired_palette_colours_the_graphics_instead_of_the_grey_ramp() {
        let sheet = ncgr(vec![0x11; 32], 1, 0);
        let mut nclr = Nclr { palette: vec![0; 32], ..Nclr::default() };
        nclr.palette[2] = 0x1f;
        let encoded = write_nclr(&nclr).unwrap();
        let mut doc = GraphicsDoc::open("pair/sheet.ncgr", &sheet);
        assert_eq!(pixel(doc.render().image.as_ref().unwrap(), 0, 0)[..3], [16, 16, 16], "index 1 of a grey ramp, in BGR555");
        // Any palette goes with any graphics, whatever it is called or wherever it is.
        assert_eq!(doc.attaches_as("elsewhere/red.nclr", &encoded), Some(true));
        assert_eq!(doc.attaches_as("elsewhere/other.ncgr", &sheet), None, "graphics are not a sheet's palette");
        assert_eq!(doc.attaches_as("elsewhere/unnamed.bin", &[0; 32]), Some(true), "a file of no known kind is read as one");
        doc.attach("elsewhere/red.nclr", &encoded);
        assert_eq!(doc.pairing.palettes, ["elsewhere/red.nclr"]);
        let [r, g, b, _] = pixel(doc.render().image.as_ref().unwrap(), 0, 0);
        assert!(r > g + 40 && r > b + 40, "{r} {g} {b}");
        assert!(!doc.dirty, "pairing is not an edit");
    }

    #[test]
    fn a_bitmap_formatted_ncgr_is_not_de_tiled() {
        // Four tiles, two across, one bright pixel at flat offset 8: linear puts it at (8, 0), tiled at (0, 1).
        let mut characters = vec![0u8; 128];
        characters[4] = 0x0f;
        let doc = GraphicsDoc::open("bmp/sheet.ncgr", &ncgr(characters, 2, 1));
        let image = doc.render().image.unwrap();
        assert_eq!((image.width, image.height), (16, 16));
        assert!(pixel(&image, 8, 0)[0] > 200);
        assert!(pixel(&image, 0, 1)[0] < 50);
    }

    #[test]
    fn a_headerless_sheet_honours_the_width_it_is_given() {
        let mut doc = GraphicsDoc::open("raw/width_ncg.bin", &vec![0u8; 8 * 32]);
        let image = doc.render().image.unwrap();
        assert_eq!((image.width, image.height), (24, 24), "the square root guesses three across");
        doc.set_width(8);
        let image = doc.render().image.unwrap();
        assert_eq!(image.width, image.height * 8);
    }

    #[test]
    fn a_screen_map_composes_with_its_graphics_and_palette() {
        // One entry: tile 1, flipped horizontally, palette row 0. Tile 1 has index 2 in its top-left pixel.
        let screen = 0x0401u16.to_le_bytes().to_vec();
        let mut doc = GraphicsDoc::open("bg/map_nsc.bin", &screen);
        assert_eq!(doc.mode, Mode::RawScreen);
        assert!(doc.render().info.contains("choose a graphics file"));
        let mut characters = vec![0u8; 64];
        characters[32] = 0x02;
        doc.set_companion_characters(Some("bg/chr_ncg.bin".to_owned()), Some(&characters));
        let mut palette = vec![0u8; 32];
        palette[4] = 0x1f;
        doc.attach_palette("bg/pal_ncl.bin", &palette);
        let image = doc.render().image.unwrap();
        assert_eq!(pixel(&image, 7, 0)[..3], [255, 0, 0], "flipped to the right edge, coloured red");
        assert_eq!(pixel(&image, 0, 0)[3], 0, "index 0 is see-through");
    }

    #[test]
    fn palette_colours_edit_in_bgr555_and_the_lz_wrapper_survives_a_save() {
        let raw = vec![0u8; 32];
        let wrapped = compress_nitro_lz10(&raw).unwrap();
        let mut doc = GraphicsDoc::open("pal/test_ncl.bin", &wrapped);
        assert_eq!(doc.mode, Mode::RawPalette);
        doc.set_color(1, [255, 0, 0]);
        let saved = doc.output().unwrap();
        assert_eq!(saved[0], 0x10, "still LZ10");
        let plain = decompress_nitro_lz(&saved).unwrap();
        assert_eq!(&plain[2..4], &[0x1f, 0x00]);
    }

    #[test]
    fn a_paired_palette_is_edited_undone_and_written_back_into_its_own_file() {
        let mut doc = GraphicsDoc::open("pair/sheet.ncgr", &ncgr(vec![0x11; 32], 1, 0));
        let nclr = write_nclr(&Nclr { palette: vec![0; 64], ..Nclr::default() }).unwrap();
        doc.attach_palette("pair/two.nclr", &nclr);
        assert_eq!((doc.palette_size(), doc.palette_count()), (16, 2), "two rows of sixteen");
        assert!(doc.render().swatches_editable);
        assert!(doc.companion_outputs().unwrap().is_empty(), "nothing changed yet");
        doc.set_color(17, [0, 0, 255]);
        let [(path, bytes)] = &doc.companion_outputs().unwrap()[..] else { panic!() };
        assert_eq!(path, "pair/two.nclr");
        assert_eq!(parse_nclr(bytes).unwrap().palette[34..36], [0x00, 0x7c]);
        doc.undo();
        assert_eq!(parse_nclr(&doc.companion_outputs().unwrap()[0].1).unwrap().palette, vec![0; 64]);
        doc.mark_saved();
        assert!(!doc.dirty && doc.companion_outputs().unwrap().is_empty());
    }

    #[test]
    fn one_sheet_is_drawn_in_any_of_several_palette_files() {
        let mut doc = GraphicsDoc::open("pair/sheet.ncgr", &ncgr(vec![0x11; 32], 1, 0));
        let colour = |slot: usize, value: u16| {
            let mut palette = vec![0u8; 32];
            palette[slot * 2..slot * 2 + 2].copy_from_slice(&value.to_le_bytes());
            write_nclr(&Nclr { palette, ..Nclr::default() }).unwrap()
        };
        doc.attach_palette("a/red.nclr", &colour(1, 0x001f));
        doc.attach_palette("b/blue.nclr", &colour(1, 0x7c00));
        assert_eq!(doc.palette_files(), ["a/red.nclr", "b/blue.nclr"], "both kept, in the order attached");
        assert_eq!(doc.pairing.palettes, ["a/red.nclr", "b/blue.nclr"]);
        let first = |doc: &GraphicsDoc| pixel(doc.render().image.as_ref().unwrap(), 0, 0);
        assert!(first(&doc)[2] > 200, "the one attached last is shown");
        doc.show_palette(0, 0);
        assert!(first(&doc)[0] > 200, "and the first can be chosen again");

        // Each palette is listed for "all palettes", and a colour edit goes to the file shown.
        assert_eq!(doc.palette_slots(), [(0, 0), (1, 0)]);
        doc.set_color(2, [0, 255, 0]);
        let outputs = doc.companion_outputs().unwrap();
        assert_eq!(outputs.iter().map(|(path, _)| path.as_str()).collect::<Vec<_>>(), ["a/red.nclr"]);

        // Remembered, they come back in their order with the same one shown, whichever arrives first.
        let pairing = doc.pairing.clone();
        let mut again = GraphicsDoc::open("pair/sheet.ncgr", &ncgr(vec![0x11; 32], 1, 0));
        again.restore_pairing(&pairing);
        again.attach_palette("b/blue.nclr", &colour(1, 0x7c00));
        again.attach_palette("a/red.nclr", &colour(1, 0x001f));
        assert_eq!(again.palette_files(), ["a/red.nclr", "b/blue.nclr"]);
        assert_eq!(again.shown_palette(), 0);

        // Detaching one leaves the other.
        doc.detach_palette(0);
        assert_eq!(doc.palette_files(), ["b/blue.nclr"]);
        assert_eq!(doc.pairing.palettes, ["b/blue.nclr"]);
        assert!(first(&doc)[2] > 200);
    }

    #[test]
    fn the_palette_of_the_same_name_is_the_default() {
        let files = ["bg/Demo_NCL.bin", "bg/other_ncl.bin", "obj/hero.nclr", "obj/hero.ncgr"];
        assert_eq!(namesake_palette("bg/demo_ncg.bin", files).as_deref(), Some("bg/Demo_NCL.bin"));
        assert_eq!(namesake_palette("bg/demo_nsc.bin", files).as_deref(), Some("bg/Demo_NCL.bin"));
        assert_eq!(namesake_palette("obj/hero.ncgr", files).as_deref(), Some("obj/hero.nclr"));
        assert_eq!(namesake_palette("bg/lonely_ncg.bin", files), None, "nothing of its name");
        assert_eq!(namesake_palette("other/demo_ncg.bin", files), None, "only beside it");
        assert_eq!(namesake_palette("bg/demo_ncl.bin", files), None, "a palette is not given one");
    }

    #[test]
    fn the_graphics_of_the_same_name_are_the_default_for_screens_and_cells() {
        let files = ["bg/demo_ncg.bin", "obj/Hero.NCGR"];
        assert_eq!(namesake_graphics("bg/demo_nsc.bin", files).as_deref(), Some("bg/demo_ncg.bin"));
        assert_eq!(namesake_graphics("obj/hero.ncer", files).as_deref(), Some("obj/Hero.NCGR"));
        assert_eq!(namesake_graphics("obj/hero.nanr", files).as_deref(), Some("obj/Hero.NCGR"));
        assert_eq!(namesake_graphics("bg/demo_ncg.bin", files), None, "graphics are not their own graphics");
        assert_eq!(namesake_graphics("bg/other_nsc.bin", files), None);
    }

    #[test]
    fn an_older_pairing_with_one_palette_reads_back_as_a_list() {
        let old: Pairing = serde_json::from_str(r#"{"palette":"x/p.nclr","characters":null,"width":0}"#).unwrap();
        assert_eq!(old.palette_files(), ["x/p.nclr"]);
        assert!(!serde_json::to_string(&old).unwrap().contains("\"palette\""));
    }

    #[test]
    fn a_paletted_texture_reads_and_writes_its_indices() {
        let texture = Tex0Texture {
            name: "t".to_owned(),
            format: TextureFormat::Palette16,
            width: 8,
            height: 2,
            original_width: 4,
            original_height: 2,
            texels: vec![0x21, 0x43, 0, 0, 0x65, 0, 0, 0],
            ..Tex0Texture::default()
        };
        let read = tex0_bitmap(&texture).unwrap();
        assert_eq!(read.pixels, [1, 2, 3, 4, 5, 6, 0, 0], "the stored width is 8, the drawn one 4");
        let mut doc = GraphicsDoc::open("t.nsbtx", &[]);
        doc.mode = Mode::Tex0;
        doc.tex0 = Some(Tex0 { textures: vec![texture], ..Tex0::default() });
        DocImages(&mut doc).set_bitmap(0, vec![15, 0, 0, 0, 0, 0, 0, 9]).unwrap();
        let texels = &doc.tex0.as_ref().unwrap().textures[0].texels;
        assert_eq!(texels[..], [0x0f, 0x00, 0, 0, 0x00, 0x90, 0, 0]);
        assert_eq!(DocImages(&mut doc).colours(), 16);
    }

    #[test]
    fn bgr555_rounds_to_the_nearest_step() {
        let mut bytes = vec![0; 2];
        pack(&mut bytes, 0, [255, 128, 0]);
        assert_eq!(unpack(&bytes, 0), [255, 132, 0, 255]);
        assert!(!GraphicsDoc::supports(b"hello", "notes.txt"));
    }
}
