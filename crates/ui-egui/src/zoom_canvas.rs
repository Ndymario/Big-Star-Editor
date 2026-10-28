//! The canvas every graphics editor paints on: an image zoomed in whole steps so pixel art stays crisp, scrolled
//! and panned, with a fine and a coarse grid and a checkerboard where the image is see-through. It reports the
//! pointer in image pixels with the button and phase, and the editor decides what a press or a drag does.

use bigstar_formats::RgbaImage;
use egui::{Color32, Key, Modifiers, PointerButton, Pos2, Rect, Sense, Stroke, StrokeKind, Ui, Vec2};

use serde::{Deserialize, Serialize};

use crate::controls::{self, Button};
use crate::icons;
use crate::paint::Point;

pub const MAX_ZOOM: f32 = 32.0;

/// The zooms the buttons, keys and wheel step through: whole numbers, so every image pixel is the same size on screen.
const ZOOM_STEPS: [f32; 11] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 8.0, 12.0, 16.0, 24.0, MAX_ZOOM];

/// How far the wheel's zoom must add up to before it takes a step.
const WHEEL_STEP: f32 = 1.2;

/// How the canvas is looked at, kept by the editor between frames.
#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    /// Screen points per image pixel; `None` fits the image to the view in whole steps.
    pub zoom: Option<f32>,
    /// Fitting fills the width and scrolls the rest, as a picker beside a map does, rather than showing it all.
    pub fit_width: bool,
    /// The button held down on the canvas, so its release is reported even off the canvas.
    down: Option<Tool>,
    /// The zoom fit settled on, for the buttons to step from.
    shown_zoom: f32,
    /// Where the scroll should move to next frame, after a pan or a zoom about the pointer.
    scroll_to: Option<Vec2>,
    /// Ctrl+wheel movement not yet taken as a step: a wheel notch, or a touchpad's glide, arrives over many frames.
    wheel: f32,
}

impl Default for CanvasState {
    fn default() -> Self {
        Self { zoom: None, fit_width: false, down: None, shown_zoom: 1.0, scroll_to: None, wheel: 1.0 }
    }
}

impl CanvasState {
    /// A canvas starting at `zoom`, or fitted.
    pub fn new(zoom: Option<f32>) -> Self {
        Self { zoom, ..Self::default() }
    }

    /// A canvas that fits its width and scrolls the rest, as a picker does.
    pub fn fitting_width() -> Self {
        Self { fit_width: true, ..Self::default() }
    }

    /// Moves to the next zoom step up (`1`) or down (`-1`).
    pub fn zoom_by(&mut self, steps: i32) {
        let current = self.zoom.unwrap_or(self.shown_zoom);
        let next = if steps > 0 {
            ZOOM_STEPS.iter().copied().find(|&z| z > current).unwrap_or(MAX_ZOOM)
        } else {
            ZOOM_STEPS.iter().rev().copied().find(|&z| z < current).unwrap_or(1.0)
        };
        self.zoom = Some(next);
    }
}

/// Which grids the canvases draw, by what they mark. Shared by every canvas, so a grid turned off on one is off on
/// all, and remembered between runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Grids {
    pub pixel: bool,
    pub tile: bool,
    pub block: bool,
}

impl Default for Grids {
    fn default() -> Self {
        Self { pixel: true, tile: true, block: true }
    }
}

impl Grids {
    fn id() -> egui::Id {
        egui::Id::new("bigstar-canvas-grids")
    }

    /// The grids as they stand, if they have been set this run.
    pub fn stored(ctx: &egui::Context) -> Option<Self> {
        ctx.data(|d| d.get_temp(Self::id()))
    }

    pub fn current(ctx: &egui::Context) -> Self {
        Self::stored(ctx).unwrap_or_default()
    }

    pub fn set(self, ctx: &egui::Context) {
        ctx.data_mut(|d| d.insert_temp(Self::id(), self));
    }

    /// The switch for a grid every `step` image pixels: one pixel, a tile, or a 16-pixel block or more.
    fn switch(&mut self, step: usize) -> (&mut bool, &'static str) {
        match step {
            1 => (&mut self.pixel, "Pixel grid"),
            16.. => (&mut self.block, "Block grid"),
            _ => (&mut self.tile, "Tile grid"),
        }
    }

    fn shows(mut self, step: usize) -> bool {
        step > 0 && *self.switch(step).0
    }
}

/// Which button a gesture is made with: the primary paints the foreground, the secondary the background (or picks).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Primary,
    Secondary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Pressed,
    Held,
    Released,
}

/// What the pointer did on the canvas this frame.
#[derive(Debug, Clone, Copy, Default)]
pub struct CanvasInput {
    /// The image pixel under the pointer, which may be off the image while a drag runs past an edge.
    pub at: Option<Point>,
    pub gesture: Option<(Tool, Phase)>,
    pub modifiers: Modifiers,
    pub double_clicked: bool,
    pub hovered: bool,
}

impl CanvasInput {
    pub fn pressed(&self) -> Option<Tool> {
        self.gesture.and_then(|(tool, phase)| (phase == Phase::Pressed).then_some(tool))
    }

    /// Pressed or still held: every frame a stroke paints on.
    pub fn down(&self) -> Option<Tool> {
        self.gesture.and_then(|(tool, phase)| (phase != Phase::Released).then_some(tool))
    }

    pub fn released(&self) -> Option<Tool> {
        self.gesture.and_then(|(tool, phase)| (phase == Phase::Released).then_some(tool))
    }
}

/// A rectangle drawn over the image, in image pixels.
#[derive(Debug, Clone, Copy)]
pub struct Overlay {
    pub rect: Rect,
    pub stroke: Color32,
    pub fill: Color32,
}

impl Overlay {
    pub fn outline(rect: Rect, stroke: Color32) -> Self {
        Self { rect, stroke, fill: Color32::TRANSPARENT }
    }

    /// The cells from `a` to `b` (in cells of `size` pixels), both included.
    pub fn cells(a: Point, b: Point, size: usize, stroke: Color32) -> Self {
        let size = size as f32;
        let (left, top) = (a.0.min(b.0) as f32 * size, a.1.min(b.1) as f32 * size);
        let (right, bottom) = ((a.0.max(b.0) + 1) as f32 * size, (a.1.max(b.1) + 1) as f32 * size);
        Self::outline(Rect::from_min_max(Pos2::new(left, top), Pos2::new(right, bottom)), stroke)
    }

    pub fn filled(mut self, fill: Color32) -> Self {
        self.fill = fill;
        self
    }
}

/// The steps the canvas's grid is drawn at, in image pixels: `fine` (0 for none) and `coarse`.
#[derive(Debug, Clone, Copy)]
pub struct Steps {
    pub fine: usize,
    pub coarse: usize,
}

/// Zoom in, out, actual size and fit, and the grid toggles.
pub fn zoom_controls(ui: &mut Ui, state: &mut CanvasState, steps: Steps) {
    if Button::icon(icons::ZOOM_OUT).show(ui).on_hover_text("Zoom out (-)").clicked() {
        state.zoom_by(-1);
    }
    let label = if state.zoom.is_none() { format!("Fit {}×", state.shown_zoom) } else { format!("{}×", state.shown_zoom) };
    ui.label(label).on_hover_text("Ctrl+wheel zooms about the pointer");
    if Button::icon(icons::ZOOM_IN).show(ui).on_hover_text("Zoom in (+)").clicked() {
        state.zoom_by(1);
    }
    if Button::new("1:1").small().show(ui).on_hover_text("Actual size").clicked() {
        state.zoom = Some(1.0);
    }
    if Button::new("Fit").small().selected(state.zoom.is_none()).show(ui).on_hover_text("Fit the view (0)").clicked() {
        state.zoom = None;
    }
    let mut grids = Grids::current(ui.ctx());
    for step in [steps.fine, steps.coarse] {
        if step > 0 {
            let (on, label) = grids.switch(step);
            controls::toggle(ui, on, label);
        }
    }
    if Grids::stored(ui.ctx()) != Some(grids) {
        grids.set(ui.ctx());
    }
}

fn checkerboard(painter: &egui::Painter, rect: Rect, visible: Rect, dark: bool) {
    const SQUARE: f32 = 8.0;
    let (light, shade) = if dark { (Color32::from_gray(58), Color32::from_gray(44)) } else { (Color32::from_gray(236), Color32::from_gray(212)) };
    painter.rect_filled(rect, 0.0, light);
    let area = rect.intersect(visible);
    if !area.is_positive() {
        return;
    }
    let first_x = ((area.left() - rect.left()) / SQUARE).floor() as i64;
    let first_y = ((area.top() - rect.top()) / SQUARE).floor() as i64;
    let mut mesh = egui::Mesh::default();
    let mut y = first_y;
    while rect.top() + y as f32 * SQUARE < area.bottom() {
        let mut x = first_x;
        while rect.left() + x as f32 * SQUARE < area.right() {
            if (x + y) % 2 == 0 {
                let min = rect.min + Vec2::new(x as f32, y as f32) * SQUARE;
                mesh.add_colored_rect(Rect::from_min_size(min, Vec2::splat(SQUARE)).intersect(rect), shade);
            }
            x += 1;
        }
        y += 1;
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn grid_lines(painter: &egui::Painter, rect: Rect, visible: Rect, zoom: f32, step: usize, stroke: Stroke) {
    let step = step as f32 * zoom;
    if step < 4.0 {
        return;
    }
    let area = rect.intersect(visible);
    let mut x = rect.left() + ((area.left() - rect.left()) / step).floor() * step;
    while x <= area.right() + 0.5 {
        painter.line_segment([Pos2::new(x, area.top()), Pos2::new(x, area.bottom())], stroke);
        x += step;
    }
    let mut y = rect.top() + ((area.top() - rect.top()) / step).floor() * step;
    while y <= area.bottom() + 0.5 {
        painter.line_segment([Pos2::new(area.left(), y), Pos2::new(area.right(), y)], stroke);
        y += step;
    }
}

/// Uploads `image` when `version` moves on.
pub fn upload(ui: &Ui, name: &str, image: &RgbaImage, version: u64, texture: &mut Option<(u64, egui::TextureHandle)>) {
    if texture.as_ref().is_none_or(|(shown, _)| *shown != version) {
        let colors = egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.pixels);
        let handle = ui.ctx().load_texture(format!("graphics:{name}"), colors, egui::TextureOptions::NEAREST);
        *texture = Some((version, handle));
    }
}

/// The height a canvas leaves free for the one line of status text drawn under it.
pub fn below(ui: &Ui) -> f32 {
    ui.text_style_height(&egui::TextStyle::Body) + ui.spacing().item_spacing.y
}

/// Draws the canvas `max_height` tall at most, filling the width. `texture` caches the upload for `version`.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    name: &str,
    image: &RgbaImage,
    version: u64,
    texture: &mut Option<(u64, egui::TextureHandle)>,
    state: &mut CanvasState,
    steps: Steps,
    overlays: &[Overlay],
    max_height: f32,
) -> CanvasInput {
    let mut input = CanvasInput::default();
    if image.width == 0 || image.height == 0 || image.pixels.len() < image.width * image.height * 4 {
        ui.weak("Nothing to draw.");
        return input;
    }
    upload(ui, name, image, version, texture);
    let available = Vec2::new(ui.available_width(), max_height.min(ui.available_height()).max(64.0));
    let across = (available.x - 16.0) / image.width as f32;
    let fit = if state.fit_width { across } else { across.min((available.y - 16.0) / image.height as f32) }.floor().clamp(1.0, 16.0);
    let zoom = state.zoom.unwrap_or(fit).clamp(1.0, MAX_ZOOM);
    state.shown_zoom = zoom;
    let mut area = egui::ScrollArea::both().id_salt(("canvas", name)).max_height(available.y).auto_shrink([false, false]);
    if let Some(offset) = state.scroll_to.take() {
        area = area.scroll_offset(offset);
    }
    let mut next_zoom = None;
    let output = area.show(ui, |ui| {
        let size = Vec2::new(image.width as f32, image.height as f32) * zoom;
        let (rect, response) = ui.allocate_exact_size(size.max(Vec2::splat(1.0)), Sense::click_and_drag());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "GraphicsCanvas"));
        let visible = ui.clip_rect();
        let painter = ui.painter_at(rect);
        checkerboard(&painter, rect, visible, ui.visuals().dark_mode);
        if let Some((_, texture)) = texture {
            painter.image(texture.id(), rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        let weak = ui.visuals().weak_text_color();
        let grids = Grids::current(ui.ctx());
        if grids.shows(steps.fine) {
            grid_lines(&painter, rect, visible, zoom, steps.fine, Stroke::new(1.0, weak.gamma_multiply(0.25)));
        }
        if grids.shows(steps.coarse) {
            grid_lines(&painter, rect, visible, zoom, steps.coarse, Stroke::new(1.0, weak.gamma_multiply(0.6)));
        }
        for overlay in overlays {
            let shown = Rect::from_min_max(rect.min + overlay.rect.min.to_vec2() * zoom, rect.min + overlay.rect.max.to_vec2() * zoom);
            painter.rect(shown, 0.0, overlay.fill, Stroke::new(2.0, overlay.stroke), StrokeKind::Inside);
        }

        let (pointer, modifiers, space) = ui.input(|i| (i.pointer.latest_pos(), i.modifiers, i.key_down(Key::Space)));
        input.modifiers = modifiers;
        input.hovered = response.hovered();
        if let Some(pointer) = pointer
            && (response.hovered() || state.down.is_some())
        {
            let at = (pointer - rect.min) / zoom;
            input.at = Some((at.x.floor() as i64, at.y.floor() as i64));
        }
        // The middle button, or Space with the primary, pans instead of painting.
        let panning = response.dragged_by(PointerButton::Middle) || (space && response.dragged_by(PointerButton::Primary));
        if panning {
            state.scroll_to = Some(-response.drag_delta());
        }
        let held = |button| ui.input(|i| i.pointer.button_down(button));
        let pressed = |button| ui.input(|i| i.pointer.button_pressed(button));
        match state.down {
            Some(tool) => {
                let button = if tool == Tool::Primary { PointerButton::Primary } else { PointerButton::Secondary };
                if held(button) {
                    input.gesture = Some((tool, Phase::Held));
                } else {
                    input.gesture = Some((tool, Phase::Released));
                    state.down = None;
                }
            }
            None if response.hovered() && !space => {
                for (button, tool) in [(PointerButton::Primary, Tool::Primary), (PointerButton::Secondary, Tool::Secondary)] {
                    if pressed(button) {
                        input.gesture = Some((tool, Phase::Pressed));
                        state.down = Some(tool);
                        break;
                    }
                }
            }
            None => {}
        }
        input.double_clicked = response.double_clicked();
        if response.hovered() {
            let typing = ui.ctx().egui_wants_keyboard_input();
            let (zoom_delta, typed) = ui.input(|i| {
                let typed = if typing {
                    None
                } else if i.key_pressed(Key::Plus) || i.key_pressed(Key::Equals) {
                    Some(1)
                } else if i.key_pressed(Key::Minus) {
                    Some(-1)
                } else if i.key_pressed(Key::Num0) {
                    Some(0)
                } else {
                    None
                };
                (i.zoom_delta(), typed)
            });
            if (zoom_delta - 1.0).abs() > 0.001 {
                state.wheel *= zoom_delta;
                if state.wheel >= WHEEL_STEP || state.wheel <= 1.0 / WHEEL_STEP {
                    next_zoom = Some((if state.wheel > 1.0 { 1 } else { -1 }, pointer));
                    state.wheel = 1.0;
                }
            } else if let Some(typed) = typed {
                next_zoom = Some((typed, pointer));
            }
        }
        rect
    });
    // A pan is recorded as a delta inside; it becomes the offset the next frame scrolls to.
    if let Some(delta) = state.scroll_to.as_mut() {
        *delta = (output.state.offset + *delta).max(Vec2::ZERO);
    }
    if let Some((steps, pointer)) = next_zoom {
        if steps == 0 {
            state.zoom = None;
        } else {
            state.zoom_by(steps);
        }
        // Keep the image pixel under the pointer where it is.
        let new_zoom = state.zoom.unwrap_or(zoom);
        if let Some(pointer) = pointer {
            let within = pointer - output.inner_rect.min;
            let image_point = (within + output.state.offset) / zoom;
            state.scroll_to = Some((image_point * new_zoom - within).max(Vec2::ZERO));
        }
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_one_whole_number_at_a_time_and_stops_at_the_ends() {
        let mut state = CanvasState { zoom: Some(1.0), ..CanvasState::default() };
        let mut seen = Vec::new();
        for _ in 0..12 {
            state.zoom_by(1);
            seen.push(state.zoom.unwrap());
        }
        assert_eq!(seen[..4], [2.0, 3.0, 4.0, 5.0]);
        assert_eq!(state.zoom, Some(MAX_ZOOM));
        state.zoom = Some(7.0);
        state.zoom_by(-1);
        assert_eq!(state.zoom, Some(6.0), "a zoom between steps goes to the step below");
    }
}
