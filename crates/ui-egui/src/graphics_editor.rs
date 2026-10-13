//! The graphics editor: every NNS G2D format and TEX0, as a document. A palette lives in a different file from the
//! graphics it colours and a screen map in a third, so the editor pairs a file with its siblings, and remembers the
//! pairing per file. A port of `GraphicsEditorPanel` and `GraphicsCanvas`; the model here is pure, `show` draws it.

use crate::controls::{self, Choice as _};
use bigstar_formats::{
    GraphicsFormatKind, Nanr, Ncer, Ncgr, Nclr, Nscr, RgbaImage, Tex0, Tex0Palette, decode_bgr555_palette, identify_graphics_format, parse_nanr,
    parse_ncer, parse_ncgr, parse_nclr, parse_nscr, parse_tex0, render_ncer_cell, render_tex0_texture, write_nanr, write_ncer, write_ncgr,
    write_nclr, write_nscr, write_tex0,
};
use egui::{Color32, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};
use serde::{Deserialize, Serialize};

use crate::editors::{rewrap, unwrap_with};

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
    pub palette: Option<String>,
    pub characters: Option<String>,
    pub width: usize,
}

pub fn pairing_key(path: &str) -> String {
    format!("graphicsEditor/{path}")
}

/// BGR555 to 8-bit channels, the low bits repeating the high ones.
pub(crate) fn expand(channel: u16) -> u8 {
    ((channel << 3) | (channel >> 2)) as u8
}

fn unpack(bytes: &[u8], index: usize) -> [u8; 4] {
    let (Some(&low), Some(&high)) = (bytes.get(index * 2), bytes.get(index * 2 + 1)) else { return [0, 0, 0, 255] };
    let value = u16::from(low) | (u16::from(high) << 8);
    [expand(value & 31), expand((value >> 5) & 31), expand((value >> 10) & 31), 255]
}

/// 8-bit channels to BGR555, rounding to the nearest step.
pub fn pack(bytes: &mut [u8], index: usize, [r, g, b]: [u8; 3]) {
    let five = |channel: u8| (u16::from(channel) * 31 + 127) / 255;
    let packed = five(r) | (five(g) << 5) | (five(b) << 10);
    if let Some(target) = bytes.get_mut(index * 2..index * 2 + 2) {
        target.copy_from_slice(&packed.to_le_bytes());
    }
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
    companion_palette: Vec<u8>,
    companion_characters: Vec<u8>,
    companion_8bpp: bool,
    pub companion_error: Option<String>,
    pub selected_color: usize,
    /// Grows with every change, so a drawn texture knows when it is stale.
    pub version: u64,
    pub dirty: bool,
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
            companion_palette: Vec::new(),
            companion_characters: Vec::new(),
            companion_8bpp: false,
            companion_error: None,
            selected_color: 0,
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
    fn touched(&mut self, edited: bool) {
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

    /// Which of `siblings` could be this file's palette, and which its graphics, by name.
    pub fn candidates(siblings: &[String]) -> (Vec<String>, Vec<String>) {
        let (mut palettes, mut characters) = (Vec::new(), Vec::new());
        for path in siblings {
            match identify_graphics_format(&[], path) {
                GraphicsFormatKind::NitroPalette | GraphicsFormatKind::RawPaletteCandidate => palettes.push(path.clone()),
                GraphicsFormatKind::NitroCharacterGraphics | GraphicsFormatKind::RawCharacterGraphicsCandidate => characters.push(path.clone()),
                _ => {}
            }
        }
        (palettes, characters)
    }

    /// Takes a companion palette's bytes; an NCLR gives its palette, anything else is taken as raw BGR555.
    pub fn set_companion_palette(&mut self, path: Option<String>, bytes: Option<&[u8]>) {
        self.pairing.palette = path;
        let plain = bytes.map(|b| unwrap_with(b).1).unwrap_or_default();
        self.companion_palette = parse_nclr(&plain).map_or(plain, |nclr| nclr.palette);
        self.touched(false);
    }

    /// Takes companion graphics: an NCGR's characters (and bit depth), or raw 4bpp tiles.
    pub fn set_companion_characters(&mut self, path: Option<String>, bytes: Option<&[u8]>) {
        self.pairing.characters = path;
        let plain = bytes.map(|b| unwrap_with(b).1).unwrap_or_default();
        (self.companion_characters, self.companion_8bpp) = match parse_ncgr(&plain) {
            Ok(ncgr) => (ncgr.characters, ncgr.pixel_format == 4),
            Err(_) => (plain, false),
        };
        self.touched(false);
    }

    /// The palette a picture is drawn with: the paired one, else a grey ramp standing in for "index N".
    fn active_palette(&self, colours: usize) -> Vec<u8> {
        if !self.companion_palette.is_empty() {
            return self.companion_palette.clone();
        }
        let mut gray = vec![0; colours * 2];
        for i in 0..colours {
            let shade = (i * 255 / colours.saturating_sub(1).max(1)) as u8;
            pack(&mut gray, i, [shade, shade, shade]);
        }
        gray
    }

    /// The sheet being painted: its bytes, width in tiles, bit depth and whether it is a linear bitmap.
    fn sheet(&self) -> Option<(&[u8], usize, bool, bool)> {
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
            put(&mut image, x, y, unpack(&palette, usize::from(value)));
        }
        let colours = palette.len() / 2;
        let info = format!(
            "{} — {tiles} tiles, {}bpp, {}.{} Choose an index, then click pixels to paint.",
            if self.mode == Mode::Ncgr { "NCGR" } else { "Raw graphics" },
            if eight { 8 } else { 4 },
            if linear { "bitmap arrangement (NCBR)" } else { "8x8 tiled" },
            if self.companion_palette.is_empty() { " Indices are shown as greys until a palette file is chosen." } else { "" }
        );
        Rendered { image: Some(image), grid: 1, info, swatches: (0..colours).map(|i| unpack(&palette, i)).collect(), swatches_editable: false }
    }

    fn screen_view(&self, bytes: &[u8], declared_width: usize, declared_height: usize, name: &str) -> Rendered {
        let entries = bytes.len() / 2;
        let width = if declared_width >= 8 { declared_width / 8 } else { sheet_width(entries, 0) }.max(1);
        let height = if declared_height >= 8 { declared_height / 8 } else { entries.div_ceil(width) };
        let mut image = blank(width * 8, height * 8);
        let composed = !self.companion_characters.is_empty();
        let eight = self.companion_8bpp;
        let palette = self.active_palette(if eight { 256 } else { 16 });
        for (i, pair) in bytes.chunks_exact(2).enumerate() {
            let value = u16::from_le_bytes([pair.first().copied().unwrap_or(0), pair.get(1).copied().unwrap_or(0)]);
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
            let row = if eight { 0 } else { usize::from(value >> 12) * 16 };
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
            if composed { "" } else { " Tile colours are identifiers; choose a graphics file to see the picture." }
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

    /// Paints the selected palette index into the sheet at image pixel (`x`, `y`). A BMP-arranged sheet is one
    /// row-major bitmap; a CHAR one is 8x8 tiles in reading order.
    pub fn paint(&mut self, x: usize, y: usize) {
        let Some((_, width_tiles, eight, linear)) = self.sheet() else { return };
        let index = if linear { y * width_tiles * 8 + x } else { ((y / 8) * width_tiles + x / 8) * 64 + (y % 8) * 8 + x % 8 };
        let color = self.selected_color;
        let bytes = match self.mode {
            Mode::Ncgr => match &mut self.ncgr {
                Some(ncgr) => &mut ncgr.characters,
                None => return,
            },
            _ => &mut self.body,
        };
        if eight {
            let Some(byte) = bytes.get_mut(index) else { return };
            *byte = color as u8;
        } else {
            let Some(byte) = bytes.get_mut(index / 2) else { return };
            let shift = (index & 1) * 4;
            *byte = (*byte & !(15 << shift)) | (((color & 15) as u8) << shift);
        }
        self.touched(true);
    }

    /// Sets palette colour `index` of this file's own palette.
    pub fn set_color(&mut self, index: usize, rgb: [u8; 3]) {
        let bytes = match self.mode {
            Mode::Nclr => self.nclr.as_mut().map(|n| &mut n.palette),
            Mode::RawPalette => Some(&mut self.body),
            Mode::Tex0 => {
                let secondary = self.secondary;
                self.tex0.as_mut().and_then(|t| t.palettes.get_mut(secondary)).map(|p| &mut p.colors)
            }
            _ => None,
        };
        if let Some(bytes) = bytes {
            pack(bytes, index, rgb);
            self.touched(true);
        }
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

/// Draws an image zoomed to fit 512 points in whole steps, at most 16, so pixel art stays crisp; a grid every `grid`
/// pixels when cells are big enough on screen to tell apart. `texture` caches the upload for `version`. Returns the
/// pixel clicked or dragged over, for painting.
pub(crate) fn picture(
    ui: &mut Ui,
    name: &str,
    image: &RgbaImage,
    grid: usize,
    version: u64,
    texture: &mut Option<(u64, egui::TextureHandle)>,
    max_height: f32,
) -> Option<(usize, usize)> {
    if image.width == 0 || image.height == 0 || image.pixels.len() < image.width * image.height * 4 {
        ui.weak("Nothing to draw.");
        return None;
    }
    let longest = image.width.max(image.height).max(1);
    let zoom = (512 / longest).clamp(1, 16) as f32;
    if texture.as_ref().is_none_or(|(shown, _)| *shown != version) {
        let colors = egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.pixels);
        let handle = ui.ctx().load_texture(format!("graphics:{name}"), colors, egui::TextureOptions::NEAREST);
        *texture = Some((version, handle));
    }
    let mut painted = None;
    egui::ScrollArea::both().id_salt(("picture", name)).max_height(max_height).auto_shrink([false, false]).show(ui, |ui| {
        let size = Vec2::new(image.width as f32, image.height as f32) * zoom;
        let (rect, response) = ui.allocate_exact_size(size.max(Vec2::splat(1.0)), Sense::click_and_drag());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "GraphicsCanvas"));
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        if let Some((_, texture)) = texture {
            painter.image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        if grid > 0 && grid as f32 * zoom >= 8.0 {
            let stroke = Stroke::new(1.0, ui.visuals().weak_text_color().gamma_multiply(0.4));
            for x in (0..=image.width).step_by(grid) {
                let sx = rect.left() + x as f32 * zoom;
                painter.line_segment([Pos2::new(sx, rect.top()), Pos2::new(sx, rect.bottom())], stroke);
            }
            for y in (0..=image.height).step_by(grid) {
                let sy = rect.top() + y as f32 * zoom;
                painter.line_segment([Pos2::new(rect.left(), sy), Pos2::new(rect.right(), sy)], stroke);
            }
        }
        // Clicking or dragging over a sheet paints the chosen index.
        if (response.clicked() || response.dragged())
            && let Some(at) = response.interact_pointer_pos()
        {
            let (x, y) = (((at.x - rect.left()) / zoom) as usize, ((at.y - rect.top()) / zoom) as usize);
            if x < image.width && y < image.height {
                painted = Some((x, y));
            }
        }
    });
    painted
}

/// A palette as swatches to choose the painting index from; when the palette is the file's own, the chosen colour can
/// be changed, and the new colour is returned.
pub(crate) fn swatches(ui: &mut Ui, colours: &[[u8; 4]], selected: &mut usize, editable: bool) -> Option<[u8; 3]> {
    ui.horizontal_wrapped(|ui| {
        for (index, rgba) in colours.iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(Vec2::splat(20.0), Sense::click());
            response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, *selected == index, format!("Colour {index}")));
            let [r, g, b, _] = *rgba;
            ui.painter().rect(
                rect,
                2.0,
                Color32::from_rgb(r, g, b),
                Stroke::new(if *selected == index { 2.0 } else { 1.0 }, ui.visuals().strong_text_color()),
                StrokeKind::Inside,
            );
            if response.on_hover_text(format!("Index {index}")).clicked() {
                *selected = index;
            }
        }
    });
    let mut changed = None;
    if editable {
        ui.horizontal(|ui| {
            ui.label(format!("Colour {}", *selected));
            if let Some([r, g, b, _]) = colours.get(*selected).copied() {
                let mut rgb = [r, g, b];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    changed = Some(rgb);
                }
                ui.weak("BGR555 keeps five bits a channel; the colour is rounded to the nearest one.");
            }
        });
    }
    changed
}

/// What the person did in the editor that the workspace carries out.
pub enum GraphicsAction {
    /// Read this sibling as the palette (`true`) or the graphics (`false`).
    Load {
        path: String,
        palette: bool,
    },
    /// The pairing changed and should be remembered.
    Remember,
    Save,
}

/// The editor's view state: the drawn texture and which version of the document it shows.
#[derive(Default)]
pub struct GraphicsView {
    texture: Option<(u64, egui::TextureHandle)>,
}

pub fn show(ui: &mut Ui, doc: &mut GraphicsDoc, view: &mut GraphicsView, siblings: &[String]) -> Vec<GraphicsAction> {
    let mut actions = Vec::new();
    let rendered = doc.render();
    if controls::document_header(ui, &doc.path, "", doc.dirty && doc.error.is_none()) {
        actions.push(GraphicsAction::Save);
    }
    ui.add(egui::Label::new(&rendered.info).wrap());
    if let Some(error) = &doc.companion_error {
        controls::error_text(ui, error);
    }
    if doc.error.is_some() {
        return actions;
    }
    egui::Grid::new(("graphics-form", &doc.path)).num_columns(2).show(ui, |ui| {
        let primary = doc.primary_choices();
        if !primary.is_empty() {
            ui.label("Image/cell");
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
        let secondary = doc.secondary_choices();
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
        // A palette lives in a different file from the graphics it colours, and a screen map in a third: pairing them
        // is the point of one graphics editor rather than a viewer per file.
        let (palettes, characters) = GraphicsDoc::candidates(siblings);
        for (show, label, choices, current, palette) in [
            (doc.pairs_palette(), "Palette file", palettes, doc.pairing.palette.clone(), true),
            (doc.pairs_characters(), "Graphics file", characters, doc.pairing.characters.clone(), false),
        ] {
            if !show || choices.is_empty() {
                continue;
            }
            ui.label(label);
            controls::drop_down(("graphics-source", palette)).selected_text(current.clone().unwrap_or_else(|| "(none)".to_owned())).show_ui(
                ui,
                |ui| {
                    if ui.choice(current.is_none(), "(none)").clicked() {
                        if palette {
                            doc.set_companion_palette(None, None);
                        } else {
                            doc.set_companion_characters(None, None);
                        }
                        actions.push(GraphicsAction::Remember);
                    }
                    for choice in &choices {
                        if ui.choice(current.as_ref() == Some(choice), choice).clicked() {
                            actions.push(GraphicsAction::Load { path: choice.clone(), palette });
                        }
                    }
                },
            );
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
    if let Some(image) = &rendered.image {
        let height = (ui.available_height() - 130.0).max(120.0);
        if let Some((x, y)) = picture(ui, &doc.path, image, rendered.grid, doc.version, &mut view.texture, height) {
            doc.paint(x, y);
        }
    }
    if !rendered.swatches.is_empty() {
        ui.separator();
        if let Some(rgb) = swatches(ui, &rendered.swatches, &mut doc.selected_color, rendered.swatches_editable) {
            doc.set_color(doc.selected_color, rgb);
        }
    }
    actions
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
        doc.selected_color = 3;
        doc.paint(0, 0);
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
        let (palettes, _) = GraphicsDoc::candidates(&["pair/red.nclr".to_owned(), "pair/other.ncgr".to_owned()]);
        assert_eq!(palettes, ["pair/red.nclr"]);
        doc.set_companion_palette(Some("pair/red.nclr".to_owned()), Some(&encoded));
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
        doc.set_companion_palette(Some("bg/pal_ncl.bin".to_owned()), Some(&palette));
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
    fn bgr555_rounds_to_the_nearest_step() {
        let mut bytes = vec![0; 2];
        pack(&mut bytes, 0, [255, 128, 0]);
        assert_eq!(unpack(&bytes, 0), [255, 132, 0, 255]);
        assert!(!GraphicsDoc::supports(b"hello", "notes.txt"));
    }
}
