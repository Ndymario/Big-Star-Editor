//! The level canvas: the tile array, the actors and every other entity, drawn from what the host serves, and the
//! gestures that edit them. A port of `LevelPreviewWidget`'s painting and input; what a gesture means is decided in
//! `level_view`, which can be tested without a window.

use std::collections::HashMap;
use std::sync::Arc;

use bigstar_level::{LEVEL_CHUNK_CELLS, LevelTilemap};
use bigstar_protocol::{ATLAS_COLUMNS, DrawCommand, DrawKind, LevelEdit, TextureData, TextureId};
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, Event, FontId, Key, Mesh, MouseWheelUnit, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, ViewportCommand,
};

use crate::clipboard;
use crate::controls::{self, Button, Choice as _};
use crate::level_view::{Gesture, Handle, Item, Kind, LevelView, TILE, fit_zoom, handle_centres, node_rect};
use crate::properties::Tab;

/// The textures the host has sent, uploaded the first time something draws them.
#[derive(Default)]
pub struct TexturePool {
    uploaded: HashMap<TextureId, egui::TextureHandle>,
}

impl TexturePool {
    fn get(&mut self, ctx: &egui::Context, store: &HashMap<TextureId, Arc<TextureData>>, id: &str) -> Option<(egui::TextureId, Vec2)> {
        if let Some(handle) = self.uploaded.get(id) {
            return Some((handle.id(), handle.size_vec2()));
        }
        let data = store.get(id)?;
        let size = [data.width as usize, data.height as usize];
        if data.rgba.len() != size[0].checked_mul(size[1])?.checked_mul(4)? {
            return None;
        }
        // Nearest-neighbour: tile and sprite art is pixel art, and a smoothed blit lets neighbouring blocks bleed.
        let image = egui::ColorImage::from_rgba_unmultiplied(size, &data.rgba);
        let handle = ctx.load_texture(format!("bigstar:{id}"), image, egui::TextureOptions::NEAREST);
        let found = (handle.id(), handle.size_vec2());
        self.uploaded.insert(id.to_owned(), handle);
        Some(found)
    }
}

/// The canvas paints itself, so every chrome colour is derived from the active theme.
struct Colors {
    surface: Color32,
    frame: Color32,
    coarse_grid: Color32,
    fine_grid: Color32,
    outline: Color32,
    label_text: Color32,
    marker: Color32,
    highlight: Color32,
    dark: bool,
}

fn blend(from: Color32, to: Color32, ratio: f32) -> Color32 {
    let mix = |a: u8, b: u8| (f32::from(a) + (f32::from(b) - f32::from(a)) * ratio).round() as u8;
    Color32::from_rgb(mix(from.r(), to.r()), mix(from.g(), to.g()), mix(from.b(), to.b()))
}

fn with_alpha(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

impl Colors {
    fn of(visuals: &egui::Visuals) -> Self {
        let (surface, text, dark) = (visuals.extreme_bg_color, visuals.text_color(), visuals.dark_mode);
        Self {
            surface,
            frame: visuals.widgets.noninteractive.bg_stroke.color,
            coarse_grid: blend(surface, text, if dark { 0.24 } else { 0.12 }),
            fine_grid: with_alpha(blend(surface, text, 0.55), 70),
            outline: blend(surface, text, 0.82),
            label_text: text,
            marker: if dark { Color32::from_rgb(246, 248, 251) } else { Color32::WHITE },
            highlight: visuals.selection.bg_fill,
            dark,
        }
    }

    /// Entity colours keep their hue; on a dark canvas their strokes are lifted so they stay visible.
    fn entity_stroke(&self, color: Color32) -> Color32 {
        let scale = |c: u8, by: f32| (f32::from(c) * by).round().min(255.0) as u8;
        let by = if self.dark { 1.4 } else { 1.0 / 1.6 };
        Color32::from_rgb(scale(color.r(), by), scale(color.g(), by), scale(color.b(), by))
    }
}

fn object_color(slot: u8) -> Color32 {
    match slot {
        0 => rgba(63, 126, 197, 135),
        1 => rgba(72, 156, 103, 145),
        2 => rgba(221, 143, 54, 145),
        _ => rgba(132, 92, 165, 125),
    }
}

const ZONE_COLOR: Color32 = Color32::from_rgb(84, 154, 110);
const VIEW_COLOR: Color32 = Color32::from_rgb(65, 122, 180);
const OUT_OF_BOUNDS: Color32 = Color32::from_rgb(198, 74, 74);
/// Knobs are a screen-space affordance, so they keep their size at any zoom.
const KNOB: f32 = 9.0;

/// Level pixels to screen points.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Frame {
    pub(crate) origin: Pos2,
    pub(crate) pan: Vec2,
    pub(crate) zoom: f32,
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

    fn world_rect(&self, r: Rect) -> Rect {
        Rect::from_min_max(self.world(r.min), self.world(r.max))
    }

    /// `rect` (level pixels) fitted into `target` (screen), as the minimap shows the level.
    fn fitted(target: Rect, rect: Rect) -> Self {
        let zoom = fit_zoom(target.size(), rect);
        Self { origin: target.min, pan: rect.center().to_vec2() - target.size() / zoom / 2.0, zoom }
    }
}

pub(crate) struct Art<'a> {
    pub(crate) ctx: &'a egui::Context,
    pub(crate) store: &'a HashMap<TextureId, Arc<TextureData>>,
    pub(crate) pool: &'a mut TexturePool,
}

impl Art<'_> {
    pub(crate) fn texture(&mut self, id: Option<&TextureId>) -> Option<(egui::TextureId, Vec2)> {
        self.pool.get(self.ctx, self.store, id?)
    }
}

/// Draws the level and reacts to the pointer and keyboard. Returns the edits a finished gesture makes, in order.
pub fn show(
    ui: &mut Ui,
    view: &mut LevelView,
    store: &HashMap<TextureId, Arc<TextureData>>,
    pool: &mut TexturePool,
    busy: bool,
    keyboard: bool,
) -> CanvasOutput {
    toolbar(ui, view);
    let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    if rect.width() < 1.0 || rect.height() < 1.0 {
        return CanvasOutput::default();
    }
    // A level nobody has zoomed stays fitted, so a resized window reframes it instead of cropping it.
    let resized = (view.canvas.size() - rect.size()).length() > 0.5;
    view.canvas = rect;
    if view.needs_fit || (resized && !view.zoom_pinned) {
        view.fit(rect.size());
    }

    let mut out = CanvasOutput::default();
    pointer(ui, &response, view, busy, &mut out.edits);
    if keyboard {
        keys(ui, view, busy || !out.edits.is_empty(), &mut out);
    }

    let colors = Colors::of(ui.visuals());
    let painter = ui.painter_at(rect);
    let tilemap = view.tilemap();
    let mut art = Art { ctx: ui.ctx(), store, pool };
    let frame = Frame { origin: rect.min, pan: view.pan, zoom: view.zoom };
    painter.rect(rect, 0.0, colors.surface, Stroke::new(1.0, colors.frame), StrokeKind::Inside);
    draw_map(&painter, view, &tilemap, frame, &mut art, &colors, true);
    overlays(&painter, view, frame, &colors);
    if view.options.show_minimap {
        minimap(&painter, view, &tilemap, frame, &mut art, &colors);
    }
    out
}

/// What the canvas did this frame: the edits to send, in order, and anything to tell the person.
#[derive(Debug, Default)]
pub struct CanvasOutput {
    pub edits: Vec<LevelEdit>,
    pub error: Option<String>,
}

fn toolbar(ui: &mut Ui, view: &mut LevelView) {
    // One line whatever is chosen, scrolling sideways when narrow: a toolbar that wrapped differently per kind would
    // resize the canvas under the person, and re-fit a level they have not zoomed.
    egui::ScrollArea::horizontal().id_salt("canvas-toolbar").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                controls::compact(ui);
                toolbar_items(ui, view);
            });
        },
    );
}

fn toolbar_items(ui: &mut Ui, view: &mut LevelView) {
    {
        // What right-click creates. Choosing it never touches the selection.
        ui.label("Create");
        controls::drop_down("create-kind").selected_text(view.placement.kind.name()).show_ui(ui, |ui| {
            for kind in Kind::ALL {
                ui.choice_value(&mut view.placement.kind, kind, kind.name());
            }
        });
        match view.placement.kind {
            Kind::Object => {
                ui.add(controls::number(&mut view.placement.object_slot).range(0..=2).prefix("slot "));
                ui.add(controls::number(&mut view.placement.object_id).range(0..=4095).prefix("object "));
            }
            Kind::Sprite => match view.placement.sprite_hash {
                Some(hash) => {
                    ui.weak(format!("module actor {hash:08x}"));
                }
                None => {
                    ui.add(controls::number(&mut view.placement.sprite_type).prefix("type "));
                }
            },
            _ => {}
        }
        ui.separator();
        let options = &mut view.options;
        controls::toggle(ui, &mut options.snap, "Snap");
        let mut eight = options.snap_size < TILE;
        if controls::toggle(ui, &mut eight, "8 px").changed() {
            options.snap_size = if eight { 8.0 } else { TILE };
        }
        controls::toggle(ui, &mut options.show_grid, "Grid");
        controls::toggle(ui, &mut options.show_minimap, "Minimap");
        controls::toggle(ui, &mut options.show_ds_screen, "DS Screen");
        ui.separator();
        if Button::new("Fit").show(ui).clicked() {
            view.needs_fit = true;
            view.zoom_pinned = false;
        }
        for (label, zoom) in [("50%", 0.5), ("100%", 1.0), ("200%", 2.0)] {
            if Button::new(label).show(ui).clicked() {
                view.zoom_about(zoom, view.canvas.center());
            }
        }
        ui.weak(format!("{:.0}%", view.zoom * 100.0));
        if Button::new("Fullscreen").show(ui).clicked() {
            let fullscreen = ui.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ui.ctx().send_viewport_cmd(ViewportCommand::Fullscreen(!fullscreen));
        }
    }
}

fn handle_cursor(handle: Handle) -> CursorIcon {
    match handle {
        Handle::TopLeft | Handle::BottomRight => CursorIcon::ResizeNwSe,
        Handle::TopRight | Handle::BottomLeft => CursorIcon::ResizeNeSw,
        Handle::Top | Handle::Bottom => CursorIcon::ResizeVertical,
        Handle::Left | Handle::Right => CursorIcon::ResizeHorizontal,
    }
}

/// Where a gesture started and where the pointer was last, kept between frames.
#[derive(Debug, Clone, Copy, Default)]
struct Press {
    world: Pos2,
    last: Pos2,
}

fn pointer(ui: &Ui, response: &egui::Response, view: &mut LevelView, busy: bool, edits: &mut Vec<LevelEdit>) {
    let id = response.id.with("press");
    let mut press: Press = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let (events, space, latest) = ui.input(|i| (i.events.clone(), i.key_down(Key::Space), i.pointer.latest_pos()));
    let over = response.contains_pointer();
    let mut busy = busy;
    for event in &events {
        match event {
            Event::PointerButton { pos, button, pressed: true, modifiers } if over && view.canvas.contains(*pos) => {
                press = Press { world: view.to_world(*pos), last: *pos };
                // Middle button, a held Space or Alt pans, whatever is under the pointer.
                if *button == PointerButton::Middle || space || modifiers.alt {
                    view.gesture = Some(Gesture::Pan);
                    continue;
                }
                // Nothing new starts while the host is still answering the last edit.
                if busy || view.gesture.is_some() {
                    continue;
                }
                if *button == PointerButton::Secondary {
                    // Right-click creates whatever is chosen; left-click never does.
                    if let Some(edit) = view.placement_edit(press.world, modifiers.alt) {
                        edits.push(edit);
                        busy = true;
                    }
                    continue;
                }
                if *button != PointerButton::Primary {
                    continue;
                }
                if let Some(handle) = view.handle_at(*pos) {
                    view.gesture = Some(Gesture::Resize { handle, delta: Vec2::ZERO, submitted: false });
                    continue;
                }
                match view.hit_test(press.world) {
                    // Ctrl-drag drags a copy.
                    Some(hit) if modifiers.command && !modifiers.shift => {
                        if let Some(edit) = view.begin_clone_drag(hit) {
                            edits.push(edit);
                            busy = true;
                        }
                    }
                    Some(hit) => {
                        let existing = view.selection.iter().position(|s| *s == hit);
                        if modifiers.shift {
                            match existing {
                                Some(at) => {
                                    view.selection.remove(at);
                                }
                                None => view.selection.push(hit),
                            }
                        } else if existing.is_none() {
                            view.selection = vec![hit];
                        }
                        if !view.selection.is_empty() {
                            view.gesture = Some(Gesture::Move { delta: Vec2::ZERO, submitted: false });
                        }
                    }
                    None => {
                        if !modifiers.shift {
                            view.selection.clear();
                        }
                        view.gesture = Some(Gesture::Marquee { from: *pos, to: *pos });
                    }
                }
            }
            Event::PointerMoved(pos) => {
                let free = ui.input(|i| i.modifiers.alt);
                match &mut view.gesture {
                    Some(Gesture::Pan) => {
                        view.pan -= (*pos - press.last) / view.zoom;
                    }
                    Some(Gesture::Move { submitted: false, .. } | Gesture::Resize { submitted: false, .. }) => {
                        let raw = view.to_world(*pos) - press.world;
                        let snapped = view.snap(raw, free);
                        if let Some(Gesture::Move { delta, .. } | Gesture::Resize { delta, .. }) = &mut view.gesture {
                            *delta = snapped;
                        }
                    }
                    Some(Gesture::Marquee { to, .. }) => *to = *pos,
                    _ => {}
                }
                press.last = *pos;
            }
            Event::PointerButton { pressed: false, .. } => match view.gesture {
                Some(Gesture::Pan) => view.gesture = None,
                Some(Gesture::Move { delta, submitted: false }) => {
                    if view.awaiting_clone() {
                        view.release_clone_drag();
                    } else {
                        let edit = view.move_edit(delta);
                        view.gesture = edit.as_ref().map(|_| Gesture::Move { delta, submitted: true });
                        edits.extend(edit);
                    }
                }
                Some(Gesture::Resize { handle, delta, submitted: false }) => {
                    let edit = view.resize_edit(handle.edges(delta));
                    view.gesture = edit.as_ref().map(|_| Gesture::Resize { handle, delta, submitted: true });
                    edits.extend(edit);
                }
                Some(Gesture::Marquee { from, to }) => {
                    let (a, b) = (view.to_world(from), view.to_world(to));
                    view.select_within(a, b);
                    view.gesture = None;
                }
                _ => {}
            },
            // A trackpad scrolls in points: two fingers pan the level, both ways, and Cmd/Ctrl turns them into zoom.
            // Pinching arrives as `Zoom` below.
            Event::MouseWheel { unit: MouseWheelUnit::Point, delta, modifiers, .. } if over && !modifiers.command => {
                view.pan -= *delta / view.zoom;
            }
            Event::MouseWheel { unit, delta, .. } if over => {
                // A mouse wheel zooms. One notch is 120 in Qt's units; egui gives a line, or about 50 points.
                let angle = match unit {
                    MouseWheelUnit::Point => delta.y * 2.4,
                    MouseWheelUnit::Line => delta.y * 120.0,
                    MouseWheelUnit::Page => delta.y * 360.0,
                };
                if angle != 0.0
                    && let Some(at) = latest
                {
                    view.zoom_about(view.zoom * 1.0015_f32.powf(angle), at);
                }
            }
            Event::Zoom(factor) if over => {
                if let Some(at) = latest {
                    view.zoom_about(view.zoom * factor, at);
                }
            }
            _ => {}
        }
    }
    ui.data_mut(|d| d.insert_temp(id, press));

    // Feedback: a knob under the pointer, otherwise a move cursor over anything that can be grabbed.
    let cursor = match view.gesture {
        Some(Gesture::Pan) => Some(CursorIcon::Grabbing),
        Some(Gesture::Resize { handle, .. }) => Some(handle_cursor(handle)),
        Some(Gesture::Move { submitted: false, .. }) => Some(CursorIcon::Move),
        _ if over && space => Some(CursorIcon::Grab),
        _ if over => {
            latest.and_then(|at| view.handle_at(at).map(handle_cursor).or_else(|| view.hit_test(view.to_world(at)).map(|_| CursorIcon::Move)))
        }
        _ => None,
    };
    if let Some(cursor) = cursor {
        ui.ctx().set_cursor_icon(cursor);
    }
}

fn keys(ui: &Ui, view: &mut LevelView, busy: bool, out: &mut CanvasOutput) {
    let events = ui.input(|i| i.events.clone());
    let mut busy = busy;
    let edits = &mut out.edits;
    for event in events {
        // The clipboard arrives as its own events rather than as keys.
        let (key, modifiers) = match event {
            Event::Key { key, pressed: true, modifiers, .. } => (key, modifiers),
            Event::Copy | Event::Cut => {
                match clipboard::copy(view) {
                    Ok(Some(text)) => ui.ctx().copy_text(text),
                    Ok(None) => {}
                    Err(error) => out.error = Some(error),
                }
                if matches!(event, Event::Cut)
                    && !busy
                    && let Some(edit) = view.remove_edit()
                {
                    view.selection.clear();
                    edits.push(edit);
                    busy = true;
                }
                continue;
            }
            Event::Paste(text) if !busy => {
                match clipboard::paste(&text) {
                    Ok(items) => {
                        edits.push(LevelEdit::Add { items, name: "Paste selection".to_owned() });
                        busy = true;
                    }
                    Err(error) => out.error = Some(error),
                }
                continue;
            }
            _ => continue,
        };
        let plain = modifiers.is_none();
        // Sends an edit unless one is already on its way; says whether it went.
        let mut send = |edit: Option<LevelEdit>| match edit.filter(|_| !busy) {
            Some(edit) => {
                edits.push(edit);
                busy = true;
                true
            }
            None => false,
        };
        match key {
            Key::Delete | Key::Backspace if plain => {
                let edit = view.remove_edit();
                if send(edit) {
                    view.selection.clear();
                }
            }
            Key::D if modifiers.command && !modifiers.shift && !modifiers.alt => {
                send(view.clone_edit());
            }
            Key::PageUp | Key::PageDown => {
                send(view.reorder_edit(key == Key::PageUp));
            }
            Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown if plain && view.gesture.is_none() => {
                let step = view.nudge_step();
                let delta = match key {
                    Key::ArrowLeft => Vec2::new(-step, 0.0),
                    Key::ArrowRight => Vec2::new(step, 0.0),
                    Key::ArrowUp => Vec2::new(0.0, -step),
                    _ => Vec2::new(0.0, step),
                };
                send(view.move_edit(delta));
            }
            // The kind shortcuts come first, so S chooses sprites rather than scrolling down. Each takes the properties to
            // that kind's first item, as the Qt editor's do.
            Key::C if plain => view.choose_kind(Tab::Course),
            Key::O if plain => view.choose_kind(Tab::Entity(Kind::Object)),
            Key::S if plain => view.choose_kind(Tab::Entity(Kind::Sprite)),
            Key::E if plain => view.choose_kind(Tab::Entity(Kind::Entrance)),
            Key::Z if plain => view.choose_kind(Tab::Entity(Kind::Zone)),
            Key::P if plain => view.choose_kind(Tab::Entity(Kind::Path)),
            Key::G if plain => view.choose_kind(Tab::Entity(Kind::ProgressPath)),
            Key::V if plain => view.choose_kind(Tab::Entity(Kind::View)),
            Key::W if plain => view.pan.y -= 32.0 / view.zoom,
            Key::A if plain => view.pan.x -= 32.0 / view.zoom,
            Key::D if plain => view.pan.x += 32.0 / view.zoom,
            // Escape abandons the gesture in progress; nothing it would have done happens.
            Key::Escape
                if matches!(
                    view.gesture,
                    Some(Gesture::Move { submitted: false, .. } | Gesture::Resize { submitted: false, .. } | Gesture::Marquee { .. })
                ) && !view.awaiting_clone() =>
            {
                view.gesture = None;
            }
            _ => {}
        }
    }
}

/// The selection marker, in screen points.
fn marker_stroke(colors: &Colors) -> Stroke {
    Stroke::new(3.0, colors.marker)
}

fn knobs(painter: &Painter, rect: Rect, colors: &Colors) {
    for centre in handle_centres(rect) {
        let knob = Rect::from_center_size(centre, Vec2::splat(KNOB));
        painter.rect(knob, 0.0, colors.marker, Stroke::new(1.0, colors.outline), StrokeKind::Inside);
    }
}

fn scaled_font(zoom: f32, size: f32) -> Option<FontId> {
    let pixels = size * zoom;
    (pixels >= 5.0).then(|| FontId::proportional(pixels.min(64.0)))
}

#[allow(clippy::too_many_lines)]
fn draw_map(painter: &Painter, view: &LevelView, tilemap: &LevelTilemap, frame: Frame, art: &mut Art<'_>, colors: &Colors, live: bool) {
    let course = &view.snapshot.course;
    let bounds = view.scene_bounds();
    let clip = painter.clip_rect();
    let visible = frame.world_rect(clip);
    let selected = |kind: Kind, index: usize, node: Option<usize>| live && view.is_selected(kind, index, node);
    let preview = |kind: Kind, index: usize, rect: Rect| if live { view.preview(kind, index, rect) } else { rect };
    // Where an entity is drawn: where it is, moved or resized by the gesture in progress.
    let placed = |kind: Kind, index: usize| preview(kind, index, view.bounds(Item::new(kind, index)).unwrap_or(Rect::NOTHING));
    let hairline = |color: Color32| Stroke::new(1.0, color);
    let full_uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));

    let graphics = view.art.graphics.as_ref();
    if let Some((texture, _)) = art.texture(graphics.and_then(|g| g.background.texture.as_ref())) {
        painter.image(texture, frame.rect(bounds), full_uv, Color32::WHITE.gamma_multiply(0.55));
    }
    // A coarse grid of screens over the level.
    let grid = 256.0;
    let mut x = (bounds.left() / grid).floor() * grid;
    while x <= bounds.right() {
        painter.line_segment([frame.screen(Pos2::new(x, bounds.top())), frame.screen(Pos2::new(x, bounds.bottom()))], hairline(colors.coarse_grid));
        x += grid;
    }
    let mut y = (bounds.top() / grid).floor() * grid;
    while y <= bounds.bottom() {
        painter.line_segment([frame.screen(Pos2::new(bounds.left(), y)), frame.screen(Pos2::new(bounds.right(), y))], hairline(colors.coarse_grid));
        y += grid;
    }

    // Level coordinates start at the origin, so everything above or left of it is off the map. The strip is tinted and
    // its edge drawn so the limit shows while something is dragged towards it; it goes under the entities, so anything
    // that did end up out there stays readable.
    let region = bounds.union(visible);
    if region.left() < 0.0 || region.top() < 0.0 {
        let tint = with_alpha(OUT_OF_BOUNDS, if colors.dark { 44 } else { 34 });
        if region.left() < 0.0 {
            painter.rect_filled(frame.rect(Rect::from_min_max(region.min, Pos2::new(0.0, region.bottom()))), 0.0, tint);
        }
        if region.top() < 0.0 {
            painter.rect_filled(
                frame.rect(Rect::from_min_max(Pos2::new(region.left().max(0.0), region.top()), Pos2::new(region.right(), 0.0))),
                0.0,
                tint,
            );
        }
        let edge = hairline(with_alpha(OUT_OF_BOUNDS, 165));
        if region.left() < 0.0 && region.bottom() > 0.0 {
            painter.line_segment([frame.screen(Pos2::new(0.0, region.top().max(0.0))), frame.screen(Pos2::new(0.0, region.bottom()))], edge);
        }
        if region.top() < 0.0 && region.right() > 0.0 {
            painter.line_segment([frame.screen(Pos2::new(region.left().max(0.0), 0.0)), frame.screen(Pos2::new(region.right(), 0.0))], edge);
        }
    }

    // Zones and views are containers: the tint goes underneath, where it cannot wash out what they hold, and the
    // boundary goes over everything at the end.
    for (kind, count, tint) in
        [(Kind::Zone, course.zones.len(), with_alpha(ZONE_COLOR, 34)), (Kind::View, course.views.len(), with_alpha(VIEW_COLOR, 25))]
    {
        for index in 0..count {
            painter.rect_filled(frame.rect(placed(kind, index)), 0.0, tint);
        }
    }

    // The tile art is the array, not the objects: whichever object won a cell has already won it.
    tiles(painter, view, tilemap, frame, visible, art);
    for (index, object) in view.snapshot.objects.iter().enumerate() {
        let rect = frame.rect(placed(Kind::Object, index));
        // An object that put no cells in the array shows as a plain rectangle rather than as nothing at all.
        if !view.object_layable(object) {
            painter.rect(rect, CornerRadius::same(2), object_color(object.tileset_slot), hairline(colors.outline), StrokeKind::Inside);
        }
        if selected(Kind::Object, index, None) {
            painter.rect_stroke(rect, 0.0, marker_stroke(colors), StrokeKind::Middle);
            knobs(painter, rect, colors);
        }
    }

    for (index, sprite) in course.sprites.iter().enumerate() {
        let moving = selected(Kind::Sprite, index, None);
        let offset = if moving { view.move_delta() } else { Vec2::ZERO };
        let grab = view.sprite_rect(index, false).unwrap_or(Rect::NOTHING).translate(offset);
        let render = view.sprite_rect(index, true).unwrap_or(Rect::NOTHING).translate(offset);
        let mut rendered = false;
        if let Some((draw, moved)) = view.actor_draw(index) {
            for command in &draw.commands {
                paint_command(painter, frame, command, moved + offset, art);
            }
            rendered = draw.custom_rendered;
        } else if view.actor_pending(index) {
            // Its draw list is on its way; a marker for a frame would only flicker.
            rendered = true;
        }
        // An actor with no rendering program of its own gets the stock marker and its number.
        if !rendered {
            let r = frame.rect(render);
            painter.rect(r, CornerRadius::same(2), rgba(110, 110, 110, 190), hairline(colors.entity_stroke(colors.label_text)), StrokeKind::Inside);
            if let Some(font) = scaled_font(frame.zoom, 10.0) {
                painter.text(r.center(), Align2::CENTER_CENTER, sprite.id.to_string(), font.clone(), colors.label_text);
                let hash = view.sprite_hash(index);
                let label = match (view.art.actors.actor(sprite.id, hash), hash) {
                    (Some(actor), _) => actor.definition.name.clone(),
                    (None, Some(hash)) => format!("0x{hash:08x} (foreign)"),
                    (None, None) => format!("Actor {}", sprite.id),
                };
                painter.text(Pos2::new(r.right() + 2.0 * frame.zoom, r.top() - 2.0 * frame.zoom), Align2::LEFT_TOP, label, font, colors.label_text);
            }
        }
        if moving {
            painter.rect_stroke(frame.rect(grab.expand(3.0)), 0.0, marker_stroke(colors), StrokeKind::Middle);
        }
    }

    for index in 0..course.entrances.len() {
        let rect = frame.rect(placed(Kind::Entrance, index));
        painter.add(Shape::ellipse_filled(rect.center(), rect.size() / 2.0, rgba(92, 190, 214, 190)));
        painter.add(Shape::ellipse_stroke(rect.center(), rect.size() / 2.0, hairline(Color32::from_rgb(34, 94, 122))));
        if selected(Kind::Entrance, index, None) {
            let ring = rect.expand(3.0 * frame.zoom);
            painter.add(Shape::ellipse_stroke(ring.center(), ring.size() / 2.0, marker_stroke(colors)));
        }
    }

    for (kind, paths, color) in
        [(Kind::Path, &course.paths, Color32::from_rgb(199, 76, 76)), (Kind::ProgressPath, &course.progress_paths, Color32::from_rgb(155, 92, 180))]
    {
        let pen = Stroke::new(2.0, color);
        for (index, path) in paths.iter().enumerate() {
            let node_at = |node: usize| {
                let offset = if live { view.node_offset(kind, index, node) } else { Vec2::ZERO };
                path.nodes.get(node).map(|n| node_rect(n).translate(offset))
            };
            for node in 1..path.nodes.len() {
                if let (Some(before), Some(after)) = (node_at(node - 1), node_at(node)) {
                    painter.line_segment([frame.screen(before.center()), frame.screen(after.center())], pen);
                }
            }
            for node in 0..path.nodes.len() {
                let Some(rect) = node_at(node).map(|r| frame.rect(r)) else { continue };
                let fill = if node == 0 { Color32::from_rgb(79, 178, 98) } else { Color32::from_rgb(235, 105, 95) };
                painter.add(Shape::ellipse_filled(rect.center(), rect.size() / 2.0, fill));
                painter.add(Shape::ellipse_stroke(rect.center(), rect.size() / 2.0, pen));
                if selected(kind, index, Some(node)) {
                    let ring = rect.expand(3.0 * frame.zoom);
                    painter.add(Shape::ellipse_stroke(ring.center(), ring.size() / 2.0, marker_stroke(colors)));
                }
            }
        }
    }

    if let Some((texture, _)) = art.texture(graphics.and_then(|g| g.foreground.texture.as_ref())) {
        painter.image(texture, frame.rect(bounds), full_uv, Color32::WHITE.gamma_multiply(0.35));
    }

    // Last, so nothing -- not even the foreground -- dims the edges a zone or a view is sized against.
    for (kind, count, color, accent) in [
        (Kind::Zone, course.zones.len(), ZONE_COLOR, Color32::from_rgb(24, 111, 58)),
        (Kind::View, course.views.len(), VIEW_COLOR, Color32::from_rgb(35, 88, 145)),
    ] {
        for index in 0..count {
            let rect = frame.rect(placed(kind, index));
            if selected(kind, index, None) {
                painter.rect_stroke(rect, 0.0, Stroke::new(3.0, accent), StrokeKind::Middle);
                knobs(painter, rect, colors);
            } else {
                painter.rect_stroke(rect, 0.0, hairline(color), StrokeKind::Middle);
            }
        }
    }
}

/// The tile array, one textured quad per written cell, one mesh per tileset slot.
fn tiles(painter: &Painter, view: &LevelView, tilemap: &LevelTilemap, frame: Frame, visible: Rect, art: &mut Art<'_>) {
    let Some(graphics) = &view.art.graphics else { return };
    let mut meshes: Vec<Option<(Mesh, Vec2, u32)>> = graphics
        .tilesets
        .iter()
        .map(|slot| art.texture(slot.atlas.as_ref()).map(|(texture, size)| (Mesh::with_texture(texture), size, slot.blocks)))
        .collect();
    let cells = LEVEL_CHUNK_CELLS as i32;
    let side = TILE * LEVEL_CHUNK_CELLS as f32;
    for chunk in &tilemap.chunks {
        let corner = Pos2::new(chunk.x as f32 * side, chunk.y as f32 * side);
        if !Rect::from_min_size(corner, Vec2::splat(side)).intersects(visible) {
            continue;
        }
        for (offset, cell) in chunk.cells.iter().enumerate() {
            if !cell.draws() {
                continue;
            }
            let Some(Some((mesh, size, blocks))) = meshes.get_mut(usize::from(cell.slot)) else { continue };
            let block = cell.block as u32;
            if block >= *blocks || size.x <= 0.0 || size.y <= 0.0 {
                continue;
            }
            let (column, row) = ((block % ATLAS_COLUMNS) as f32, (block / ATLAS_COLUMNS) as f32);
            let uv = Rect::from_min_size(Pos2::new(column * TILE / size.x, row * TILE / size.y), Vec2::new(TILE / size.x, TILE / size.y));
            let (x, y) = ((offset as i32 % cells) as f32, (offset as i32 / cells) as f32);
            let world = Rect::from_min_size(corner + Vec2::new(x, y) * TILE, Vec2::splat(TILE));
            mesh.add_rect_with_uv(frame.rect(world), uv, Color32::WHITE);
        }
    }
    for (mesh, _, _) in meshes.into_iter().flatten() {
        if !mesh.is_empty() {
            painter.add(Shape::mesh(mesh));
        }
    }
}

fn command_color(packed: u32, opacity: f32) -> Color32 {
    let [r, g, b, a] = packed.to_be_bytes();
    Color32::from_rgba_unmultiplied(r, g, b, a).gamma_multiply(opacity)
}

/// One command of an actor's draw list, moved by `offset` level pixels: rotated about its centre and flipped, the way
/// the script asked.
pub(crate) fn paint_command(painter: &Painter, frame: Frame, command: &DrawCommand, offset: Vec2, art: &mut Art<'_>) {
    let destination =
        Rect::from_min_size(Pos2::new(command.x as f32, command.y as f32) + offset, Vec2::new(command.width as f32, command.height as f32));
    let centre = destination.center();
    let (sin, cos) = (command.rotation as f32).to_radians().sin_cos();
    let flip = Vec2::new(if command.flip_x { -1.0 } else { 1.0 }, if command.flip_y { -1.0 } else { 1.0 });
    let place = |p: Pos2| {
        let d = (p - centre) * flip;
        frame.screen(centre + Vec2::new(d.x * cos - d.y * sin, d.x * sin + d.y * cos))
    };
    let corners = [destination.left_top(), destination.right_top(), destination.right_bottom(), destination.left_bottom()].map(place);
    let opacity = command.opacity.clamp(0.0, 1.0) as f32;
    let color = command_color(command.color, opacity);
    let width = if command.stroke_width > 0.0 { command.stroke_width as f32 * frame.zoom } else { 1.0 };
    match command.kind {
        DrawKind::FillRect => {
            painter.add(Shape::convex_polygon(corners.to_vec(), color, Stroke::NONE));
        }
        DrawKind::StrokeRect => {
            painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(width, color)));
        }
        DrawKind::FillEllipse => {
            let (radius, segments) = (destination.size() / 2.0, 32);
            let points = (0..segments)
                .map(|i| {
                    let angle = i as f32 / segments as f32 * std::f32::consts::TAU;
                    place(centre + Vec2::new(angle.cos() * radius.x, angle.sin() * radius.y))
                })
                .collect();
            painter.add(Shape::convex_polygon(points, color, Stroke::NONE));
        }
        DrawKind::Line => {
            let end = Pos2::new(command.x2 as f32, command.y2 as f32) + offset;
            painter.line_segment([place(destination.min), place(end)], Stroke::new(width, color));
        }
        DrawKind::Text | DrawKind::Image => {}
    }
    if !command.text.is_empty() {
        if let Some(font) = scaled_font(frame.zoom, 10.0) {
            let at = place(destination.min + Vec2::new(0.0, 8.0));
            painter.text(at, Align2::LEFT_BOTTOM, &command.text, font, Color32::WHITE.gamma_multiply(opacity));
        }
    } else if let Some((texture, size)) = art.texture(command.texture.as_ref()) {
        let [sx, sy] = command.source.map(|v| v as f32);
        let [sw, sh] = if command.source_size == [0, 0] { [size.x, size.y] } else { command.source_size.map(|v| v as f32) };
        let uv = Rect::from_min_size(Pos2::new(sx / size.x, sy / size.y), Vec2::new(sw / size.x, sh / size.y));
        let mut mesh = Mesh::with_texture(texture);
        let tint = Color32::WHITE.gamma_multiply(opacity);
        for (corner, uv) in corners.into_iter().zip([uv.left_top(), uv.right_top(), uv.right_bottom(), uv.left_bottom()]) {
            mesh.vertices.push(egui::epaint::Vertex { pos: corner, uv, color: tint });
        }
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(0, 2, 3);
        painter.add(Shape::mesh(mesh));
    }
}

/// The fine grid, the DS screens and the marquee: drawn over the level, never part of it.
fn overlays(painter: &Painter, view: &LevelView, frame: Frame, colors: &Colors) {
    let visible = frame.world_rect(painter.clip_rect());
    // Below a few points apart the lines are a wash, and there would be thousands of them.
    if view.options.show_grid && TILE * frame.zoom >= 2.0 {
        let stroke = Stroke::new(1.0, colors.fine_grid);
        let mut x = (visible.left() / TILE).floor() * TILE;
        while x <= visible.right() {
            painter.line_segment([frame.screen(Pos2::new(x, visible.top())), frame.screen(Pos2::new(x, visible.bottom()))], stroke);
            x += TILE;
        }
        let mut y = (visible.top() / TILE).floor() * TILE;
        while y <= visible.bottom() {
            painter.line_segment([frame.screen(Pos2::new(visible.left(), y)), frame.screen(Pos2::new(visible.right(), y))], stroke);
            y += TILE;
        }
    }
    if view.options.show_ds_screen {
        let red = Color32::from_rgb(220, 45, 55);
        for top in [0.0, 192.0] {
            let screen = frame.rect(Rect::from_min_size(Pos2::new(0.0, top), Vec2::new(256.0, 192.0)));
            painter.rect(screen, 0.0, with_alpha(red, 18), Stroke::new(1.0, red), StrokeKind::Middle);
        }
    }
    if let Some(Gesture::Marquee { from, to }) = view.gesture {
        let band = Rect::from_two_pos(from, to);
        painter.rect_filled(band, 0.0, with_alpha(colors.highlight, 35));
        let corners = vec![band.left_top(), band.right_top(), band.right_bottom(), band.left_bottom(), band.left_top()];
        painter.extend(Shape::dashed_line(&corners, Stroke::new(1.0, colors.highlight), 4.0, 3.0));
    }
}

/// The whole level in a thumbnail in the corner, whatever the canvas zoom is, with the part on screen marked.
fn minimap(painter: &Painter, view: &LevelView, tilemap: &LevelTilemap, frame: Frame, art: &mut Art<'_>, colors: &Colors) {
    let canvas = view.canvas;
    let mini = Rect::from_min_size(Pos2::new(canvas.right() - 190.0, canvas.bottom() - 130.0), Vec2::new(180.0, 120.0));
    if !canvas.contains_rect(mini) {
        return;
    }
    let mini_painter = painter.with_clip_rect(mini);
    mini_painter.rect(mini, 0.0, colors.surface, Stroke::new(1.0, colors.frame), StrokeKind::Inside);
    let mini_frame = Frame::fitted(mini, view.scene_bounds());
    draw_map(&mini_painter, view, tilemap, mini_frame, art, colors, false);
    // Clipped, because zoomed out past the fit the part on screen is larger than the level.
    let on_screen = mini_frame.rect(frame.world_rect(canvas));
    mini_painter.rect(on_screen, 0.0, with_alpha(colors.highlight, 25), Stroke::new(2.0, colors.highlight), StrokeKind::Middle);
    painter.rect_stroke(mini, 0.0, Stroke::new(2.0, colors.outline), StrokeKind::Middle);
}

/// The Preview tab: the whole level fitted into the space, read-only, with what it is and what it holds above it. It
/// never moves the level's own camera, which a document of the same level may be using.
pub fn show_preview(ui: &mut Ui, view: &mut LevelView, store: &HashMap<TextureId, Arc<TextureData>>, pool: &mut TexturePool) {
    let snapshot = &view.snapshot;
    let course = &snapshot.course;
    ui.heading(&snapshot.title);
    ui.weak(&snapshot.area);
    ui.label(format!(
        "Objects {}   Sprites {}   Entrances {}   Views {}   Zones {}",
        snapshot.objects.len(),
        course.sprites.len(),
        course.entrances.len(),
        course.views.len(),
        course.zones.len()
    ));
    let r = snapshot.tileset_references;
    ui.label(format!(
        "Tileset {}   BG graphics {}   BG screen {}   FG graphics {}   FG screen {}",
        r.tileset_id, r.background_graphics_id, r.background_screen_id, r.foreground_graphics_id, r.foreground_screen_id
    ));
    let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
    if rect.width() < 20.0 || rect.height() < 20.0 {
        return;
    }
    let colors = Colors::of(ui.visuals());
    let painter = ui.painter_at(rect);
    painter.rect(rect, 0.0, colors.surface, Stroke::new(1.0, colors.frame), StrokeKind::Inside);
    let frame = Frame::fitted(rect, view.scene_bounds());
    let tilemap = view.tilemap();
    let mut art = Art { ctx: ui.ctx(), store, pool };
    draw_map(&painter, view, &tilemap, frame, &mut art, &colors, false);
}
