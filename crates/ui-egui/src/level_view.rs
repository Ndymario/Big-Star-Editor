//! A level as the frontend holds it: the host's snapshot plus what is only the frontend's business (selection, a drag in
//! progress, where the camera is). Positions here are level pixels, 16 to a tile.

use std::collections::BTreeSet;

use bigstar_level::{IndexedLevelValue, LevelEntitySelection, LevelEntityUpdates};
use bigstar_protocol::{EditOutcome, LevelEdit, LevelSnapshot};
use egui::{Pos2, Rect, Vec2};

pub const TILE: f32 = 16.0;
/// Entrances, views, zones and path nodes sit on pixels and move in steps of this many; objects and sprites sit on tiles.
const FREE_STEP: f32 = 8.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    Object,
    Sprite,
    Entrance,
    View,
    Zone,
    Path,
    ProgressPath,
}

pub type Item = (Kind, usize);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DragMode {
    /// Moving the selection; `delta` is how far the pointer has gone from where it started, in level pixels.
    Move {
        delta: Vec2,
        submitted: bool,
    },
    Pan,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelView {
    pub snapshot: LevelSnapshot,
    pub selection: BTreeSet<Item>,
    pub drag: Option<DragMode>,
    pub zoom: f32,
    /// The level position at the canvas' top-left corner.
    pub pan: Vec2,
    /// Frame the whole level the next time the canvas knows its size.
    pub needs_fit: bool,
    /// Where the canvas was drawn last frame, in screen points.
    pub canvas: Rect,
}

fn snap(value: f32, step: f32) -> i32 {
    ((value / step).round() * step) as i32
}

fn clamp_u16(value: i32) -> u16 {
    value.clamp(0, i32::from(u16::MAX)) as u16
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

impl LevelView {
    pub fn new(snapshot: LevelSnapshot) -> Self {
        Self { snapshot, selection: BTreeSet::new(), drag: None, zoom: 1.0, pan: Vec2::ZERO, needs_fit: true, canvas: Rect::NOTHING }
    }

    /// Takes a newer snapshot of the same level, keeping what is still there to be selected.
    pub fn replace(&mut self, snapshot: LevelSnapshot) {
        self.snapshot = snapshot;
        self.drag = None;
        let (snapshot, selection) = (&self.snapshot, &mut self.selection);
        selection.retain(|&(kind, index)| index < count(snapshot, kind));
    }

    pub fn edited(&mut self, snapshot: LevelSnapshot, outcome: &EditOutcome) {
        self.replace(snapshot);
        if let EditOutcome::Added(added) = outcome {
            self.selection = items_of(added);
        }
    }

    /// Whatever was in flight has been answered.
    pub fn settled(&mut self) {
        if matches!(self.drag, Some(DragMode::Move { submitted: true, .. })) {
            self.drag = None;
        }
    }

    /// How far an item is drawn from where the host has it: the drag in progress, snapped.
    pub fn offset(&self, item: Item) -> Vec2 {
        match self.drag {
            Some(DragMode::Move { delta, .. }) if self.selection.contains(&item) => shift(item.0, delta),
            _ => Vec2::ZERO,
        }
    }

    /// The pixels an item covers (a path: the box around its nodes), before any drag.
    pub fn bounds(&self, (kind, index): Item) -> Option<Rect> {
        let course = &self.snapshot.course;
        let rect = |x: f32, y: f32, w: f32, h: f32| Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h));
        match kind {
            Kind::Object => {
                let o = self.snapshot.objects.get(index)?;
                Some(rect(f32::from(o.x) * TILE, f32::from(o.y) * TILE, f32::from(o.width) * TILE, f32::from(o.height) * TILE))
            }
            Kind::Sprite => course.sprites.get(index).map(|s| rect(f32::from(s.x) * TILE, f32::from(s.y) * TILE, TILE, TILE)),
            Kind::Entrance => course.entrances.get(index).map(|e| rect(f32::from(e.x), f32::from(e.y), TILE, TILE)),
            Kind::View => course.views.get(index).map(|v| rect(f32::from(v.x), f32::from(v.y), f32::from(v.width), f32::from(v.height))),
            Kind::Zone => course.zones.get(index).map(|z| rect(f32::from(z.x), f32::from(z.y), f32::from(z.width), f32::from(z.height))),
            Kind::Path | Kind::ProgressPath => {
                let nodes = self.path(kind, index)?;
                // A node is a tile-sized square at its position.
                let mut squares = nodes.iter().map(|n| Rect::from_min_size(Pos2::new(f32::from(n.x), f32::from(n.y)), Vec2::splat(TILE)));
                let first = squares.next()?;
                Some(squares.fold(first, |all, square| all.union(square)))
            }
        }
    }

    pub fn path(&self, kind: Kind, index: usize) -> Option<&Vec<bigstar_formats::NsmbLevelPathNode>> {
        let course = &self.snapshot.course;
        match kind {
            Kind::Path => course.paths.get(index).map(|p| &p.nodes),
            Kind::ProgressPath => course.progress_paths.get(index).map(|p| &p.nodes),
            _ => None,
        }
    }

    /// The box around everything in the level, or `None` for an empty one.
    pub fn content_bounds(&self) -> Option<Rect> {
        let mut all: Option<Rect> = None;
        for kind in ALL_KINDS {
            for index in 0..count(&self.snapshot, kind) {
                if let Some(rect) = self.bounds((kind, index)) {
                    all = Some(all.map_or(rect, |a| a.union(rect)));
                }
            }
        }
        all
    }

    /// What is under `point`, the nearest to the viewer first. `slack` widens thin things such as borders and nodes.
    pub fn hit(&self, point: Pos2, slack: f32) -> Option<Item> {
        let course = &self.snapshot.course;
        let last = |kind, len: usize| (0..len).rev().map(move |i| (kind, i));
        let filled = [(Kind::Sprite, course.sprites.len()), (Kind::Entrance, course.entrances.len())];
        for (kind, len) in filled {
            for item in last(kind, len) {
                if self.bounds(item).is_some_and(|r| r.expand(slack / 2.0).contains(point)) {
                    return Some(item);
                }
            }
        }
        for kind in [Kind::Path, Kind::ProgressPath] {
            for item in last(kind, count(&self.snapshot, kind)) {
                let near = self.path(kind, item.1).is_some_and(|nodes| {
                    nodes
                        .iter()
                        .any(|n| (Pos2::new(f32::from(n.x) + TILE / 2.0, f32::from(n.y) + TILE / 2.0) - point).length() <= slack.max(TILE / 2.0))
                });
                if near {
                    return Some(item);
                }
            }
        }
        for item in last(Kind::Object, self.snapshot.objects.len()) {
            if self.bounds(item).is_some_and(|r| r.contains(point)) {
                return Some(item);
            }
        }
        // Views and zones are big and sit behind everything, so only their edges take a click.
        for kind in [Kind::Zone, Kind::View] {
            for item in last(kind, count(&self.snapshot, kind)) {
                if self.bounds(item).is_some_and(|r| r.expand(slack).contains(point) && !r.shrink(slack).contains(point)) {
                    return Some(item);
                }
            }
        }
        None
    }

    /// The selection moved by the drag in progress, as one undo step; `None` when nothing would change.
    pub fn move_edit(&self) -> Option<LevelEdit> {
        let Some(DragMode::Move { delta, .. }) = self.drag else {
            return None;
        };
        let course = &self.snapshot.course;
        let mut updates = LevelEntityUpdates::default();
        for &(kind, index) in &self.selection {
            let d = shift(kind, delta);
            let (dx, dy) = (d.x as i32, d.y as i32);
            if dx == 0 && dy == 0 {
                continue;
            }
            match kind {
                Kind::Object => {
                    if let Some(o) = self.snapshot.objects.get(index) {
                        let mut value = o.clone();
                        value.x = clamp_i16(i32::from(o.x) + dx / TILE as i32);
                        value.y = clamp_i16(i32::from(o.y) + dy / TILE as i32);
                        updates.objects.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Sprite => {
                    if let Some(s) = course.sprites.get(index) {
                        let mut value = s.clone();
                        value.x = clamp_u16(i32::from(s.x) + dx / TILE as i32);
                        value.y = clamp_u16(i32::from(s.y) + dy / TILE as i32);
                        updates.sprites.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Entrance => {
                    if let Some(e) = course.entrances.get(index) {
                        let mut value = e.clone();
                        value.x = clamp_u16(i32::from(e.x) + dx);
                        value.y = clamp_u16(i32::from(e.y) + dy);
                        updates.entrances.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::View => {
                    if let Some(v) = course.views.get(index) {
                        let mut value = v.clone();
                        value.x = clamp_u16(i32::from(v.x) + dx);
                        value.y = clamp_u16(i32::from(v.y) + dy);
                        updates.views.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Zone => {
                    if let Some(z) = course.zones.get(index) {
                        let mut value = z.clone();
                        value.x = clamp_u16(i32::from(z.x) + dx);
                        value.y = clamp_u16(i32::from(z.y) + dy);
                        updates.zones.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Path | Kind::ProgressPath => {
                    let list = if kind == Kind::Path { &course.paths } else { &course.progress_paths };
                    if let Some(p) = list.get(index) {
                        let mut value = p.clone();
                        for node in &mut value.nodes {
                            node.x = clamp_i16(i32::from(node.x) + dx);
                            node.y = clamp_i16(i32::from(node.y) + dy);
                        }
                        let target = if kind == Kind::Path { &mut updates.paths } else { &mut updates.progress_paths };
                        target.push(IndexedLevelValue { index, value });
                    }
                }
            }
        }
        let empty = updates.objects.is_empty()
            && updates.sprites.is_empty()
            && updates.entrances.is_empty()
            && updates.views.is_empty()
            && updates.zones.is_empty()
            && updates.paths.is_empty()
            && updates.progress_paths.is_empty();
        let name = if self.selection.len() == 1 { "Move" } else { "Move selection" };
        (!empty).then(|| LevelEdit::Update { updates, name: name.to_owned() })
    }

    pub fn remove_edit(&self) -> Option<LevelEdit> {
        if self.selection.is_empty() {
            return None;
        }
        let mut selection = LevelEntitySelection::default();
        for &(kind, index) in &self.selection {
            match kind {
                Kind::Object => selection.objects.push(index),
                Kind::Sprite => selection.sprites.push(index),
                Kind::Entrance => selection.entrances.push(index),
                Kind::View => selection.views.push(index),
                Kind::Zone => selection.zones.push(index),
                Kind::Path => selection.paths.push(index),
                Kind::ProgressPath => selection.progress_paths.push(index),
            }
        }
        Some(LevelEdit::Remove { selection, name: "Delete".to_owned() })
    }
}

const ALL_KINDS: [Kind; 7] = [Kind::Object, Kind::Sprite, Kind::Entrance, Kind::View, Kind::Zone, Kind::Path, Kind::ProgressPath];

pub fn count(snapshot: &LevelSnapshot, kind: Kind) -> usize {
    let course = &snapshot.course;
    match kind {
        Kind::Object => snapshot.objects.len(),
        Kind::Sprite => course.sprites.len(),
        Kind::Entrance => course.entrances.len(),
        Kind::View => course.views.len(),
        Kind::Zone => course.zones.len(),
        Kind::Path => course.paths.len(),
        Kind::ProgressPath => course.progress_paths.len(),
    }
}

/// A drag of `delta` level pixels, snapped the way `kind` snaps.
fn shift(kind: Kind, delta: Vec2) -> Vec2 {
    let step = if matches!(kind, Kind::Object | Kind::Sprite) { TILE } else { FREE_STEP };
    Vec2::new(snap(delta.x, step) as f32, snap(delta.y, step) as f32)
}

fn items_of(selection: &LevelEntitySelection) -> BTreeSet<Item> {
    let mut items = BTreeSet::new();
    let lists = [
        (Kind::Object, &selection.objects),
        (Kind::Sprite, &selection.sprites),
        (Kind::Entrance, &selection.entrances),
        (Kind::View, &selection.views),
        (Kind::Zone, &selection.zones),
        (Kind::Path, &selection.paths),
        (Kind::ProgressPath, &selection.progress_paths),
    ];
    for (kind, indices) in lists {
        items.extend(indices.iter().map(|&index| (kind, index)));
    }
    items
}

#[cfg(test)]
mod tests {
    use bigstar_formats::{NsmbLevelCourse, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelStageObject, NsmbLevelZone};

    use super::*;

    pub(crate) fn snapshot() -> LevelSnapshot {
        let course = NsmbLevelCourse {
            sprites: vec![NsmbLevelStageObject { id: 1, x: 6, y: 4, ..Default::default() }],
            zones: vec![NsmbLevelZone { x: 0, y: 0, width: 400, height: 200, ..Default::default() }],
            paths: vec![NsmbLevelPath {
                id: 0,
                nodes: vec![NsmbLevelPathNode { x: 300, y: 100, ..Default::default() }, NsmbLevelPathNode { x: 340, y: 100, ..Default::default() }],
                unknown: 0,
            }],
            ..Default::default()
        };
        LevelSnapshot {
            level: 1,
            area: "A01_1".to_owned(),
            title: "1-1".to_owned(),
            revision: 1,
            course,
            objects: vec![NsmbLevelObject { object_id: 1, tileset_slot: 0, x: 2, y: 3, width: 4, height: 1 }],
            tileset_references: Default::default(),
            glue_error: None,
            dirty: false,
            can_undo: false,
            can_redo: false,
            undo_names: Vec::new(),
            redo_names: Vec::new(),
        }
    }

    #[test]
    fn the_nearest_thing_wins_a_click() {
        let view = LevelView::new(snapshot());
        assert_eq!(view.hit(Pos2::new(104.0, 72.0), 2.0), Some((Kind::Sprite, 0)));
        assert_eq!(view.hit(Pos2::new(40.0, 56.0), 2.0), Some((Kind::Object, 0)));
        assert_eq!(view.hit(Pos2::new(308.0, 108.0), 2.0), Some((Kind::Path, 0)));
        // Inside a zone is nothing; its edge is the zone.
        assert_eq!(view.hit(Pos2::new(200.0, 150.0), 2.0), None);
        assert_eq!(view.hit(Pos2::new(400.0, 150.0), 2.0), Some((Kind::Zone, 0)));
    }

    #[test]
    fn a_drag_snaps_per_kind_and_makes_one_edit() {
        let mut view = LevelView::new(snapshot());
        view.selection = [(Kind::Object, 0), (Kind::Sprite, 0)].into_iter().collect();
        view.drag = Some(DragMode::Move { delta: Vec2::new(35.0, -13.0), submitted: false });
        // Objects and sprites go by tiles (35 px is 2 tiles, -13 px is -1), everything else by 8 px.
        assert_eq!(view.offset((Kind::Object, 0)), Vec2::new(32.0, -16.0));
        assert_eq!(view.offset((Kind::Sprite, 0)), Vec2::new(32.0, -16.0));
        let Some(LevelEdit::Update { updates, name }) = view.move_edit() else {
            panic!("no edit");
        };
        assert_eq!(name, "Move selection");
        assert_eq!((updates.objects[0].value.x, updates.objects[0].value.y), (4, 2));
        assert_eq!((updates.sprites[0].value.x, updates.sprites[0].value.y), (8, 3));
    }

    #[test]
    fn a_drag_that_goes_nowhere_makes_no_edit() {
        let mut view = LevelView::new(snapshot());
        view.selection.insert((Kind::Sprite, 0));
        view.drag = Some(DragMode::Move { delta: Vec2::new(3.0, -2.0), submitted: false });
        assert_eq!(view.move_edit(), None);
    }

    #[test]
    fn moves_stay_inside_the_number_range() {
        let mut view = LevelView::new(snapshot());
        view.selection.insert((Kind::Sprite, 0));
        view.drag = Some(DragMode::Move { delta: Vec2::new(-500.0, 0.0), submitted: false });
        let Some(LevelEdit::Update { updates, .. }) = view.move_edit() else {
            panic!("no edit");
        };
        assert_eq!(updates.sprites[0].value.x, 0);
    }

    #[test]
    fn a_path_moves_by_all_its_nodes() {
        let mut view = LevelView::new(snapshot());
        view.selection.insert((Kind::Path, 0));
        view.drag = Some(DragMode::Move { delta: Vec2::new(16.0, 8.0), submitted: false });
        let Some(LevelEdit::Update { updates, .. }) = view.move_edit() else {
            panic!("no edit");
        };
        let nodes = &updates.paths[0].value.nodes;
        assert_eq!((nodes[0].x, nodes[0].y, nodes[1].x, nodes[1].y), (316, 108, 356, 108));
    }

    #[test]
    fn a_newer_snapshot_keeps_only_selections_that_still_exist() {
        let mut view = LevelView::new(snapshot());
        view.selection = [(Kind::Sprite, 0), (Kind::Sprite, 5), (Kind::Object, 0)].into_iter().collect();
        view.drag = Some(DragMode::Pan);
        view.replace(snapshot());
        assert_eq!(view.selection, [(Kind::Object, 0), (Kind::Sprite, 0)].into_iter().collect());
        assert_eq!(view.drag, None);
    }

    #[test]
    fn deleting_names_every_selected_kind() {
        let mut view = LevelView::new(snapshot());
        assert_eq!(view.remove_edit(), None);
        view.selection = [(Kind::Object, 0), (Kind::Zone, 0)].into_iter().collect();
        let Some(LevelEdit::Remove { selection, .. }) = view.remove_edit() else {
            panic!("no edit");
        };
        assert_eq!((selection.objects, selection.zones, selection.sprites), (vec![0], vec![0], vec![]));
    }

    #[test]
    fn content_bounds_cover_everything() {
        let view = LevelView::new(snapshot());
        let b = view.content_bounds();
        assert_eq!(b, Some(Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(400.0, 200.0))));
    }
}
