//! The permanent picker: what a right-click on the canvas places. Objects per tileset slot, previewed at their own size;
//! actors by category, each drawn by its own program; module actors by module and component. The entries are built
//! here as data, so their names and grouping are testable; the previews are painted from the textures the host sent.

use std::collections::BTreeMap;

use bigstar_formats::{OBJECT_LAYOUT_EMPTY_TILE, OBJECT_LAYOUT_UNWRITTEN, ObjectLayout, render_object_layout};
use bigstar_level::{LevelObjectEffect, ModuleDataCatalog, level_object_effect};
use bigstar_protocol::{ATLAS_COLUMNS, TilesetGraphics};
use egui::{Color32, CornerRadius, Pos2, Rect, Sense, Shape, Stroke, StrokeKind, Ui, Vec2};

use crate::block_icons;
use crate::canvas::{Art, Frame, paint_command};
use crate::controls::{self, Choice as _, Row, Width};
use crate::level_view::{ActorKey, Kind, LevelView, TILE};

/// Repeat-marked rows would tile forever, so a preview stops at a size that still fits a button.
const PREVIEW_CELLS: usize = 8;
const ICON: f32 = 40.0;
const BUTTON: Vec2 = Vec2::new(82.0, 72.0);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PickerTab {
    #[default]
    Objects,
    Sprites,
    Custom,
}

#[derive(Debug, Clone, Default)]
pub struct PickerState {
    pub tab: PickerTab,
    pub slot: usize,
    pub search: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectEntry {
    pub slot: u8,
    pub id: u16,
    pub label: String,
    pub tooltip: String,
    pub effect: LevelObjectEffect,
    /// The size the preview is drawn at, in cells.
    pub preview: (usize, usize),
}

/// One page per tileset slot that has objects; an empty slot gets no page.
pub fn object_entries(view: &LevelView) -> Vec<(u8, Vec<ObjectEntry>)> {
    let Some(graphics) = &view.art.graphics else { return Vec::new() };
    let mut pages = Vec::new();
    for (slot, tileset) in graphics.tilesets.iter().enumerate() {
        let Ok(slot) = u8::try_from(slot) else { continue };
        if tileset.error.is_some() || tileset.objects.is_empty() {
            continue;
        }
        let entries =
            tileset.objects.iter().enumerate().filter_map(|(id, layout)| Some(object_entry(slot, u16::try_from(id).ok()?, layout))).collect();
        pages.push((slot, entries));
    }
    pages
}

/// Object `id`'s entry. An object at one tile is a corner, which says nothing about what is placed, so it is
/// previewed at its own extent. The two kinds of object that draw nothing are told apart: the eraser clears the cells
/// it covers, and a block the game never writes leaves them holding whatever was there.
pub fn object_entry(slot: u8, id: u16, layout: &ObjectLayout) -> ObjectEntry {
    let mut width = layout.rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let mut height = layout.rows.len().max(1);
    // One step of a slope is not a slope; two show the diagonal and what goes under it.
    if layout.slope {
        (width, height) = (width * 2, height * 2);
    }
    let effect = level_object_effect(layout, slot);
    let mut label = format!("Object {slot}:{id}");
    let mut tooltip = format!("Object {id} of tileset slot {slot} - {width}x{height} tiles");
    match effect {
        LevelObjectEffect::Erases => {
            label.push_str(" - eraser");
            tooltip.push_str("\nWrites the empty block, so it clears the cells it covers.");
        }
        LevelObjectEffect::Inert => {
            label.push_str(" - draws nothing");
            tooltip.push_str("\nNames a block the game never writes, so it leaves the cells alone.");
        }
        LevelObjectEffect::Draws => {}
    }
    ObjectEntry { slot, id, label, tooltip, effect, preview: (width.clamp(1, PREVIEW_CELLS), height.clamp(1, PREVIEW_CELLS)) }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpriteEntry {
    pub sprite_type: u16,
    pub label: String,
    pub has_renderer: bool,
}

/// Every actor type the ROM's table lists, by category; one no actor speaks for is filed under "Other actors".
pub fn sprite_entries(view: &LevelView) -> BTreeMap<String, Vec<SpriteEntry>> {
    let mut groups: BTreeMap<String, Vec<SpriteEntry>> = BTreeMap::new();
    let count = view.art.actors.catalog.sprite_object_ids.len().min(usize::from(u16::MAX));
    for sprite_type in (0..count).filter_map(|t| u16::try_from(t).ok()) {
        let actor = view.art.actors.actor(sprite_type, None);
        let name = actor.map_or_else(|| format!("Actor {sprite_type}"), |a| a.definition.name.clone());
        let category = actor.map(|a| a.definition.category.clone()).filter(|c| !c.is_empty()).unwrap_or_else(|| "Other actors".to_owned());
        let has_renderer = actor.is_some_and(|a| a.has_renderer);
        groups.entry(category).or_default().push(SpriteEntry { sprite_type, label: format!("{name} ({sprite_type})"), has_renderer });
    }
    groups
}

/// The module actors, by module and then component.
pub fn custom_entries(modules: &ModuleDataCatalog) -> BTreeMap<String, BTreeMap<String, Vec<(u32, String)>>> {
    let mut tree: BTreeMap<String, BTreeMap<String, Vec<(u32, String)>>> = BTreeMap::new();
    for actor in &modules.stage_objects {
        let label = format!("{}  [object {}]", actor.name, actor.object_id);
        tree.entry(actor.module.clone()).or_default().entry(actor.component.clone()).or_default().push((actor.hash, label));
    }
    tree
}

/// What an actor's still is drawn from: its program at the origin with nothing set.
fn still(sprite_type: u16, hash: Option<u32>) -> ActorKey {
    ActorKey { sprite_type, hash, x: 0, y: 0, settings: 0, events: [0, 0] }
}

fn matches(filter: &str, texts: &[&str]) -> bool {
    filter.is_empty() || texts.iter().any(|t| t.to_lowercase().contains(filter))
}

/// The picker. Choosing an entry changes what a right-click places, and with it which kind the properties show.
pub fn show(ui: &mut Ui, view: &mut LevelView, state: &mut PickerState, modules: Option<&ModuleDataCatalog>, art: &mut Art<'_>) {
    controls::search(ui, &mut state.search, "Filter palette…", Width::Fill);
    controls::tabs(
        ui,
        "picker",
        &mut state.tab,
        &[(PickerTab::Objects, "Objects"), (PickerTab::Sprites, "Sprites"), (PickerTab::Custom, "Custom actors")],
    );
    ui.add_space(2.0);
    let filter = state.search.to_lowercase();
    match state.tab {
        PickerTab::Objects => objects(ui, view, state, &filter, art),
        PickerTab::Sprites => sprites(ui, view, &filter, art),
        PickerTab::Custom => custom(ui, view, modules, &filter, art),
    }
}

/// A grid of preview buttons, three to a row. Returns the index clicked.
fn grid<T>(ui: &mut Ui, items: &[T], mut button: impl FnMut(&mut Ui, &T) -> bool) -> Option<usize> {
    let mut clicked = None;
    let columns = ((ui.available_width() / (BUTTON.x + 4.0)).floor() as usize).max(1);
    egui::Grid::new(ui.id().with("grid")).spacing([4.0, 4.0]).show(ui, |ui| {
        for (index, item) in items.iter().enumerate() {
            if button(ui, item) {
                clicked = Some(index);
            }
            if (index + 1) % columns == 0 {
                ui.end_row();
            }
        }
    });
    clicked
}

/// One button: a 40-point icon over its label, highlighted when it is what a right-click places.
fn preview_button(ui: &mut Ui, label: &str, tooltip: &str, chosen: bool, paint: impl FnOnce(&egui::Painter, Rect)) -> bool {
    let (rect, response) = ui.allocate_exact_size(BUTTON, Sense::click());
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, chosen, label));
    let visuals = ui.style().interact_selectable(&response, chosen);
    let painter = ui.painter_at(rect);
    painter.rect(rect, CornerRadius::same(4), visuals.weak_bg_fill, visuals.bg_stroke, StrokeKind::Inside);
    let icon = Rect::from_center_size(Pos2::new(rect.center().x, rect.top() + 4.0 + ICON / 2.0), Vec2::splat(ICON));
    paint(&painter, icon);
    let galley = ui.painter().layout(label.to_owned(), egui::FontId::proportional(10.0), visuals.text_color(), rect.width() - 6.0);
    painter.galley(Pos2::new(rect.center().x - galley.size().x / 2.0, icon.bottom() + 3.0), galley, visuals.text_color());
    response.on_hover_text(tooltip).clicked()
}

fn objects(ui: &mut Ui, view: &mut LevelView, state: &mut PickerState, filter: &str, art: &mut Art<'_>) {
    let pages = object_entries(view);
    if pages.is_empty() {
        ui.weak("Open a level to list its tileset objects.");
        return;
    }
    if !pages.iter().any(|(slot, _)| usize::from(*slot) == state.slot) {
        state.slot = pages.first().map_or(0, |(slot, _)| usize::from(*slot));
    }
    ui.horizontal(|ui| {
        for (slot, _) in &pages {
            ui.choice_value(&mut state.slot, usize::from(*slot), format!("Tileset {slot}"));
        }
    });
    let Some((slot, entries)) = pages.into_iter().find(|(slot, _)| usize::from(*slot) == state.slot) else { return };
    let tileset = view.art.graphics.as_ref().and_then(|g| g.tilesets.get(usize::from(slot))).cloned().unwrap_or_default();
    let category = format!("Tileset {slot}");
    let shown: Vec<ObjectEntry> = entries.into_iter().filter(|e| matches(filter, &[&e.label, &category])).collect();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        let placement = &view.placement;
        let chosen = |e: &ObjectEntry| placement.kind == Kind::Object && placement.object_slot == e.slot && placement.object_id == e.id;
        let clicked = grid(ui, &shown, |ui, entry| {
            preview_button(ui, &entry.label, &entry.tooltip, chosen(entry), |painter, icon| object_preview(painter, icon, entry, &tileset, art))
        });
        if let Some(entry) = clicked.and_then(|i| shown.get(i)) {
            view.placement.set_object(entry.slot, entry.id);
        }
    });
}

/// The object laid out at its preview size, from the slot's atlas; or, for one that draws nothing, the glyph that says
/// which nothing it is.
fn object_preview(painter: &egui::Painter, icon: Rect, entry: &ObjectEntry, tileset: &TilesetGraphics, art: &mut Art<'_>) {
    match entry.effect {
        LevelObjectEffect::Erases => return eraser_glyph(painter, icon),
        LevelObjectEffect::Inert => {
            let square = icon.shrink(icon.width() * 10.0 / 64.0);
            let corners = vec![square.left_top(), square.right_top(), square.right_bottom(), square.left_bottom(), square.left_top()];
            painter.extend(Shape::dashed_line(&corners, Stroke::new(2.0, Color32::from_rgb(150, 150, 155)), 5.0, 4.0));
            return;
        }
        LevelObjectEffect::Draws => {}
    }
    let Some(layout) = tileset.objects.get(usize::from(entry.id)) else { return };
    let (width, height) = entry.preview;
    let Ok(cells) = render_object_layout(layout, width, height) else { return };
    let Some((texture, size)) = art.texture(tileset.atlas.as_ref()) else { return };
    let cell = (icon.width() / width as f32).min(icon.height() / height as f32);
    let origin = icon.center() - Vec2::new(width as f32, height as f32) * cell / 2.0;
    let mut mesh = egui::Mesh::with_texture(texture);
    // What a question block or a brick holds, over it, as the canvas shows it.
    let mut icons = art.contents_sheet().map(egui::Mesh::with_texture);
    for (index, &block) in cells.iter().enumerate() {
        if block == OBJECT_LAYOUT_UNWRITTEN || block == OBJECT_LAYOUT_EMPTY_TILE || block < 0 || block as u32 >= tileset.blocks {
            continue;
        }
        let block = block as u32;
        let (column, row) = ((block % ATLAS_COLUMNS) as f32, (block / ATLAS_COLUMNS) as f32);
        let uv = Rect::from_min_size(Pos2::new(column * TILE / size.x, row * TILE / size.y), Vec2::new(TILE / size.x, TILE / size.y));
        let at = Rect::from_min_size(origin + Vec2::new((index % width) as f32, (index / width) as f32) * cell, Vec2::splat(cell));
        mesh.add_rect_with_uv(at, uv, Color32::WHITE);
        if let (Some(icons), Some(contents)) = (icons.as_mut(), tileset.behaviors.get(block as usize).and_then(|&b| block_icons::contents_cell(b))) {
            block_icons::add(icons, at, contents);
        }
    }
    painter.add(Shape::mesh(mesh));
    if let Some(icons) = icons.filter(|icons| !icons.is_empty()) {
        painter.add(Shape::mesh(icons));
    }
}

/// A pencil eraser, tilted, rubbing edge down.
fn eraser_glyph(painter: &egui::Painter, icon: Rect) {
    let scale = icon.width() / 64.0;
    let centre = icon.min + Vec2::new(32.0, 34.0) * scale;
    let (sin, cos) = (-32.0_f32).to_radians().sin_cos();
    let place = |x: f32, y: f32| centre + Vec2::new(x * cos - y * sin, x * sin + y * cos) * scale;
    let quad = |top: f32, bottom: f32| vec![place(-20.0, top), place(20.0, top), place(20.0, bottom), place(-20.0, bottom)];
    let outline = Stroke::new(2.0, Color32::from_rgb(58, 44, 44));
    painter.add(Shape::convex_polygon(quad(-22.0, 22.0), Color32::from_rgb(226, 122, 130), outline));
    painter.add(Shape::convex_polygon(quad(4.0, 22.0), Color32::from_rgb(244, 226, 226), outline));
}

/// An actor's still, scaled to the icon, or a marker for one with no program.
fn actor_still(painter: &egui::Painter, icon: Rect, view: &LevelView, key: &ActorKey, art: &mut Art<'_>) {
    let Some(draw) = view.art.draws.get(key).filter(|d| !d.commands.is_empty()) else {
        let marker = icon.shrink(10.0);
        painter.rect(marker, CornerRadius::same(2), Color32::from_rgba_unmultiplied(110, 110, 110, 190), Stroke::NONE, StrokeKind::Inside);
        painter.text(marker.center(), egui::Align2::CENTER_CENTER, key.sprite_type.to_string(), egui::FontId::proportional(10.0), Color32::WHITE);
        return;
    };
    let bounds = draw
        .commands
        .iter()
        .map(|c| Rect::from_min_size(Pos2::new(c.x as f32, c.y as f32), Vec2::new(c.width as f32, c.height as f32)))
        .reduce(Rect::union)
        .unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::splat(16.0)));
    let zoom = (icon.width() / bounds.width().max(1.0)).min(icon.height() / bounds.height().max(1.0)).min(2.0);
    let frame = Frame { origin: icon.center(), pan: bounds.center().to_vec2(), zoom };
    let painter = painter.with_clip_rect(icon.expand(2.0));
    for command in &draw.commands {
        paint_command(&painter, frame, command, Vec2::ZERO, art);
    }
}

fn sprites(ui: &mut Ui, view: &mut LevelView, filter: &str, art: &mut Art<'_>) {
    let groups = sprite_entries(view);
    if groups.is_empty() {
        ui.weak("Open a level to list its actors.");
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (category, entries) in groups {
            let shown: Vec<&SpriteEntry> = entries.iter().filter(|e| matches(filter, &[&e.label, &category])).collect();
            if shown.is_empty() {
                continue;
            }
            controls::group(ui, ("sprites", &category), format!("{category} ({})", shown.len()), !filter.is_empty(), |ui| {
                // Stills are asked for only once their category is open.
                view.art.wanted.extend(shown.iter().filter(|e| e.has_renderer).map(|e| still(e.sprite_type, None)));
                let placement = view.placement.clone();
                let clicked = grid(ui, &shown, |ui, entry| {
                    let chosen = placement.kind == Kind::Sprite && placement.sprite_hash.is_none() && placement.sprite_type == entry.sprite_type;
                    let key = still(entry.sprite_type, None);
                    preview_button(ui, &entry.label, &entry.label, chosen, |painter, icon| actor_still(painter, icon, view, &key, art))
                });
                if let Some(entry) = clicked.and_then(|i| shown.get(i)) {
                    view.placement.set_sprite(entry.sprite_type);
                }
            });
        }
    });
}

fn custom(ui: &mut Ui, view: &mut LevelView, modules: Option<&ModuleDataCatalog>, filter: &str, art: &mut Art<'_>) {
    let tree = modules.map(custom_entries).unwrap_or_default();
    if tree.is_empty() {
        ui.weak("No actors are defined in this project's modules.");
        return;
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (module, components) in &tree {
            let visible = components.iter().any(|(_, actors)| actors.iter().any(|(_, label)| matches(filter, &[label])));
            if !visible {
                continue;
            }
            controls::group(ui, ("custom", module), module, true, |ui| {
                for (component, actors) in components {
                    let shown: Vec<&(u32, String)> = actors.iter().filter(|(_, label)| matches(filter, &[label])).collect();
                    if shown.is_empty() {
                        continue;
                    }
                    controls::group(ui, ("custom", module, component), component, true, |ui| {
                        for (hash, label) in shown {
                            let key = still(0, Some(*hash));
                            view.art.wanted.push(key);
                            let chosen = view.placement.sprite_hash == Some(*hash);
                            ui.horizontal(|ui| {
                                let (icon, _) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::hover());
                                actor_still(&ui.painter_at(icon), icon, view, &key, art);
                                if Row::new(label).selected(chosen).show(ui).clicked() {
                                    view.placement.set_custom_sprite(*hash);
                                }
                            });
                        }
                    });
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use bigstar_formats::ObjectLayoutTile;
    use bigstar_level::StageObjectDeclaration;

    use super::*;

    fn layout(rows: Vec<Vec<i32>>) -> ObjectLayout {
        ObjectLayout { rows: rows.into_iter().map(|r| r.into_iter().map(|t| ObjectLayoutTile::new(t, 0)).collect()).collect(), ..Default::default() }
    }

    #[test]
    fn objects_preview_at_their_own_size_per_slot() {
        let wide = object_entry(0, 0, &layout(vec![vec![7, 8], vec![9, 10]]));
        assert_eq!(wide.preview, (2, 2));
        assert!(wide.tooltip.contains("2x2"), "{}", wide.tooltip);
        let huge = object_entry(1, 3, &layout(vec![vec![7; 20]; 12]));
        assert_eq!(huge.preview, (8, 8), "capped to fit a button");
        assert_eq!(huge.label, "Object 1:3");
    }

    #[test]
    fn objects_that_draw_nothing_say_which_nothing_they_are() {
        let eraser = object_entry(0, 0, &layout(vec![vec![OBJECT_LAYOUT_EMPTY_TILE]]));
        let inert = object_entry(0, 1, &layout(vec![vec![1]]));
        let ordinary = object_entry(0, 2, &layout(vec![vec![7]]));
        assert_eq!((eraser.label.as_str(), eraser.effect), ("Object 0:0 - eraser", LevelObjectEffect::Erases));
        assert!(eraser.tooltip.contains("clears the cells it covers"));
        assert_eq!((inert.label.as_str(), inert.effect), ("Object 0:1 - draws nothing", LevelObjectEffect::Inert));
        assert!(inert.tooltip.contains("leaves the cells alone"));
        assert_eq!(ordinary.label, "Object 0:2");
    }

    #[test]
    fn module_actors_group_by_module_and_component() {
        let actor = |name: &str, module: &str, component: &str, hash| StageObjectDeclaration {
            name: name.to_owned(),
            module: module.to_owned(),
            component: component.to_owned(),
            hash,
            object_id: 400 + hash,
            ..Default::default()
        };
        let modules = ModuleDataCatalog {
            stage_objects: vec![actor("Sign", "coop", "props", 1), actor("Door", "coop", "props", 2), actor("Ghost", "spooky", "enemies", 3)],
            ..Default::default()
        };
        let tree = custom_entries(&modules);
        assert_eq!(tree.len(), 2);
        assert_eq!(tree["coop"]["props"], [(1, "Sign  [object 401]".to_owned()), (2, "Door  [object 402]".to_owned())]);
    }
}
