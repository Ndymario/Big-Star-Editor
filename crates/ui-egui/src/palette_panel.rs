//! The palette docked beside a pixel canvas, as NSMB Editor's: the primary button chooses the colour the primary
//! button paints, the secondary the secondary's. Below the swatches the chosen colour is edited in the DS's own five
//! bits a channel. A 4bpp image paints in one row of sixteen; choosing a swatch in another row moves to that row.

use bigstar_formats::{bgr555_to_rgb, expand_5_to_8, shrink_8_to_5};
use egui::{Color32, Sense, Stroke, StrokeKind, Ui, Vec2};

use crate::controls::{self, TextField, Width};

/// The colours the two buttons paint, as indices into the whole palette.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colours {
    pub fg: usize,
    pub bg: usize,
}

impl Default for Colours {
    fn default() -> Self {
        Self { fg: 1, bg: 0 }
    }
}

impl Colours {
    pub fn swap(&mut self) {
        std::mem::swap(&mut self.fg, &mut self.bg);
    }

    /// The row the colours are in, for an image that paints in rows of `row` colours.
    pub fn row(&self, row: usize) -> usize {
        self.fg / row.max(1)
    }

    /// The index a pixel stores for `colour`: its place within its row.
    pub fn value(colour: usize, row: usize) -> u8 {
        (colour % row.max(1)) as u8
    }

    /// Chooses `index` for one button; a swatch in another row takes the other button there too.
    pub fn choose(&mut self, index: usize, primary: bool, row: usize) {
        let row = row.max(1);
        let (chosen, other) = if primary { (&mut self.fg, &mut self.bg) } else { (&mut self.bg, &mut self.fg) };
        *chosen = index;
        if *other / row != index / row {
            *other = (index / row) * row + *other % row;
        }
    }

    /// Picks the colour a pixel stores into a button, in the row being painted with.
    pub fn pick(&mut self, value: u8, primary: bool, row: usize) {
        let index = self.row(row) * row.max(1) + usize::from(value);
        if primary {
            self.fg = index;
        } else {
            self.bg = index;
        }
    }
}

/// An 8-bit colour as the DS's five bits a channel.
fn bgr555([r, g, b, _]: [u8; 4]) -> [u16; 3] {
    [shrink_8_to_5(r), shrink_8_to_5(g), shrink_8_to_5(b)]
}

fn swatch(ui: &mut Ui, rgba: [u8; 4], size: f32) -> (egui::Rect, egui::Response) {
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let [r, g, b, _] = rgba;
    ui.painter().rect_filled(rect.shrink(1.0), 1.0, Color32::from_rgb(r, g, b));
    (rect, response)
}

/// Draws the palette. `row` is how many colours an image paints with (16 for 4bpp, 256 for 8bpp); `editable` says
/// whether the colours belong to the file being edited. Returns a colour the person changed, as its index and RGB.
pub fn show(ui: &mut Ui, colours: &[[u8; 4]], chosen: &mut Colours, row: usize, editable: bool) -> Option<(usize, [u8; 3])> {
    if colours.is_empty() {
        ui.weak("No palette.");
        return None;
    }
    let row = row.max(1);
    let strong = ui.visuals().strong_text_color();
    let selection = ui.visuals().selection.stroke.color;

    // The two chosen colours, and the swap between them.
    ui.horizontal(|ui| {
        for (label, index, primary) in [("Left", chosen.fg, true), ("Right", chosen.bg, false)] {
            let (rect, response) = swatch(ui, colours.get(index).copied().unwrap_or([0, 0, 0, 255]), 28.0);
            ui.painter().rect_stroke(rect, 1.0, Stroke::new(1.0, strong), StrokeKind::Inside);
            response.on_hover_text(format!("{label} button paints colour {index}"));
            ui.label(format!("{index}"));
            if primary && controls::Button::new("Swap").small().show(ui).on_hover_text("Swap the two colours (X)").clicked() {
                chosen.swap();
            }
        }
    });

    // Sixteen across in one tight block, a pixel between squares, sized to the panel up to a point.
    const GAP: f32 = 1.0;
    let size = ((ui.available_width() - 15.0 * GAP) / 16.0).floor().clamp(8.0, 16.0);
    let rows = colours.len().div_ceil(16);
    let step = size + GAP;
    let (block, _) = ui.allocate_exact_size(Vec2::new(16.0 * step - GAP, rows as f32 * step - GAP), Sense::hover());
    let active_row = chosen.row(row);
    for (index, rgba) in colours.iter().enumerate() {
        let rect = egui::Rect::from_min_size(block.min + Vec2::new((index % 16) as f32, (index / 16) as f32) * step, Vec2::splat(size));
        let [r, g, b, _] = *rgba;
        ui.painter().rect_filled(rect, 0.0, Color32::from_rgb(r, g, b));
        if row < colours.len() && index / row != active_row {
            ui.painter().rect_filled(rect, 0.0, ui.visuals().panel_fill.gamma_multiply(0.55));
        }
        if index == chosen.fg {
            ui.painter().rect_stroke(rect.expand(1.0), 0.0, Stroke::new(2.0, strong), StrokeKind::Inside);
        } else if index == chosen.bg {
            ui.painter().rect_stroke(rect.expand(1.0), 0.0, Stroke::new(2.0, selection), StrokeKind::Inside);
        }
        let response = ui.interact(rect, ui.id().with(("swatch", index)), Sense::click());
        let [r, g, b] = bgr555(*rgba);
        let response = response.on_hover_text(format!("Colour {index} (row {}, index {})\nBGR555 {r}, {g}, {b}", index / 16, index % row));
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, chosen.fg == index, format!("Colour {index}")));
        if response.clicked() {
            chosen.choose(index, true, row);
        }
        if response.secondary_clicked() {
            chosen.choose(index, false, row);
        }
    }
    if !editable {
        ui.weak("These colours come from another file; open it to change them.");
        return None;
    }

    // The left button's colour, in the five bits a channel the DS stores.
    let index = chosen.fg;
    let rgba = colours.get(index).copied()?;
    let mut channels = bgr555(rgba);
    let before = channels;
    ui.add_space(4.0);
    ui.label(format!("Colour {index}"));
    for (name, channel) in ["R", "G", "B"].iter().zip(channels.iter_mut()) {
        ui.horizontal(|ui| {
            ui.label(*name);
            ui.add(egui::Slider::new(channel, 0..=31));
        });
    }
    let mut changed = None;
    if channels != before {
        let [r, g, b] = channels;
        changed = Some([expand_5_to_8(r), expand_5_to_8(g), expand_5_to_8(b)]);
    }
    ui.horizontal(|ui| {
        let packed = channels[0] | (channels[1] << 5) | (channels[2] << 10);
        let id = egui::Id::new(("palette-hex-text", index));
        let mut text = ui.data(|d| d.get_temp::<String>(id)).unwrap_or_else(|| format!("{packed:04X}"));
        let response = ui.add(TextField::new(&mut text).width(Width::Px(56.0)).code()).on_hover_text("BGR555, as the file stores it");
        if response.lost_focus() {
            if let Ok(value) = u16::from_str_radix(text.trim(), 16) {
                changed = Some(bgr555_to_rgb(value));
            }
            ui.data_mut(|d| d.remove::<String>(id));
        } else if response.has_focus() {
            ui.data_mut(|d| d.insert_temp(id, text));
        }
        let mut rgb = [rgba[0], rgba[1], rgba[2]];
        if ui.color_edit_button_srgb(&mut rgb).changed() {
            changed = Some(rgb);
        }
    });
    changed.map(|rgb| (index, rgb))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swatch_in_another_row_takes_both_buttons_there() {
        let mut colours = Colours { fg: 1, bg: 0 };
        colours.choose(35, true, 16);
        assert_eq!((colours.fg, colours.bg, colours.row(16)), (35, 32, 2));
        assert_eq!(Colours::value(colours.fg, 16), 3);
        colours.pick(7, false, 16);
        assert_eq!(colours.bg, 39);
        colours.swap();
        assert_eq!((colours.fg, colours.bg), (39, 35));
        colours.choose(200, false, 256);
        assert_eq!((colours.fg, colours.bg), (39, 200), "an 8bpp image paints from the whole palette");
    }
}
