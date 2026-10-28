//! Painting indexed pixels, as NSMB Editor's graphics editor does: the left button paints the left colour and the
//! right button the right one, Ctrl+click picks a colour from the image whatever the tool, and a drag joins its
//! points so a fast stroke has no gaps. Shared by every sheet the editors paint: a character file, a tileset's
//! graphics, a resource's picture. The document keeps the pixels and the history; this keeps the tool.

use bigstar_formats::RgbaImage;
use egui::{Color32, Event, Key, Pos2, Rect, Ui, Vec2};

use crate::controls::Button;
use crate::icons::{self, Icon};
use crate::paint::{self, Grid, Pattern, Point};
use crate::palette_panel::Colours;
use crate::zoom_canvas::{self, CanvasInput, CanvasState, Overlay, Phase, Steps, Tool};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelTool {
    Brush,
    Eraser,
    Eyedropper,
    Fill,
    Line,
    Rectangle,
    Select,
}

/// Each tool, its icon, its key and what it does, for the toolbar.
pub const TOOLS: [(PixelTool, Icon, Key, &str); 7] = [
    (PixelTool::Brush, icons::PENCIL, Key::B, "Brush (B): left paints the left colour, right the right one"),
    (PixelTool::Eraser, icons::ERASER, Key::E, "Eraser (E): paints colour 0, which is see-through"),
    (
        PixelTool::Eyedropper,
        icons::EYEDROPPER,
        Key::I,
        "Eyedropper (I): picks a colour into the button clicked with; Ctrl+click does this with any tool",
    ),
    (PixelTool::Fill, icons::BUCKET, Key::G, "Fill (G): fills the area of one colour"),
    (PixelTool::Line, icons::LINE, Key::L, "Line (L): drag from one end to the other"),
    (PixelTool::Rectangle, icons::RECTANGLE, Key::R, "Rectangle (R): drag corner to corner; hold Shift to fill it"),
    (PixelTool::Select, icons::SELECT, Key::M, "Select (M): drag a rectangle to copy, cut, flip or delete; Ctrl+V pastes under the pointer"),
];

impl PixelTool {
    pub fn step_name(self) -> &'static str {
        match self {
            Self::Brush => "Brush",
            Self::Eraser => "Erase",
            Self::Eyedropper => "Pick colour",
            Self::Fill => "Fill",
            Self::Line => "Line",
            Self::Rectangle => "Rectangle",
            Self::Select => "Edit selection",
        }
    }
}

/// What the toolbar or the keyboard asked for that changes the pixels or the clipboard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Copy,
    Cut,
    Paste(String),
    Delete,
    FlipHorizontal,
    FlipVertical,
    SelectAll,
    Deselect,
}

impl Command {
    pub fn edits(&self) -> bool {
        matches!(self, Self::Cut | Self::Paste(_) | Self::Delete | Self::FlipHorizontal | Self::FlipVertical)
    }

    pub fn step_name(&self) -> &'static str {
        match self {
            Self::Cut => "Cut",
            Self::Paste(_) => "Paste",
            Self::Delete => "Delete selection",
            Self::FlipHorizontal => "Flip horizontally",
            Self::FlipVertical => "Flip vertically",
            _ => "Selection",
        }
    }
}

const CLIPBOARD_HEADER: &str = "BigStar-Pixels/1";

/// A copied rectangle of pixels as text, so it travels through the system clipboard between documents.
pub fn encode_pixels(pattern: &Pattern<u8>) -> String {
    let hex: String = pattern.cells.iter().map(|v| format!("{v:02x}")).collect();
    format!("{CLIPBOARD_HEADER} {} {}\n{hex}", pattern.width, pattern.height)
}

pub fn decode_pixels(text: &str) -> Option<Pattern<u8>> {
    let (head, body) = text.split_once('\n')?;
    let mut words = head.split_whitespace();
    if words.next()? != CLIPBOARD_HEADER {
        return None;
    }
    let (width, height): (usize, usize) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?);
    let body = body.trim();
    if width == 0 || height == 0 || body.len() != width * height * 2 {
        return None;
    }
    let cells = (0..width * height).map(|i| body.get(i * 2..i * 2 + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect::<Option<Vec<_>>>()?;
    Some(Pattern { width, height, cells })
}

pub struct PixelEditor {
    pub tool: PixelTool,
    pub colours: Colours,
    pub canvas: CanvasState,
    texture: Option<(u64, egui::TextureHandle)>,
    /// The point the stroke last painted, the one a line or rectangle started from, and where the pointer is.
    last: Option<Point>,
    start: Option<Point>,
    hover: Option<Point>,
    /// Two opposite corners of the selected rectangle.
    pub selection: Option<(Point, Point)>,
}

impl Default for PixelEditor {
    fn default() -> Self {
        Self {
            tool: PixelTool::Brush,
            colours: Colours::default(),
            canvas: CanvasState::new(None),
            texture: None,
            last: None,
            start: None,
            hover: None,
            selection: None,
        }
    }
}

/// What the toolbar's history buttons asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryButton {
    Undo,
    Redo,
}

/// Undo and redo buttons, for an editor with its own history.
pub fn history_buttons(ui: &mut Ui, can_undo: bool, can_redo: bool) -> Option<HistoryButton> {
    let mut asked = None;
    if Button::icon(icons::UNDO).enabled(can_undo).name("Undo").show(ui).on_hover_text("Undo (Ctrl+Z)").clicked() {
        asked = Some(HistoryButton::Undo);
    }
    if Button::icon(icons::REDO).enabled(can_redo).name("Redo").show(ui).on_hover_text("Redo (Ctrl+Y)").clicked() {
        asked = Some(HistoryButton::Redo);
    }
    asked
}

impl PixelEditor {
    pub fn step_name(&self, commands: &[Command]) -> &'static str {
        commands.iter().find(|c| c.edits()).map_or(self.tool.step_name(), Command::step_name)
    }

    /// The tools, the selection's commands and the zoom; first undo and redo, for an editor whose history is
    /// `history` (whether each can be taken) rather than its owner's.
    pub fn toolbar(&mut self, ui: &mut Ui, history: Option<(bool, bool)>, commands: &mut Vec<Command>) -> Option<HistoryButton> {
        let mut asked = None;
        ui.horizontal_wrapped(|ui| {
            if let Some((can_undo, can_redo)) = history {
                asked = history_buttons(ui, can_undo, can_redo);
                ui.separator();
            }
            for (tool, icon, _, tip) in TOOLS {
                if Button::icon(icon).selected(self.tool == tool).name(tool.step_name()).show(ui).on_hover_text(tip).clicked() {
                    self.tool = tool;
                }
            }
            ui.separator();
            let selected = self.selection.is_some();
            if Button::icon(icons::FLIP_HORIZONTAL).enabled(selected).show(ui).on_hover_text("Flip the selection horizontally").clicked() {
                commands.push(Command::FlipHorizontal);
            }
            if Button::icon(icons::FLIP_VERTICAL).enabled(selected).show(ui).on_hover_text("Flip the selection vertically").clicked() {
                commands.push(Command::FlipVertical);
            }
            ui.separator();
            zoom_canvas::zoom_controls(ui, &mut self.canvas, Steps { fine: 1, coarse: 8 });
        });
        asked
    }

    /// Keys the editor answers while the pointer is over its canvas and nothing is being typed into.
    pub fn keys(&mut self, ui: &Ui, input: &CanvasInput, commands: &mut Vec<Command>) {
        if !input.hovered || ui.ctx().egui_wants_keyboard_input() {
            return;
        }
        for event in ui.input(|i| i.events.clone()) {
            match event {
                Event::Copy => commands.push(Command::Copy),
                Event::Cut => commands.push(Command::Cut),
                Event::Paste(text) => commands.push(Command::Paste(text)),
                Event::Key { key, pressed: true, modifiers, .. } if modifiers.command && key == Key::A => commands.push(Command::SelectAll),
                Event::Key { key, pressed: true, modifiers, .. } if !modifiers.command && !modifiers.alt => match key {
                    Key::X => self.colours.swap(),
                    Key::Delete | Key::Backspace => commands.push(Command::Delete),
                    Key::Escape => commands.push(Command::Deselect),
                    _ => {
                        if let Some((tool, ..)) = TOOLS.iter().find(|(_, _, k, _)| *k == key) {
                            self.tool = *tool;
                        }
                    }
                },
                _ => {}
            }
        }
    }

    /// Draws the canvas with what the tool would do under the pointer.
    pub fn canvas(&mut self, ui: &mut Ui, name: &str, image: &RgbaImage, version: u64, max_height: f32) -> CanvasInput {
        let mut overlays = Vec::new();
        let accent = Color32::from_rgb(255, 200, 40);
        if let Some((a, b)) = self.selection {
            overlays.push(Overlay::cells(a, b, 1, accent).filled(Color32::from_rgba_unmultiplied(255, 200, 40, 30)));
        }
        match (self.tool, self.start, self.hover) {
            (PixelTool::Line, Some(start), Some(end)) => {
                for point in paint::line_points(start, end) {
                    overlays.push(Overlay::cells(point, point, 1, accent));
                }
            }
            (PixelTool::Rectangle, Some(start), Some(end)) => overlays.push(Overlay::cells(start, end, 1, accent)),
            (_, None, Some(at)) if self.tool != PixelTool::Select => {
                let cell = Rect::from_min_size(Pos2::new(at.0 as f32, at.1 as f32), Vec2::splat(1.0));
                overlays.push(Overlay::outline(cell, ui.visuals().strong_text_color()));
            }
            _ => {}
        }
        let steps = Steps { fine: 1, coarse: 8 };
        let input = zoom_canvas::show(ui, name, image, version, &mut self.texture, &mut self.canvas, steps, &overlays, max_height);
        self.hover = input.at;
        input
    }

    /// The pixel under the pointer, for the status line.
    pub fn hover(&self) -> Option<Point> {
        self.hover
    }

    /// Carries out the pointer's gesture and the commands on `grid`, an image painting in rows of `row` colours.
    /// Returns whether a pixel may have changed.
    pub fn apply(&mut self, ctx: &egui::Context, input: &CanvasInput, commands: &[Command], grid: &mut dyn Grid<Cell = u8>, row: usize) -> bool {
        let mut changed = false;
        for command in commands {
            changed |= self.command(ctx, command, grid);
        }
        let Some((button, phase)) = input.gesture else { return changed };
        let primary = button == Tool::Primary;
        let colour = Colours::value(if primary { self.colours.fg } else { self.colours.bg }, row);
        let at = input.at.or(self.last);
        let Some(at) = at else { return changed };
        // Ctrl+click picks, whatever the tool.
        if phase == Phase::Pressed && input.modifiers.command && self.tool != PixelTool::Select {
            if let Some(value) = paint::get(grid, at) {
                self.colours.pick(value, primary, row);
            }
            self.start = None;
            self.last = None;
            return changed;
        }
        match (self.tool, phase) {
            (PixelTool::Brush | PixelTool::Eraser, Phase::Pressed) => {
                let value = if self.tool == PixelTool::Eraser { 0 } else { colour };
                paint::put(grid, at, value);
                self.last = Some(at);
                changed = true;
            }
            (PixelTool::Brush | PixelTool::Eraser, Phase::Held) => {
                let value = if self.tool == PixelTool::Eraser { 0 } else { colour };
                paint::line(grid, self.last.unwrap_or(at), at, value);
                self.last = Some(at);
                changed = true;
            }
            (PixelTool::Eyedropper, Phase::Pressed | Phase::Held) => {
                if let Some(value) = paint::get(grid, at) {
                    self.colours.pick(value, primary, row);
                }
            }
            (PixelTool::Fill, Phase::Pressed) => {
                paint::flood_fill(grid, at, colour);
                changed = true;
            }
            (PixelTool::Line | PixelTool::Rectangle | PixelTool::Select, Phase::Pressed) => {
                self.start = Some(at);
                if self.tool == PixelTool::Select {
                    self.selection = Some((at, at));
                }
            }
            (PixelTool::Select, Phase::Held) => {
                if let Some((start, _)) = self.selection {
                    self.selection = Some((start, at));
                }
            }
            (PixelTool::Line, Phase::Released) => {
                if let Some(start) = self.start {
                    paint::line(grid, start, at, colour);
                    changed = true;
                }
            }
            (PixelTool::Rectangle, Phase::Released) => {
                if let Some(start) = self.start {
                    paint::rect(grid, start, at, colour, input.modifiers.shift);
                    changed = true;
                }
            }
            (PixelTool::Select, Phase::Released) => {
                // Clipped to the image, so the commands act on what is shown selected.
                self.selection = self
                    .start
                    .and_then(|start| paint::region(grid, start, at))
                    .map(|(x, y, w, h)| ((x as i64, y as i64), ((x + w - 1) as i64, (y + h - 1) as i64)));
            }
            _ => {}
        }
        if phase == Phase::Released {
            self.start = None;
            self.last = None;
        }
        changed
    }

    fn command(&mut self, ctx: &egui::Context, command: &Command, grid: &mut dyn Grid<Cell = u8>) -> bool {
        let (width, height) = grid.size();
        match command {
            Command::SelectAll if width > 0 && height > 0 => {
                self.selection = Some(((0, 0), (width as i64 - 1, height as i64 - 1)));
                false
            }
            Command::Deselect => {
                self.selection = None;
                false
            }
            Command::Copy | Command::Cut => {
                let Some((a, b)) = self.selection else { return false };
                if let Some(pattern) = paint::copy(grid, a, b) {
                    ctx.copy_text(encode_pixels(&pattern));
                }
                if *command == Command::Cut {
                    paint::rect(grid, a, b, 0, true);
                    return true;
                }
                false
            }
            Command::Paste(text) => {
                let Some(pattern) = decode_pixels(text) else { return false };
                let at = self.hover.or(self.selection.map(|(a, b)| (a.0.min(b.0), a.1.min(b.1)))).unwrap_or((0, 0));
                paint::stamp(grid, &pattern, at);
                self.selection = Some((at, (at.0 + pattern.width as i64 - 1, at.1 + pattern.height as i64 - 1)));
                self.tool = PixelTool::Select;
                true
            }
            Command::Delete => {
                let Some((a, b)) = self.selection else { return false };
                paint::rect(grid, a, b, 0, true);
                true
            }
            Command::FlipHorizontal | Command::FlipVertical => {
                let Some((a, b)) = self.selection else { return false };
                paint::flip_region(grid, a, b, *command == Command::FlipHorizontal, *command == Command::FlipVertical, |v| v);
                true
            }
            Command::SelectAll => false,
        }
    }
}

/// "x, y" of the pixel under the pointer and the 8x8 tile it is in, for a status line.
pub fn position_text(at: Option<Point>, image: &RgbaImage) -> String {
    match at {
        Some((x, y)) if x >= 0 && y >= 0 && (x as usize) < image.width && (y as usize) < image.height => {
            format!("{x}, {y} · tile {}", (y as usize / 8) * (image.width / 8).max(1) + x as usize / 8)
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::IndexedSheet;

    fn input(at: Point, tool: Tool, phase: Phase) -> CanvasInput {
        CanvasInput { at: Some(at), gesture: Some((tool, phase)), hovered: true, ..CanvasInput::default() }
    }

    #[test]
    fn the_two_buttons_paint_their_own_colours_and_a_drag_leaves_no_gaps() {
        let ctx = egui::Context::default();
        let mut bytes = vec![0u8; 32];
        let mut sheet = IndexedSheet { bytes: &mut bytes, width_tiles: 1, eight: false, linear: false };
        let mut editor = PixelEditor { colours: Colours { fg: 3, bg: 5 }, ..PixelEditor::default() };
        editor.apply(&ctx, &input((0, 0), Tool::Primary, Phase::Pressed), &[], &mut sheet, 16);
        editor.apply(&ctx, &input((4, 0), Tool::Primary, Phase::Held), &[], &mut sheet, 16);
        editor.apply(&ctx, &input((4, 0), Tool::Primary, Phase::Released), &[], &mut sheet, 16);
        assert!((0..5).all(|x| sheet.get(x, 0) == Some(3)));
        editor.apply(&ctx, &input((7, 7), Tool::Secondary, Phase::Pressed), &[], &mut sheet, 16);
        assert_eq!(sheet.get(7, 7), Some(5));

        // Ctrl+click picks into the button clicked with, in the row being painted.
        let mut pick = input((7, 7), Tool::Primary, Phase::Pressed);
        pick.modifiers.command = true;
        editor.colours = Colours { fg: 17, bg: 16 };
        editor.apply(&ctx, &pick, &[], &mut sheet, 16);
        assert_eq!(editor.colours.fg, 21);
    }

    #[test]
    fn a_selection_copies_and_pastes_through_the_clipboard_text() {
        let ctx = egui::Context::default();
        let mut bytes = vec![0u8; 64];
        let mut sheet = IndexedSheet { bytes: &mut bytes, width_tiles: 1, eight: true, linear: false };
        sheet.set(1, 1, 9);
        let mut editor = PixelEditor { tool: PixelTool::Select, ..PixelEditor::default() };
        editor.apply(&ctx, &input((0, 0), Tool::Primary, Phase::Pressed), &[], &mut sheet, 256);
        editor.apply(&ctx, &input((20, 1), Tool::Primary, Phase::Released), &[], &mut sheet, 256);
        assert_eq!(editor.selection, Some(((0, 0), (7, 1))), "clipped to the image");
        let copied = paint::copy(&sheet, (0, 0), (1, 1)).unwrap();
        let text = encode_pixels(&copied);
        assert_eq!(decode_pixels(&text), Some(copied));
        editor.hover = Some((6, 6));
        editor.apply(&ctx, &CanvasInput::default(), &[Command::Paste(text)], &mut sheet, 256);
        assert_eq!(sheet.get(7, 7), Some(9));
        editor.apply(&ctx, &CanvasInput::default(), &[Command::FlipHorizontal], &mut sheet, 256);
        assert_eq!(sheet.get(6, 7), Some(9));
        assert_eq!(decode_pixels("hello\nworld"), None);
    }
}
