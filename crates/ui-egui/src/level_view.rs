//! A level as the frontend holds it: the host's snapshot plus what is only the frontend's business (selection, a
//! gesture in progress, where the camera is, the graphics the host has served). Positions here are level pixels, 16 to
//! a tile. This is `LevelPreviewWidget` without the painting: every rule about what a click, a drag or a key does lives
//! here, so it can be tested without a window.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

use bigstar_formats::{NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelSprite, NsmbLevelView, NsmbLevelZone};
use bigstar_level::{
    BackgroundLayer, DS_SCREEN_HEIGHT, DS_SCREEN_WIDTH, DecodedTileset, IndexedLevelValue, LEVEL_WRAPS_AROUND, LayerScroll, LevelEntityInsertions,
    LevelEntitySelection, LevelEntityUpdates, LevelTilemap, ReorderKind, SpriteBounds, SpriteInsertion, ViewFrame, camera_block, entrance_camera,
    layer_scroll, rasterize_level_objects,
};
use bigstar_protocol::{ActorCatalog, ActorDraw, ActorInfo, ActorInput, EditOutcome, LevelEdit, LevelGraphics, LevelSnapshot, TextureId};
use egui::{Pos2, Rect, Vec2};
use serde::{Deserialize, Serialize};

use crate::properties::{PanelState, Tab, item_count};

pub const TILE: f32 = 16.0;
pub const MIN_ZOOM: f32 = 0.02;
pub const MAX_ZOOM: f32 = 16.0;
/// What an empty level frames: 512 by 256 tiles.
const EMPTY_SCENE: Rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(512.0 * 16.0, 256.0 * 16.0));
/// Room left around everything when framing a level.
const SCENE_MARGIN: f32 = 128.0;
/// How far outside its rectangle an entity still takes a click, in level pixels.
const HIT_SLACK: f32 = 3.0;
/// A knob takes a click this far from its centre, in screen points.
const KNOB_REACH: f32 = 6.0;
/// The room, in level pixels, left around something brought into view.
const REVEAL_MARGIN: f32 = 32.0;

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

impl Kind {
    pub const ALL: [Kind; 7] = [Kind::Object, Kind::Sprite, Kind::Entrance, Kind::Zone, Kind::Path, Kind::ProgressPath, Kind::View];

    /// Only rectangular entities can be resized: tile objects, zones and views.
    pub fn resizable(self) -> bool {
        matches!(self, Kind::Object | Kind::Zone | Kind::View)
    }

    /// Actors, entrances, zones and views store their position unsigned, so none of them can sit above or left of the
    /// origin. Objects and path nodes are signed and are free to go out there.
    pub fn unsigned(self) -> bool {
        matches!(self, Kind::Sprite | Kind::Entrance | Kind::Zone | Kind::View)
    }

    /// Paths and progress paths are the only kinds selected down to a node.
    pub fn path(self) -> bool {
        matches!(self, Kind::Path | Kind::ProgressPath)
    }

    /// Placed and sized in tiles rather than pixels.
    fn tiled(self) -> bool {
        matches!(self, Kind::Object | Kind::Sprite)
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Object => "Object",
            Kind::Sprite => "Sprite",
            Kind::Entrance => "Entrance",
            Kind::View => "View",
            Kind::Zone => "Zone",
            Kind::Path => "Path",
            Kind::ProgressPath => "Progress path",
        }
    }
}

/// One selected thing. A path is selected down to one of its nodes; `node` is `None` for every other kind, and for a
/// path that was selected as a whole (a pasted one).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item {
    pub kind: Kind,
    pub index: usize,
    pub node: Option<usize>,
}

impl Item {
    pub const fn new(kind: Kind, index: usize) -> Self {
        Self { kind, index, node: None }
    }

    pub const fn node(kind: Kind, index: usize, node: usize) -> Self {
        Self { kind, index, node: Some(node) }
    }
}

/// The eight resize knobs, clockwise from the top left, corners alternating with edge midpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Handle {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

impl Handle {
    pub const ALL: [Handle; 8] =
        [Handle::TopLeft, Handle::Top, Handle::TopRight, Handle::Right, Handle::BottomRight, Handle::Bottom, Handle::BottomLeft, Handle::Left];

    /// Each knob owns the edges it sits on; the rest of the rectangle stays put.
    pub fn edges(self, delta: Vec2) -> ResizeEdges {
        use Handle::*;
        let left = matches!(self, TopLeft | Left | BottomLeft);
        let right = matches!(self, TopRight | Right | BottomRight);
        let top = matches!(self, TopLeft | Top | TopRight);
        let bottom = matches!(self, BottomLeft | Bottom | BottomRight);
        let pick = |owned: bool, value: f32| if owned { value } else { 0.0 };
        ResizeEdges { left: pick(left, delta.x), top: pick(top, delta.y), right: pick(right, delta.x), bottom: pick(bottom, delta.y) }
    }
}

/// The knob centres of a rectangle, in [`Handle::ALL`] order.
pub fn handle_centres(rect: Rect) -> [Pos2; 8] {
    let (middle_x, middle_y) = (rect.center().x, rect.center().y);
    [
        rect.left_top(),
        Pos2::new(middle_x, rect.top()),
        rect.right_top(),
        Pos2::new(rect.right(), middle_y),
        rect.right_bottom(),
        Pos2::new(middle_x, rect.bottom()),
        rect.left_bottom(),
        Pos2::new(rect.left(), middle_y),
    ]
}

/// How far each edge of a rectangle is being dragged, in level pixels.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ResizeEdges {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl ResizeEdges {
    pub fn apply(self, rect: Rect) -> Rect {
        Rect::from_min_max(Pos2::new(rect.min.x + self.left, rect.min.y + self.top), Pos2::new(rect.max.x + self.right, rect.max.y + self.bottom))
    }

    fn is_zero(self) -> bool {
        [self.left, self.top, self.right, self.bottom].iter().all(|v| v.abs() < f32::EPSILON)
    }
}

/// What the pointer is doing. A move or resize that has been sent to the host stays on screen (`submitted`) until the
/// host answers, so the dropped things do not jump back for a frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    /// Moving the selection by `delta` level pixels, already snapped.
    Move {
        delta: Vec2,
        submitted: bool,
    },
    Resize {
        handle: Handle,
        delta: Vec2,
        submitted: bool,
    },
    Pan,
    /// Dragging out an object a right-press began, between two cells; it is created when the button comes up.
    Place {
        from: (i16, i16),
        to: (i16, i16),
        submitted: bool,
    },
    /// A rubber band, in screen points.
    Marquee {
        from: Pos2,
        to: Pos2,
    },
}

/// The canvas toolbar's switches. Every canvas shows the same ones, and they are kept between runs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CanvasOptions {
    pub snap: bool,
    /// The step of everything that is not placed in tiles: 16, or 8 with "8 px" on.
    pub snap_size: f32,
    pub show_minimap: bool,
    pub show_grid: bool,
    pub show_ds_screen: bool,
    pub backgrounds: Backgrounds,
    /// Whether the layers move as the game moves them: their clouds drifting, their lava and torches turning over.
    pub animate_backgrounds: bool,
}

/// Where the canvas draws a level's background and foreground layers.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Backgrounds {
    Hidden,
    /// Only inside the views, where the game shows them.
    #[default]
    InViews,
    /// Over the whole level, past the views' edges.
    Overflowing,
}

impl Backgrounds {
    pub const ALL: [Self; 3] = [Self::Hidden, Self::InViews, Self::Overflowing];

    pub fn name(self) -> &'static str {
        match self {
            Self::Hidden => "No backgrounds",
            Self::InViews => "Backgrounds inside views",
            Self::Overflowing => "Backgrounds overflowing",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Self::Hidden => "BG off",
            Self::InViews => "BG in views",
            Self::Overflowing => "BG all over",
        }
    }
}

impl Default for CanvasOptions {
    fn default() -> Self {
        Self {
            snap: true,
            snap_size: TILE,
            show_minimap: true,
            show_grid: true,
            show_ds_screen: false,
            backgrounds: Backgrounds::default(),
            animate_backgrounds: true,
        }
    }
}

/// What a right-click creates. Changing it never touches the selection.
#[derive(Debug, Clone, PartialEq)]
pub struct Placement {
    pub kind: Kind,
    pub object_slot: u8,
    pub object_id: u16,
    pub sprite_type: u16,
    pub sprite_hash: Option<u32>,
}

impl Default for Placement {
    fn default() -> Self {
        Self { kind: Kind::Object, object_slot: 0, object_id: 0, sprite_type: 0, sprite_hash: None }
    }
}

impl Placement {
    pub fn set_object(&mut self, slot: u8, object_id: u16) {
        self.object_slot = slot.min(2);
        self.object_id = object_id.min(4095);
        self.kind = Kind::Object;
    }

    pub fn set_sprite(&mut self, sprite_type: u16) {
        self.sprite_type = sprite_type;
        self.sprite_hash = None;
        self.kind = Kind::Sprite;
    }

    pub fn set_custom_sprite(&mut self, hash: u32) {
        self.sprite_hash = Some(hash);
        self.sprite_type = 0;
        self.kind = Kind::Sprite;
    }
}

/// One sprite as the host renders it: everything its draw list can depend on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorKey {
    pub sprite_type: u16,
    pub hash: Option<u32>,
    pub x: u16,
    pub y: u16,
    pub settings: u32,
    pub events: [u8; 2],
}

impl ActorKey {
    pub fn of(sprite: &NsmbLevelSprite, hash: Option<u32>) -> Self {
        Self { sprite_type: sprite.id, hash, x: sprite.x, y: sprite.y, settings: sprite.settings, events: sprite.events }
    }

    pub fn input(&self) -> ActorInput {
        ActorInput {
            sprite_type: self.sprite_type,
            hash: self.hash,
            x: i32::from(self.x),
            y: i32::from(self.y),
            pixel_x: f64::from(self.x) * 16.0,
            pixel_y: f64::from(self.y) * 16.0,
            settings: self.settings,
            events: self.events,
        }
    }

    /// The same actor, wherever it stands: a draw list made for one can stand in for the other, moved.
    fn same_actor(&self, other: &ActorKey) -> bool {
        (self.sprite_type, self.hash, self.settings, self.events) == (other.sprite_type, other.hash, other.settings, other.events)
    }
}

/// The actor catalog, indexed the two ways a sprite finds its actor.
#[derive(Debug, Clone, Default)]
pub struct ActorIndex {
    pub catalog: Arc<ActorCatalog>,
    by_object_id: HashMap<u32, usize>,
    by_hash: HashMap<u32, usize>,
}

impl ActorIndex {
    pub fn new(catalog: Arc<ActorCatalog>) -> Self {
        let (mut by_object_id, mut by_hash) = (HashMap::new(), HashMap::new());
        for (index, actor) in catalog.actors.iter().enumerate() {
            match actor.definition.hash {
                Some(hash) => by_hash.entry(hash).or_insert(index),
                None => by_object_id.entry(actor.definition.object_id).or_insert(index),
            };
        }
        Self { catalog, by_object_id, by_hash }
    }

    /// The actor a sprite of `sprite_type` (or a module sprite registered as `hash`) is drawn by.
    pub fn actor(&self, sprite_type: u16, hash: Option<u32>) -> Option<&ActorInfo> {
        let index = match hash {
            Some(hash) => self.by_hash.get(&hash),
            None => self.catalog.sprite_object_ids.get(usize::from(sprite_type)).and_then(|id| self.by_object_id.get(id)),
        }?;
        self.catalog.actors.get(*index)
    }
}

/// What the host has served for the open level.
#[derive(Debug, Clone, Default)]
pub struct LevelArt {
    pub graphics: Option<LevelGraphics>,
    /// The object layouts of each slot whose tileset decoded; the canvas draws blocks from the atlases.
    pub tilesets: [Option<DecodedTileset>; 3],
    pub actors: ActorIndex,
    pub draws: HashMap<ActorKey, ActorDraw>,
    /// Draw lists from before the host's files changed, drawn until their replacements arrive.
    stale: HashMap<ActorKey, ActorDraw>,
    /// Asked for and not answered yet.
    pub requested: HashSet<ActorKey>,
    /// Draw lists something other than a sprite wants this frame: the picker's stills.
    pub wanted: Vec<ActorKey>,
    /// Grows whenever the tilesets change, so the tile array is rebuilt.
    serial: u64,
}

impl LevelArt {
    pub fn set_graphics(&mut self, graphics: LevelGraphics) {
        for (target, slot) in self.tilesets.iter_mut().zip(&graphics.tilesets) {
            // A slot that did not decode keeps nothing, and its objects show as plain rectangles.
            *target = slot.error.is_none().then(|| DecodedTileset { map16_tiles: Vec::new(), objects: slot.objects.clone() });
        }
        self.graphics = Some(graphics);
        self.serial += 1;
    }

    /// The textures the level's art names.
    pub fn texture_ids(&self) -> Vec<TextureId> {
        let Some(graphics) = &self.graphics else { return Vec::new() };
        graphics
            .tilesets
            .iter()
            .filter_map(|slot| slot.atlas.clone())
            .chain([&graphics.background, &graphics.foreground].into_iter().flat_map(|layer| {
                // The still, and every picture the layer plays through.
                layer.texture.iter().cloned().chain(layer.frames.iter().map(|frame| frame.texture.clone()))
            }))
            .collect()
    }

    /// Forgets every draw list, for a change on the host that may have changed any of them.
    pub fn forget_draws(&mut self) {
        let draws = std::mem::take(&mut self.draws);
        self.stale.extend(draws);
        self.requested.clear();
    }

    fn draw(&self, key: &ActorKey) -> Option<&ActorDraw> {
        self.draws.get(key).or_else(|| self.stale.get(key))
    }
}

#[derive(Debug, Clone)]
pub struct LevelView {
    pub snapshot: LevelSnapshot,
    /// In the order things were selected; the first one is what the properties panel shows.
    pub selection: Vec<Item>,
    pub gesture: Option<Gesture>,
    pub zoom: f32,
    /// The level position at the canvas' top-left corner.
    pub pan: Vec2,
    /// Set once the person chooses a zoom; until then the level stays fitted to the canvas.
    pub zoom_pinned: bool,
    /// Frame the whole level the next time the canvas knows its size.
    pub needs_fit: bool,
    /// Where the canvas was drawn last frame, in screen points.
    pub canvas: Rect,
    pub options: CanvasOptions,
    pub placement: Placement,
    pub art: LevelArt,
    pub panel: PanelState,
    /// The selection and create kind the panel last followed, so it can tell a change made elsewhere.
    panel_seen: Vec<Item>,
    panel_kind: Kind,
    /// The sprites of the snapshot before this one, so an actor whose new draw list is still on its way is drawn from
    /// its old one instead of flickering to a marker.
    previous_keys: Vec<ActorKey>,
    tile_cache: Option<(u64, Arc<LevelTilemap>)>,
    /// A Ctrl-drag has asked the host for the copies it will drag; the drag waits for them.
    awaiting_clone: bool,
    /// The Ctrl-drag was released before the copies arrived: move them as soon as they do.
    release_after_clone: bool,
    /// The next `Added` is a placement, and a placed path is selected by its first node.
    placing: bool,
    /// Where the pointer is over the canvas, in level pixels: what the DS screen follows.
    pub hover: Option<Pos2>,
}

/// One screen of the DS screen overlay: what the game would show with its camera here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DsScreen {
    pub rect: Rect,
    /// The view whose blocks scroll the layers, when there is one to go by.
    pub view: Option<usize>,
    /// Where the selected entrance puts the camera, rather than where the pointer is.
    pub spawn: bool,
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
    Rect::from_min_size(Pos2::new(x, y), Vec2::new(width, height))
}

fn bounds_rect(bounds: SpriteBounds) -> Rect {
    rect(f32::from(bounds.x), f32::from(bounds.y), f32::from(bounds.width), f32::from(bounds.height))
}

fn object_rect(object: &NsmbLevelObject) -> Rect {
    rect(f32::from(object.x) * TILE, f32::from(object.y) * TILE, f32::from(object.width.max(1)) * TILE, f32::from(object.height.max(1)) * TILE)
}

fn entrance_rect(entrance: &NsmbLevelEntrance) -> Rect {
    rect(f32::from(entrance.x), f32::from(entrance.y), TILE, TILE)
}

fn view_rect(view: &NsmbLevelView) -> Rect {
    rect(f32::from(view.x), f32::from(view.y), f32::from(view.width.max(1)), f32::from(view.height.max(1)))
}

fn zone_rect(zone: &NsmbLevelZone) -> Rect {
    rect(f32::from(zone.x), f32::from(zone.y), f32::from(zone.width.max(1)), f32::from(zone.height.max(1)))
}

pub fn node_rect(node: &NsmbLevelPathNode) -> Rect {
    rect(f32::from(node.x), f32::from(node.y), TILE, TILE)
}

/// `qRound`: halves away from zero.
fn round(value: f32) -> i32 {
    value.round() as i32
}

fn clamp_u16(value: i32) -> u16 {
    value.clamp(0, i32::from(u16::MAX)) as u16
}

fn clamp_i16(value: i32) -> i16 {
    value.clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// The scale that puts all of `bounds` inside `size`.
pub fn fit_zoom(size: Vec2, bounds: Rect) -> f32 {
    if bounds.width() <= 0.0 || bounds.height() <= 0.0 || size.x <= 0.0 || size.y <= 0.0 {
        return 1.0;
    }
    (size.x / bounds.width()).min(size.y / bounds.height())
}

impl LevelView {
    pub fn new(snapshot: LevelSnapshot) -> Self {
        Self {
            snapshot,
            selection: Vec::new(),
            gesture: None,
            zoom: 1.0,
            pan: Vec2::ZERO,
            zoom_pinned: false,
            needs_fit: true,
            canvas: Rect::NOTHING,
            options: CanvasOptions::default(),
            placement: Placement::default(),
            art: LevelArt::default(),
            panel: PanelState::default(),
            panel_seen: Vec::new(),
            panel_kind: Kind::Object,
            previous_keys: Vec::new(),
            tile_cache: None,
            awaiting_clone: false,
            release_after_clone: false,
            placing: false,
            hover: None,
        }
    }

    /// Takes a newer snapshot of the same level, keeping what is still there to be selected.
    pub fn replace(&mut self, snapshot: LevelSnapshot) {
        self.previous_keys = self.actor_keys();
        self.snapshot = snapshot;
        // A gesture already sent is in the new snapshot; one still in progress carries on.
        if matches!(
            self.gesture,
            Some(Gesture::Move { submitted: true, .. } | Gesture::Resize { submitted: true, .. } | Gesture::Place { submitted: true, .. })
        ) {
            self.gesture = None;
        }
        let snapshot = &self.snapshot;
        self.selection.retain(|item| exists(snapshot, *item));
    }

    /// The host's answer to an edit. Returns the move a Ctrl-drag released early still owes.
    pub fn edited(&mut self, snapshot: LevelSnapshot, outcome: &EditOutcome) -> Option<LevelEdit> {
        self.replace(snapshot);
        match outcome {
            EditOutcome::Added(added) => {
                let first_node = std::mem::take(&mut self.placing);
                self.selection = items_of(added, first_node);
                if std::mem::take(&mut self.awaiting_clone) && std::mem::take(&mut self.release_after_clone) {
                    let Some(Gesture::Move { delta, .. }) = self.gesture else { return None };
                    let edit = self.move_edit(delta);
                    self.gesture = edit.as_ref().map(|_| Gesture::Move { delta, submitted: true });
                    return edit;
                }
            }
            EditOutcome::Moved(index) => {
                if let [item] = self.selection.as_mut_slice() {
                    item.index = *index;
                }
            }
            EditOutcome::None => {}
        }
        None
    }

    /// Whatever was in flight has been answered, or refused.
    pub fn settled(&mut self) {
        if matches!(
            self.gesture,
            Some(Gesture::Move { submitted: true, .. } | Gesture::Resize { submitted: true, .. } | Gesture::Place { submitted: true, .. })
        ) || self.awaiting_clone
        {
            self.gesture = None;
        }
        self.awaiting_clone = false;
        self.release_after_clone = false;
        self.placing = false;
    }

    pub fn sprite_hash(&self, index: usize) -> Option<u32> {
        self.snapshot.sprite_hashes.get(index).copied().flatten()
    }

    pub fn actor_keys(&self) -> Vec<ActorKey> {
        self.snapshot.course.sprites.iter().enumerate().map(|(index, sprite)| ActorKey::of(sprite, self.sprite_hash(index))).collect()
    }

    /// The draw list for sprite `index` and how far to move it: its own, or the one it had before it moved while the
    /// new one is on its way.
    pub fn actor_draw(&self, index: usize) -> Option<(&ActorDraw, Vec2)> {
        let key = ActorKey::of(self.snapshot.course.sprites.get(index)?, self.sprite_hash(index));
        if let Some(draw) = self.art.draw(&key) {
            return Some((draw, Vec2::ZERO));
        }
        let previous = self.previous_keys.get(index).filter(|old| old.same_actor(&key))?;
        let draw = self.art.draw(previous)?;
        let moved = Vec2::new((f32::from(key.x) - f32::from(previous.x)) * TILE, (f32::from(key.y) - f32::from(previous.y)) * TILE);
        Some((draw, moved))
    }

    /// Whether sprite `index`'s draw list has been asked for and not answered.
    pub fn actor_pending(&self, index: usize) -> bool {
        self.snapshot.course.sprites.get(index).is_some_and(|sprite| self.art.requested.contains(&ActorKey::of(sprite, self.sprite_hash(index))))
    }

    /// The sprites whose draw lists are neither here nor asked for; marks them asked for.
    pub fn take_missing_actor_keys(&mut self) -> Vec<ActorKey> {
        let mut missing = Vec::new();
        let wanted = std::mem::take(&mut self.art.wanted);
        for key in self.actor_keys().into_iter().chain(wanted) {
            if !self.art.draws.contains_key(&key) && self.art.requested.insert(key) {
                missing.push(key);
            }
        }
        missing
    }

    // --- geometry --------------------------------------------------------------------------------------------------

    /// Where sprite `index` can be grabbed (`render` false) or what it covers when drawn (`render` true). A bounds
    /// function's answer is already in level pixels; the definition's bounds are an offset from the sprite's tile.
    pub fn sprite_rect(&self, index: usize, render: bool) -> Option<Rect> {
        let sprite = self.snapshot.course.sprites.get(index)?;
        let actor = self.art.actors.actor(sprite.id, self.sprite_hash(index));
        let bounds = actor.map_or_else(SpriteBounds::default, |a| if render { a.definition.render_bounds } else { a.definition.grab_bounds });
        if actor.is_some()
            && let Some((draw, moved)) = self.actor_draw(index)
            && let Some(dynamic) = if render { draw.render } else { draw.grab }
        {
            return Some(bounds_rect(dynamic).translate(moved));
        }
        Some(bounds_rect(bounds).translate(Vec2::new(f32::from(sprite.x) * TILE, f32::from(sprite.y) * TILE)))
    }

    /// The rectangle an item covers, before any gesture. A path item is its node.
    pub fn bounds(&self, item: Item) -> Option<Rect> {
        let course = &self.snapshot.course;
        match item.kind {
            Kind::Object => self.snapshot.objects.get(item.index).map(object_rect),
            Kind::Sprite => self.sprite_rect(item.index, false),
            Kind::Entrance => course.entrances.get(item.index).map(entrance_rect),
            Kind::View => course.views.get(item.index).map(view_rect),
            Kind::Zone => course.zones.get(item.index).map(zone_rect),
            Kind::Path | Kind::ProgressPath => {
                let nodes = self.path(item.kind, item.index)?;
                match item.node {
                    Some(node) => nodes.get(node).map(node_rect),
                    None => nodes.iter().map(node_rect).reduce(Rect::union),
                }
            }
        }
    }

    pub fn path(&self, kind: Kind, index: usize) -> Option<&Vec<NsmbLevelPathNode>> {
        let course = &self.snapshot.course;
        match kind {
            Kind::Path => course.paths.get(index).map(|p| &p.nodes),
            Kind::ProgressPath => course.progress_paths.get(index).map(|p| &p.nodes),
            _ => None,
        }
    }

    /// Every grabbable rectangle, back to front: whatever comes last is picked first. Views and zones lead precisely
    /// because they are the big rectangles everything else sits inside; picked first, they would make everything in
    /// them unreachable.
    pub fn entity_rects(&self) -> Vec<(Item, Rect)> {
        let course = &self.snapshot.course;
        let mut entities = Vec::new();
        entities.extend(course.views.iter().enumerate().map(|(i, v)| (Item::new(Kind::View, i), view_rect(v))));
        entities.extend(course.zones.iter().enumerate().map(|(i, z)| (Item::new(Kind::Zone, i), zone_rect(z))));
        entities.extend(self.snapshot.objects.iter().enumerate().map(|(i, o)| (Item::new(Kind::Object, i), object_rect(o))));
        for index in 0..course.sprites.len() {
            if let Some(rect) = self.sprite_rect(index, false) {
                entities.push((Item::new(Kind::Sprite, index), rect));
            }
        }
        entities.extend(course.entrances.iter().enumerate().map(|(i, e)| (Item::new(Kind::Entrance, i), entrance_rect(e))));
        for (kind, paths) in [(Kind::Path, &course.paths), (Kind::ProgressPath, &course.progress_paths)] {
            for (index, path) in paths.iter().enumerate() {
                entities.extend(path.nodes.iter().enumerate().map(|(node, value)| (Item::node(kind, index, node), node_rect(value))));
            }
        }
        entities
    }

    /// What a click at `point` grabs: front to back, across every layer.
    pub fn hit_test(&self, point: Pos2) -> Option<Item> {
        self.entity_rects().into_iter().rev().find(|(_, rect)| rect.expand(HIT_SLACK).contains(point)).map(|(item, _)| item)
    }

    /// The box around everything in the level, with room around it. A sprite counts as the tile it sits on, not as
    /// what its actor draws: that arrives from the host after the level does, and a view fitted to these bounds would
    /// jump when it did. The margin takes the overhang.
    pub fn scene_bounds(&self) -> Rect {
        let course = &self.snapshot.course;
        let mut rects: Vec<Rect> = Vec::new();
        rects.extend(course.zones.iter().map(zone_rect));
        rects.extend(course.views.iter().map(view_rect));
        rects.extend(self.snapshot.objects.iter().map(object_rect));
        rects.extend(course.sprites.iter().map(|sprite| rect(f32::from(sprite.x) * TILE, f32::from(sprite.y) * TILE, TILE, TILE)));
        rects.extend(course.entrances.iter().map(entrance_rect));
        rects.extend(course.paths.iter().chain(&course.progress_paths).flat_map(|p| p.nodes.iter().map(node_rect)));
        rects.into_iter().reduce(Rect::union).unwrap_or(EMPTY_SCENE).expand(SCENE_MARGIN)
    }

    /// Whether an entity is selected; `node` `None` asks about the item as a whole. A path selected as a whole counts
    /// for each of its nodes.
    pub fn is_selected(&self, kind: Kind, index: usize, node: Option<usize>) -> bool {
        self.selection.iter().any(|s| s.kind == kind && s.index == index && (node.is_none() || s.node.is_none() || s.node == node))
    }

    /// The entrance the DS screen goes by: the only thing selected, where a drag in progress has it.
    fn screen_entrance(&self) -> Option<NsmbLevelEntrance> {
        let [item] = self.selection.as_slice() else { return None };
        if item.kind != Kind::Entrance {
            return None;
        }
        let mut entrance = self.snapshot.course.entrances.get(item.index)?.clone();
        let at = self.preview(Kind::Entrance, item.index, entrance_rect(&entrance)).min;
        entrance.x = at.x.round().clamp(0.0, 65535.0) as u16;
        entrance.y = at.y.round().clamp(0.0, 65535.0) as u16;
        Some(entrance)
    }

    /// The screen the game opens on when the player comes in through `entrance`: the entrance's type and alignment
    /// place the player, its view bounds the camera, and the view's camera settings say how far down the screen the
    /// player is kept.
    pub fn spawn_screen(&self, entrance: &NsmbLevelEntrance) -> DsScreen {
        let course = &self.snapshot.course;
        let entered = course.views.iter().position(|view| view.id == entrance.view_id);
        let corner = match entered.and_then(|index| course.views.get(index)) {
            Some(view) => {
                let wraps = course.header.settings & LEVEL_WRAPS_AROUND != 0;
                entrance_camera(entrance, ViewFrame::of(view), camera_block(&course.cameras, view.camera_settings_id), wraps, None)
            }
            // No view to keep the camera in: the player centred, as far down the screen as the game likes them.
            None => {
                let (x, y) = bigstar_level::entrance_spawn_point(entrance);
                (x - DS_SCREEN_WIDTH / 2, y - 128)
            }
        };
        let size = Vec2::new(DS_SCREEN_WIDTH as f32, DS_SCREEN_HEIGHT as f32);
        DsScreen { rect: Rect::from_min_size(Pos2::new(corner.0 as f32, corner.1 as f32), size), view: entered, spawn: true }
    }

    /// What the DS screen overlay shows. A screen follows the pointer, centred on it, scrolled by the view the pointer
    /// is in. With one entrance selected the view is that entrance's instead, wherever the pointer goes, and a second
    /// screen stands where the game puts the camera when the player comes in through it.
    pub fn ds_screens(&self) -> Vec<DsScreen> {
        let mut screens = Vec::new();
        if !self.options.show_ds_screen {
            return screens;
        }
        let course = &self.snapshot.course;
        let size = Vec2::new(DS_SCREEN_WIDTH as f32, DS_SCREEN_HEIGHT as f32);
        let entrance = self.screen_entrance();
        let spawn = entrance.as_ref().map(|entrance| self.spawn_screen(entrance));
        let entered = spawn.and_then(|screen| screen.view);
        screens.extend(spawn);
        if let Some(hover) = self.hover {
            let corner = Pos2::new((hover.x - size.x / 2.0).round(), (hover.y - size.y / 2.0).round());
            let under = course.views.iter().position(|view| view_rect(view).contains(hover));
            screens.push(DsScreen { rect: Rect::from_min_size(corner, size), view: if entrance.is_some() { entered } else { under }, spawn: false });
        }
        screens
    }

    /// How the game scrolls a layer with its camera on `screen`, or nothing when the screen has no view to go by or
    /// the view shows no such layer.
    pub fn screen_layer(&self, screen: &DsScreen, layer: BackgroundLayer) -> Option<LayerScroll> {
        let course = &self.snapshot.course;
        let view = course.views.get(screen.view?)?;
        let (blocks, block) = match layer {
            BackgroundLayer::Background => (&course.backgrounds, view.background_id),
            BackgroundLayer::Foreground => (&course.foregrounds, view.foreground_id),
        };
        let wraps = course.header.settings & LEVEL_WRAPS_AROUND != 0;
        // The game lets tileset 30's foreground scroll on past the view as well.
        let free = layer == BackgroundLayer::Foreground && course.header.tileset_theme[0] == 0x1e;
        let camera = (screen.rect.left() as i32, screen.rect.top() as i32);
        let scroll = layer_scroll(layer, blocks, block, ViewFrame::of(view), camera, !wraps && !free)?;
        // What the layer's script says the game rewrites of it once it has scrolled far enough.
        let swaps = self
            .art
            .graphics
            .as_ref()
            .and_then(|graphics| graphics.backgrounds.find(layer, scroll.screen_id))
            .map_or(&[][..], |definition| &definition.swaps);
        Some(scroll.with_swaps(swaps))
    }

    /// The single selected rectangle the knobs belong to.
    pub fn selected_resizable_rect(&self) -> Option<Rect> {
        match self.selection.as_slice() {
            [item] if item.kind.resizable() => self.bounds(*item),
            _ => None,
        }
    }

    pub fn to_screen(&self, point: Pos2) -> Pos2 {
        self.canvas.min + (point.to_vec2() - self.pan) * self.zoom
    }

    pub fn to_world(&self, point: Pos2) -> Pos2 {
        (self.pan + (point - self.canvas.min) / self.zoom).to_pos2()
    }

    pub fn screen_rect(&self, rect: Rect) -> Rect {
        Rect::from_min_max(self.to_screen(rect.min), self.to_screen(rect.max))
    }

    /// The knob under a screen point, if the selection has knobs.
    pub fn handle_at(&self, screen: Pos2) -> Option<Handle> {
        let rect = self.screen_rect(self.selected_resizable_rect()?);
        Handle::ALL
            .into_iter()
            .zip(handle_centres(rect))
            .find(|(_, centre)| Rect::from_center_size(*centre, Vec2::splat(KNOB_REACH * 2.0)).contains(screen))
            .map(|(handle, _)| handle)
    }

    /// The level fitted to a canvas of `size`.
    pub fn fit(&mut self, size: Vec2) {
        let bounds = self.scene_bounds();
        self.zoom = fit_zoom(size, bounds).clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = bounds.center().to_vec2() - size / self.zoom / 2.0;
        self.needs_fit = false;
    }

    /// Pans just far enough to bring `rect` (level pixels) onto the canvas with some room around it, centring what
    /// is too big to fit. Nothing moves when it is already in view.
    pub fn reveal(&mut self, rect: Rect) {
        let size = self.canvas.size() / self.zoom;
        if !(size.x > 0.0 && size.y > 0.0) {
            return;
        }
        let rect = rect.expand(REVEAL_MARGIN);
        let along = |pan: f32, size: f32, min: f32, max: f32| if max - min > size { (min + max - size) / 2.0 } else { pan.clamp(max - size, min) };
        self.pan = Vec2::new(along(self.pan.x, size.x, rect.min.x, rect.max.x), along(self.pan.y, size.y, rect.min.y, rect.max.y));
    }

    /// Zooms to an absolute scale about a screen point, pinning the zoom.
    pub fn zoom_about(&mut self, zoom: f32, about: Pos2) {
        let before = self.to_world(about);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.pan = before.to_vec2() - (about - self.canvas.min) / self.zoom;
        self.zoom_pinned = true;
    }

    // --- gestures --------------------------------------------------------------------------------------------------

    /// The drag in progress, if it moves things.
    pub fn move_delta(&self) -> Vec2 {
        match self.gesture {
            Some(Gesture::Move { delta, .. }) => delta,
            _ => Vec2::ZERO,
        }
    }

    pub fn resize_edges(&self) -> Option<ResizeEdges> {
        match self.gesture {
            Some(Gesture::Resize { handle, delta, .. }) => Some(handle.edges(delta)),
            _ => None,
        }
    }

    /// An entity's rectangle as the gesture in progress shows it: a resize adjusts the edges the knob owns, a move
    /// translates.
    pub fn preview(&self, kind: Kind, index: usize, rect: Rect) -> Rect {
        if !self.is_selected(kind, index, None) {
            return rect;
        }
        match self.gesture {
            Some(Gesture::Resize { handle, delta, .. }) if kind.resizable() => handle.edges(delta).apply(rect),
            Some(Gesture::Move { delta, .. }) => rect.translate(delta),
            _ => rect,
        }
    }

    /// How far a path node is drawn from where it is.
    pub fn node_offset(&self, kind: Kind, index: usize, node: usize) -> Vec2 {
        if self.is_selected(kind, index, Some(node)) { self.move_delta() } else { Vec2::ZERO }
    }

    /// A raw drag snapped the way the selection snaps: tiles when it is all objects and sprites, otherwise the snap
    /// size. Alt, or Snap off, drags freely.
    pub fn snap(&self, raw: Vec2, free: bool) -> Vec2 {
        if !self.options.snap || free {
            return raw;
        }
        let grid = if self.selection.iter().all(|s| s.kind.tiled()) { TILE } else { self.options.snap_size };
        Vec2::new((raw.x / grid).round() * grid, (raw.y / grid).round() * grid)
    }

    /// The step an arrow key moves the selection.
    pub fn nudge_step(&self) -> f32 {
        if self.selection.iter().all(|s| s.kind.tiled()) { TILE } else { self.options.snap_size }
    }

    pub fn awaiting_clone(&self) -> bool {
        self.awaiting_clone
    }

    /// The Ctrl-drag ended before its copies arrived.
    pub fn release_clone_drag(&mut self) {
        self.release_after_clone = true;
    }

    // --- edits -----------------------------------------------------------------------------------------------------

    /// Whether the move would push a selected unsigned entity past the origin. Actors are placed in tiles, the rest in
    /// pixels.
    fn moves_out_of_bounds(&self, delta: Vec2) -> bool {
        let course = &self.snapshot.course;
        let beyond = |origin: u16, moved: f32, scale: f32| i32::from(origin) + round(moved / scale) < 0;
        self.selection.iter().any(|s| {
            let position = match s.kind {
                Kind::Sprite => course.sprites.get(s.index).map(|v| (v.x, v.y, TILE)),
                Kind::Entrance => course.entrances.get(s.index).map(|v| (v.x, v.y, 1.0)),
                Kind::Zone => course.zones.get(s.index).map(|v| (v.x, v.y, 1.0)),
                Kind::View => course.views.get(s.index).map(|v| (v.x, v.y, 1.0)),
                _ => None,
            };
            position.is_some_and(|(x, y, scale)| beyond(x, delta.x, scale) || beyond(y, delta.y, scale))
        })
    }

    /// The selection moved by `delta` level pixels, as one undo step. `None` when nothing moves, and when an unsigned
    /// entity would land past the origin: the drag is one gesture, so the whole of it is dropped and everything snaps
    /// back rather than piling up in the corner.
    pub fn move_edit(&self, delta: Vec2) -> Option<LevelEdit> {
        if (delta.x.abs() < f32::EPSILON && delta.y.abs() < f32::EPSILON) || self.selection.is_empty() || self.moves_out_of_bounds(delta) {
            return None;
        }
        let course = &self.snapshot.course;
        let (tiles_x, tiles_y) = (round(delta.x / TILE), round(delta.y / TILE));
        let (pixels_x, pixels_y) = (round(delta.x), round(delta.y));
        let mut updates = LevelEntityUpdates::default();
        // Nodes are grouped by their path, so moving several nodes of one path is one update of it.
        let mut nodes: BTreeMap<(Kind, usize), Option<BTreeSet<usize>>> = BTreeMap::new();
        let mut seen = BTreeSet::new();
        for item in &self.selection {
            if item.kind.path() {
                let entry = nodes.entry((item.kind, item.index)).or_insert_with(|| Some(BTreeSet::new()));
                match (item.node, entry.as_mut()) {
                    (Some(node), Some(set)) => {
                        set.insert(node);
                    }
                    _ => *entry = None,
                }
                continue;
            }
            if !seen.insert((item.kind, item.index)) {
                continue;
            }
            let index = item.index;
            match item.kind {
                Kind::Object => {
                    if let Some(o) = self.snapshot.objects.get(index) {
                        let mut value = o.clone();
                        value.x = clamp_i16(i32::from(o.x) + tiles_x);
                        value.y = clamp_i16(i32::from(o.y) + tiles_y);
                        updates.objects.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Sprite => {
                    if let Some(s) = course.sprites.get(index) {
                        let mut value = s.clone();
                        value.x = clamp_u16(i32::from(s.x) + tiles_x);
                        value.y = clamp_u16(i32::from(s.y) + tiles_y);
                        updates.sprites.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Entrance => {
                    if let Some(e) = course.entrances.get(index) {
                        let mut value = e.clone();
                        value.x = clamp_u16(i32::from(e.x) + pixels_x);
                        value.y = clamp_u16(i32::from(e.y) + pixels_y);
                        updates.entrances.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Zone => {
                    if let Some(z) = course.zones.get(index) {
                        let mut value = z.clone();
                        value.x = clamp_u16(i32::from(z.x) + pixels_x);
                        value.y = clamp_u16(i32::from(z.y) + pixels_y);
                        updates.zones.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::View => {
                    if let Some(v) = course.views.get(index) {
                        let mut value = v.clone();
                        value.x = clamp_u16(i32::from(v.x) + pixels_x);
                        value.y = clamp_u16(i32::from(v.y) + pixels_y);
                        updates.views.push(IndexedLevelValue { index, value });
                    }
                }
                Kind::Path | Kind::ProgressPath => {}
            }
        }
        for ((kind, index), selected) in nodes {
            let list = if kind == Kind::Path { &course.paths } else { &course.progress_paths };
            let Some(path) = list.get(index) else { continue };
            let mut value = path.clone();
            for (number, node) in value.nodes.iter_mut().enumerate() {
                if selected.as_ref().is_none_or(|set| set.contains(&number)) {
                    node.x = clamp_i16(i32::from(node.x) + pixels_x);
                    node.y = clamp_i16(i32::from(node.y) + pixels_y);
                }
            }
            let target = if kind == Kind::Path { &mut updates.paths } else { &mut updates.progress_paths };
            target.push(IndexedLevelValue { index, value });
        }
        Some(LevelEdit::Update { updates, name: "Move selection".to_owned() })
    }

    /// Whether the resize would drag the origin of an unsigned entity past the origin of the map. Left unchecked the
    /// clamp would pin the near edge at zero while the far edge kept travelling, and the rectangle would grow out of
    /// the drag.
    fn resize_leaves_bounds(&self, item: Item, edges: ResizeEdges) -> bool {
        let course = &self.snapshot.course;
        let origin = match item.kind {
            Kind::Zone => course.zones.get(item.index).map(|z| (z.x, z.y)),
            Kind::View => course.views.get(item.index).map(|v| (v.x, v.y)),
            _ => None,
        };
        origin.is_some_and(|(x, y)| i32::from(x) + round(edges.left) < 0 || i32::from(y) + round(edges.top) < 0)
    }

    /// The single selected rectangle resized by `edges`, as one undo step.
    pub fn resize_edit(&self, edges: ResizeEdges) -> Option<LevelEdit> {
        let [item] = self.selection.as_slice() else { return None };
        if edges.is_zero() || self.resize_leaves_bounds(*item, edges) {
            return None;
        }
        // Grabbing a top or left knob moves the origin and shrinks the extent by the same amount, so the opposite edge
        // holds still. `scale` converts level pixels into the entity's own units.
        let resized = |origin: i32, extent: i32, near: f32, far: f32, scale: f32| {
            let (near, far) = (round(near / scale), round(far / scale));
            let (mut origin, mut extent) = (origin + near, extent - near + far);
            if extent < 1 {
                // Dragged past the far edge: pin the size and stop the origin there.
                origin -= 1 - extent;
                extent = 1;
            }
            (origin, extent)
        };
        let size = |value: i32| value.clamp(1, i32::from(u16::MAX)) as u16;
        let index = item.index;
        let mut updates = LevelEntityUpdates::default();
        match item.kind {
            Kind::Object => {
                let mut object = self.snapshot.objects.get(index)?.clone();
                let (x, width) = resized(i32::from(object.x), i32::from(object.width), edges.left, edges.right, TILE);
                let (y, height) = resized(i32::from(object.y), i32::from(object.height), edges.top, edges.bottom, TILE);
                (object.x, object.y, object.width, object.height) = (clamp_i16(x), clamp_i16(y), size(width), size(height));
                updates.objects.push(IndexedLevelValue { index, value: object });
            }
            Kind::Zone => {
                let mut zone = self.snapshot.course.zones.get(index)?.clone();
                let (x, width) = resized(i32::from(zone.x), i32::from(zone.width), edges.left, edges.right, 1.0);
                let (y, height) = resized(i32::from(zone.y), i32::from(zone.height), edges.top, edges.bottom, 1.0);
                (zone.x, zone.y, zone.width, zone.height) = (clamp_u16(x), clamp_u16(y), size(width), size(height));
                updates.zones.push(IndexedLevelValue { index, value: zone });
            }
            Kind::View => {
                let mut view = self.snapshot.course.views.get(index)?.clone();
                let (x, width) = resized(i32::from(view.x), i32::from(view.width), edges.left, edges.right, 1.0);
                let (y, height) = resized(i32::from(view.y), i32::from(view.height), edges.top, edges.bottom, 1.0);
                (view.x, view.y, view.width, view.height) = (clamp_u16(x), clamp_u16(y), size(width), size(height));
                updates.views.push(IndexedLevelValue { index, value: view });
            }
            _ => return None,
        }
        Some(LevelEdit::Update { updates, name: "Resize selection".to_owned() })
    }

    /// The cell a right-click at level point `point` puts an object in.
    fn place_cell(&self, point: Pos2, free: bool) -> (i16, i16) {
        let snap = |v: f32| if self.options.snap && !free { (v / TILE).round() * TILE } else { v };
        let cell = |v: f32| (round(snap(v)).clamp(0, i32::from(u16::MAX)) / 16) as i16;
        (cell(point.x), cell(point.y))
    }

    /// The chosen object, covering the cells from `from` to `to` whichever way round they are.
    fn object_between(&self, from: (i16, i16), to: (i16, i16)) -> NsmbLevelObject {
        NsmbLevelObject {
            object_id: self.placement.object_id,
            tileset_slot: self.placement.object_slot,
            x: from.0.min(to.0),
            y: from.1.min(to.1),
            width: from.0.abs_diff(to.0) + 1,
            height: from.1.abs_diff(to.1) + 1,
        }
    }

    /// A right-press at `point`. An object is not created yet: it is dragged out to its size first, as in NSMB
    /// Editor, and `false` says the chosen kind is one a press creates outright.
    pub fn begin_place(&mut self, point: Pos2, free: bool) -> bool {
        if self.placement.kind != Kind::Object {
            return false;
        }
        let cell = self.place_cell(point, free);
        self.gesture = Some(Gesture::Place { from: cell, to: cell, submitted: false });
        true
    }

    /// The pointer moved to `point` with an object being dragged out.
    pub fn drag_place(&mut self, point: Pos2, free: bool) {
        let cell = self.place_cell(point, free);
        if let Some(Gesture::Place { to, submitted: false, .. }) = &mut self.gesture {
            *to = cell;
        }
    }

    /// The button came up: the object dragged out is created, as one step.
    pub fn finish_place(&mut self) -> Option<LevelEdit> {
        let Some(Gesture::Place { from, to, submitted: false }) = self.gesture else { return None };
        let mut items = LevelEntityInsertions::default();
        items.objects.push(self.object_between(from, to));
        self.gesture = Some(Gesture::Place { from, to, submitted: true });
        self.placing = true;
        Some(LevelEdit::Add { items, name: "Add object".to_owned() })
    }

    /// Where the object being dragged out lies, in level pixels.
    pub fn placing_rect(&self) -> Option<Rect> {
        let Some(Gesture::Place { from, to, .. }) = self.gesture else { return None };
        Some(object_rect(&self.object_between(from, to)))
    }

    /// What a right-click at level point `point` creates. `free` (Alt) places without snapping. `None` for an unsigned
    /// kind out of bounds: dropping it at the origin instead would put it somewhere nobody clicked.
    pub fn placement_edit(&mut self, point: Pos2, free: bool) -> Option<LevelEdit> {
        let kind = self.placement.kind;
        let mut point = point;
        if self.options.snap && !free {
            let grid = if kind.tiled() { TILE } else { self.options.snap_size };
            point = Pos2::new((point.x / grid).round() * grid, (point.y / grid).round() * grid);
        }
        if kind.unsigned() && (point.x < 0.0 || point.y < 0.0) {
            return None;
        }
        let pixel_x = round(point.x).clamp(0, i32::from(u16::MAX));
        let pixel_y = round(point.y).clamp(0, i32::from(u16::MAX));
        let (tile_x, tile_y) = (pixel_x / 16, pixel_y / 16);
        let mut items = LevelEntityInsertions::default();
        let placement = &self.placement;
        let name = match kind {
            Kind::Object => {
                let cell = (tile_x as i16, tile_y as i16);
                items.objects.push(self.object_between(cell, cell));
                "Add object"
            }
            Kind::Sprite => {
                let value = NsmbLevelSprite { id: placement.sprite_type, x: tile_x as u16, y: tile_y as u16, ..Default::default() };
                items.sprites.push(SpriteInsertion { value, hash: placement.sprite_hash });
                if placement.sprite_hash.is_some() { "Add custom sprite" } else { "Add sprite" }
            }
            Kind::Entrance => {
                items.entrances.push(NsmbLevelEntrance { x: pixel_x as u16, y: pixel_y as u16, ..Default::default() });
                "Add entrance"
            }
            Kind::Zone => {
                items.zones.push(NsmbLevelZone { x: pixel_x as u16, y: pixel_y as u16, width: 256, height: 192, ..Default::default() });
                "Add zone"
            }
            Kind::Path | Kind::ProgressPath => {
                let node = NsmbLevelPathNode { x: pixel_x as i16, y: pixel_y as i16, ..Default::default() };
                let path = NsmbLevelPath { nodes: vec![node], ..Default::default() };
                if kind == Kind::Path {
                    items.paths.push(path);
                    "Add path"
                } else {
                    items.progress_paths.push(path);
                    "Add progress path"
                }
            }
            Kind::View => {
                items.views.push(NsmbLevelView { x: pixel_x as u16, y: pixel_y as u16, width: 256, height: 192, ..Default::default() });
                "Add view"
            }
        };
        self.placing = true;
        Some(LevelEdit::Add { items, name: name.to_owned() })
    }

    /// A copy of the selection one tile down and to the right, which is what pasting it would make.
    pub fn clone_edit(&self) -> Option<LevelEdit> {
        let course = &self.snapshot.course;
        let mut items = LevelEntityInsertions::default();
        let mut seen = BTreeSet::new();
        let shift_u16 = |value: u16| value.saturating_add(16);
        let shift_i16 = |value: i16| value.saturating_add(16);
        for item in &self.selection {
            if !seen.insert((item.kind, item.index)) {
                continue;
            }
            let index = item.index;
            match item.kind {
                Kind::Object => {
                    if let Some(o) = self.snapshot.objects.get(index) {
                        items.objects.push(NsmbLevelObject { x: o.x.saturating_add(1), y: o.y.saturating_add(1), ..o.clone() });
                    }
                }
                Kind::Sprite => {
                    if let Some(s) = course.sprites.get(index) {
                        let value = NsmbLevelSprite { x: s.x.saturating_add(1), y: s.y.saturating_add(1), ..s.clone() };
                        items.sprites.push(SpriteInsertion { value, hash: self.sprite_hash(index) });
                    }
                }
                Kind::Entrance => {
                    if let Some(e) = course.entrances.get(index) {
                        items.entrances.push(NsmbLevelEntrance { x: shift_u16(e.x), y: shift_u16(e.y), ..e.clone() });
                    }
                }
                Kind::Zone => {
                    if let Some(z) = course.zones.get(index) {
                        items.zones.push(NsmbLevelZone { x: shift_u16(z.x), y: shift_u16(z.y), ..z.clone() });
                    }
                }
                Kind::View => {
                    if let Some(v) = course.views.get(index) {
                        items.views.push(NsmbLevelView { x: shift_u16(v.x), y: shift_u16(v.y), ..v.clone() });
                    }
                }
                Kind::Path | Kind::ProgressPath => {
                    let list = if item.kind == Kind::Path { &course.paths } else { &course.progress_paths };
                    if let Some(p) = list.get(index) {
                        let mut path = p.clone();
                        for node in &mut path.nodes {
                            (node.x, node.y) = (shift_i16(node.x), shift_i16(node.y));
                        }
                        if item.kind == Kind::Path { items.paths.push(path) } else { items.progress_paths.push(path) }
                    }
                }
            }
        }
        (!seen.is_empty()).then(|| LevelEdit::Add { items, name: "Paste selection".to_owned() })
    }

    /// Starts a Ctrl-drag: the copies are made first and the drag carries them.
    pub fn begin_clone_drag(&mut self, item: Item) -> Option<LevelEdit> {
        self.selection = vec![item];
        let edit = self.clone_edit()?;
        self.awaiting_clone = true;
        self.release_after_clone = false;
        self.gesture = Some(Gesture::Move { delta: Vec2::ZERO, submitted: false });
        Some(edit)
    }

    /// One step forward or back in draw order, for a single selected entity.
    pub fn reorder_edit(&self, forward: bool) -> Option<LevelEdit> {
        let [item] = self.selection.as_slice() else { return None };
        let kind = match item.kind {
            Kind::Object => ReorderKind::Object,
            Kind::Sprite => ReorderKind::Sprite,
            Kind::Entrance => ReorderKind::Entrance,
            Kind::Zone => ReorderKind::Zone,
            Kind::Path => ReorderKind::Path,
            Kind::ProgressPath => ReorderKind::ProgressPath,
            Kind::View => return None,
        };
        Some(LevelEdit::Reorder { kind, index: item.index, forward })
    }

    pub fn remove_edit(&self) -> Option<LevelEdit> {
        if self.selection.is_empty() {
            return None;
        }
        let mut selection = LevelEntitySelection::default();
        let mut seen = BTreeSet::new();
        for item in &self.selection {
            if !seen.insert((item.kind, item.index)) {
                continue;
            }
            let list = match item.kind {
                Kind::Object => &mut selection.objects,
                Kind::Sprite => &mut selection.sprites,
                Kind::Entrance => &mut selection.entrances,
                Kind::View => &mut selection.views,
                Kind::Zone => &mut selection.zones,
                Kind::Path => &mut selection.paths,
                Kind::ProgressPath => &mut selection.progress_paths,
            };
            list.push(item.index);
        }
        Some(LevelEdit::Remove { selection, name: "Delete selection".to_owned() })
    }

    /// Everything a marquee from `a` to `b` (level pixels) touches, added to the selection.
    pub fn select_within(&mut self, a: Pos2, b: Pos2) {
        let area = Rect::from_two_pos(a, b);
        for (item, rect) in self.entity_rects() {
            if area.intersects(rect) && !self.selection.contains(&item) {
                self.selection.push(item);
            }
        }
    }

    // --- the properties panel ---------------------------------------------------------------------------------------

    /// Brings the panel in line with a selection or create kind that changed elsewhere: the canvas, its toolbar or a
    /// shortcut. A selection of one kind shows that kind; nothing selected shows the course; a mixed selection leaves
    /// the panel where it was.
    pub fn sync_panel(&mut self) {
        if self.selection != self.panel_seen {
            self.panel_seen = self.selection.clone();
            self.panel.reveal = true;
            let kinds: BTreeSet<Kind> = self.selection.iter().map(|s| s.kind).collect();
            match (kinds.len(), self.selection.first().copied()) {
                (0, _) => {
                    self.panel.tab = Tab::Course;
                    self.panel.multi.clear();
                }
                (1, Some(first)) => {
                    let indices: BTreeSet<usize> = self.selection.iter().map(|s| s.index).collect();
                    self.panel.tab = Tab::Entity(first.kind);
                    self.panel.item = first.index;
                    self.panel.node = first.node.unwrap_or(0);
                    self.panel.multi = if indices.len() > 1 { indices.into_iter().collect() } else { Vec::new() };
                    // Whatever the panel shows is also what a right-click creates.
                    self.placement.kind = first.kind;
                }
                _ => {}
            }
            self.place_like_selection();
            self.panel.dragging = None;
            self.panel.settings_text = None;
        }
        if self.placement.kind != self.panel_kind {
            self.panel_kind = self.placement.kind;
            if self.panel.tab != Tab::Entity(self.placement.kind) {
                self.panel.tab = Tab::Entity(self.placement.kind);
                self.panel.multi.clear();
            }
        }
    }

    /// Points the picker at the selection, as NSMB Editor's does: the first object selected is the object a
    /// right-click places, and the first sprite the sprite. Which of the two kinds it places is left alone.
    fn place_like_selection(&mut self) {
        let kind = self.placement.kind;
        let first = |kind: Kind| self.selection.iter().find(|s| s.kind == kind).map(|s| s.index);
        if let Some(object) = first(Kind::Object).and_then(|index| self.snapshot.objects.get(index)) {
            self.placement.set_object(object.tileset_slot, object.object_id);
        }
        if let Some(index) = first(Kind::Sprite) {
            match (self.sprite_hash(index), self.snapshot.course.sprites.get(index)) {
                (Some(hash), _) => self.placement.set_custom_sprite(hash),
                (None, Some(sprite)) => self.placement.set_sprite(sprite.id),
                (None, None) => {}
            }
        }
        self.placement.kind = kind;
    }

    /// Selects on the canvas what the panel now shows, and remembers it as seen.
    fn select_from_panel(&mut self) {
        self.panel.multi.clear();
        self.panel.dragging = None;
        self.panel.settings_text = None;
        let count = item_count(&self.snapshot, self.panel.tab);
        self.selection = match self.panel.tab {
            Tab::Entity(kind) if count > 0 => {
                self.panel.item = self.panel.item.min(count - 1);
                let node = kind.path().then_some(self.panel.node);
                vec![Item { kind, index: self.panel.item, node }]
            }
            _ => Vec::new(),
        };
        if let Tab::Entity(kind) = self.panel.tab {
            self.placement.kind = kind;
        }
        self.place_like_selection();
        self.panel_seen = self.selection.clone();
        self.panel_kind = self.placement.kind;
    }

    /// The person picked a line of the panel's list: an item, a whole path or one node of it. It becomes the
    /// selection, or with `add` joins or leaves it, and the canvas brings it into view.
    pub fn pick(&mut self, item: Item, add: bool) {
        if add {
            let before = self.selection.len();
            self.selection.retain(|s| !(s.kind == item.kind && s.index == item.index && (item.node.is_none() || s.node == item.node)));
            if self.selection.len() == before {
                self.selection.push(item);
            }
        } else {
            self.selection = vec![item];
        }
        self.sync_panel();
        // The list is where the pick was made, so it has nothing to scroll to.
        self.panel.reveal = false;
        if let Some(rect) = self.bounds(item).filter(|_| self.selection.contains(&item)) {
            self.reveal(rect);
        }
    }

    /// The person picked a tab.
    pub fn panel_tab(&mut self, tab: Tab) {
        self.panel.tab = tab;
        self.select_from_panel();
    }

    pub fn panel_item(&mut self, item: usize) {
        self.panel.item = item;
        self.select_from_panel();
    }

    pub fn panel_node(&mut self, node: usize) {
        self.panel.node = node;
        self.select_from_panel();
    }

    /// A kind shortcut: the panel jumps to the kind's first item, as the Qt editor's does.
    pub fn choose_kind(&mut self, tab: Tab) {
        self.panel.tab = tab;
        self.panel.item = 0;
        self.panel.node = 0;
        self.select_from_panel();
    }

    /// The next `Added` is something the panel or a placement made, and selects a path by its first node.
    pub fn expect_placement(&mut self) {
        self.placing = true;
    }

    // --- tiles -----------------------------------------------------------------------------------------------------

    /// The objects as the gesture in progress shows them, rounded back to whole cells, which is all the tile array
    /// can hold.
    fn preview_objects(&self) -> Vec<NsmbLevelObject> {
        let mut objects = self.snapshot.objects.clone();
        match self.gesture {
            None => return objects,
            // The object being dragged out is laid over the rest, where it will be once it exists.
            Some(Gesture::Place { from, to, .. }) => {
                objects.push(self.object_between(from, to));
                return objects;
            }
            Some(_) => {}
        }
        for (index, object) in objects.iter_mut().enumerate() {
            if !self.is_selected(Kind::Object, index, None) {
                continue;
            }
            let rect = self.preview(Kind::Object, index, object_rect(object));
            object.x = clamp_i16(round(rect.left() / TILE));
            object.y = clamp_i16(round(rect.top() / TILE));
            object.width = round(rect.width() / TILE).clamp(1, i32::from(u16::MAX)) as u16;
            object.height = round(rect.height() / TILE).clamp(1, i32::from(u16::MAX)) as u16;
        }
        objects
    }

    /// The tile array the game would build from the objects, rebuilt only when the objects, the gesture over them or
    /// the tilesets change.
    pub fn tilemap(&mut self) -> Arc<LevelTilemap> {
        let objects = self.preview_objects();
        let mut hasher = DefaultHasher::new();
        self.art.serial.hash(&mut hasher);
        for o in &objects {
            (o.object_id, o.tileset_slot, o.x, o.y, o.width, o.height).hash(&mut hasher);
        }
        let key = hasher.finish();
        if let Some((cached, tilemap)) = &self.tile_cache
            && *cached == key
        {
            return tilemap.clone();
        }
        let [a, b, c] = &self.art.tilesets;
        let tilemap = Arc::new(rasterize_level_objects(&objects, &[a.as_ref(), b.as_ref(), c.as_ref()]));
        self.tile_cache = Some((key, tilemap.clone()));
        tilemap
    }

    /// Whether an object put cells in the array, or shows as a plain rectangle: its tileset never decoded, or has no
    /// such object, or cannot lay it out.
    pub fn object_layable(&self, object: &NsmbLevelObject) -> bool {
        self.art
            .tilesets
            .get(usize::from(object.tileset_slot))
            .and_then(Option::as_ref)
            .and_then(|tileset| tileset.objects.get(usize::from(object.object_id)))
            .is_some_and(|layout| layout.rows.iter().any(|row| !row.is_empty()))
    }
}

fn exists(snapshot: &LevelSnapshot, item: Item) -> bool {
    let course = &snapshot.course;
    match item.kind {
        Kind::Object => item.index < snapshot.objects.len(),
        Kind::Sprite => item.index < course.sprites.len(),
        Kind::Entrance => item.index < course.entrances.len(),
        Kind::View => item.index < course.views.len(),
        Kind::Zone => item.index < course.zones.len(),
        Kind::Path | Kind::ProgressPath => {
            let paths = if item.kind == Kind::Path { &course.paths } else { &course.progress_paths };
            paths.get(item.index).is_some_and(|p| item.node.is_none_or(|node| node < p.nodes.len()))
        }
    }
}

/// The items an `Added` outcome names. A placed path is selected by its first node, a pasted one as a whole.
fn items_of(selection: &LevelEntitySelection, first_node: bool) -> Vec<Item> {
    let lists = [
        (Kind::Object, &selection.objects),
        (Kind::Sprite, &selection.sprites),
        (Kind::Entrance, &selection.entrances),
        (Kind::Zone, &selection.zones),
        (Kind::Path, &selection.paths),
        (Kind::ProgressPath, &selection.progress_paths),
        (Kind::View, &selection.views),
    ];
    let mut items = Vec::new();
    for (kind, indices) in lists {
        let node = (first_node && kind.path()).then_some(0);
        items.extend(indices.iter().map(|&index| Item { kind, index, node }));
    }
    items
}

#[cfg(test)]
pub(crate) mod tests_support {
    use bigstar_formats::{NsmbLevelCourse, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelStageObject, NsmbLevelZone};
    use bigstar_protocol::LevelSnapshot;

    /// A level with one of most things in it.
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
            glue: Default::default(),
            stage_object_id_base: 326,
            registration_hashes: Vec::new(),
            sprite_hashes: Vec::new(),
            dirty: false,
            can_undo: false,
            can_redo: false,
            undo_names: Vec::new(),
            redo_names: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use bigstar_formats::NsmbLevelStageObject;
    use bigstar_level::SpriteDefinition;

    use super::tests_support::snapshot;
    use super::*;

    fn moved(view: &LevelView, delta: Vec2) -> LevelEntityUpdates {
        match view.move_edit(delta) {
            Some(LevelEdit::Update { updates, name }) => {
                assert_eq!(name, "Move selection");
                updates
            }
            other => panic!("no move: {other:?}"),
        }
    }

    #[test]
    fn the_ds_screen_follows_the_pointer_and_marks_where_an_entrance_opens() {
        use bigstar_formats::{NsmbLevelCamera, NsmbLevelScreen};
        let mut level = snapshot();
        level.course.views = vec![
            NsmbLevelView { x: 0, y: 0, width: 2000, height: 400, id: 0, camera_settings_id: 2, ..Default::default() },
            NsmbLevelView { x: 0, y: 1000, width: 2000, height: 400, id: 7, background_id: 1, ..Default::default() },
        ];
        level.course.cameras = vec![NsmbLevelCamera { id: 2, bottom_offset: -32, ..Default::default() }];
        level.course.backgrounds = vec![
            NsmbLevelScreen { id: 0, screen_id: 5, scroll_x: 1, scroll_y: 1, ..Default::default() },
            NsmbLevelScreen { id: 1, screen_id: 9, ..Default::default() },
        ];
        level.course.entrances =
            vec![NsmbLevelEntrance { x: 320, y: 304, view_id: 0, ..Default::default() }, NsmbLevelEntrance { view_id: 99, ..Default::default() }];
        let mut view = LevelView::new(level);
        view.hover = Some(Pos2::new(500.0, 1200.0));
        assert!(view.ds_screens().is_empty());

        // Under the pointer, centred, scrolled by the view the pointer is in.
        view.options.show_ds_screen = true;
        let screens = view.ds_screens();
        assert_eq!(screens, [DsScreen { rect: rect(372.0, 1104.0, 256.0, 192.0), view: Some(1), spawn: false }]);
        assert_eq!(view.screen_layer(&screens[0], BackgroundLayer::Background).map(|scroll| (scroll.screen_id, scroll.x >> 12)), Some((9, 372)));
        view.hover = Some(Pos2::new(500.0, 700.0));
        assert_eq!(view.ds_screens()[0].view, None);

        // A layer whose script swaps a quarter does once it has scrolled that far.
        let mut definition = bigstar_level::BackgroundDefinition { id: 9, ..Default::default() };
        definition.swaps = vec![bigstar_level::BackgroundSwap { after: 256 * 4096, quarter: (0, 1), shows: (1, 1) }];
        view.art.graphics =
            Some(LevelGraphics { backgrounds: bigstar_level::BackgroundCatalog { definitions: vec![definition] }, ..Default::default() });
        assert_eq!(view.screen_layer(&screens[0], BackgroundLayer::Background).unwrap().quarter(0, 1), (1, 1));
        let near = DsScreen { rect: rect(100.0, 1104.0, 256.0, 192.0), ..screens[0] };
        assert_eq!(view.screen_layer(&near, BackgroundLayer::Background).unwrap().quarter(0, 1), (0, 1));

        // An entrance adds the screen the game opens on, by its view and that view's camera settings, and lends the
        // pointer's screen its view.
        view.selection = vec![Item::new(Kind::Entrance, 0)];
        let screens = view.ds_screens();
        assert_eq!(screens[0], DsScreen { rect: rect(200.0, 208.0, 256.0, 192.0), view: Some(0), spawn: true });
        assert_eq!((screens[1].view, screens[1].spawn), (Some(0), false));
        let scroll = view.screen_layer(&screens[0], BackgroundLayer::Background).unwrap();
        assert_eq!((scroll.screen_id, scroll.x >> 12, scroll.y), (5, 100, 288));
        // Dragged, it is predicted where the drag has it.
        view.gesture = Some(Gesture::Move { delta: Vec2::new(64.0, 0.0), submitted: false });
        assert_eq!(view.ds_screens()[0].rect.left(), 264.0);
        view.gesture = None;

        // An entrance whose view does not exist still shows where the player would be framed, with nothing scrolled.
        view.selection = vec![Item::new(Kind::Entrance, 1)];
        view.hover = None;
        let screens = view.ds_screens();
        assert_eq!(screens, [DsScreen { rect: rect(8.0 - 128.0, 16.0 - 128.0, 256.0, 192.0), view: None, spawn: true }]);
        assert!(view.screen_layer(&screens[0], BackgroundLayer::Background).is_none());
    }

    #[test]
    fn the_nearest_thing_wins_a_click_and_containers_yield() {
        let view = LevelView::new(snapshot());
        assert_eq!(view.hit_test(Pos2::new(104.0, 72.0)), Some(Item::new(Kind::Sprite, 0)));
        assert_eq!(view.hit_test(Pos2::new(40.0, 56.0)), Some(Item::new(Kind::Object, 0)));
        assert_eq!(view.hit_test(Pos2::new(308.0, 108.0)), Some(Item::node(Kind::Path, 0, 0)));
        // Where nothing else is, the zone that encloses it takes the click; outside it, nothing does.
        assert_eq!(view.hit_test(Pos2::new(200.0, 150.0)), Some(Item::new(Kind::Zone, 0)));
        assert_eq!(view.hit_test(Pos2::new(500.0, 150.0)), None);
    }

    #[test]
    fn a_move_converts_to_each_kinds_units_and_is_one_edit() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::new(Kind::Object, 0), Item::new(Kind::Sprite, 0)];
        // A drag snaps once, for the whole selection: tiles, since it is all objects and sprites.
        assert_eq!(view.snap(Vec2::new(35.0, -13.0), false), Vec2::new(32.0, -16.0));
        assert_eq!(view.snap(Vec2::new(35.0, -13.0), true), Vec2::new(35.0, -13.0));
        let updates = moved(&view, Vec2::new(32.0, -16.0));
        assert_eq!((updates.objects[0].value.x, updates.objects[0].value.y), (4, 2));
        assert_eq!((updates.sprites[0].value.x, updates.sprites[0].value.y), (8, 3));
        // With anything else in the selection it snaps to the snap size.
        view.selection.push(Item::new(Kind::Zone, 0));
        view.options.snap_size = 8.0;
        assert_eq!(view.snap(Vec2::new(35.0, -13.0), false), Vec2::new(32.0, -16.0));
        assert_eq!(view.snap(Vec2::new(29.0, -13.0), false), Vec2::new(32.0, -16.0));
        assert_eq!(view.snap(Vec2::new(27.0, -11.0), false), Vec2::new(24.0, -8.0));
        assert_eq!(view.nudge_step(), 8.0);
    }

    #[test]
    fn a_drag_that_goes_nowhere_makes_no_edit() {
        let mut view = LevelView::new(snapshot());
        view.selection.push(Item::new(Kind::Sprite, 0));
        assert_eq!(view.move_edit(Vec2::ZERO), None);
    }

    #[test]
    fn unsigned_entities_snap_back_when_dragged_past_the_origin() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::new(Kind::Sprite, 0)];
        let updates = moved(&view, Vec2::new(-16.0, -16.0));
        assert_eq!((updates.sprites[0].value.x, updates.sprites[0].value.y), (5, 3));
        // Past the origin nothing moves at all, not even the object sharing the selection.
        view.selection.push(Item::new(Kind::Object, 0));
        assert_eq!(view.move_edit(Vec2::new(-128.0, 0.0)), None);
        // An object on its own is free to go out there.
        view.selection = vec![Item::new(Kind::Object, 0)];
        let updates = moved(&view, Vec2::new(-64.0, -64.0));
        assert_eq!((updates.objects[0].value.x, updates.objects[0].value.y), (-2, -1));
    }

    #[test]
    fn a_path_node_moves_alone_and_signed() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::node(Kind::Path, 0, 1)];
        let updates = moved(&view, Vec2::new(-400.0, -108.0));
        let nodes = &updates.paths[0].value.nodes;
        assert_eq!((nodes[0].x, nodes[0].y, nodes[1].x, nodes[1].y), (300, 100, -60, -8));
        // Two nodes of one path move together, as one update of it.
        view.selection.push(Item::node(Kind::Path, 0, 0));
        let updates = moved(&view, Vec2::new(8.0, 0.0));
        assert_eq!(updates.paths.len(), 1);
        assert_eq!((updates.paths[0].value.nodes[0].x, updates.paths[0].value.nodes[1].x), (308, 348));
    }

    #[test]
    fn knobs_resize_from_the_edge_they_sit_on() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::new(Kind::Zone, 0)];
        let edit = |view: &LevelView, handle: Handle, delta: Vec2| match view.resize_edit(handle.edges(delta)) {
            Some(LevelEdit::Update { updates, name }) => {
                assert_eq!(name, "Resize selection");
                let z = &updates.zones[0].value;
                (z.x, z.y, z.width, z.height)
            }
            other => panic!("no resize: {other:?}"),
        };
        assert_eq!(edit(&view, Handle::BottomRight, Vec2::new(48.0, 32.0)), (0, 0, 448, 232));
        assert_eq!(edit(&view, Handle::Right, Vec2::new(48.0, 32.0)), (0, 0, 448, 200));
        // A near edge dragged past the origin leaves everything where it was.
        assert_eq!(view.resize_edit(Handle::TopLeft.edges(Vec2::new(-8.0, 0.0))), None);
        view.snapshot.course.zones[0].x = 64;
        assert_eq!(edit(&view, Handle::TopLeft, Vec2::new(-64.0, 8.0)), (0, 8, 464, 192));
        // Dragged past the far edge, the size stops at one.
        assert_eq!(edit(&view, Handle::Left, Vec2::new(1000.0, 0.0)), (463, 0, 1, 200));
        // Objects resize in tiles.
        view.selection = vec![Item::new(Kind::Object, 0)];
        let Some(LevelEdit::Update { updates, .. }) = view.resize_edit(Handle::BottomRight.edges(Vec2::new(32.0, 16.0))) else { panic!() };
        assert_eq!((updates.objects[0].value.width, updates.objects[0].value.height), (6, 2));
        // Sprites have no knobs.
        view.selection = vec![Item::new(Kind::Sprite, 0)];
        assert_eq!(view.selected_resizable_rect(), None);
    }

    #[test]
    fn placing_snaps_and_refuses_unsigned_kinds_out_of_bounds() {
        let mut view = LevelView::new(snapshot());
        let Some(LevelEdit::Add { items, name }) = view.placement_edit(Pos2::new(100.0, 41.0), false) else { panic!() };
        assert_eq!(name, "Add object");
        assert_eq!((items.objects[0].x, items.objects[0].y, items.objects[0].width), (6, 3, 1));
        for kind in [Kind::Sprite, Kind::Entrance, Kind::Zone, Kind::View] {
            view.placement.kind = kind;
            assert_eq!(view.placement_edit(Pos2::new(-80.0, 32.0), false), None, "{kind:?}");
        }
        // An object out there is clamped to the origin.
        view.placement.set_object(1, 12);
        let Some(LevelEdit::Add { items, .. }) = view.placement_edit(Pos2::new(-80.0, 32.0), false) else { panic!() };
        assert_eq!((items.objects[0].x, items.objects[0].y, items.objects[0].tileset_slot, items.objects[0].object_id), (0, 2, 1, 12));
        // A right-press on an object's kind drags it out instead, from either corner, and creates it on release.
        assert!(view.begin_place(Pos2::new(100.0, 41.0), false));
        assert_eq!(view.preview_objects().len(), view.snapshot.objects.len() + 1, "shown before it exists");
        view.drag_place(Pos2::new(36.0, 84.0), false);
        let Some(LevelEdit::Add { items, name }) = view.finish_place() else { panic!() };
        let o = &items.objects[0];
        assert_eq!((name.as_str(), o.x, o.y, o.width, o.height, o.object_id), ("Add object", 2, 3, 5, 3, 12));
        assert_eq!(view.finish_place(), None, "once");
        view.settled();
        assert_eq!(view.gesture, None);
        view.placement.kind = Kind::Entrance;
        assert!(!view.begin_place(Pos2::new(13.0, 27.0), false));
        view.options.snap_size = 8.0;
        let Some(LevelEdit::Add { items, .. }) = view.placement_edit(Pos2::new(13.0, 27.0), false) else { panic!() };
        assert_eq!((items.entrances[0].x, items.entrances[0].y), (16, 24));
        view.placement.set_custom_sprite(0xf46a_44b9);
        let Some(LevelEdit::Add { items, name }) = view.placement_edit(Pos2::new(40.0, 40.0), false) else { panic!() };
        assert_eq!((name.as_str(), items.sprites[0].hash), ("Add custom sprite", Some(0xf46a_44b9)));
    }

    #[test]
    fn a_line_picked_in_the_list_is_selected_and_brought_into_view() {
        let mut view = LevelView::new(snapshot());
        view.canvas = Rect::from_min_size(Pos2::ZERO, Vec2::new(200.0, 100.0));
        // A path is picked whole, and the canvas pans only as far as it must: its nodes span 300..356 by 100..116.
        view.pick(Item::new(Kind::Path, 0), false);
        assert_eq!(view.selection, [Item::new(Kind::Path, 0)]);
        assert_eq!((view.panel.tab, view.panel.reveal), (Tab::Entity(Kind::Path), false));
        assert_eq!(view.pan, Vec2::new(356.0 + 32.0 - 200.0, 116.0 + 32.0 - 100.0));
        // One of its nodes, already in view, moves nothing.
        let pan = view.pan;
        view.pick(Item::node(Kind::Path, 0, 1), false);
        assert_eq!((view.selection.as_slice(), view.panel.node, view.pan), ([Item::node(Kind::Path, 0, 1)].as_slice(), 1, pan));
        // Something too big to fit is centred.
        view.pick(Item::new(Kind::Zone, 0), false);
        assert_eq!(view.pan, Vec2::new(100.0, 50.0));
        // With the modifier a line joins the selection, and leaves it when picked again.
        view.pick(Item::new(Kind::Path, 0), true);
        assert_eq!(view.selection, [Item::new(Kind::Zone, 0), Item::new(Kind::Path, 0)]);
        view.pick(Item::new(Kind::Path, 0), true);
        assert_eq!(view.selection, [Item::new(Kind::Zone, 0)]);
        // A selection made on the canvas is one the list has to scroll to.
        view.selection = vec![Item::new(Kind::Sprite, 0)];
        view.sync_panel();
        assert!(view.panel.reveal);
    }

    #[test]
    fn a_placed_path_is_selected_by_its_first_node() {
        let mut view = LevelView::new(snapshot());
        view.placement.kind = Kind::Path;
        assert!(view.placement_edit(Pos2::new(32.0, 32.0), false).is_some());
        let added = LevelEntitySelection { paths: vec![1], ..Default::default() };
        view.edited(snapshot(), &EditOutcome::Added(added.clone()));
        assert_eq!(view.selection, [Item::node(Kind::Path, 1, 0)]);
        // A pasted one is selected as a whole.
        view.edited(snapshot(), &EditOutcome::Added(added));
        assert_eq!(view.selection, [Item::new(Kind::Path, 1)]);
    }

    #[test]
    fn a_clone_is_a_paste_one_tile_along() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::new(Kind::Object, 0), Item::new(Kind::Zone, 0), Item::node(Kind::Path, 0, 0), Item::node(Kind::Path, 0, 1)];
        let Some(LevelEdit::Add { items, name }) = view.clone_edit() else { panic!() };
        assert_eq!(name, "Paste selection");
        assert_eq!((items.objects[0].x, items.objects[0].y), (3, 4));
        assert_eq!((items.zones[0].x, items.zones[0].y), (16, 16));
        assert_eq!(items.paths.len(), 1, "a path is copied once, whole, however many of its nodes are selected");
        assert_eq!(items.paths[0].nodes[1].x, 356);
    }

    #[test]
    fn a_clone_drag_released_early_moves_the_copies_when_they_arrive() {
        let mut view = LevelView::new(snapshot());
        let edit = view.begin_clone_drag(Item::new(Kind::Sprite, 0));
        assert!(matches!(edit, Some(LevelEdit::Add { .. })));
        view.gesture = Some(Gesture::Move { delta: Vec2::new(32.0, 0.0), submitted: false });
        view.release_clone_drag();
        let mut after = snapshot();
        after.course.sprites.push(NsmbLevelStageObject { id: 1, x: 7, y: 5, ..Default::default() });
        let owed = view.edited(after, &EditOutcome::Added(LevelEntitySelection { sprites: vec![1], ..Default::default() }));
        let Some(LevelEdit::Update { updates, .. }) = owed else { panic!("the move was not sent") };
        assert_eq!((updates.sprites[0].index, updates.sprites[0].value.x), (1, 9));
        assert_eq!(view.selection, [Item::new(Kind::Sprite, 1)]);
    }

    #[test]
    fn a_newer_snapshot_keeps_only_selections_that_still_exist() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::new(Kind::Sprite, 0), Item::new(Kind::Sprite, 5), Item::new(Kind::Object, 0), Item::node(Kind::Path, 0, 9)];
        view.gesture = Some(Gesture::Move { delta: Vec2::ZERO, submitted: true });
        view.replace(snapshot());
        assert_eq!(view.selection, [Item::new(Kind::Sprite, 0), Item::new(Kind::Object, 0)]);
        assert_eq!(view.gesture, None);
        // A gesture still in progress is not the host's to end.
        view.gesture = Some(Gesture::Pan);
        view.replace(snapshot());
        assert_eq!(view.gesture, Some(Gesture::Pan));
    }

    #[test]
    fn deleting_names_every_selected_kind_once() {
        let mut view = LevelView::new(snapshot());
        assert_eq!(view.remove_edit(), None);
        view.selection = vec![Item::new(Kind::Object, 0), Item::new(Kind::Zone, 0), Item::node(Kind::Path, 0, 0), Item::node(Kind::Path, 0, 1)];
        let Some(LevelEdit::Remove { selection, name }) = view.remove_edit() else { panic!() };
        assert_eq!(name, "Delete selection");
        assert_eq!((selection.objects, selection.zones, selection.paths), (vec![0], vec![0], vec![0]));
    }

    #[test]
    fn reordering_names_the_entity_and_follows_it() {
        let mut view = LevelView::new(snapshot());
        view.selection = vec![Item::new(Kind::Object, 0)];
        assert_eq!(view.reorder_edit(true), Some(LevelEdit::Reorder { kind: ReorderKind::Object, index: 0, forward: true }));
        view.edited(snapshot(), &EditOutcome::Moved(0));
        view.selection = vec![Item::new(Kind::Sprite, 0)];
        view.edited(snapshot(), &EditOutcome::Moved(0));
        assert_eq!(view.selection, [Item::new(Kind::Sprite, 0)]);
    }

    #[test]
    fn scene_bounds_cover_everything_with_a_margin() {
        let view = LevelView::new(snapshot());
        assert_eq!(view.scene_bounds(), Rect::from_min_max(Pos2::new(-128.0, -128.0), Pos2::new(528.0, 328.0)));
        let mut empty = snapshot();
        empty.course = Default::default();
        empty.objects.clear();
        assert_eq!(LevelView::new(empty).scene_bounds(), EMPTY_SCENE.expand(128.0));
    }

    #[test]
    fn sprite_definitions_drive_grab_bounds() {
        let mut view = LevelView::new(snapshot());
        view.snapshot.course.sprites[0] = NsmbLevelStageObject { id: 7, x: 10, y: 10, settings: 0xffff_fff0, ..Default::default() };
        let definition = SpriteDefinition {
            object_id: 42,
            name: "Synthetic actor".to_owned(),
            grab_bounds: SpriteBounds { x: -32, y: -16, width: 64, height: 32 },
            render_bounds: SpriteBounds { x: -32, y: -16, width: 64, height: 32 },
            ..Default::default()
        };
        let mut ids = vec![0; 8];
        ids[7] = 42;
        let catalog =
            ActorCatalog { generation: 0, actors: vec![ActorInfo { definition, has_renderer: false, init_error: None }], sprite_object_ids: ids };
        view.art.actors = ActorIndex::new(Arc::new(catalog));
        assert_eq!(view.hit_test(Pos2::new(130.0, 150.0)).map(|i| i.kind), Some(Kind::Sprite));
        // A bounds function's answer, once the host has given it, wins.
        let key = view.actor_keys()[0];
        view.art.draws.insert(key, ActorDraw { found: true, grab: Some(SpriteBounds { x: 300, y: 300, width: 8, height: 8 }), ..Default::default() });
        assert_eq!(view.sprite_rect(0, false), Some(Rect::from_min_size(Pos2::new(300.0, 300.0), Vec2::splat(8.0))));
        // What the actor draws arrives after the level, and must not reframe a view already fitted to it.
        let fitted = view.scene_bounds();
        view.art.draws.get_mut(&key).unwrap().render = Some(SpriteBounds { x: -4000, y: -4000, width: 9000, height: 9000 });
        assert_eq!(view.sprite_rect(0, true), Some(Rect::from_min_size(Pos2::new(-4000.0, -4000.0), Vec2::splat(9000.0))));
        assert_eq!(view.scene_bounds(), fitted);
        // Moved by the host, it is drawn from its old draw list until the new one arrives.
        let mut next = view.snapshot.clone();
        next.course.sprites[0].x = 11;
        view.replace(next);
        assert_eq!(view.sprite_rect(0, false), Some(Rect::from_min_size(Pos2::new(316.0, 300.0), Vec2::splat(8.0))));
    }

    #[test]
    fn the_tile_array_follows_the_gesture_and_is_cached() {
        let mut view = LevelView::new(snapshot());
        let layout = bigstar_formats::ObjectLayout { rows: vec![vec![bigstar_formats::ObjectLayoutTile::new(7, 0)]], ..Default::default() };
        view.art.tilesets[0] = Some(DecodedTileset { map16_tiles: Vec::new(), objects: vec![layout.clone(), layout] });
        let first = view.tilemap();
        assert!(Arc::ptr_eq(&first, &view.tilemap()));
        assert!(first.cell_at(2, 3).draws());
        view.selection = vec![Item::new(Kind::Object, 0)];
        view.gesture = Some(Gesture::Move { delta: Vec2::new(32.0, 0.0), submitted: false });
        let moved = view.tilemap();
        assert!(!moved.cell_at(2, 3).written() && moved.cell_at(4, 3).draws());
        view.gesture = Some(Gesture::Resize { handle: Handle::Bottom, delta: Vec2::new(0.0, 16.0), submitted: false });
        assert!(view.tilemap().cell_at(5, 4).draws());
    }
}
