//! The level canvas: the tile array, the actors and every other entity, drawn from what the host serves, and the
//! gestures that edit them. A port of `LevelPreviewWidget`'s painting and input; what a gesture means is decided in
//! `level_view`, which can be tested without a window.

use std::collections::HashMap;
use std::sync::Arc;

use bigstar_formats::{NsmbLevelCourse, entrance_flags};
use bigstar_level::{
    BackgroundLayer, BackgroundMotion, BackgroundSwap, LEVEL_CHUNK_CELLS, LayerRepeat, LayerScroll, LevelTilemap, layer_placement, still_quarter,
    strip_offsets,
};
use bigstar_protocol::{ATLAS_COLUMNS, DrawCommand, DrawKind, ImageGraphics, LevelEdit, TextureData, TextureId};
use egui::{
    Align2, Color32, CornerRadius, CursorIcon, Event, FontId, Key, Mesh, MouseWheelUnit, Painter, PointerButton, Pos2, Rect, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, ViewportCommand,
};

use crate::clipboard;
use crate::controls::{self, Button, Choice as _};
use crate::level_view::{Backgrounds, DsScreens, Gesture, Handle, Item, Kind, LevelView, TILE, fit_zoom, handle_centres, node_rect};
use crate::properties::Tab;

/// The textures the host has sent, uploaded the first time something draws them.
#[derive(Default)]
pub struct TexturePool {
    uploaded: HashMap<TextureId, egui::TextureHandle>,
    entrances: Option<egui::TextureHandle>,
}

/// NSMB Editor's entrance icons: a 16-pixel cell per type, and under each the same entrance as one that can be entered.
const ENTRANCE_SHEET: &[u8] = include_bytes!("../assets/level/entrances.png");
const ENTRANCE_SHEET_COLUMNS: u8 = 27;

/// The cell of the sheet an entrance is drawn with, as NSMB Editor picks it: doors and pipes can be walked into, so
/// one that is not exit-only wears the second row's arrow. Types past the last icon share it.
fn entrance_cell(kind: u8, flags: u8) -> (u8, u8) {
    let enterable = matches!(kind, 2..=6 | 16..=19 | 22..=25) && flags & entrance_flags::EXIT_ONLY == 0;
    (kind.min(25), u8::from(enterable))
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

    fn entrance_sheet(&mut self, ctx: &egui::Context) -> Option<egui::TextureId> {
        if self.entrances.is_none() {
            let sheet = image::load_from_memory_with_format(ENTRANCE_SHEET, image::ImageFormat::Png).ok()?.into_rgba8();
            let image = egui::ColorImage::from_rgba_unmultiplied([sheet.width() as usize, sheet.height() as usize], sheet.as_raw());
            self.entrances = Some(ctx.load_texture("bigstar:entrances", image, egui::TextureOptions::NEAREST));
        }
        self.entrances.as_ref().map(egui::TextureHandle::id)
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
        ui.menu_button(options.ds_screens.short(), |ui| {
            for choice in DsScreens::ALL {
                if Button::menu(choice.name()).selected(options.ds_screens == choice).show(ui).clicked() {
                    options.ds_screens = choice;
                    ui.close();
                }
            }
        })
        .response
        .on_hover_text(
            "The DS screen (256×192), with its backgrounds scrolled as the game scrolls them: under the cursor, at each selected entrance, or both",
        );
        // Named in short on the toolbar, which has no room to spare, and in full in the menu.
        ui.menu_button(options.backgrounds.short(), |ui| {
            for choice in Backgrounds::ALL {
                if Button::menu(choice.name()).selected(options.backgrounds == choice).show(ui).clicked() {
                    options.backgrounds = choice;
                    ui.close();
                }
            }
            ui.separator();
            if Button::menu("Animate backgrounds").selected(options.animate_backgrounds).show(ui).clicked() {
                options.animate_backgrounds = !options.animate_backgrounds;
                ui.close();
            }
        })
        .response
        .on_hover_text(
            "Where the background layers are drawn: nowhere, only inside the views as the game shows them, or over the whole level; and whether they move as the game moves them",
        );
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
    view.hover = latest.filter(|at| over && view.canvas.contains(*at)).map(|at| view.to_world(at));
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
                    // Right-click creates whatever is chosen; left-click never does. An object waits for the button
                    // to come up, so it can be dragged out to its size.
                    if view.begin_place(press.world, modifiers.alt) {
                        continue;
                    }
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
                    Some(Gesture::Place { submitted: false, .. }) => {
                        let at = view.to_world(*pos);
                        view.drag_place(at, free);
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
                Some(Gesture::Place { submitted: false, .. }) => edits.extend(view.finish_place()),
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
                    Some(
                        Gesture::Move { submitted: false, .. }
                            | Gesture::Resize { submitted: false, .. }
                            | Gesture::Place { submitted: false, .. }
                            | Gesture::Marquee { .. }
                    )
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

/// `rect` without the part `cut` covers, as up to four rectangles.
fn subtract(rect: Rect, cut: Rect) -> Vec<Rect> {
    let overlap = rect.intersect(cut);
    if !overlap.is_positive() {
        return vec![rect];
    }
    [
        Rect::from_min_max(rect.min, Pos2::new(rect.max.x, overlap.min.y)),
        Rect::from_min_max(Pos2::new(rect.min.x, overlap.max.y), rect.max),
        Rect::from_min_max(Pos2::new(rect.min.x, overlap.min.y), Pos2::new(overlap.min.x, overlap.max.y)),
        Rect::from_min_max(Pos2::new(overlap.max.x, overlap.min.y), Pos2::new(rect.max.x, overlap.max.y)),
    ]
    .into_iter()
    .filter(|piece| piece.is_positive())
    .collect()
}

/// The area the rectangles cover, as rectangles that do not overlap, so a layer drawn over each is drawn once. Each
/// piece comes with the rectangle it was cut from: where two overlap, the earlier one keeps the ground.
fn disjoint(rects: impl IntoIterator<Item = Rect>) -> Vec<(Rect, usize)> {
    let mut covered: Vec<(Rect, usize)> = Vec::new();
    for (index, rect) in rects.into_iter().enumerate() {
        let mut pieces = vec![rect];
        for (done, _) in &covered {
            pieces = pieces.into_iter().flat_map(|piece| subtract(piece, *done)).collect();
        }
        covered.extend(pieces.into_iter().map(|piece| (piece, index)));
    }
    covered
}

/// Which rows of a layer's image land where down a stretch of the level: `(level y of the piece's top, first image row,
/// one past its last)`, covering `top..bottom`. The image's first row is at `origin`; how it goes on past its own
/// `height` rows is the layer's vertical mode.
fn layer_pieces(origin: f32, height: f32, repeat: LayerRepeat, top: f32, bottom: f32) -> Vec<(f32, f32, f32)> {
    let mut pieces = Vec::new();
    if height < 1.0 {
        return pieces;
    }
    let half = height / 2.0;
    let wrap_above = |pieces: &mut Vec<(f32, f32, f32)>| {
        let mut y = origin - height;
        while y + height > top {
            pieces.push((y, 0.0, height));
            y -= height;
        }
    };
    let wrap_below = |pieces: &mut Vec<(f32, f32, f32)>| {
        let mut y = origin + height;
        while y < bottom {
            pieces.push((y, 0.0, height));
            y += height;
        }
    };
    pieces.push((origin, 0.0, height));
    match repeat {
        LayerRepeat::Wrap => {
            wrap_above(&mut pieces);
            wrap_below(&mut pieces);
        }
        LayerRepeat::TopHalfAbove => {
            let mut y = origin - half;
            while y + half > top {
                pieces.push((y, 0.0, half));
                y -= half;
            }
            wrap_below(&mut pieces);
        }
        LayerRepeat::BottomHalfBelow => {
            wrap_above(&mut pieces);
            let mut y = origin + height;
            while y < bottom {
                pieces.push((y, half, height));
                y += half;
            }
        }
    }
    pieces.retain(|&(y, first, last)| y < bottom && y + (last - first) > top);
    pieces
}

/// A background or foreground layer over `area` (level pixels), one image pixel to one level pixel. The image's top
/// left corner sits at `origin` and it repeats sideways; `bands` are the rows that move together and how far left each
/// has moved, as the game shifts a layer scanline by scanline. `swaps` are the quarters the layer's script has the game
/// replace as it scrolls, which the picture does by how far a column is from `origin`.
#[allow(clippy::too_many_arguments)]
fn layer(
    painter: &Painter,
    frame: Frame,
    texture: egui::TextureId,
    size: Vec2,
    area: Rect,
    origin: Pos2,
    repeat: LayerRepeat,
    bands: &[(u16, u16, f32)],
    swaps: &[BackgroundSwap],
    tint: Color32,
) {
    if size.x < 1.0 || size.y < 1.0 || !area.is_positive() {
        return;
    }
    let half = size / 2.0;
    for (piece_top, first_row, last_row) in layer_pieces(origin.y, size.y, repeat, area.top(), area.bottom()) {
        for &(band_first, band_last, moved) in bands {
            let (rows_first, rows_last) = (f32::from(band_first).max(first_row), f32::from(band_last).min(last_row));
            if rows_last <= rows_first {
                continue;
            }
            let (top, bottom) = (piece_top + rows_first - first_row, piece_top + rows_last - first_row);
            if bottom <= area.top() || top >= area.bottom() {
                continue;
            }
            let start = origin.x - moved;
            let mut x = start + ((area.left() - start) / size.x).floor() * size.x;
            while x < area.right() {
                let tile = Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + size.x, bottom));
                let part = tile.intersect(area);
                // Cut where the image passes from one quarter into the next and where a swap takes hold or lets go,
                // so every cell shows one quarter throughout.
                let (mut columns, mut rows) = (vec![part.left(), part.right()], vec![part.top(), part.bottom()]);
                if !swaps.is_empty() && part.is_positive() {
                    columns.push(x + half.x);
                    columns.push(origin.x);
                    columns.extend(swaps.iter().map(|swap| origin.x + swap.still_reach() as f32));
                    columns.retain(|cut| part.left() <= *cut && *cut <= part.right());
                    columns.sort_by(f32::total_cmp);
                    columns.dedup();
                    rows.push(top + half.y - rows_first);
                    rows.retain(|cut| part.top() <= *cut && *cut <= part.bottom());
                    rows.sort_by(f32::total_cmp);
                    rows.dedup();
                }
                for (&cell_top, &cell_bottom) in rows.iter().zip(rows.iter().skip(1)) {
                    for (&cell_left, &cell_right) in columns.iter().zip(columns.iter().skip(1)) {
                        let cell = Rect::from_min_max(Pos2::new(cell_left, cell_top), Pos2::new(cell_right, cell_bottom));
                        if !cell.is_positive() {
                            continue;
                        }
                        let mut uv = Rect::from_min_max(
                            Pos2::new((cell.left() - x) / size.x, (rows_first + cell.top() - top) / size.y),
                            Pos2::new((cell.right() - x) / size.x, (rows_first + cell.bottom() - top) / size.y),
                        );
                        if !swaps.is_empty() {
                            let own = (u8::from(cell.left() - x >= half.x), u8::from(rows_first + cell.top() - top >= half.y));
                            let shown = still_quarter(swaps, own, (cell.left() - origin.x).floor() as i32);
                            uv = uv.translate(Vec2::new(f32::from(shown.0) - f32::from(own.0), f32::from(shown.1) - f32::from(own.1)) / 2.0);
                        }
                        painter.image(texture, frame.rect(cell), uv, tint);
                    }
                }
                x += size.x;
            }
        }
    }
}

/// A layer inside one DS screen (`screen`, level pixels), scrolled as the game has it: each run of rows that moves
/// together, cut wherever it crosses from one quarter of the image into the next, since which quarter shows where is
/// the game's to choose.
fn screen_layer(
    painter: &Painter,
    frame: Frame,
    texture: egui::TextureId,
    screen: Rect,
    scroll: &LayerScroll,
    bands: &[(u16, u16, f32)],
    tint: Color32,
) {
    const QUARTER: i32 = 256;
    let (first, last) = (scroll.y, scroll.y + screen.height() as i32);
    let width = screen.width() as i32;
    // The band a row belongs to; rows off either end of the image take the nearest, as the game's table walk does.
    let band_of = |row: i32| {
        let row = row.clamp(0, QUARTER * 2 - 1) as u16;
        bands.iter().find(|band| band.0 <= row && row < band.1).or(bands.last()).map_or(0, |band| band.2 as i32)
    };
    let mut cuts: Vec<i32> = bands.iter().flat_map(|band| [i32::from(band.0), i32::from(band.1)]).collect();
    cuts.extend((first.div_euclid(QUARTER)..=last.div_euclid(QUARTER)).map(|block| block * QUARTER));
    cuts.retain(|cut| first < *cut && *cut < last);
    cuts.extend([first, last]);
    cuts.sort_unstable();
    cuts.dedup();
    for (&top, &bottom) in cuts.iter().zip(cuts.iter().skip(1)) {
        let left = band_of(top);
        let mut x = left;
        while x < left + width {
            let next = ((x.div_euclid(QUARTER) + 1) * QUARTER).min(left + width);
            let (column, row) = scroll.quarter(x.div_euclid(QUARTER), top.div_euclid(QUARTER));
            let corner = Pos2::new(
                f32::from(column) * QUARTER as f32 + x.rem_euclid(QUARTER) as f32,
                f32::from(row) * QUARTER as f32 + top.rem_euclid(QUARTER) as f32,
            );
            let size = Vec2::new((next - x) as f32, (bottom - top) as f32);
            let uv = Rect::from_min_size((corner.to_vec2() / (QUARTER * 2) as f32).to_pos2(), size / (QUARTER * 2) as f32);
            let at = Rect::from_min_size(screen.min + Vec2::new((x - left) as f32, (top - first) as f32), size);
            painter.image(texture, frame.rect(at), uv, tint);
            x = next;
        }
    }
}

/// The picture an animated layer shows `ticks` frames of the game in: its frames played in order and round again.
fn layer_frame(graphics: &ImageGraphics, ticks: u64) -> Option<&TextureId> {
    let total: u64 = graphics.frames.iter().map(|frame| u64::from(frame.ticks)).sum();
    if total == 0 {
        return graphics.texture.as_ref();
    }
    let mut at = ticks % total;
    for frame in &graphics.frames {
        if at < u64::from(frame.ticks) {
            return Some(&frame.texture);
        }
        at -= u64::from(frame.ticks);
    }
    graphics.texture.as_ref()
}

/// The view the level opens in: the one its starting entrance names, or failing that its first.
fn start_view(course: &NsmbLevelCourse) -> Option<usize> {
    let start = course.header.entrances[0];
    let named =
        course.entrances.iter().find(|entrance| entrance.id == start).and_then(|entrance| course.views.iter().position(|v| v.id == entrance.view_id));
    named.or(if course.views.is_empty() { None } else { Some(0) })
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

    let graphics = view.art.graphics.as_ref();
    // Where the background and foreground show, as the toolbar chooses: nowhere; inside the views, as in the game,
    // following a view while it is dragged; or everywhere, which is also what a level without views gets. Each area
    // comes with the view whose blocks place the layers in it: the game picks a layer's block, and so where it sits,
    // how it repeats and whether it shows at all, by the view the player entered.
    let layer_areas: Vec<(Rect, Option<usize>)> = match view.options.backgrounds {
        Backgrounds::Hidden => Vec::new(),
        Backgrounds::InViews if !course.views.is_empty() => disjoint((0..course.views.len()).map(|index| placed(Kind::View, index)))
            .into_iter()
            .map(|(area, index)| (area.intersect(visible), Some(index)))
            .collect(),
        Backgrounds::InViews | Backgrounds::Overflowing => vec![(bounds.intersect(visible), start_view(course))],
    };
    // The far layer, then the near one (the game's "foreground", which is still behind the level), both under
    // everything the level holds. Darkened rather than faded, so the near layer hides the far one where it is solid and
    // the level stands out against both.
    let shade = Color32::from_gray(140);
    let animate = live && view.options.animate_backgrounds;
    // The game runs at sixty frames a second, and its animations count in them.
    let ticks = if animate { (art.ctx.input(|input| input.time) * 60.0) as u64 } else { 0 };
    let mut moving = false;
    if let Some(graphics) = graphics {
        let references = view.snapshot.tileset_references;
        let layers = [
            (BackgroundLayer::Background, &graphics.background, &course.backgrounds, references.background_screen_id),
            (BackgroundLayer::Foreground, &graphics.foreground, &course.foregrounds, references.foreground_screen_id),
        ];
        for (which, image, blocks, loaded) in layers {
            let Some((texture, size)) = art.texture(layer_frame(image, ticks)) else { continue };
            for &(area, shown_in) in &layer_areas {
                if !area.is_positive() {
                    continue;
                }
                // A level without views has nothing to place the layer by, and shows it from its own origin.
                let (origin, repeat, screen, in_view) = match shown_in.and_then(|index| Some((index, course.views.get(index)?))) {
                    Some((index, v)) => {
                        let rect = placed(Kind::View, index);
                        let block = if which == BackgroundLayer::Background { v.background_id } else { v.foreground_id };
                        let Some(placement) = layer_placement(blocks, block, rect.top() as i32, rect.height() as i32) else { continue };
                        (Pos2::new(rect.left(), placement.origin_y as f32), placement.repeat, placement.screen_id, true)
                    }
                    None => (Pos2::ZERO, LayerRepeat::Wrap, loaded, false),
                };
                let definition = graphics.backgrounds.find(which, screen);
                let strips = definition.map_or(&[][..], |definition| definition.strips.as_slice());
                // A swap counts the layer's scroll from the view's left edge, so it needs a view to count from.
                let swaps = definition.filter(|_| in_view).map_or(&[][..], |definition| definition.swaps.as_slice());
                // A ripple is drawn a few rows at a time: as many as make a screen pixel or two.
                let wave_rows = (2.0 / frame.zoom.max(0.01)).ceil().clamp(2.0, 64.0) as u16;
                let bands = strip_offsets(strips, size.y as u16, 0, ticks, wave_rows);
                moving |= !image.frames.is_empty() || strips.iter().any(|strip| !matches!(strip.motion, BackgroundMotion::Scroll { .. }));
                layer(painter, frame, texture, size, area, origin, repeat, &bands, swaps, shade);
            }
        }
        // Inside a DS screen the layers are where the game scrolls them to with its camera there, at full strength
        // over the still picture.
        for screen in if live { view.ds_screens() } else { Vec::new() } {
            if screen.view.is_none() || !screen.rect.intersects(visible) {
                continue;
            }
            painter.rect_filled(frame.rect(screen.rect), 0.0, colors.surface);
            for (which, image, ..) in layers {
                let Some(scroll) = view.screen_layer(&screen, which) else { continue };
                let Some((texture, size)) = art.texture(layer_frame(image, ticks)) else { continue };
                let strips = graphics.backgrounds.find(which, scroll.screen_id).map_or(&[][..], |definition| definition.strips.as_slice());
                let bands = strip_offsets(strips, size.y as u16, scroll.x, ticks, 1);
                moving |= !image.frames.is_empty() || strips.iter().any(|strip| !matches!(strip.motion, BackgroundMotion::Scroll { .. }));
                screen_layer(painter, frame, texture, screen.rect, &scroll, &bands, Color32::WHITE);
            }
        }
    }
    if animate && moving {
        art.ctx.request_repaint_after(std::time::Duration::from_millis(16));
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

    if let Some(rect) = view.placing_rect() {
        painter.rect_stroke(frame.rect(rect), 0.0, marker_stroke(colors), StrokeKind::Middle);
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

    let sheet = art.pool.entrance_sheet(art.ctx);
    for (index, entrance) in course.entrances.iter().enumerate() {
        let rect = frame.rect(placed(Kind::Entrance, index));
        match sheet {
            Some(sheet) => {
                let (column, row) = entrance_cell(entrance.kind, entrance.flags);
                let cell = Vec2::new(1.0 / f32::from(ENTRANCE_SHEET_COLUMNS), 0.5);
                let corner = Pos2::new(f32::from(column) * cell.x, f32::from(row) * cell.y);
                painter.image(sheet, rect, Rect::from_min_size(corner, cell), Color32::WHITE);
            }
            None => {
                painter.add(Shape::ellipse_filled(rect.center(), rect.size() / 2.0, rgba(92, 190, 214, 190)));
                painter.add(Shape::ellipse_stroke(rect.center(), rect.size() / 2.0, hairline(Color32::from_rgb(34, 94, 122))));
            }
        }
        if selected(Kind::Entrance, index, None) {
            // A square on the icon's own edge, as NSMB Editor outlines whatever is selected.
            painter.rect_stroke(rect, 0.0, marker_stroke(colors), StrokeKind::Middle);
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

    // Last, so nothing dims the edges a zone or a view is sized against.
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
    // The DS screens: hollow, so the level shows through as the game would show it. The one an entrance starts the
    // camera on is dashed; the one under the pointer is not.
    let red = Color32::from_rgb(220, 45, 55);
    for screen in view.ds_screens() {
        let rect = frame.rect(screen.rect);
        if screen.spawn {
            let corners = vec![rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom(), rect.left_top()];
            painter.extend(Shape::dashed_line(&corners, Stroke::new(2.0, red), 6.0, 4.0));
        } else {
            painter.rect_stroke(rect, 0.0, Stroke::new(2.0, red), StrokeKind::Outside);
        }
        let mut text = format!("{}{}, {}", if screen.spawn { "Spawn camera " } else { "" }, screen.rect.left(), screen.rect.top());
        for (name, layer) in [("BG", BackgroundLayer::Background), ("FG", BackgroundLayer::Foreground)] {
            if let Some(scroll) = view.screen_layer(&screen, layer) {
                text.push_str(&format!("   {name} {}, {}", scroll.x >> 12, scroll.y));
            }
        }
        let galley = painter.layout_no_wrap(text, FontId::proportional(11.0), Color32::WHITE);
        let label = Rect::from_min_size(rect.left_top() - Vec2::new(1.0, galley.size().y + 6.0), galley.size() + Vec2::new(8.0, 4.0));
        painter.rect_filled(label, 2.0, with_alpha(red, 220));
        painter.galley(label.min + Vec2::new(4.0, 2.0), galley, Color32::WHITE);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    fn area(rects: &[Rect]) -> f32 {
        rects.iter().map(Rect::area).sum()
    }

    #[test]
    fn an_entrance_wears_its_type_and_whether_it_can_be_entered() {
        let sheet = image::load_from_memory_with_format(ENTRANCE_SHEET, image::ImageFormat::Png).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (u32::from(ENTRANCE_SHEET_COLUMNS) * 16, 32));
        assert_eq!(entrance_cell(0, 0), (0, 0), "nothing walks into a plain spawn point");
        assert_eq!((entrance_cell(3, 0), entrance_cell(3, entrance_flags::EXIT_ONLY)), ((3, 1), (3, 0)));
        assert_eq!((entrance_cell(24, 0), entrance_cell(99, 0)), ((24, 1), (25, 0)));
    }

    #[test]
    fn overlapping_views_cover_their_union_once() {
        // Two views sharing a 64×64 corner, and a third apart from both.
        let views = [rect(0.0, 0.0, 256.0, 192.0), rect(192.0, 128.0, 256.0, 192.0), rect(1000.0, 0.0, 100.0, 100.0)];
        let cut = disjoint(views);
        // The corner the first two share stays the first one's.
        assert!(cut.iter().all(|&(piece, from)| from == 0 || !piece.intersect(views[0]).is_positive()));
        let pieces: Vec<Rect> = cut.into_iter().map(|(piece, _)| piece).collect();
        assert_eq!(area(&pieces), 256.0 * 192.0 * 2.0 - 64.0 * 64.0 + 100.0 * 100.0);
        for (i, a) in pieces.iter().enumerate() {
            for b in pieces.iter().skip(i + 1) {
                assert!(!a.intersect(*b).is_positive(), "{a:?} overlaps {b:?}");
            }
        }
        // A view inside another adds nothing; apart, a rectangle is itself.
        let nested: Vec<Rect> = disjoint([rect(0.0, 0.0, 100.0, 100.0), rect(10.0, 10.0, 20.0, 20.0)]).into_iter().map(|(piece, _)| piece).collect();
        assert_eq!(area(&nested), 10_000.0);
        assert_eq!(subtract(rect(0.0, 0.0, 10.0, 10.0), rect(50.0, 50.0, 5.0, 5.0)), [rect(0.0, 0.0, 10.0, 10.0)]);
    }
}
