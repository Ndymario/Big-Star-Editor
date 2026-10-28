//! NSMB Editor's image manager: the bitmaps an editor holds, listed beside the palettes they can be drawn in. Choosing
//! one of each is what the canvas shows and paints; the buttons below move them through PNGs, one bitmap in one
//! palette, or every bitmap stacked once per palette so an outside editor sees and changes all of them together.

use std::collections::BTreeMap;

use bigstar_formats::RgbaImage;
use egui::Ui;

use crate::controls::{self, Button, Choice as _};
use crate::paint::Grid;
use crate::services::Services;
use crate::tileset_editor::{decode_png, encode_png};

/// A bitmap's palette indices, row by row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bitmap {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
}

impl Bitmap {
    /// Every cell of `grid`, a cell past the end being index 0.
    pub fn read(grid: &dyn Grid<Cell = u8>) -> Self {
        let (width, height) = grid.size();
        let pixels = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).map(|(x, y)| grid.get(x, y).unwrap_or(0)).collect();
        Self { width, height, pixels }
    }

    /// Writes the indices back into `grid`, which is the size they were read at.
    pub fn write(&self, grid: &mut dyn Grid<Cell = u8>) {
        for (i, &value) in self.pixels.iter().enumerate() {
            grid.set(i % self.width.max(1), i / self.width.max(1), value);
        }
    }
}

/// Colours as eight bits a channel.
pub type Palette = Vec<[u8; 3]>;

/// Draws `bitmap` in `palette`: index 0 is clear, and an index the palette has no colour for is magenta, as NSMB
/// Editor shows it.
pub fn render(bitmap: &Bitmap, palette: &[[u8; 3]]) -> RgbaImage {
    let mut pixels = Vec::with_capacity(bitmap.pixels.len() * 4);
    for &index in &bitmap.pixels {
        match palette.get(usize::from(index)) {
            _ if index == 0 => pixels.extend([0, 0, 0, 0]),
            Some(&[r, g, b]) => pixels.extend([r, g, b, 255]),
            None => pixels.extend([255, 0, 255, 255]),
        }
    }
    RgbaImage { width: bitmap.width, height: bitmap.height, pixels }
}

fn rgba(image: &RgbaImage, x: usize, y: usize) -> [u8; 4] {
    let at = (y * image.width + x) * 4;
    image.pixels.get(at..at + 4).and_then(|p| p.try_into().ok()).unwrap_or([0; 4])
}

/// The colour a DS palette would store for an eight-bit one: five bits a channel.
fn five(rgb: [u8; 4]) -> [u16; 3] {
    let five = |c: u8| (u16::from(c) * 31 + 127) / 255;
    [five(rgb[0]), five(rgb[1]), five(rgb[2])]
}

fn eight(channel: u16) -> u8 {
    ((channel << 3) | (channel >> 2)) as u8
}

/// A clear pixel is one more than half transparent.
const CLEAR: u8 = 128;

/// The nearest colour of `palette` (past index 0, which is clear) for every pixel of `image`.
pub fn index_with(image: &RgbaImage, palette: &[[u8; 3]], width: usize, height: usize) -> Result<Vec<u8>, String> {
    if (image.width, image.height) != (width, height) {
        return Err(format!("The PNG is {}×{}; this bitmap is {width}×{height}.", image.width, image.height));
    }
    if palette.len() < 2 {
        return Err("The palette has no colours to match against.".to_owned());
    }
    let mut cache: BTreeMap<[u8; 4], u8> = BTreeMap::new();
    let mut pixels = Vec::with_capacity(width * height);
    for y in 0..height {
        for x in 0..width {
            let colour = rgba(image, x, y);
            if colour[3] < CLEAR {
                pixels.push(0);
                continue;
            }
            let index = *cache.entry(colour).or_insert_with(|| {
                let distance = |c: &[u8; 3]| c.iter().zip(colour).map(|(&a, b)| (i32::from(a) - i32::from(b)).pow(2)).sum::<i32>();
                let best = palette.iter().enumerate().skip(1).min_by_key(|(_, c)| distance(c)).map_or(1, |(i, _)| i);
                u8::try_from(best).unwrap_or(u8::MAX)
            });
            pixels.push(index);
        }
    }
    Ok(pixels)
}

/// Every bitmap side by side, the row repeated once per palette: NSMB Editor's "all bitmaps with all palettes".
#[cfg(test)]
pub fn export_all(bitmaps: &[Bitmap], palettes: &[Palette]) -> RgbaImage {
    let drawn: Vec<Vec<RgbaImage>> = bitmaps.iter().map(|b| palettes.iter().map(|p| render(b, p)).collect()).collect();
    stack(&drawn)
}

/// Lays out `columns`, each a bitmap drawn once per palette: the bitmaps side by side, a strip per palette.
pub fn stack(columns: &[Vec<RgbaImage>]) -> RgbaImage {
    let width: usize = columns.iter().filter_map(|c| c.first()).map(|i| i.width).sum();
    let strip = columns.iter().flatten().map(|i| i.height).max().unwrap_or(0);
    let strips = columns.iter().map(Vec::len).max().unwrap_or(0);
    let mut image = RgbaImage { width, height: strip * strips, pixels: vec![0; width * strip * strips * 4] };
    let mut left = 0;
    for column in columns {
        let column_width = column.first().map_or(0, |i| i.width);
        for (p, drawn) in column.iter().enumerate() {
            let row = drawn.width.min(column_width) * 4;
            for y in 0..drawn.height {
                let from = y * drawn.width * 4;
                let to = ((p * strip + y) * width + left) * 4;
                if let (Some(source), Some(target)) = (drawn.pixels.get(from..from + row), image.pixels.get_mut(to..to + row)) {
                    target.copy_from_slice(source);
                }
            }
        }
        left += column_width;
    }
    image
}

/// What an import of every palette gives: each bitmap's new indices, and each palette's new colours.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Imported {
    pub bitmaps: Vec<Vec<u8>>,
    pub palettes: Vec<Palette>,
}

/// Reads an image laid out as [`export_all`] lays it out back into bitmaps of `sizes` and `palettes` palettes of
/// `colours` colours each. A pixel is one index in every palette, so what is kept apart is the set of colours a
/// place has across all the strips: those are cut down to the `colours - 1` that fit (index 0 stays clear), and each
/// palette gets its strip's share of them.
pub fn import_all(image: &RgbaImage, sizes: &[(usize, usize)], palettes: usize, colours: usize) -> Result<Imported, String> {
    let width: usize = sizes.iter().map(|s| s.0).sum();
    let strip = sizes.iter().map(|s| s.1).max().unwrap_or(0);
    if palettes == 0 || sizes.is_empty() {
        return Err("There is nothing to import into.".to_owned());
    }
    if (image.width, image.height) != (width, strip * palettes) {
        return Err(format!(
            "The PNG is {}×{}; {} bitmap{} with {palettes} palette{} export as {width}×{}.",
            image.width,
            image.height,
            sizes.len(),
            if sizes.len() == 1 { "" } else { "s" },
            if palettes == 1 { "" } else { "s" },
            strip * palettes
        ));
    }
    if !(2..=256).contains(&colours) {
        return Err("A palette needs between 2 and 256 colours.".to_owned());
    }
    // Each place's colours across the strips, five bits a channel; `None` is clear.
    let place = |x: usize, y: usize| -> Option<Vec<u16>> {
        if rgba(image, x, y)[3] < CLEAR {
            return None;
        }
        Some((0..palettes).flat_map(|p| five(rgba(image, x, p * strip + y))).collect())
    };
    let mut counts: BTreeMap<Vec<u16>, usize> = BTreeMap::new();
    let mut left = 0;
    for &(w, h) in sizes {
        for y in 0..h {
            for x in 0..w {
                if let Some(key) = place(left + x, y) {
                    *counts.entry(key).or_default() += 1;
                }
            }
        }
        left += w;
    }
    let samples: Vec<(Vec<u16>, usize)> = counts.into_iter().collect();
    let boxes = median_cut(&samples, colours - 1, palettes * 3);
    let mut index_of: BTreeMap<&[u16], u8> = BTreeMap::new();
    let mut result = vec![vec![[0u8; 3]; colours]; palettes];
    for (b, members) in boxes.iter().enumerate() {
        let index = b + 1;
        let total: usize = members.iter().filter_map(|&m| samples.get(m)).map(|s| s.1).sum::<usize>().max(1);
        for dimension in 0..palettes * 3 {
            let sum: usize =
                members.iter().filter_map(|&m| samples.get(m)).map(|(key, n)| usize::from(key.get(dimension).copied().unwrap_or(0)) * n).sum();
            let value = eight(((sum + total / 2) / total) as u16);
            if let Some(channel) = result.get_mut(dimension / 3).and_then(|p| p.get_mut(index)).and_then(|c| c.get_mut(dimension % 3)) {
                *channel = value;
            }
        }
        for &m in members {
            if let Some((key, _)) = samples.get(m) {
                index_of.insert(key, u8::try_from(index).unwrap_or(u8::MAX));
            }
        }
    }
    let mut bitmaps = Vec::with_capacity(sizes.len());
    let mut left = 0;
    for &(w, h) in sizes {
        let mut pixels = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                pixels.push(place(left + x, y).and_then(|key| index_of.get(key.as_slice()).copied()).unwrap_or(0));
            }
        }
        bitmaps.push(pixels);
        left += w;
    }
    Ok(Imported { bitmaps, palettes: result })
}

/// Splits `samples` (points with a weight) into at most `count` boxes, each time cutting the box with the widest
/// spread across its widest dimension at the weighted median. Returns the members of each box.
fn median_cut(samples: &[(Vec<u16>, usize)], count: usize, dimensions: usize) -> Vec<Vec<usize>> {
    let value = |i: usize, d: usize| samples.get(i).and_then(|s| s.0.get(d)).copied().unwrap_or(0);
    let weight = |i: usize| samples.get(i).map_or(0, |s| s.1);
    let spread = |members: &[usize], d: usize| {
        let (low, high) = members.iter().map(|&i| value(i, d)).fold((u16::MAX, 0), |(l, h), v| (l.min(v), h.max(v)));
        high.saturating_sub(low)
    };
    let widest = |members: &[usize]| (0..dimensions).map(|d| (spread(members, d), d)).max_by_key(|&(s, d)| (s, std::cmp::Reverse(d)));
    let mut boxes: Vec<Vec<usize>> = if samples.is_empty() { Vec::new() } else { vec![(0..samples.len()).collect()] };
    while boxes.len() < count {
        let Some((chosen, (_, dimension))) = boxes
            .iter()
            .enumerate()
            .filter(|(_, members)| members.len() > 1)
            .filter_map(|(i, members)| widest(members).map(|w| (i, w)))
            .max_by_key(|&(i, (s, _))| (s, std::cmp::Reverse(i)))
        else {
            break;
        };
        let Some(members) = boxes.get_mut(chosen) else { break };
        members.sort_by_key(|&i| value(i, dimension));
        let total: usize = members.iter().map(|&i| weight(i)).sum();
        let mut seen = 0;
        let mut split = 1;
        while split < members.len() {
            seen += members.get(split - 1).map_or(0, |&i| weight(i));
            if seen * 2 >= total {
                break;
            }
            split += 1;
        }
        let right = members.split_off(split.min(members.len() - 1));
        boxes.push(right);
    }
    boxes
}

// --- The lists and buttons ---

/// Which bitmap and palette are chosen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ManagerState {
    pub bitmap: usize,
    pub palette: usize,
    /// Choosing bitmap N chooses palette N too, as NSMB Editor's "Auto palette select".
    pub auto_palette: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagerAction {
    ExportThis,
    ImportThis,
    ImportRecreate,
    ExportAll,
    ImportAll,
    /// The palette file heading group `n` of the Palettes list was taken off.
    Detach(usize),
}

/// A heading in the Palettes list: the file the palettes from `start` on come from, with a button to take it off.
pub struct Group {
    pub name: String,
    pub start: usize,
}

/// What the lists show.
pub struct Lists<'a> {
    pub bitmaps: &'a [String],
    pub palettes: &'a [String],
    /// The files the palettes come from, as headings over them; none when the palettes are the editor's own.
    pub groups: &'a [Group],
    /// Every bitmap is drawn in the same palettes, so "all" means all of them; otherwise it is the chosen bitmap in
    /// each of its own palettes.
    pub shared: bool,
    /// Why a PNG can't be brought in, when it can't.
    pub no_import: Option<&'a str>,
}

/// Draws the lists and the buttons. Returns whether the choice changed, and a button pressed.
pub fn show(ui: &mut Ui, id: &str, state: &mut ManagerState, lists: &Lists) -> (bool, Option<ManagerAction>) {
    let before = *state;
    state.bitmap = state.bitmap.min(lists.bitmaps.len().saturating_sub(1));
    state.palette = state.palette.min(lists.palettes.len().saturating_sub(1));
    let mut detached = None;
    let mut list = |ui: &mut Ui, title: &str, names: &[String], groups: &[Group], chosen: &mut usize| {
        ui.strong(title);
        egui::Frame::group(ui.style()).inner_margin(2.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::ScrollArea::vertical().id_salt((id, title)).max_height(180.0).auto_shrink([false, true]).show(ui, |ui| {
                if names.is_empty() {
                    ui.weak("(none)");
                }
                for (i, name) in names.iter().enumerate() {
                    for (n, group) in groups.iter().enumerate().filter(|(_, g)| g.start == i) {
                        ui.horizontal(|ui| {
                            ui.add(egui::Label::new(egui::RichText::new(&group.name).strong()).truncate());
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if controls::icon_button(ui, crate::icons::CLOSE, &format!("Detach {}", group.name)).clicked() {
                                    detached = Some(n);
                                }
                            });
                        });
                    }
                    ui.choice_value(chosen, i, name);
                }
            });
        });
    };
    let mut bitmap = state.bitmap;
    list(ui, "Bitmaps", lists.bitmaps, &[], &mut bitmap);
    if bitmap != state.bitmap {
        state.bitmap = bitmap;
        if state.auto_palette && bitmap < lists.palettes.len() {
            state.palette = bitmap;
        }
    }
    list(ui, "Palettes", lists.palettes, lists.groups, &mut state.palette);
    controls::checkbox(ui, &mut state.auto_palette, "Auto palette select")
        .on_hover_text("Choosing a bitmap chooses the palette in the same place in the list");
    ui.add_space(4.0);

    let mut action = detached.map(ManagerAction::Detach);
    let chosen = !lists.bitmaps.is_empty() && !lists.palettes.is_empty();
    let all_label = if lists.shared { "all bitmaps" } else { "this bitmap" };
    let imports = chosen && lists.no_import.is_none();
    let buttons = [
        (ManagerAction::ExportThis, "Export this bitmap…".to_owned(), chosen, "A PNG of the chosen bitmap in the chosen palette"),
        (
            ManagerAction::ImportThis,
            "Import this bitmap with existing palette…".to_owned(),
            imports,
            "Reads a PNG the bitmap's size into the bitmap, each colour matched to the nearest in the chosen palette",
        ),
        (
            ManagerAction::ImportRecreate,
            "Import this bitmap and recreate palette…".to_owned(),
            imports,
            "Reads a PNG the bitmap's size, making the chosen palette from its colours; colour 0 stays clear",
        ),
        (
            ManagerAction::ExportAll,
            format!("Export {all_label} with all palettes…"),
            chosen,
            "One PNG: the bitmaps side by side, repeated once for every palette, top to bottom",
        ),
        (
            ManagerAction::ImportAll,
            format!("Import {all_label} with all palettes…"),
            imports,
            "Reads a PNG laid out as the export lays it out, remaking the bitmaps and every palette together",
        ),
    ];
    for (which, label, enabled, tip) in buttons {
        let response = Button::new(label).enabled(enabled).show(ui);
        let response = match lists.no_import {
            Some(why) if !enabled && which != ManagerAction::ExportThis && which != ManagerAction::ExportAll => response.on_disabled_hover_text(why),
            _ => response.on_hover_text(tip),
        };
        if response.clicked() {
            action = Some(which);
        }
    }
    (*state != before, action)
}

/// The bitmaps and palettes an editor holds, for [`run`].
pub trait Images {
    /// Bitmap `index`'s indices.
    fn bitmap(&self, index: usize) -> Option<Bitmap>;
    /// Palette `index`'s colours. Colour 0 is the clear one.
    fn palette(&self, index: usize) -> Option<Palette>;
    /// How many colours a bitmap's pixels can name: 4, 16 or 256.
    fn colours(&self) -> usize;
    fn set_bitmap(&mut self, index: usize, pixels: Vec<u8>) -> Result<(), String>;
    /// Writes palette `index`'s colours from 1 on: colour 0 is left as it was.
    fn set_palette(&mut self, index: usize, colours: &[[u8; 3]]);
    /// Bitmap `bitmap` drawn in palette `palette`, as the canvas shows it.
    fn draw(&self, bitmap: usize, palette: usize) -> Option<RgbaImage> {
        Some(render(&self.bitmap(bitmap)?, &self.palette(palette)?))
    }
}

/// Carries out `action` on the chosen bitmap and palette, through the file dialogs. `bitmaps` and `palettes` are
/// the indices "all" covers; `name` names the files. Returns whether anything was changed.
#[allow(clippy::too_many_arguments)]
pub fn run(
    action: ManagerAction,
    state: &ManagerState,
    images: &mut dyn Images,
    bitmaps: &[usize],
    palettes: &[usize],
    services: &dyn Services,
    name: &str,
) -> Result<bool, String> {
    let bitmap = |images: &dyn Images, index: usize| images.bitmap(index).ok_or_else(|| "No bitmap is chosen.".to_owned());
    let palette = |images: &dyn Images, index: usize| images.palette(index).ok_or_else(|| "No palette is chosen.".to_owned());
    let open = |title: &str| match services.open_file(title, Some("png")) {
        Some(Ok((_, bytes))) => decode_png(&bytes).map(Some),
        Some(Err(error)) => Err(error),
        None => Ok(None),
    };
    let colours = images.colours();
    match action {
        ManagerAction::ExportThis => {
            let image = images.draw(state.bitmap, state.palette).ok_or("No bitmap and palette are chosen.")?;
            services.save_file("Export this bitmap", &format!("{name}.png"), "png", &encode_png(&image)?)?;
            Ok(false)
        }
        ManagerAction::ExportAll => {
            let drawn = bitmaps.iter().map(|&b| palettes.iter().map(|&p| images.draw(b, p).ok_or("A bitmap could not be drawn.")).collect());
            let image = stack(&drawn.collect::<Result<Vec<Vec<_>>, _>>()?);
            services.save_file("Export with all palettes", &format!("{name} all palettes.png"), "png", &encode_png(&image)?)?;
            Ok(false)
        }
        ManagerAction::ImportThis => {
            let Some(image) = open("Import this bitmap with existing palette")? else { return Ok(false) };
            let target = bitmap(images, state.bitmap)?;
            let mut chosen = palette(images, state.palette)?;
            chosen.truncate(colours);
            let pixels = index_with(&image, &chosen, target.width, target.height)?;
            images.set_bitmap(state.bitmap, pixels)?;
            Ok(true)
        }
        ManagerAction::ImportRecreate => {
            let Some(image) = open("Import this bitmap and recreate palette")? else { return Ok(false) };
            let target = bitmap(images, state.bitmap)?;
            let imported = import_all(&image, &[(target.width, target.height)], 1, colours)?;
            let (Some(pixels), Some(colours)) = (imported.bitmaps.into_iter().next(), imported.palettes.first()) else { return Ok(false) };
            images.set_bitmap(state.bitmap, pixels)?;
            images.set_palette(state.palette, colours);
            Ok(true)
        }
        ManagerAction::ImportAll => {
            let Some(image) = open("Import with all palettes")? else { return Ok(false) };
            let sizes: Vec<(usize, usize)> = bitmaps.iter().map(|&i| bitmap(images, i).map(|b| (b.width, b.height))).collect::<Result<_, _>>()?;
            let imported = import_all(&image, &sizes, palettes.len(), colours)?;
            for (&index, pixels) in bitmaps.iter().zip(imported.bitmaps) {
                images.set_bitmap(index, pixels)?;
            }
            for (&index, colours) in palettes.iter().zip(&imported.palettes) {
                images.set_palette(index, colours);
            }
            Ok(true)
        }
        ManagerAction::Detach(_) => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: usize, height: usize, colour: impl Fn(usize, usize) -> [u8; 4]) -> RgbaImage {
        let pixels = (0..height).flat_map(|y| (0..width).map(move |x| (x, y))).flat_map(|(x, y)| colour(x, y)).collect();
        RgbaImage { width, height, pixels }
    }

    #[test]
    fn a_png_is_matched_to_the_nearest_colours_with_clear_kept_as_zero() {
        let palette = vec![[0, 0, 0], [255, 0, 0], [0, 0, 255], [250, 10, 10]];
        let image = picture(2, 2, |x, y| [[0, 0, 0, 0], [240, 0, 0, 255], [0, 8, 240, 255], [252, 12, 10, 255]][y * 2 + x]);
        assert_eq!(index_with(&image, &palette, 2, 2).unwrap(), [0, 1, 2, 3]);
        assert!(index_with(&image, &palette, 4, 2).unwrap_err().contains("2×2"));
    }

    #[test]
    fn exporting_every_palette_round_trips_through_an_import() {
        // Two bitmaps of different heights, drawn in two palettes.
        let tall = Bitmap { width: 2, height: 2, pixels: vec![0, 1, 2, 1] };
        let short = Bitmap { width: 1, height: 1, pixels: vec![2] };
        let palettes = vec![vec![[0, 0, 0], [255, 0, 0], [0, 255, 0]], vec![[0, 0, 0], [0, 0, 255], [255, 255, 0]]];
        let image = export_all(&[tall.clone(), short.clone()], &palettes);
        assert_eq!((image.width, image.height), (3, 4));
        assert_eq!(rgba(&image, 1, 2), [0, 0, 255, 255], "the second strip draws the same pixels in the second palette");
        assert_eq!(rgba(&image, 2, 1)[3], 0, "the shorter bitmap's padding is clear");

        let imported = import_all(&image, &[(2, 2), (1, 1)], 2, 3).unwrap();
        let [first, second] = imported.bitmaps.as_slice() else { panic!("two bitmaps") };
        // The indices may be numbered differently; what each place looks like in each palette is what matters.
        let again = export_all(&[Bitmap { pixels: first.clone(), ..tall }, Bitmap { pixels: second.clone(), ..short }], &imported.palettes);
        assert_eq!(again, image);
        assert!(import_all(&image, &[(2, 2)], 2, 3).unwrap_err().contains("export as 2×4"));
    }

    #[test]
    fn more_colours_than_fit_are_cut_down_to_the_palette() {
        let image = picture(16, 1, |x, _| [(x * 16) as u8, 0, 0, 255]);
        let imported = import_all(&image, &[(16, 1)], 1, 5).unwrap();
        let pixels = imported.bitmaps.first().unwrap();
        assert!(pixels.iter().all(|&p| (1..5).contains(&p)), "{pixels:?}");
        let palette = imported.palettes.first().unwrap();
        assert_eq!(palette.len(), 5);
        let reds: Vec<u8> = pixels.iter().map(|&p| palette[usize::from(p)][0]).collect();
        assert!(reds.windows(2).all(|w| w[0] <= w[1]), "a ramp stays a ramp: {reds:?}");
    }

    #[test]
    fn a_bitmap_reads_and_writes_through_its_grid() {
        let mut tiles = vec![bigstar_formats::IndexedTile8Bpp::default(); 3];
        let mut sheet = crate::paint::TileSheet { tiles: &mut tiles, width_tiles: 2 };
        let mut bitmap = Bitmap::read(&sheet);
        assert_eq!((bitmap.width, bitmap.height), (16, 16));
        bitmap.pixels[8] = 5; // the second tile's first pixel
        bitmap.pixels[16 * 8 + 8] = 9; // past the last tile: nowhere to go
        bitmap.write(&mut sheet);
        assert_eq!(tiles[1].pixels[0], 5);
        assert_eq!(render(&Bitmap { width: 2, height: 1, pixels: vec![0, 7] }, &[[1, 2, 3]]).pixels, [0, 0, 0, 0, 255, 0, 255, 255]);
    }
}
