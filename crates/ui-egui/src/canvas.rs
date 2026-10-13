//! The schematic level canvas: every kind of entity as a plain shape. Real tiles and actors come in a later phase; the
//! gestures here (click, drag, pan, zoom) are the ones that stay.

use bigstar_protocol::LevelEdit;
use egui::{Align2, Color32, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

use crate::level_view::{DragMode, Item, Kind, LevelView, TILE};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 8.0;
/// A click this close to a thin thing (a node, an edge) counts, in screen pixels.
const REACH: f32 = 6.0;
/// Level used for framing when it has nothing in it.
const EMPTY_FRAME: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(512.0, 256.0));

struct Palette {
    object: Color32,
    sprite: Color32,
    entrance: Color32,
    view: Color32,
    zone: Color32,
    path: Color32,
    progress: Color32,
    selected: Color32,
    grid: Color32,
    grid_major: Color32,
}

impl Palette {
    fn get(dark: bool) -> Self {
        let (grid, grid_major) =
            if dark { (Color32::from_gray(44), Color32::from_gray(70)) } else { (Color32::from_gray(222), Color32::from_gray(190)) };
        Self {
            object: Color32::from_rgb(70, 130, 200),
            sprite: Color32::from_rgb(214, 90, 60),
            entrance: Color32::from_rgb(70, 170, 90),
            view: Color32::from_rgb(90, 150, 230),
            zone: Color32::from_rgb(220, 160, 50),
            path: Color32::from_rgb(160, 90, 200),
            progress: Color32::from_rgb(40, 170, 150),
            selected: Color32::from_rgb(255, 214, 0),
            grid,
            grid_major,
        }
    }
}

struct Frame {
    origin: Pos2,
    pan: Vec2,
    zoom: f32,
}

impl Frame {
    fn screen(&self, p: Pos2) -> Pos2 {
        self.origin + (p.to_vec2() - self.pan) * self.zoom
    }

    fn world(&self, s: Pos2) -> Pos2 {
        (self.pan + (s - self.origin) / self.zoom).to_pos2()
    }

    fn rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.screen(r.min), self.screen(r.max))
    }
}

fn fit(view: &mut LevelView, size: Vec2) {
    let bounds = view.content_bounds().unwrap_or(EMPTY_FRAME);
    let zoom = (size.x / bounds.width().max(1.0)).min(size.y / bounds.height().max(1.0)) * 0.9;
    view.zoom = zoom.clamp(MIN_ZOOM, 4.0);
    view.pan = bounds.center().to_vec2() - size / view.zoom / 2.0;
    view.needs_fit = false;
}

/// Draws the level and reacts to the pointer. Returns an edit when a gesture finished that changes the level.
pub fn show(ui: &mut Ui, view: &mut LevelView, busy: bool) -> Option<LevelEdit> {
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return None;
    }
    view.canvas = rect;
    if view.needs_fit {
        fit(view, rect.size());
    }

    // Zoom about the pointer, so what is under it stays put.
    if response.hovered() {
        let (scroll, pinch) = ui.input(|i| (i.smooth_scroll_delta.y, i.zoom_delta()));
        let factor = pinch * (1.0 + scroll * 0.002);
        if (factor - 1.0).abs() > f32::EPSILON
            && let Some(pointer) = response.hover_pos()
        {
            let before = Frame { origin: rect.min, pan: view.pan, zoom: view.zoom }.world(pointer);
            view.zoom = (view.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
            view.pan = before.to_vec2() - (pointer - rect.min) / view.zoom;
        }
    }

    let frame = Frame { origin: rect.min, pan: view.pan, zoom: view.zoom };
    let mut edit = None;

    // Pointer: a click selects, a drag on a thing moves it, a drag on nothing (or with the middle button) pans.
    let shift = ui.input(|i| i.modifiers.shift);
    let reach = REACH / view.zoom;
    if let Some(pointer) = response.interact_pointer_pos().or_else(|| response.hover_pos()) {
        let under = view.hit(frame.world(pointer), reach);
        if response.hovered() && under.is_some() && view.drag.is_none() {
            ui.set_cursor_icon(CursorIcon::Grab);
        }
        if response.drag_started_by(PointerButton::Primary) && !busy {
            // The pointer has already moved a little by the time a drag is recognised; what was grabbed is what was under it
            // when the button went down.
            let grabbed = ui.input(|i| i.pointer.press_origin()).and_then(|origin| view.hit(frame.world(origin), reach));
            match grabbed {
                Some(item) => {
                    if !view.selection.contains(&item) {
                        if !shift {
                            view.selection.clear();
                        }
                        view.selection.insert(item);
                    }
                    view.drag = Some(DragMode::Move { delta: Vec2::ZERO, submitted: false });
                }
                None => view.drag = Some(DragMode::Pan),
            }
        } else if response.drag_started_by(PointerButton::Middle) {
            view.drag = Some(DragMode::Pan);
        } else if response.clicked_by(PointerButton::Primary) {
            match under {
                Some(item) if shift => {
                    if !view.selection.remove(&item) {
                        view.selection.insert(item);
                    }
                }
                Some(item) => view.selection = std::iter::once(item).collect(),
                None if !shift => view.selection.clear(),
                None => {}
            }
        }
    }
    if response.dragged() {
        let moved = response.drag_delta();
        match &mut view.drag {
            Some(DragMode::Pan) => {
                ui.set_cursor_icon(CursorIcon::Grabbing);
                view.pan -= moved / view.zoom;
            }
            Some(DragMode::Move { delta, submitted: false }) => {
                ui.set_cursor_icon(CursorIcon::Grabbing);
                *delta += moved / view.zoom;
            }
            _ => {}
        }
    }
    if response.drag_stopped() {
        match view.drag {
            Some(DragMode::Move { delta, submitted: false }) => {
                edit = view.move_edit();
                // The moved things stay where they were dropped until the host answers.
                view.drag = if edit.is_some() { Some(DragMode::Move { delta, submitted: true }) } else { None };
            }
            Some(DragMode::Pan) => view.drag = None,
            _ => {}
        }
    }

    paint(ui, view, &frame, rect);
    edit
}

fn paint(ui: &Ui, view: &LevelView, frame: &Frame, rect: Rect) {
    let dark = ui.visuals().dark_mode;
    let colors = Palette::get(dark);
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);

    // Grid: tiles when they are big enough to tell apart, and every sixteenth tile (one screen) always.
    let (top_left, bottom_right) = (frame.world(rect.min), frame.world(rect.max));
    if view.zoom * TILE >= 6.0 {
        let step = TILE;
        let mut x = (top_left.x / step).floor() * step;
        while x <= bottom_right.x {
            let major = (x / (step * 16.0)).fract() == 0.0;
            let sx = frame.screen(Pos2::new(x, 0.0)).x;
            painter.line_segment(
                [Pos2::new(sx, rect.min.y), Pos2::new(sx, rect.max.y)],
                Stroke::new(1.0, if major { colors.grid_major } else { colors.grid }),
            );
            x += step;
        }
        let mut y = (top_left.y / step).floor() * step;
        while y <= bottom_right.y {
            let major = (y / (step * 16.0)).fract() == 0.0;
            let sy = frame.screen(Pos2::new(0.0, y)).y;
            painter.line_segment(
                [Pos2::new(rect.min.x, sy), Pos2::new(rect.max.x, sy)],
                Stroke::new(1.0, if major { colors.grid_major } else { colors.grid }),
            );
            y += step;
        }
    }

    let text = ui.visuals().text_color();
    let label = |at: Pos2, anchor: Align2, text_value: String, color: Color32| {
        if view.zoom >= 0.5 {
            painter.text(at, anchor, text_value, FontId::proportional(11.0), color);
        }
    };
    let selected = |item: Item| view.selection.contains(&item);
    let outline = |item: Item, color: Color32| if selected(item) { Stroke::new(2.5, colors.selected) } else { Stroke::new(1.0, color) };

    let course = &view.snapshot.course;
    for (index, zone) in course.zones.iter().enumerate() {
        let item = (Kind::Zone, index);
        if let Some(r) = view.bounds(item) {
            let r = frame.rect(r.translate(view.offset(item)));
            painter.rect_stroke(r, 0.0, outline(item, colors.zone), StrokeKind::Inside);
            label(r.left_top() + Vec2::new(4.0, 2.0), Align2::LEFT_TOP, format!("Zone {}", zone.id), colors.zone);
        }
    }
    for (index, v) in course.views.iter().enumerate() {
        let item = (Kind::View, index);
        if let Some(r) = view.bounds(item) {
            let r = frame.rect(r.translate(view.offset(item)));
            painter.rect_filled(r, 0.0, colors.view.gamma_multiply(0.06));
            painter.rect_stroke(r, 0.0, outline(item, colors.view), StrokeKind::Inside);
            label(r.right_bottom() - Vec2::new(4.0, 2.0), Align2::RIGHT_BOTTOM, format!("View {}", v.id), colors.view);
        }
    }
    for (index, object) in view.snapshot.objects.iter().enumerate() {
        let item = (Kind::Object, index);
        if let Some(r) = view.bounds(item) {
            let r = frame.rect(r.translate(view.offset(item)));
            painter.rect_filled(r, 0.0, colors.object.gamma_multiply(0.55));
            painter.rect_stroke(r, 0.0, outline(item, colors.object), StrokeKind::Inside);
            label(r.left_top() + Vec2::new(3.0, 1.0), Align2::LEFT_TOP, format!("{}", object.object_id), text);
        }
    }
    for (kind, list, color) in [(Kind::Path, &course.paths, colors.path), (Kind::ProgressPath, &course.progress_paths, colors.progress)] {
        for (index, path) in list.iter().enumerate() {
            let item = (kind, index);
            let offset = view.offset(item);
            let points: Vec<Pos2> =
                path.nodes.iter().map(|n| frame.screen(Pos2::new(f32::from(n.x) + TILE / 2.0, f32::from(n.y) + TILE / 2.0) + offset)).collect();
            let radius = (TILE / 2.0 * view.zoom).max(3.0);
            let stroke = outline(item, color);
            painter.add(Shape::line(points.clone(), stroke));
            for point in &points {
                painter.circle_filled(*point, radius, color);
                painter.circle_stroke(*point, radius, Stroke::new(1.0, text));
            }
            if let Some(first) = points.first() {
                label(*first + Vec2::new(6.0, -6.0), Align2::LEFT_BOTTOM, format!("Path {}", path.id), color);
            }
        }
    }
    for (index, entrance) in course.entrances.iter().enumerate() {
        let item = (Kind::Entrance, index);
        if let Some(r) = view.bounds(item) {
            let r = frame.rect(r.translate(view.offset(item)));
            painter.circle_filled(r.center(), r.width().max(8.0) / 2.0, colors.entrance.gamma_multiply(0.8));
            painter.circle_stroke(r.center(), r.width().max(8.0) / 2.0, outline(item, colors.entrance));
            label(r.center(), Align2::CENTER_CENTER, format!("{}", entrance.id), Color32::WHITE);
        }
    }
    for (index, sprite) in course.sprites.iter().enumerate() {
        let item = (Kind::Sprite, index);
        if let Some(r) = view.bounds(item) {
            let r = frame.rect(r.translate(view.offset(item)));
            painter.rect_filled(r, 3.0, colors.sprite.gamma_multiply(0.85));
            painter.rect_stroke(r, 3.0, outline(item, colors.sprite), StrokeKind::Inside);
            label(r.center(), Align2::CENTER_CENTER, format!("{}", sprite.id), Color32::WHITE);
        }
    }
}
