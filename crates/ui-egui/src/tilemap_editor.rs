//! Painting a tilemap from the tile picker beside it, as NSMB Editor's tilemap editor does, and then some: a drag
//! paints as it goes instead of on release, Shift-drag fills a rectangle with the brush's pattern as NSMB Editor's
//! Draw does, and the right button picks tiles from the map itself into the brush. Shared by backgrounds, screen map
//! files and the Map16 sheet; the document keeps the entries and the history, this keeps the tool.

use bigstar_formats::{RgbaImage, ScreenMapEntry};
use egui::{Color32, Event, Key, Ui};

use crate::controls::{self, Button};
use crate::icons::{self, Icon};
use crate::paint::{self, Grid, Pattern, Point, mirror_entry};
use crate::pixel_editor::{Command, HistoryButton, history_buttons};
use crate::zoom_canvas::{self, CanvasInput, CanvasState, Overlay, Phase, Steps, Tool};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TileTool {
    Brush,
    Select,
    FlipX,
    FlipY,
    Palette,
}

pub const TOOLS: [(TileTool, Icon, Key, &str); 5] = [
    (
        TileTool::Brush,
        icons::STAMP,
        Key::D,
        "Draw (D): drag paints the picked tiles; Shift-drag fills a rectangle with them; right-click or right-drag picks tiles from the map",
    ),
    (TileTool::Select, icons::SELECT, Key::C, "Select (C): drag a rectangle to copy, cut or delete; Ctrl+V or V pastes under the pointer"),
    (TileTool::FlipX, icons::FLIP_HORIZONTAL, Key::X, "Flip X (X): drag a rectangle to mirror it left to right"),
    (TileTool::FlipY, icons::FLIP_VERTICAL, Key::Y, "Flip Y (Y): drag a rectangle to mirror it top to bottom"),
    (TileTool::Palette, icons::BUCKET, Key::P, "Palette (P): drag a rectangle to give it the brush's palette"),
];

impl TileTool {
    pub fn step_name(self) -> &'static str {
        match self {
            Self::Brush => "Draw tiles",
            Self::Select => "Edit selection",
            Self::FlipX => "Flip X",
            Self::FlipY => "Flip Y",
            Self::Palette => "Change palette",
        }
    }
}

const CLIPBOARD_HEADER: &str = "BigStar-Tiles/1";

/// Copied entries as text: `tile:palette:flips` each, flips as `h`, `v`, `hv` or `-`.
pub fn encode_tiles(pattern: &Pattern<ScreenMapEntry>) -> String {
    let cells: Vec<String> = pattern
        .cells
        .iter()
        .map(|e| {
            let flips = match (e.horizontal_flip, e.vertical_flip) {
                (false, false) => "-",
                (true, false) => "h",
                (false, true) => "v",
                (true, true) => "hv",
            };
            format!("{}:{}:{flips}", e.tile, e.palette)
        })
        .collect();
    format!("{CLIPBOARD_HEADER} {} {}\n{}", pattern.width, pattern.height, cells.join(" "))
}

pub fn decode_tiles(text: &str) -> Option<Pattern<ScreenMapEntry>> {
    let (head, body) = text.split_once('\n')?;
    let mut words = head.split_whitespace();
    if words.next()? != CLIPBOARD_HEADER {
        return None;
    }
    let (width, height): (usize, usize) = (words.next()?.parse().ok()?, words.next()?.parse().ok()?);
    let cells = body
        .split_whitespace()
        .map(|cell| {
            let mut parts = cell.split(':');
            let (tile, palette, flips) = (parts.next()?.parse().ok()?, parts.next()?.parse().ok()?, parts.next()?);
            Some(ScreenMapEntry::new(tile, palette, flips.contains('h'), flips.contains('v')))
        })
        .collect::<Option<Vec<_>>>()?;
    (width > 0 && height > 0 && cells.len() == width * height).then_some(Pattern { width, height, cells })
}

pub struct TilemapEditor {
    pub tool: TileTool,
    /// What the brush paints, before its flips and palette: the picker's selection, or tiles picked from the map.
    pub brush: Pattern<ScreenMapEntry>,
    /// Applied over the brush: its palette, unless `keep_palette`, and its flips.
    pub palette: i16,
    pub flip_x: bool,
    pub flip_y: bool,
    /// The brush came from the map, so it paints with the palettes and flips the map had.
    pub picked: bool,
    pub canvas: CanvasState,
    texture: Option<(u64, egui::TextureHandle)>,
    start: Option<Point>,
    last: Option<Point>,
    hover: Option<Point>,
    /// The right button's pick in progress.
    picking: Option<Point>,
    pub selection: Option<(Point, Point)>,
    /// Cells a selection and the grid snap to: two on the Map16 sheet, so a selection is whole blocks.
    pub snap: usize,
}

impl Default for TilemapEditor {
    fn default() -> Self {
        Self {
            tool: TileTool::Brush,
            brush: Pattern::single(ScreenMapEntry::default()),
            palette: 0,
            flip_x: false,
            flip_y: false,
            picked: false,
            canvas: CanvasState::default(),
            texture: None,
            start: None,
            last: None,
            hover: None,
            picking: None,
            selection: None,
            snap: 1,
        }
    }
}

/// What a gesture on the map told the document, beyond the entries it changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Outcome {
    pub changed: bool,
    /// The cell clicked, for the document's inspector.
    pub clicked: Option<(usize, usize)>,
    /// Tiles were picked from the map into the brush.
    pub picked: bool,
}

impl TilemapEditor {
    /// The brush as it paints: flipped, and on its palette unless it was picked from the map.
    pub fn effective_brush(&self) -> Pattern<ScreenMapEntry> {
        let mut brush = self.brush.flipped(self.flip_x, self.flip_y, mirror_entry(self.flip_x, self.flip_y));
        if !self.picked {
            for entry in &mut brush.cells {
                entry.palette = self.palette;
            }
        }
        brush
    }

    /// Paints with `brush` (from the picker) from now on.
    pub fn set_brush(&mut self, brush: Pattern<ScreenMapEntry>) {
        self.brush = brush;
        self.picked = false;
    }

    pub fn step_name(&self, commands: &[Command]) -> &'static str {
        commands.iter().find(|c| c.edits()).map_or(self.tool.step_name(), Command::step_name)
    }

    /// The tools and the zoom, then the brush's palette and flips; first undo and redo, for an editor whose
    /// history is `history` rather than its owner's.
    pub fn toolbar(&mut self, ui: &mut Ui, history: Option<(bool, bool)>, palettes: usize, commands: &mut Vec<Command>) -> Option<HistoryButton> {
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
            if Button::new("Delete").small().enabled(self.selection.is_some()).show(ui).on_hover_text("Clear the selection (Delete)").clicked() {
                commands.push(Command::Delete);
            }
            ui.separator();
            zoom_canvas::zoom_controls(ui, &mut self.canvas, Steps { fine: 8, coarse: 8 * self.snap.max(2) });
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("Brush");
            if palettes > 1 {
                ui.label("palette");
                let before = self.palette;
                ui.add(controls::number(&mut self.palette).range(0..=palettes as i16 - 1));
                if before != self.palette {
                    self.picked = false;
                }
            }
            controls::toggle(ui, &mut self.flip_x, "Flip X").on_hover_text("Paint the brush mirrored left to right (Shift+X)");
            controls::toggle(ui, &mut self.flip_y, "Flip Y").on_hover_text("Paint the brush mirrored top to bottom (Shift+Y)");
            if self.picked {
                ui.weak("picked from the map");
            }
        });
        asked
    }

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
                Event::Key { key, pressed: true, modifiers, .. } if modifiers.shift && !modifiers.command => match key {
                    Key::X => self.flip_x = !self.flip_x,
                    Key::Y => self.flip_y = !self.flip_y,
                    _ => {}
                },
                Event::Key { key, pressed: true, modifiers, .. } if modifiers.is_none() => match key {
                    Key::Delete | Key::Backspace => commands.push(Command::Delete),
                    Key::Escape => commands.push(Command::Deselect),
                    // NSMB Editor's paste key, which pastes what this editor copied last.
                    Key::V => {
                        if let Some(text) = ui.data(|d| d.get_temp::<String>(egui::Id::new("tilemap-clipboard"))) {
                            commands.push(Command::Paste(text));
                        }
                    }
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

    /// Draws the map with the brush or the rectangle the drag covers.
    pub fn canvas(&mut self, ui: &mut Ui, name: &str, image: &RgbaImage, version: u64, max_height: f32) -> CanvasInput {
        let accent = Color32::from_rgb(255, 200, 40);
        let pick = Color32::from_rgb(80, 180, 255);
        let mut overlays = Vec::new();
        if let Some((a, b)) = self.selection {
            overlays.push(Overlay::cells(a, b, 8, accent).filled(Color32::from_rgba_unmultiplied(255, 200, 40, 30)));
        }
        if let (Some(start), Some(at)) = (self.picking, self.hover) {
            overlays.push(Overlay::cells(start, at, 8, pick));
        } else if let (Some(start), Some(at)) = (self.start, self.hover)
            && (self.tool != TileTool::Brush || self.last.is_none())
        {
            overlays.push(Overlay::cells(start, at, 8, accent));
        } else if let Some(at) = self.hover
            && self.tool == TileTool::Brush
        {
            let at = self.aligned(at);
            let end = (at.0 + self.brush.width as i64 - 1, at.1 + self.brush.height as i64 - 1);
            overlays.push(Overlay::cells(at, end, 8, ui.visuals().strong_text_color()));
        }
        let steps = Steps { fine: 8, coarse: 8 * self.snap.max(2) };
        let input = zoom_canvas::show(ui, name, image, version, &mut self.texture, &mut self.canvas, steps, &overlays, max_height);
        self.hover = input.at.map(|(x, y)| (x.div_euclid(8), y.div_euclid(8)));
        input
    }

    /// The map cell under the pointer.
    pub fn hover(&self) -> Option<Point> {
        self.hover
    }

    /// Where a stroke's brush lands for the cell `at`: in whole brush steps from where the stroke began, so a
    /// pattern repeats instead of smearing.
    fn aligned(&self, at: Point) -> Point {
        let Some(origin) = self.last.and(self.start) else { return at };
        let (w, h) = (self.brush.width.max(1) as i64, self.brush.height.max(1) as i64);
        (origin.0 + (at.0 - origin.0).div_euclid(w) * w, origin.1 + (at.1 - origin.1).div_euclid(h) * h)
    }

    fn snapped(&self, a: Point, b: Point) -> (Point, Point) {
        let snap = self.snap.max(1) as i64;
        let (left, top) = (a.0.min(b.0).div_euclid(snap) * snap, a.1.min(b.1).div_euclid(snap) * snap);
        let (right, bottom) = ((a.0.max(b.0).div_euclid(snap) + 1) * snap - 1, (a.1.max(b.1).div_euclid(snap) + 1) * snap - 1);
        ((left, top), (right, bottom))
    }

    pub fn apply(&mut self, ctx: &egui::Context, input: &CanvasInput, commands: &[Command], grid: &mut dyn Grid<Cell = ScreenMapEntry>) -> Outcome {
        let mut outcome = Outcome::default();
        for command in commands {
            outcome.changed |= self.command(ctx, command, grid);
        }
        let Some((button, phase)) = input.gesture else { return outcome };
        let Some(at) = input.at.map(|(x, y)| (x.div_euclid(8), y.div_euclid(8))).or(self.hover) else { return outcome };
        if phase == Phase::Pressed
            && let Some((x, y)) = paint::get(grid, at).and(Some(at))
        {
            outcome.clicked = Some((x as usize, y as usize));
        }
        // The right button picks tiles from the map into the brush, with any tool.
        if button == Tool::Secondary {
            match phase {
                Phase::Pressed => self.picking = Some(at),
                Phase::Held => {}
                Phase::Released => {
                    if let Some(pattern) = self.picking.take().and_then(|start| paint::copy(grid, start, at)) {
                        self.brush = pattern;
                        self.picked = true;
                        self.flip_x = false;
                        self.flip_y = false;
                        self.tool = TileTool::Brush;
                        outcome.picked = true;
                    }
                }
            }
            return outcome;
        }
        match (self.tool, phase) {
            (TileTool::Brush, Phase::Pressed) if input.modifiers.shift => self.start = Some(at),
            (TileTool::Brush, Phase::Pressed) => {
                self.start = Some(at);
                self.last = Some(at);
                paint::stamp(grid, &self.effective_brush(), at);
                outcome.changed = true;
            }
            (TileTool::Brush, Phase::Held) if self.last.is_some() => {
                let to = self.aligned(at);
                if Some(to) != self.last {
                    paint::stamp(grid, &self.effective_brush(), to);
                    self.last = Some(to);
                    outcome.changed = true;
                }
            }
            (TileTool::Brush, Phase::Released) if self.last.is_none() => {
                if let Some(start) = self.start {
                    paint::fill_pattern(grid, start, at, &self.effective_brush());
                    outcome.changed = true;
                }
            }
            (_, Phase::Pressed) => {
                self.start = Some(at);
                if self.tool == TileTool::Select {
                    self.selection = Some(self.snapped(at, at));
                }
            }
            (TileTool::Select, Phase::Held) => {
                if let Some(start) = self.start {
                    self.selection = Some(self.snapped(start, at));
                }
            }
            (TileTool::Select, Phase::Released) => {
                self.selection = self.start.and_then(|start| {
                    let (a, b) = self.snapped(start, at);
                    paint::region(grid, a, b).map(|(x, y, w, h)| ((x as i64, y as i64), ((x + w - 1) as i64, (y + h - 1) as i64)))
                });
            }
            (TileTool::FlipX | TileTool::FlipY, Phase::Released) => {
                if let Some(start) = self.start {
                    let horizontal = self.tool == TileTool::FlipX;
                    paint::flip_region(grid, start, at, horizontal, !horizontal, mirror_entry(horizontal, !horizontal));
                    outcome.changed = true;
                }
            }
            (TileTool::Palette, Phase::Released) => {
                if let Some(start) = self.start {
                    let palette = self.palette;
                    paint::map_region(grid, start, at, |mut entry| {
                        entry.palette = palette;
                        entry
                    });
                    outcome.changed = true;
                }
            }
            _ => {}
        }
        if phase == Phase::Released {
            self.start = None;
            self.last = None;
        }
        outcome
    }

    fn command(&mut self, ctx: &egui::Context, command: &Command, grid: &mut dyn Grid<Cell = ScreenMapEntry>) -> bool {
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
                    let text = encode_tiles(&pattern);
                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("tilemap-clipboard"), text.clone()));
                    ctx.copy_text(text);
                }
                if *command == Command::Cut {
                    paint::rect(grid, a, b, ScreenMapEntry::default(), true);
                    return true;
                }
                false
            }
            Command::Paste(text) => {
                let Some(pattern) = decode_tiles(text) else { return false };
                let at = self.hover.or(self.selection.map(|(a, b)| (a.0.min(b.0), a.1.min(b.1)))).unwrap_or((0, 0));
                let snap = self.snap.max(1) as i64;
                let at = (at.0.div_euclid(snap) * snap, at.1.div_euclid(snap) * snap);
                paint::stamp(grid, &pattern, at);
                self.selection = Some((at, (at.0 + pattern.width as i64 - 1, at.1 + pattern.height as i64 - 1)));
                true
            }
            Command::Delete => {
                let Some((a, b)) = self.selection else { return false };
                paint::rect(grid, a, b, ScreenMapEntry::default(), true);
                true
            }
            Command::FlipHorizontal | Command::FlipVertical => {
                let Some((a, b)) = self.selection else { return false };
                let horizontal = *command == Command::FlipHorizontal;
                paint::flip_region(grid, a, b, horizontal, !horizontal, mirror_entry(horizontal, !horizontal));
                true
            }
            Command::SelectAll => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paint::Screen;

    fn input(at: Point, tool: Tool, phase: Phase, shift: bool) -> CanvasInput {
        let mut input = CanvasInput { at: Some((at.0 * 8 + 3, at.1 * 8 + 3)), gesture: Some((tool, phase)), hovered: true, ..CanvasInput::default() };
        input.modifiers.shift = shift;
        input
    }

    fn entry(tile: i32) -> ScreenMapEntry {
        ScreenMapEntry::new(tile, 0, false, false)
    }

    #[test]
    fn a_drag_paints_the_pattern_in_whole_steps_and_shift_fills_a_rectangle() {
        let ctx = egui::Context::default();
        let mut entries = vec![ScreenMapEntry::default(); 6 * 2];
        let mut screen = Screen { entries: &mut entries, width: 6 };
        let mut editor = TilemapEditor::default();
        editor.set_brush(Pattern { width: 2, height: 1, cells: vec![entry(1), entry(2)] });
        editor.palette = 3;
        editor.apply(&ctx, &input((0, 0), Tool::Primary, Phase::Pressed, false), &[], &mut screen);
        editor.apply(&ctx, &input((3, 0), Tool::Primary, Phase::Held, false), &[], &mut screen);
        editor.apply(&ctx, &input((3, 0), Tool::Primary, Phase::Released, false), &[], &mut screen);
        let tiles: Vec<i32> = entries[..6].iter().map(|e| e.tile).collect();
        assert_eq!(tiles, [1, 2, 1, 2, 0, 0], "the second stamp lands one brush width on, not one cell");
        assert_eq!(entries[0].palette, 3);

        let mut screen = Screen { entries: &mut entries, width: 6 };
        editor.apply(&ctx, &input((0, 1), Tool::Primary, Phase::Pressed, true), &[], &mut screen);
        editor.apply(&ctx, &input((4, 1), Tool::Primary, Phase::Released, true), &[], &mut screen);
        let tiles: Vec<i32> = entries[6..].iter().map(|e| e.tile).collect();
        assert_eq!(tiles, [1, 2, 1, 2, 1, 0]);
    }

    #[test]
    fn the_right_button_picks_tiles_with_their_flips_and_palette() {
        let ctx = egui::Context::default();
        let mut entries = vec![ScreenMapEntry::new(5, 2, true, false), entry(6), entry(0), entry(0)];
        let mut screen = Screen { entries: &mut entries, width: 4 };
        let mut editor = TilemapEditor { tool: TileTool::FlipX, ..TilemapEditor::default() };
        editor.apply(&ctx, &input((0, 0), Tool::Secondary, Phase::Pressed, false), &[], &mut screen);
        let outcome = editor.apply(&ctx, &input((1, 0), Tool::Secondary, Phase::Released, false), &[], &mut screen);
        assert!(outcome.picked && !outcome.changed);
        assert_eq!(editor.tool, TileTool::Brush);
        editor.apply(&ctx, &input((2, 0), Tool::Primary, Phase::Pressed, false), &[], &mut screen);
        assert_eq!(entries[2], ScreenMapEntry::new(5, 2, true, false), "a picked brush keeps the map's palette and flips");
        assert_eq!(entries[3].tile, 6);
    }

    #[test]
    fn region_tools_flip_and_recolour_and_tiles_round_trip_the_clipboard() {
        let ctx = egui::Context::default();
        let mut entries = vec![entry(1), entry(2)];
        let mut screen = Screen { entries: &mut entries, width: 2 };
        let mut editor = TilemapEditor { tool: TileTool::FlipX, ..TilemapEditor::default() };
        editor.apply(&ctx, &input((0, 0), Tool::Primary, Phase::Pressed, false), &[], &mut screen);
        editor.apply(&ctx, &input((1, 0), Tool::Primary, Phase::Released, false), &[], &mut screen);
        assert_eq!(entries, [ScreenMapEntry::new(2, 0, true, false), ScreenMapEntry::new(1, 0, true, false)]);
        let mut screen = Screen { entries: &mut entries, width: 2 };
        editor.tool = TileTool::Palette;
        editor.palette = 7;
        editor.apply(&ctx, &input((1, 0), Tool::Primary, Phase::Pressed, false), &[], &mut screen);
        editor.apply(&ctx, &input((1, 0), Tool::Primary, Phase::Released, false), &[], &mut screen);
        assert_eq!((entries[0].palette, entries[1].palette), (0, 7));

        let pattern = Pattern { width: 2, height: 1, cells: entries.clone() };
        assert_eq!(decode_tiles(&encode_tiles(&pattern)), Some(pattern));
        assert_eq!(decode_tiles("BigStar-Tiles/1 2 1\n1:0:-"), None);
    }

    #[test]
    fn a_selection_on_the_map16_sheet_is_whole_blocks() {
        let ctx = egui::Context::default();
        let mut entries = vec![ScreenMapEntry::default(); 16];
        let mut screen = Screen { entries: &mut entries, width: 4 };
        let mut editor = TilemapEditor { tool: TileTool::Select, snap: 2, ..TilemapEditor::default() };
        editor.apply(&ctx, &input((1, 1), Tool::Primary, Phase::Pressed, false), &[], &mut screen);
        editor.apply(&ctx, &input((2, 1), Tool::Primary, Phase::Released, false), &[], &mut screen);
        assert_eq!(editor.selection, Some(((0, 0), (3, 1))));
    }
}
