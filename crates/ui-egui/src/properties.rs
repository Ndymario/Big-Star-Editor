//! The properties panel as data: which rows a tab shows for the selection, what editor each row wears, what it says
//! about itself, and what turning one writes back. A port of `LevelPropertiesWidget`'s `refresh` and `applyFields`;
//! `properties_ui` only draws these rows.

use bigstar_core::Fx32;
use bigstar_formats::{
    NsmbLevelCamera, NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelScreen, NsmbLevelSprite, NsmbLevelView,
    NsmbLevelZone,
};
use bigstar_level::{
    BackgroundLayer, IndexedLevelValue, LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, LevelTilesetReferences,
    SpriteFieldDefinition, read_sprite_field, sprite_field_max, sprite_field_min, write_sprite_field,
};
use bigstar_protocol::{ActorInfo, LevelEdit, LevelSnapshot, ObjectBankOverlays};

use crate::level_view::{Gesture, Item, Kind, LevelView, TILE};

/// The panel's side tabs, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Course,
    Banks,
    Entity(Kind),
}

pub const TABS: [(Tab, &str); 9] = [
    (Tab::Course, "Course"),
    (Tab::Banks, "Banks"),
    (Tab::Entity(Kind::Object), "Objects"),
    (Tab::Entity(Kind::Sprite), "Sprites"),
    (Tab::Entity(Kind::Entrance), "Entrances"),
    (Tab::Entity(Kind::View), "Views"),
    (Tab::Entity(Kind::Zone), "Zones"),
    (Tab::Entity(Kind::Path), "Paths"),
    (Tab::Entity(Kind::ProgressPath), "Progress"),
];

/// Object ID, X, Y and the two event bytes come before an actor's own fields.
pub const SPRITE_FIELD_ROW: usize = 5;

/// What the panel is looking at. The canvas selection drives it, and it drives the canvas selection back when the
/// person picks a tab, an item or a node here.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelState {
    pub tab: Tab,
    pub item: usize,
    pub node: usize,
    /// Two or more items of the tab's kind, when the canvas selected several.
    pub multi: Vec<usize>,
    pub hex: bool,
    /// A field being dragged: shown at this value and written when the drag ends.
    pub dragging: Option<(usize, i64)>,
    /// The raw settings box as typed, while it does not hold a whole value.
    pub settings_text: Option<String>,
    pub show_actor_info: bool,
    /// The selection changed somewhere other than the list, which has yet to scroll to it.
    pub reveal: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            tab: Tab::Course,
            item: 0,
            node: 0,
            multi: Vec::new(),
            hex: false,
            dragging: None,
            settings_text: None,
            show_actor_info: false,
            reveal: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Editor {
    Number {
        min: i64,
        max: i64,
    },
    /// The values an actor names. They are sparse, so each entry carries the number it stands for.
    Choice(Vec<(i64, String)>),
    /// On or off, whatever value turning it on writes.
    Boolean,
}

/// Whether a row shares its line with the field beside it. Only true peers pair up: X with Y, Width with Height.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Half,
    Full,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub label: String,
    /// `None` when the row stands for more than one value across the selection.
    pub value: Option<i64>,
    pub editor: Editor,
    pub span: Span,
    /// What a row and the peer it shares a line with are together: X and Y are a position. Empty for a row on its own.
    pub group: String,
    /// What the actor says about the field, shown beside it when there is anything to say.
    pub note: String,
    /// False when the value is one the editor cannot express: bits a choice list does not name, or checkbox bits
    /// that are neither off nor the value that turns it on.
    pub representable: bool,
}

impl Row {
    fn number(label: &str, value: Option<i64>, min: i64, max: i64, span: Span) -> Self {
        Self {
            label: label.to_owned(),
            value,
            editor: Editor::Number { min, max },
            span,
            group: String::new(),
            note: String::new(),
            representable: true,
        }
    }
}

/// One reading of a value, so a choice entry and the box beside it never disagree about the base.
pub fn format_value(value: i64, hex: bool) -> String {
    match (hex, value < 0) {
        (false, _) => value.to_string(),
        (true, false) => format!("0x{value:X}"),
        (true, true) => format!("-0x{:X}", value.unsigned_abs()),
    }
}

/// Either base, either case, an optional `0x`.
pub fn parse_value(text: &str, hex: bool) -> Option<i64> {
    let text = text.trim();
    if !hex {
        return text.parse().ok();
    }
    let (negative, digits) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    let digits = digits.strip_prefix("0x").or_else(|| digits.strip_prefix("0X")).unwrap_or(digits);
    let value = i64::from_str_radix(digits, 16).ok()?;
    Some(if negative { -value } else { value })
}

/// The settings word from its high byte down: `12 34 56 78`. The two event bytes are not settings, and each has its own
/// row.
pub fn format_settings(settings: u32) -> String {
    settings.to_be_bytes().iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// Eight hex digits, spaces anywhere; anything else is not a value yet.
pub fn parse_settings(text: &str) -> Option<u32> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    if digits.len() != 8 {
        return None;
    }
    u32::from_str_radix(&digits, 16).ok()
}

/// A sprite's settings and event bytes as the one 48-bit value its fields are read out of.
pub fn sprite_data(sprite: &NsmbLevelSprite) -> u64 {
    u64::from(sprite.settings) | (u64::from(sprite.events[1]) << 32) | (u64::from(sprite.events[0]) << 40)
}

fn set_sprite_data(sprite: &mut NsmbLevelSprite, data: u64) {
    sprite.settings = data as u32;
    sprite.events[1] = (data >> 32) as u8;
    sprite.events[0] = (data >> 40) as u8;
}

pub fn item_count(snapshot: &LevelSnapshot, tab: Tab) -> usize {
    let course = &snapshot.course;
    match tab {
        Tab::Course => 1,
        Tab::Banks => course.object_banks.len(),
        Tab::Entity(Kind::Object) => snapshot.objects.len(),
        Tab::Entity(Kind::Sprite) => course.sprites.len(),
        Tab::Entity(Kind::Entrance) => course.entrances.len(),
        Tab::Entity(Kind::View) => course.views.len(),
        Tab::Entity(Kind::Zone) => course.zones.len(),
        Tab::Entity(Kind::Path) => course.paths.len(),
        Tab::Entity(Kind::ProgressPath) => course.progress_paths.len(),
    }
}

fn paths(snapshot: &LevelSnapshot, kind: Kind) -> &Vec<NsmbLevelPath> {
    if kind == Kind::ProgressPath { &snapshot.course.progress_paths } else { &snapshot.course.paths }
}

/// What the game calls each entrance type, in the order it numbers them. Its tables end at `Vine`; the four after it
/// are in Nintendo's own levels all the same, and NSMB Editor names them.
const ENTRANCE_TYPES: [&str; 26] = [
    "Normal",
    "Normal (overridden)",
    "Door",
    "Pipe up",
    "Pipe down",
    "Pipe left",
    "Pipe right",
    "Falling",
    "Ground pound",
    "Sliding",
    "Swimming",
    "Transit",
    "Transit door",
    "Mario vs. Luigi pipe",
    "Invisible door (no sound)",
    "Invisible door",
    "Mini pipe up",
    "Mini pipe down",
    "Mini pipe left",
    "Mini pipe right",
    "Jumping",
    "Vine",
    "Connected mini pipe up",
    "Connected mini pipe down",
    "Connected mini pipe left",
    "Connected mini pipe right",
];

/// The row an entrance's type is on, and the row its flags start at: one checkbox a bit, from the lowest.
const ENTRANCE_TYPE_ROW: usize = 5;
const ENTRANCE_FLAG_ROW: usize = 13;

/// What each bit of an entrance's flags does, read off `Entrance::tryUseEntrance` and `Entrance::accessEntrance`.
const ENTRANCE_FLAGS: [(&str, &str); 8] = [
    ("Bottom screen", "The player arrives on the bottom screen, and the fade into it crosses screens."),
    ("Closed to player 2", "The second player cannot enter this entrance."),
    ("Closed to player 1", "The first player cannot enter this entrance."),
    (
        "Connected pipe",
        "The player rides a path to the other end rather than warping. Target stage is then the path's ID, and Target area is kept for the ride: the game's headers call it travelling the path backwards.",
    ),
    ("Wipe fade", "Entering fades with a wipe from one screen to the other, in place of the character's own shape."),
    ("Unknown 0x20", "Nothing in the game's entrance code reads this bit."),
    ("Exact X", "The player arrives at the entrance's own X. Without it most types move the player half a tile right, onto the middle of it."),
    ("Exit only", "The player can arrive here but cannot enter it."),
];

/// The lines of the panel's list for `kind`: every item of it, and under each selected path its nodes. Kinds the
/// level refers to by ID are listed by it, as Miyamoto lists them, since that is the number the person is looking
/// for; objects and sprites have none and keep the order the level holds them in.
pub fn list_lines(view: &LevelView, kind: Kind) -> Vec<Item> {
    let course = &view.snapshot.course;
    let ids: Vec<u8> = match kind {
        Kind::Object | Kind::Sprite => Vec::new(),
        Kind::Entrance => course.entrances.iter().map(|e| e.id).collect(),
        Kind::View => course.views.iter().map(|v| v.id).collect(),
        Kind::Zone => course.zones.iter().map(|z| z.id).collect(),
        Kind::Path | Kind::ProgressPath => paths(&view.snapshot, kind).iter().map(|p| p.id).collect(),
    };
    let mut order: Vec<usize> = (0..item_count(&view.snapshot, Tab::Entity(kind))).collect();
    order.sort_by_key(|&index| ids.get(index).copied());
    let mut lines = Vec::with_capacity(order.len());
    for index in order {
        lines.push(Item::new(kind, index));
        if kind.path() && view.is_selected(kind, index, None) {
            let nodes = view.path(kind, index).map_or(0, Vec::len);
            lines.extend((0..nodes).map(|node| Item::node(kind, index, node)));
        }
    }
    lines
}

/// What a line of the list says: enough to tell the item from its neighbours without looking at the canvas.
pub fn line_label(view: &LevelView, item: Item) -> String {
    let course = &view.snapshot.course;
    let index = item.index;
    let label = match item.kind {
        Kind::Object => {
            view.snapshot.objects.get(index).map(|o| format!("Object {}:{} ({}, {}) {}×{}", o.tileset_slot, o.object_id, o.x, o.y, o.width, o.height))
        }
        Kind::Sprite => course.sprites.get(index).map(|s| {
            let actor = view.art.actors.actor(s.id, view.sprite_hash(index));
            let name = actor.map_or_else(|| format!("Actor {}", s.id), |a| a.definition.name.clone());
            format!("{name} ({}, {})", s.x, s.y)
        }),
        Kind::Entrance => course.entrances.get(index).map(|e| {
            let kind = ENTRANCE_TYPES.get(usize::from(e.kind)).map_or_else(|| format!("Type {}", e.kind), |name| (*name).to_owned());
            format!("Entrance {} · {kind} ({}, {})", e.id, e.x, e.y)
        }),
        Kind::View => course.views.get(index).map(|v| format!("View {} ({}, {}) {}×{}", v.id, v.x, v.y, v.width, v.height)),
        Kind::Zone => course.zones.get(index).map(|z| format!("Zone {} ({}, {}) {}×{}", z.id, z.x, z.y, z.width, z.height)),
        Kind::Path | Kind::ProgressPath => paths(&view.snapshot, item.kind).get(index).and_then(|path| match item.node {
            Some(node) => path.nodes.get(node).map(|n| format!("Node {node} ({}, {})", n.x, n.y)),
            None => Some(format!("{} {} · {} node{}", item.kind.name(), path.id, path.nodes.len(), if path.nodes.len() == 1 { "" } else { "s" })),
        }),
    };
    label.unwrap_or_default()
}

/// Reads one byte field of an entrance.
type EntranceByte = fn(&NsmbLevelEntrance) -> i64;

/// What the panel shows: the rows, and for a sprite the actor it resolved to.
#[derive(Debug, Clone, PartialEq)]
pub struct Rows {
    pub rows: Vec<Row>,
    /// The selected items the rows read, first first. Empty when there is nothing of the tab's kind.
    pub indices: Vec<usize>,
    pub actor: Option<ActorInfo>,
    /// The declared field each sprite row from `SPRITE_FIELD_ROW` on stands for.
    pub fields: Vec<SpriteFieldDefinition>,
    /// The node the path rows read, clamped to the path.
    pub node: usize,
}

impl Rows {
    pub fn multi(&self) -> bool {
        self.indices.len() > 1
    }
}

/// The first of a view's rows that reads its camera settings rather than the view.
const VIEW_CAMERA_ROW: usize = 12;

/// What a view's background or foreground number means: the block of that layer it names, and what the block shows.
fn layer_note(view: &LevelView, layer: BackgroundLayer, blocks: &[NsmbLevelScreen], block_id: u8) -> String {
    let what = layer.name();
    let lead = format!("The {what} block this view scrolls by: its speeds, its height and whether the {what} shows here at all.");
    let Some(block) = bigstar_level::layer_block(blocks, block_id) else { return format!("{lead} The level has no {what} blocks.") };
    let fallback =
        if block.id == u16::from(block_id) { String::new() } else { format!(" No block has this id, so the game uses block {}.", block.id) };
    if block.screen_id == u16::MAX || block.scroll_y == u16::MAX {
        return format!("{lead}{fallback} That block names no screen, so this view has no {what}.");
    }
    let name = view.art.graphics.as_ref().and_then(|graphics| graphics.backgrounds.name(layer, block.screen_id));
    match name {
        Some(name) => format!("{lead}{fallback} It shows screen {}, {name}.", block.screen_id),
        None => format!("{lead}{fallback} It shows screen {}.", block.screen_id),
    }
}

/// Where the game opens on coming in through an entrance: the screen's corner and how far each layer has scrolled.
fn spawn_note(view: &LevelView, entrance: &NsmbLevelEntrance) -> String {
    let screen = view.spawn_screen(entrance);
    let (player_x, player_y) = bigstar_level::entrance_spawn_point(entrance);
    let mut note = format!(
        "\n\nComing in here the player stands at {player_x}, {player_y} and the screen's top left corner is at {}, {}.",
        screen.rect.left(),
        screen.rect.top()
    );
    if screen.view.is_none() {
        note.push_str(" No view has this ID, so nothing bounds the camera or scrolls the layers.");
        return note;
    }
    for layer in [BackgroundLayer::Background, BackgroundLayer::Foreground] {
        match view.screen_layer(&screen, layer) {
            Some(scroll) => note.push_str(&format!(" The {} is scrolled to {}, {}.", layer.name(), scroll.x >> 12, scroll.y)),
            None => note.push_str(&format!(" The view shows no {}.", layer.name())),
        }
    }
    note.push_str(" Turn on DS Screen to see it.");
    if !matches!(entrance.camera_zoom, 0 | 16 | 255) {
        note.push_str(" This entrance changes the zoom, which the prediction leaves out.");
    }
    note
}

/// The rows of the panel's tab for what it is looking at. `overlays` labels each object bank with the code overlay it
/// loads.
pub fn rows(view: &LevelView, panel: &PanelState, overlays: Option<&ObjectBankOverlays>) -> Rows {
    let snapshot = &view.snapshot;
    let course = &snapshot.course;
    let count = item_count(snapshot, panel.tab);
    let mut indices: Vec<usize> = panel.multi.iter().copied().filter(|&i| i < count).collect();
    if indices.len() < 2 {
        indices = if count == 0 { Vec::new() } else { vec![panel.item.min(count - 1)] };
    }
    let mut out = Rows { rows: Vec::new(), indices: indices.clone(), actor: None, fields: Vec::new(), node: 0 };
    let Some(&index) = indices.first() else { return out };
    let rows = &mut out.rows;
    // The value every selected item agrees on, or none.
    fn common<T>(indices: &[usize], list: &[T], get: impl Fn(&T) -> i64) -> Option<i64> {
        let mut values = indices.iter().filter_map(|&i| list.get(i).map(&get));
        let first = values.next()?;
        values.all(|v| v == first).then_some(first)
    }
    let full = |rows: &mut Vec<Row>, label: &str, value: Option<i64>, min: i64, max: i64| rows.push(Row::number(label, value, min, max, Span::Full));
    let pair = |rows: &mut Vec<Row>, group: &str, (a, av): (&str, Option<i64>), (b, bv): (&str, Option<i64>), min: i64, max: i64| {
        for (label, value) in [(a, av), (b, bv)] {
            rows.push(Row { group: group.to_owned(), ..Row::number(label, value, min, max, Span::Half) });
        }
    };
    match panel.tab {
        Tab::Course => {
            let r = snapshot.tileset_references;
            // In the order `apply` reads them back.
            for (label, value, layer) in [
                ("Tileset", r.tileset_id, None),
                ("BG graphics", r.background_graphics_id, Some(BackgroundLayer::Background)),
                ("BG screen", r.background_screen_id, Some(BackgroundLayer::Background)),
                ("FG graphics", r.foreground_graphics_id, Some(BackgroundLayer::Foreground)),
                ("FG screen", r.foreground_screen_id, Some(BackgroundLayer::Foreground)),
                ("BG palette", r.background_palette_id, Some(BackgroundLayer::Background)),
                ("FG palette", r.foreground_palette_id, Some(BackgroundLayer::Foreground)),
            ] {
                full(rows, label, Some(i64::from(value)), 0, 255);
                // The game's tables hold a background's graphics, palette and screen at the same number, so the name
                // the project's scripts give that number names all three.
                let named = view.art.graphics.as_ref().zip(layer).map(|(graphics, layer)| graphics.backgrounds.layer(layer)).unwrap_or_default();
                if let Some(row) = rows.last_mut().filter(|_| !named.is_empty()) {
                    row.representable = named.iter().any(|definition| i64::from(definition.id) == i64::from(value));
                    row.editor = Editor::Choice(named.iter().map(|definition| (i64::from(definition.id), definition.name.clone())).collect());
                }
            }
        }
        Tab::Banks => {
            let bank = course.object_banks.get(index).copied().unwrap_or(0);
            // A slot with no banks holds 0xFFFFFFFF there: no overlay to name.
            let overlay = overlays
                .filter(|_| (1..=10).contains(&bank))
                .and_then(|table| table.get(index)?.get(usize::from(bank) - 1))
                .filter(|&&overlay| overlay != u32::MAX);
            let label = overlay.map_or_else(|| "Bank ID".to_owned(), |overlay| format!("Bank ID (overlay {overlay})"));
            full(rows, &label, Some(i64::from(bank)), 0, 10);
        }
        Tab::Entity(Kind::Object) => {
            let list = &snapshot.objects;
            full(rows, "Object ID", common(&indices, list, |o| i64::from(o.object_id)), 0, 4095);
            full(rows, "Tileset slot", common(&indices, list, |o| i64::from(o.tileset_slot)), 0, 15);
            pair(
                rows,
                "Position (tiles)",
                ("X", common(&indices, list, |o| i64::from(o.x))),
                ("Y", common(&indices, list, |o| i64::from(o.y))),
                -32768,
                32767,
            );
            pair(
                rows,
                "Size (tiles)",
                ("W", common(&indices, list, |o| i64::from(o.width))),
                ("H", common(&indices, list, |o| i64::from(o.height))),
                1,
                65535,
            );
        }
        Tab::Entity(Kind::Sprite) => {
            let list = &course.sprites;
            // Which actor this is decides what the rest of the panel has to say.
            let actor = if indices.len() == 1 {
                list.get(index).and_then(|sprite| view.art.actors.actor(sprite.id, view.sprite_hash(index))).cloned()
            } else {
                None
            };
            full(rows, "Object ID", common(&indices, list, |s| i64::from(s.id)), 0, 65535);
            if let (Some(actor), Some(row)) = (&actor, rows.last_mut()) {
                row.note.clone_from(&actor.definition.notes);
            }
            pair(
                rows,
                "Position (tiles)",
                ("X", common(&indices, list, |s| i64::from(s.x))),
                ("Y", common(&indices, list, |s| i64::from(s.y))),
                0,
                65535,
            );
            // One names the event this actor raises when it fires, the other the event that lets it act.
            pair(
                rows,
                "",
                ("Target event", common(&indices, list, |s| i64::from(s.events[0]))),
                ("Trigger event", common(&indices, list, |s| i64::from(s.events[1]))),
                0,
                255,
            );
            if let Some(row) = rows.get_mut(3) {
                row.note = "The event this actor activates when it fires.".to_owned();
            }
            if let Some(row) = rows.get_mut(4) {
                row.note = "The event that has to be active for this actor to act.".to_owned();
            }
            if let (Some(actor), Some(sprite)) = (&actor, list.get(index)) {
                for field in &actor.definition.fields {
                    rows.push(declared_row(field, read_sprite_field(sprite_data(sprite), field)));
                    out.fields.push(field.clone());
                }
            }
            out.actor = actor;
        }
        Tab::Entity(Kind::Entrance) => {
            let list = &course.entrances;
            let get = |f: fn(&NsmbLevelEntrance) -> i64| common(&indices, list, f);
            pair(rows, "Position", ("X", get(|e| i64::from(e.x))), ("Y", get(|e| i64::from(e.y))), 0, 65535);
            pair(rows, "Camera offset", ("X", get(|e| i64::from(e.camera_x))), ("Y", get(|e| i64::from(e.camera_y))), 0, 65535);
            let bytes: [(&str, EntranceByte, &str); 11] = [
                ("ID", |e| i64::from(e.id), "The number other entrances and the level header name this one by."),
                ("Type", |e| i64::from(e.kind), ""),
                (
                    "Target area",
                    |e| i64::from(e.target_stage_area),
                    "0 stays in this area, and the stage and group are then ignored. Otherwise the area's number plus one.",
                ),
                (
                    "Target stage / pipe path",
                    |e| i64::from(e.target_stage_index),
                    "0 is this stage, otherwise the stage's number plus one. On a connected pipe, the ID of the path it follows.",
                ),
                ("Target group", |e| i64::from(e.target_stage_group), "0 is this world, otherwise the world's number plus one."),
                ("Target entrance", |e| i64::from(e.target_entrance), "The ID of the entrance the player comes out of."),
                (
                    "Camera zoom",
                    |e| i64::from(e.camera_zoom),
                    "In sixteenths: 16 is the normal zoom, and so is 0. The game keeps it between 8 and 24; 255 leaves the zoom as it is.",
                ),
                ("View ID", |e| i64::from(e.view_id), "The view the player arrives in, which bounds the camera and picks the music. 255 is none."),
                (
                    "World-map node",
                    |e| i64::from(e.target_worldmap_node),
                    "0: a normal entrance. 1 to 254: entering it exits the level to the world map, with the player on map node (this number minus 1). 255: exits to the world map where the player already stood.",
                ),
                ("Unknown 0x10", |e| i64::from(e.unknown_10), "Nothing in the game's entrance code reads this byte."),
                (
                    "Unknown 0x11",
                    |e| i64::from(e.unknown_11),
                    "Anything but 0 replaces the warp: entering fades out and back in within this area, ignoring the target. One below this number, 0 to 7, is kept for something that has not been found.",
                ),
            ];
            let flag_rows = |rows: &mut Vec<Row>| {
                for (bit, (label, note)) in ENTRANCE_FLAGS.into_iter().enumerate() {
                    let value = common(&indices, list, |e| i64::from(e.flags >> bit & 1));
                    rows.push(Row {
                        label: label.to_owned(),
                        value,
                        editor: Editor::Boolean,
                        span: Span::Full,
                        group: String::new(),
                        note: note.to_owned(),
                        representable: true,
                    });
                }
            };
            for (label, read, note) in bytes {
                // The flags sit between what the entrance does and the two bytes nobody has named.
                if label == "Unknown 0x10" {
                    flag_rows(rows);
                }
                full(rows, label, get(read), 0, 255);
                if let Some(row) = rows.last_mut() {
                    row.note = note.to_owned();
                }
            }
            if let Some(row) = rows.get_mut(ENTRANCE_TYPE_ROW) {
                let named = row.value.is_none_or(|v| usize::try_from(v).is_ok_and(|v| v < ENTRANCE_TYPES.len()));
                row.editor = Editor::Choice((0i64..).zip(ENTRANCE_TYPES).map(|(value, name)| (value, name.to_owned())).collect());
                row.representable = named;
                row.note = "How the player arrives, and for doors and pipes how they leave. The game's tables stop at Vine: Nintendo's levels use the connected mini pipes after it, but the game reads past its tables for them.".to_owned();
            }
            // Where all of the above puts the camera and the two layers, for the one entrance being looked at.
            if let ([only], Some(row)) = (indices.as_slice(), rows.iter_mut().find(|row| row.label == "View ID"))
                && let Some(entrance) = list.get(*only)
            {
                row.note.push_str(&spawn_note(view, entrance));
            }
        }
        Tab::Entity(Kind::View) => {
            // A view reads only the first selected one, and a selection of views edits nothing: as the Qt panel.
            if let Some(v) = course.views.get(index) {
                pair(rows, "Position", ("X", Some(i64::from(v.x))), ("Y", Some(i64::from(v.y))), 0, 65535);
                pair(rows, "Size", ("W", Some(i64::from(v.width))), ("H", Some(i64::from(v.height))), 1, 65535);
                for (label, value) in [
                    ("ID", v.id),
                    ("Camera settings", v.camera_settings_id),
                    ("BGM", v.bgm_id),
                    ("Background", v.background_id),
                    ("Tileset", v.tileset_id),
                    ("Foreground", v.foreground_id),
                    ("Light", v.light_type),
                    ("Progress path", v.progress_path_id),
                ] {
                    full(rows, label, Some(i64::from(value)), 0, 255);
                    // A view names a block of the layer, and the block says which screen scrolls there and how.
                    let (layer, blocks) = match label {
                        "Background" => (BackgroundLayer::Background, &course.backgrounds),
                        "Foreground" => (BackgroundLayer::Foreground, &course.foregrounds),
                        _ => continue,
                    };
                    if let Some(row) = rows.last_mut() {
                        row.note = layer_note(view, layer, blocks, value);
                    }
                }
                // The camera settings the view names, which every view naming them shares.
                let found = bigstar_level::camera_block(&course.cameras, v.camera_settings_id);
                let own = found.is_some_and(|camera| camera.id & 0xff == u16::from(v.camera_settings_id));
                let camera = found.cloned().unwrap_or_default();
                let shared = course.views.iter().filter(|other| other.camera_settings_id == v.camera_settings_id).count();
                let state = match (found.is_some(), own) {
                    (_, true) if shared > 1 => format!(" Shared by the {shared} views that name camera settings {}.", v.camera_settings_id),
                    (_, true) => String::new(),
                    (true, false) => format!(
                        " No camera settings have ID {}, so the game uses the level's first ({}); changing this gives the view its own.",
                        v.camera_settings_id, camera.id
                    ),
                    (false, _) => " The level has no camera settings; changing this adds some.".to_owned(),
                };
                let lines = "The game moves the camera up or down only as far as keeps the player between two lines down the screen.";
                for (label, value, min, max, note) in [
                    ("Camera top offset", i64::from(camera.top_offset), i64::from(i32::MIN), i64::from(i32::MAX), format!("{lines} The upper is 64 pixels down plus this.")),
                    ("Camera bottom offset", i64::from(camera.bottom_offset), i64::from(i32::MIN), i64::from(i32::MAX), format!("{lines} The lower is 128 pixels down plus this.")),
                    (
                        "Camera top offset (special)",
                        i64::from(camera.top_offset_special),
                        i64::from(i32::MIN),
                        i64::from(i32::MAX),
                        "The upper line in the game's second camera mode. Which of the player's moves switch to it has not been traced.".to_owned(),
                    ),
                    (
                        "Camera bottom offset (special)",
                        i64::from(camera.bottom_offset_special),
                        i64::from(i32::MIN),
                        i64::from(i32::MAX),
                        "The lower line in that second mode.".to_owned(),
                    ),
                    ("Camera scroll height", i64::from(camera.scroll_height), 0, 65535, "15 is none. Anything else has the game note a camera height this many tiles above where it rests on the view's floor; what reads it has not been traced.".to_owned()),
                    ("Camera unknown", i64::from(camera.unknown), 0, i64::from(u32::MAX), String::new()),
                ] {
                    full(rows, label, Some(value), min, max);
                    if let Some(row) = rows.last_mut() {
                        row.note = format!("{note}{state}");
                    }
                }
            }
        }
        Tab::Entity(Kind::Zone) => {
            let list = &course.zones;
            pair(rows, "Position", ("X", common(&indices, list, |z| i64::from(z.x))), ("Y", common(&indices, list, |z| i64::from(z.y))), 0, 65535);
            pair(
                rows,
                "Size",
                ("W", common(&indices, list, |z| i64::from(z.width))),
                ("H", common(&indices, list, |z| i64::from(z.height))),
                1,
                65535,
            );
            full(rows, "ID", common(&indices, list, |z| i64::from(z.id)), 0, 255);
        }
        Tab::Entity(kind @ (Kind::Path | Kind::ProgressPath)) => {
            if let Some(path) = paths(snapshot, kind).get(index) {
                out.node = panel.node.min(path.nodes.len().saturating_sub(1));
                full(rows, "Path ID", Some(i64::from(path.id)), 0, 255);
                if let Some(node) = path.nodes.get(out.node) {
                    pair(rows, "Position", ("X", Some(i64::from(node.x))), ("Y", Some(i64::from(node.y))), -32768, 32767);
                    let (raw_min, raw_max) = (-i64::from(i32::MAX), i64::from(i32::MAX));
                    full(rows, "Speed (fx32 raw)", Some(i64::from(node.speed.raw())), raw_min, raw_max);
                    full(rows, "Acceleration (fx32 raw)", Some(i64::from(node.acceleration.raw())), raw_min, raw_max);
                    full(rows, "Delay", Some(i64::from(node.delay)), 0, 65535);
                    full(rows, "Flags", Some(i64::from(node.flags)), 0, 65535);
                }
            }
        }
    }
    preview_gesture(view, &mut out);
    out
}

/// A field an actor declares. One that is on or off is a checkbox, one that names its values is picked from them, and
/// the rest are typed; no actor declares both.
fn declared_row(field: &SpriteFieldDefinition, value: i32) -> Row {
    let value = i64::from(value);
    let (editor, representable) = if field.checkbox_value != 0 {
        (Editor::Boolean, value == 0 || value == 1)
    } else if !field.choices.is_empty() {
        let choices: Vec<(i64, String)> = field.choices.iter().map(|c| (i64::from(c.value), c.label.clone())).collect();
        let named = choices.iter().any(|(v, _)| *v == value);
        (Editor::Choice(choices), named)
    } else {
        (Editor::Number { min: i64::from(sprite_field_min(field)), max: i64::from(sprite_field_max(field)) }, true)
    };
    let mut note = field.notes.clone();
    if !representable {
        if !note.is_empty() {
            note.push_str("\n\n");
        }
        note.push_str(&format!("This actor does not name the value {value}, which is what these bits hold."));
    }
    Row { label: field.label.clone(), value: Some(value), editor, span: Span::Full, group: String::new(), note, representable }
}

/// The rows as the canvas gesture in progress would leave them: the inspector follows a drag before it is committed.
fn preview_gesture(view: &LevelView, out: &mut Rows) {
    let Some(gesture) = view.gesture else { return };
    let Some(&first) = out.indices.first() else { return };
    let Some(kind) = view.selection.first().map(|s| s.kind) else { return };
    let snapshot = &view.snapshot;
    let round = |v: f32| v.round() as i64;
    let mut shown: Vec<(usize, i64)> = Vec::new();
    match gesture {
        Gesture::Move { delta, .. } => {
            let (tx, ty) = (round(delta.x / TILE), round(delta.y / TILE));
            let (px, py) = (round(delta.x), round(delta.y));
            let clamp_u = |v: i64| v.clamp(0, 65535);
            let clamp_s = |v: i64| v.clamp(-32768, 32767);
            let rows = &out.rows;
            let at = |row: usize| rows.get(row).and_then(|r| r.value);
            let shifted: Vec<(usize, i64)> = match kind {
                Kind::Object => [(2, tx), (3, ty)].into_iter().filter_map(|(r, d)| at(r).map(|v| (r, clamp_s(v + d)))).collect(),
                Kind::Sprite => [(1, tx), (2, ty)].into_iter().filter_map(|(r, d)| at(r).map(|v| (r, clamp_u(v + d)))).collect(),
                Kind::Entrance | Kind::Zone | Kind::View => {
                    [(0, px), (1, py)].into_iter().filter_map(|(r, d)| at(r).map(|v| (r, clamp_u(v + d)))).collect()
                }
                Kind::Path | Kind::ProgressPath if out.indices.len() == 1 && view.is_selected(kind, first, Some(out.node)) => {
                    [(1, px), (2, py)].into_iter().filter_map(|(r, d)| at(r).map(|v| (r, clamp_s(v + d)))).collect()
                }
                _ => Vec::new(),
            };
            shown = shifted;
        }
        Gesture::Resize { handle, delta, .. } if out.indices.len() == 1 => {
            let edges = handle.edges(delta);
            let resized = |origin: i64, extent: i64, near: f32, far: f32, scale: f32| {
                let (near, far) = (round(near / scale), round(far / scale));
                let (mut origin, mut extent) = (origin + near, extent - near + far);
                if extent < 1 {
                    origin -= 1 - extent;
                    extent = 1;
                }
                (origin, extent)
            };
            let rect = match kind {
                Kind::Object => {
                    snapshot.objects.get(first).map(|o| (i64::from(o.x), i64::from(o.y), i64::from(o.width), i64::from(o.height), TILE, 2))
                }
                Kind::Zone => {
                    snapshot.course.zones.get(first).map(|z| (i64::from(z.x), i64::from(z.y), i64::from(z.width), i64::from(z.height), 1.0, 0))
                }
                Kind::View => {
                    snapshot.course.views.get(first).map(|v| (i64::from(v.x), i64::from(v.y), i64::from(v.width), i64::from(v.height), 1.0, 0))
                }
                _ => None,
            };
            if let Some((x, y, w, h, scale, row)) = rect {
                let (x, w) = resized(x, w, edges.left, edges.right, scale);
                let (y, h) = resized(y, h, edges.top, edges.bottom, scale);
                let (low, high) = if kind == Kind::Object { (-32768, 32767) } else { (0, 65535) };
                shown = vec![(row, x.clamp(low, high)), (row + 1, y.clamp(low, high)), (row + 2, w.clamp(0, 65535)), (row + 3, h.clamp(0, 65535))];
            }
        }
        _ => {}
    }
    // A row that stands for several values stays undecided.
    for (row, value) in shown {
        if let Some(r) = out.rows.get_mut(row).filter(|r| r.value.is_some()) {
            r.value = Some(value);
        }
    }
}

fn narrow<T: TryFrom<i64> + Copy>(value: i64, fallback: T) -> T {
    T::try_from(value).unwrap_or(fallback)
}

/// The edit turning row `changed` to `value` makes, for every selected item: only that field changes. For a sprite
/// only the declared field that was turned is written into the settings, so the rest of the word survives.
pub fn apply(view: &LevelView, panel: &PanelState, rows: &Rows, changed: usize, value: i64) -> Option<LevelEdit> {
    let snapshot = &view.snapshot;
    let course = &snapshot.course;
    let multi = rows.multi();
    let mut updates = LevelEntityUpdates::default();
    let name = |single: &str| if multi { "Edit selection".to_owned() } else { single.to_owned() };
    match panel.tab {
        Tab::Course => {
            let mut r: LevelTilesetReferences = snapshot.tileset_references;
            let target = match changed {
                0 => &mut r.tileset_id,
                1 => &mut r.background_graphics_id,
                2 => &mut r.background_screen_id,
                3 => &mut r.foreground_graphics_id,
                4 => &mut r.foreground_screen_id,
                5 => &mut r.background_palette_id,
                6 => &mut r.foreground_palette_id,
                _ => return None,
            };
            *target = narrow(value, *target);
            Some(LevelEdit::SetTilesetReferences { references: r })
        }
        Tab::Banks => {
            let slot = *rows.indices.first()?;
            (changed == 0).then(|| LevelEdit::SetObjectBank { slot, value: narrow(value, 0) })
        }
        Tab::Entity(Kind::Object) => {
            for &index in &rows.indices {
                let mut o: NsmbLevelObject = snapshot.objects.get(index)?.clone();
                match changed {
                    0 => o.object_id = narrow(value, o.object_id),
                    1 => o.tileset_slot = narrow(value, o.tileset_slot),
                    2 => o.x = narrow(value, o.x),
                    3 => o.y = narrow(value, o.y),
                    4 => o.width = narrow(value, o.width),
                    5 => o.height = narrow(value, o.height),
                    _ => return None,
                }
                updates.objects.push(IndexedLevelValue { index, value: o });
            }
            Some(LevelEdit::Update { updates, name: name("Edit object") })
        }
        Tab::Entity(Kind::Sprite) => {
            for &index in &rows.indices {
                let mut s: NsmbLevelSprite = course.sprites.get(index)?.clone();
                match changed {
                    0 => s.id = narrow(value, s.id),
                    1 => s.x = narrow(value, s.x),
                    2 => s.y = narrow(value, s.y),
                    3 => s.events[0] = narrow(value, s.events[0]),
                    4 => s.events[1] = narrow(value, s.events[1]),
                    row if !multi => {
                        let field = rows.fields.get(row.checked_sub(SPRITE_FIELD_ROW)?)?;
                        let data = write_sprite_field(sprite_data(&s), field, narrow(value, 0));
                        set_sprite_data(&mut s, data);
                    }
                    _ => return None,
                }
                updates.sprites.push(IndexedLevelValue { index, value: s });
            }
            Some(LevelEdit::Update { updates, name: name("Edit sprite") })
        }
        Tab::Entity(Kind::Entrance) => {
            for &index in &rows.indices {
                let mut e: NsmbLevelEntrance = course.entrances.get(index)?.clone();
                match changed {
                    0 => e.x = narrow(value, e.x),
                    1 => e.y = narrow(value, e.y),
                    2 => e.camera_x = narrow(value, e.camera_x),
                    3 => e.camera_y = narrow(value, e.camera_y),
                    row if (ENTRANCE_FLAG_ROW..ENTRANCE_FLAG_ROW + ENTRANCE_FLAGS.len()).contains(&row) => {
                        let bit = 1u8 << (row - ENTRANCE_FLAG_ROW);
                        e.flags = if value != 0 { e.flags | bit } else { e.flags & !bit };
                    }
                    row => {
                        let byte = match row {
                            4 => &mut e.id,
                            ENTRANCE_TYPE_ROW => &mut e.kind,
                            6 => &mut e.target_stage_area,
                            7 => &mut e.target_stage_index,
                            8 => &mut e.target_stage_group,
                            9 => &mut e.target_entrance,
                            10 => &mut e.camera_zoom,
                            11 => &mut e.view_id,
                            12 => &mut e.target_worldmap_node,
                            21 => &mut e.unknown_10,
                            22 => &mut e.unknown_11,
                            _ => return None,
                        };
                        *byte = narrow(value, *byte);
                    }
                }
                updates.entrances.push(IndexedLevelValue { index, value: e });
            }
            Some(LevelEdit::Update { updates, name: name("Edit entrance") })
        }
        Tab::Entity(Kind::View) => {
            if multi {
                return None;
            }
            let index = *rows.indices.first()?;
            let mut v: NsmbLevelView = course.views.get(index)?.clone();
            // The rows after the view's own are its camera settings: a block of their own, made for this view's ID
            // from the one the game falls back to when none has it yet.
            if changed >= VIEW_CAMERA_ROW {
                let mut cameras = course.cameras.clone();
                let id = u16::from(v.camera_settings_id);
                let at = match cameras.iter().position(|camera| camera.id & 0xff == id) {
                    Some(at) => at,
                    None => {
                        let fresh = NsmbLevelCamera { id, ..cameras.first().cloned().unwrap_or_default() };
                        cameras.push(fresh);
                        cameras.len() - 1
                    }
                };
                let camera = cameras.get_mut(at)?;
                match changed - VIEW_CAMERA_ROW {
                    0 => camera.top_offset = narrow(value, camera.top_offset),
                    1 => camera.bottom_offset = narrow(value, camera.bottom_offset),
                    2 => camera.top_offset_special = narrow(value, camera.top_offset_special),
                    3 => camera.bottom_offset_special = narrow(value, camera.bottom_offset_special),
                    4 => camera.scroll_height = narrow(value, camera.scroll_height),
                    5 => camera.unknown = narrow(value, camera.unknown),
                    _ => return None,
                }
                return Some(LevelEdit::SetCameras { cameras, name: "Edit camera settings".to_owned() });
            }
            match changed {
                0 => v.x = narrow(value, v.x),
                1 => v.y = narrow(value, v.y),
                2 => v.width = narrow(value, v.width),
                3 => v.height = narrow(value, v.height),
                row => {
                    let byte = match row {
                        4 => &mut v.id,
                        5 => &mut v.camera_settings_id,
                        6 => &mut v.bgm_id,
                        7 => &mut v.background_id,
                        8 => &mut v.tileset_id,
                        9 => &mut v.foreground_id,
                        10 => &mut v.light_type,
                        11 => &mut v.progress_path_id,
                        _ => return None,
                    };
                    *byte = narrow(value, *byte);
                }
            }
            updates.views.push(IndexedLevelValue { index, value: v });
            Some(LevelEdit::Update { updates, name: "Edit view".to_owned() })
        }
        Tab::Entity(Kind::Zone) => {
            for &index in &rows.indices {
                let mut z: NsmbLevelZone = course.zones.get(index)?.clone();
                match changed {
                    0 => z.x = narrow(value, z.x),
                    1 => z.y = narrow(value, z.y),
                    2 => z.width = narrow(value, z.width),
                    3 => z.height = narrow(value, z.height),
                    4 => z.id = narrow(value, z.id),
                    _ => return None,
                }
                updates.zones.push(IndexedLevelValue { index, value: z });
            }
            Some(LevelEdit::Update { updates, name: name("Edit zone") })
        }
        Tab::Entity(kind @ (Kind::Path | Kind::ProgressPath)) => {
            if multi {
                return None;
            }
            let index = *rows.indices.first()?;
            let mut path = paths(snapshot, kind).get(index)?.clone();
            if changed == 0 {
                path.id = narrow(value, path.id);
            } else {
                let node = path.nodes.get_mut(rows.node)?;
                match changed {
                    1 => node.x = narrow(value, node.x),
                    2 => node.y = narrow(value, node.y),
                    3 => node.speed = Fx32::from_raw(narrow(value, node.speed.raw())),
                    4 => node.acceleration = Fx32::from_raw(narrow(value, node.acceleration.raw())),
                    5 => node.delay = narrow(value, node.delay),
                    6 => node.flags = narrow(value, node.flags),
                    _ => return None,
                }
            }
            Some(path_update(kind, index, path))
        }
    }
}

fn path_update(kind: Kind, index: usize, path: NsmbLevelPath) -> LevelEdit {
    let mut updates = LevelEntityUpdates::default();
    let progress = kind == Kind::ProgressPath;
    let target = if progress { &mut updates.progress_paths } else { &mut updates.paths };
    target.push(IndexedLevelValue { index, value: path });
    LevelEdit::Update { updates, name: if progress { "Edit progress path" } else { "Edit path" }.to_owned() }
}

/// The raw settings box's edit: the whole settings word, for every selected sprite.
pub fn apply_settings(view: &LevelView, rows: &Rows, settings: u32) -> Option<LevelEdit> {
    let mut updates = LevelEntityUpdates::default();
    for &index in &rows.indices {
        let mut sprite = view.snapshot.course.sprites.get(index)?.clone();
        sprite.settings = settings;
        updates.sprites.push(IndexedLevelValue { index, value: sprite });
    }
    let name = if rows.multi() { "Edit selection" } else { "Edit sprite" };
    (!updates.sprites.is_empty()).then(|| LevelEdit::Update { updates, name: name.to_owned() })
}

/// The settings word every selected sprite shares, or none.
pub fn common_settings(view: &LevelView, rows: &Rows) -> Option<u32> {
    let sprites = &view.snapshot.course.sprites;
    let first = sprites.get(*rows.indices.first()?)?.settings;
    rows.indices.iter().all(|&i| sprites.get(i).is_some_and(|s| s.settings == first)).then_some(first)
}

fn first_unused(used: impl Iterator<Item = u8>, start: u8) -> u8 {
    let used: std::collections::BTreeSet<u8> = used.collect();
    (start..=u8::MAX).find(|n| !used.contains(n)).unwrap_or(start)
}

/// The Add button: a new entity of the tab's kind with the first id nothing uses.
pub fn add_edit(snapshot: &LevelSnapshot, tab: Tab) -> Option<LevelEdit> {
    let course = &snapshot.course;
    let mut items = LevelEntityInsertions::default();
    let Tab::Entity(kind) = tab else { return None };
    let name = match kind {
        Kind::Object => {
            items.objects.push(NsmbLevelObject { width: 1, height: 1, ..Default::default() });
            "Add object"
        }
        Kind::Sprite => {
            items.sprites.push(bigstar_level::SpriteInsertion { value: NsmbLevelSprite::default(), hash: None });
            "Add sprite"
        }
        Kind::Entrance => {
            items.entrances.push(NsmbLevelEntrance { id: first_unused(course.entrances.iter().map(|e| e.id), 0), ..Default::default() });
            "Add entrance"
        }
        Kind::View => {
            let id = first_unused(course.views.iter().map(|v| v.id), 0);
            items.views.push(NsmbLevelView { width: 256, height: 192, id, ..Default::default() });
            "Add view"
        }
        Kind::Zone => {
            items.zones.push(NsmbLevelZone { width: 256, height: 192, id: first_unused(course.zones.iter().map(|z| z.id), 0), ..Default::default() });
            "Add zone"
        }
        Kind::Path | Kind::ProgressPath => {
            let progress = kind == Kind::ProgressPath;
            let id = first_unused(paths(snapshot, kind).iter().map(|p| p.id), if progress { 90 } else { 0 });
            let path = NsmbLevelPath { id, nodes: vec![NsmbLevelPathNode::default()], ..Default::default() };
            if progress {
                items.progress_paths.push(path);
                "Add progress path"
            } else {
                items.paths.push(path);
                "Add path"
            }
        }
    };
    Some(LevelEdit::Add { items, name: name.to_owned() })
}

/// The Delete button: the selected items of the tab's kind.
pub fn delete_edit(tab: Tab, rows: &Rows) -> Option<LevelEdit> {
    let Tab::Entity(kind) = tab else { return None };
    if rows.indices.is_empty() {
        return None;
    }
    let mut selection = LevelEntitySelection::default();
    let (list, single) = match kind {
        Kind::Object => (&mut selection.objects, "Delete object"),
        Kind::Sprite => (&mut selection.sprites, "Delete sprite"),
        Kind::Entrance => (&mut selection.entrances, "Delete entrance"),
        Kind::View => (&mut selection.views, "Delete view"),
        Kind::Zone => (&mut selection.zones, "Delete zone"),
        Kind::Path => (&mut selection.paths, "Delete path"),
        Kind::ProgressPath => (&mut selection.progress_paths, "Delete progress path"),
    };
    list.extend(&rows.indices);
    let name = if rows.multi() { "Delete selection" } else { single };
    Some(LevelEdit::Remove { selection, name: name.to_owned() })
}

/// Add Node: a copy of the last node, a tile to the right.
pub fn add_node_edit(snapshot: &LevelSnapshot, tab: Tab, rows: &Rows) -> Option<LevelEdit> {
    let Tab::Entity(kind @ (Kind::Path | Kind::ProgressPath)) = tab else { return None };
    let index = *rows.indices.first()?;
    let mut path = paths(snapshot, kind).get(index)?.clone();
    let mut node = path.nodes.last().cloned().unwrap_or_default();
    node.x = node.x.saturating_add(16);
    path.nodes.push(node);
    Some(path_update(kind, index, path))
}

pub fn delete_node_edit(snapshot: &LevelSnapshot, tab: Tab, rows: &Rows) -> Option<LevelEdit> {
    let Tab::Entity(kind @ (Kind::Path | Kind::ProgressPath)) = tab else { return None };
    let index = *rows.indices.first()?;
    let mut path = paths(snapshot, kind).get(index)?.clone();
    if rows.node >= path.nodes.len() {
        return None;
    }
    path.nodes.remove(rows.node);
    Some(path_update(kind, index, path))
}

/// The sprite picker: every type the ROM's actor table maps to a named actor, by name.
pub fn sprite_types(view: &LevelView) -> Vec<(u16, String)> {
    let catalog = &view.art.actors.catalog;
    let mut entries: Vec<(u16, String)> = (0..catalog.sprite_object_ids.len())
        .filter_map(|t| u16::try_from(t).ok())
        .filter_map(|t| view.art.actors.actor(t, None).filter(|a| !a.definition.name.is_empty()).map(|a| (t, a.definition.name.clone())))
        .collect();
    entries.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    entries
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bigstar_formats::{NsmbLevelObject, NsmbLevelStageObject};
    use bigstar_level::{SpriteDefinition, SpriteFieldChoice, SpriteFieldDefinition};
    use bigstar_protocol::{ActorCatalog, ActorInfo, LevelEdit};

    use super::*;
    use crate::level_view::tests_support::snapshot;
    use crate::level_view::{ActorIndex, Gesture, Handle, Item};

    fn field(key: &str, label: &str, bit_offset: u8, bit_width: u8) -> SpriteFieldDefinition {
        SpriteFieldDefinition { key: key.to_owned(), label: label.to_owned(), bit_offset, bit_width, ..Default::default() }
    }

    fn choices(values: &[(u32, &str)]) -> Vec<SpriteFieldChoice> {
        values.iter().map(|(value, label)| SpriteFieldChoice { value: *value, label: (*label).to_owned() }).collect()
    }

    /// A level with one sprite of type 7 carrying `settings`, read by actor 42 declaring `fields`; type 9 is actor 43.
    fn sprite_view(settings: u32, fields: Vec<SpriteFieldDefinition>) -> LevelView {
        let mut level = snapshot();
        level.course.sprites = vec![NsmbLevelStageObject { id: 7, x: 10, y: 10, settings, ..Default::default() }];
        let mut view = LevelView::new(level);
        let definition = |object_id, name: &str, fields| SpriteDefinition {
            object_id,
            name: name.to_owned(),
            category: "Enemies".to_owned(),
            notes: "Spawned by Chain Chomp/Log.".to_owned(),
            fields,
            ..Default::default()
        };
        let mut ids = vec![0; 10];
        ids[7] = 42;
        ids[9] = 43;
        let actor = |definition| ActorInfo { definition, has_renderer: false, init_error: None };
        let catalog = ActorCatalog {
            generation: 0,
            actors: vec![actor(definition(42, "Synthetic actor", fields)), actor(definition(43, "Another actor", Vec::new()))],
            sprite_object_ids: ids,
        };
        view.art.actors = ActorIndex::new(Arc::new(catalog));
        view.selection = vec![Item::new(Kind::Sprite, 0)];
        view.sync_panel();
        view
    }

    fn shown(view: &LevelView) -> Rows {
        rows(view, &view.panel, None)
    }

    fn written(view: &LevelView, row: usize, value: i64) -> LevelEntityUpdates {
        match apply(view, &view.panel, &shown(view), row, value) {
            Some(LevelEdit::Update { updates, .. }) => updates,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn values_read_and_write_in_either_base() {
        assert_eq!((format_value(31, true), format_value(-31, true), format_value(31, false)), ("0x1F".into(), "-0x1F".into(), "31".into()));
        assert_eq!((parse_value("0x1f", true), parse_value("1F", true), parse_value("-0X1f", true)), (Some(31), Some(31), Some(-31)));
        assert_eq!(parse_value("1f", false), None);
        assert_eq!(format_settings(0x1234_5678), "12 34 56 78");
        assert_eq!((parse_settings("22 33 44 55"), parse_settings("22 33 44")), (Some(0x2233_4455), None));
    }

    #[test]
    fn a_multi_object_edit_is_one_step_and_disagreeing_rows_are_undecided() {
        let mut level = snapshot();
        level.objects = vec![
            NsmbLevelObject { object_id: 1, x: 1, y: 1, width: 1, height: 1, ..Default::default() },
            NsmbLevelObject { object_id: 2, x: 2, y: 1, width: 1, height: 1, ..Default::default() },
        ];
        let mut view = LevelView::new(level);
        view.selection = vec![Item::new(Kind::Object, 0), Item::new(Kind::Object, 1)];
        view.sync_panel();
        let rows = shown(&view);
        assert_eq!((rows.rows[0].value, rows.rows[4].value), (None, Some(1)), "ids differ, widths agree");
        let Some(LevelEdit::Update { updates, name }) = apply(&view, &view.panel, &rows, 0, 99) else { panic!() };
        assert_eq!(name, "Edit selection");
        assert_eq!(updates.objects.iter().map(|u| (u.index, u.value.object_id, u.value.x)).collect::<Vec<_>>(), [(0, 99, 1), (1, 99, 2)]);
    }

    #[test]
    fn sprite_definitions_drive_the_fields() {
        let view = sprite_view(0xffff_fff0, vec![field("speed", "Speed", 4, 3)]);
        let rows = shown(&view);
        assert_eq!(rows.rows[5].label, "Speed");
        let updates = written(&view, 5, 2);
        assert_eq!(updates.sprites[0].value.settings, 0xffff_ffa0);
    }

    #[test]
    fn sprite_settings_survive_the_fields_that_read_them() {
        let mut view = sprite_view(0x1234_5678, vec![field("speed", "Speed", 4, 3)]);
        view.snapshot.course.sprites[0].events = [0xab, 0xcd];
        let rows = shown(&view);
        assert_eq!(common_settings(&view, &rows).map(format_settings).as_deref(), Some("12 34 56 78"));
        // A field edit lands in the raw value, and only in the bits it owns.
        assert_eq!(written(&view, 5, 2).sprites[0].value.settings, 0x1234_5628);
        // Changing the type keeps the settings; the sprite is now read by an actor that declares nothing.
        let retyped = written(&view, 0, 9);
        assert_eq!((retyped.sprites[0].value.id, retyped.sprites[0].value.settings), (9, 0x1234_5678));
        view.snapshot.course.sprites[0].id = 9;
        assert_eq!(shown(&view).rows.len(), SPRITE_FIELD_ROW);
        // The raw box writes the settings word and leaves the event bytes to the rows that name them.
        let Some(LevelEdit::Update { updates, name }) = apply_settings(&view, &shown(&view), 0x2233_4455) else { panic!() };
        assert_eq!((updates.sprites[0].value.settings, updates.sprites[0].value.events, name.as_str()), (0x2233_4455, [0xab, 0xcd], "Edit sprite"));
        let rows = shown(&view);
        assert_eq!((rows.rows[3].label.as_str(), rows.rows[3].value), ("Target event", Some(0xab)));
        assert_eq!((rows.rows[4].label.as_str(), rows.rows[4].value), ("Trigger event", Some(0xcd)));
    }

    #[test]
    fn a_view_shows_and_edits_the_camera_settings_it_names() {
        let mut level = snapshot();
        level.course.views =
            vec![NsmbLevelView { camera_settings_id: 2, ..Default::default() }, NsmbLevelView { id: 1, camera_settings_id: 5, ..Default::default() }];
        level.course.cameras = vec![NsmbLevelCamera { id: 2, top_offset: -8, bottom_offset: 16, ..Default::default() }];
        let mut view = LevelView::new(level);
        view.panel.tab = Tab::Entity(Kind::View);
        let shown = rows(&view, &view.panel, None);
        let row = |label: &str| shown.rows.iter().position(|row| row.label == label).unwrap();
        assert_eq!(row("Camera top offset"), VIEW_CAMERA_ROW);
        assert_eq!(shown.rows[row("Camera bottom offset")].value, Some(16));
        // Changing one writes the block the view names and leaves the view alone.
        match apply(&view, &view.panel, &shown, row("Camera bottom offset"), -32) {
            Some(LevelEdit::SetCameras { cameras, .. }) => {
                assert_eq!(cameras, [NsmbLevelCamera { id: 2, top_offset: -8, bottom_offset: -32, ..Default::default() }])
            }
            other => panic!("no camera edit: {other:?}"),
        }
        // A view naming settings that do not exist reads the first, as the game does, and gets its own on an edit.
        view.panel.item = 1;
        let shown = rows(&view, &view.panel, None);
        assert_eq!(shown.rows[VIEW_CAMERA_ROW].value, Some(-8));
        assert!(shown.rows[VIEW_CAMERA_ROW].note.contains("gives the view its own"));
        match apply(&view, &view.panel, &shown, VIEW_CAMERA_ROW, 4) {
            Some(LevelEdit::SetCameras { cameras, .. }) => {
                assert_eq!(cameras.len(), 2);
                assert_eq!(cameras[1], NsmbLevelCamera { id: 5, top_offset: 4, bottom_offset: 16, ..Default::default() });
            }
            other => panic!("no camera edit: {other:?}"),
        }
    }

    #[test]
    fn actor_notes_stay_on_the_field_they_belong_to() {
        let mut speed = field("speed", "Speed", 4, 3);
        speed.choices = choices(&[(1, "Slow"), (2, "Fast")]);
        speed.notes = "8 is used by ones thrown out of the Monty Tank.".to_owned();
        let mut view = sprite_view(0, vec![speed]);
        let rows = shown(&view);
        assert_eq!((rows.rows[0].label.as_str(), rows.rows[0].note.as_str()), ("Object ID", "Spawned by Chain Chomp/Log."));
        assert!(rows.rows[5].note.starts_with("8 is used by ones thrown out of the Monty Tank."));
        assert_eq!(rows.actor.as_ref().map(|a| (a.definition.name.as_str(), a.definition.category.as_str())), Some(("Synthetic actor", "Enemies")));
        assert_eq!(rows.rows[5].editor, Editor::Choice(vec![(1, "Slow".to_owned()), (2, "Fast".to_owned())]));
        // The settings are zero and this actor does not name zero, so the row says so rather than snapping elsewhere.
        assert_eq!((rows.rows[5].value, rows.rows[5].representable), (Some(0), false));
        assert!(rows.rows[5].note.contains("does not name the value 0"));
        // A row that stops carrying a note carries none.
        view.panel_tab(Tab::Entity(Kind::Zone));
        assert!(shown(&view).rows.iter().all(|r| r.note.is_empty()));
    }

    #[test]
    fn a_choice_writes_the_sparse_value_it_stands_for() {
        let mut item = field("item", "Item", 8, 5);
        item.choices = choices(&[(0, "A"), (5, "B"), (25, "C")]);
        let view = sprite_view(0xff, vec![item]);
        assert_eq!(written(&view, 5, 25).sprites[0].value.settings, (25 << 8) | 0xff);
    }

    #[test]
    fn a_checkbox_toggles_only_the_bits_it_owns() {
        let on = SpriteFieldDefinition { checkbox_value: 8, ..field("on", "On", 4, 4) };
        let mut view = sprite_view(0x0f0f, vec![on]);
        let rows = shown(&view);
        assert_eq!((&rows.rows[5].editor, rows.rows[5].value, rows.rows[5].label.as_str()), (&Editor::Boolean, Some(0), "On"));
        assert_eq!(written(&view, 5, 1).sprites[0].value.settings, 0x0f8f);
        view.snapshot.course.sprites[0].settings = 0x0f8f;
        assert_eq!(shown(&view).rows[5].value, Some(1));
        assert_eq!(written(&view, 5, 0).sprites[0].value.settings, 0x0f0f);
    }

    #[test]
    fn values_outside_what_an_editor_says_are_shown_rather_than_rewritten() {
        let mut pick = field("pick", "Pick", 0, 3);
        pick.choices = choices(&[(0, "A"), (1, "B"), (2, "C")]);
        let view = sprite_view(4, vec![pick, field("speed", "Speed", 8, 4)]);
        let rows = shown(&view);
        assert_eq!((rows.rows[5].value, rows.rows[5].representable), (Some(4), false));
        // Turning the field beside it does not carry the unnamed value away.
        assert_eq!(written(&view, 6, 3).sprites[0].value.settings, (3 << 8) | 4);

        // A four-bit field turned on by 1 can hold 5: undecided, and the first click decides it.
        let flag = SpriteFieldDefinition { checkbox_value: 1, ..field("flag", "Flag", 0, 4) };
        let view = sprite_view(5, vec![flag]);
        let rows = shown(&view);
        assert_eq!((rows.rows[5].value, rows.rows[5].representable), (Some(5), false));
        assert_eq!(written(&view, 5, 1).sprites[0].value.settings, 1);
        assert_eq!(written(&view, 5, 0).sprites[0].value.settings, 0);
    }

    #[test]
    fn the_sprite_picker_lists_types_by_name() {
        let view = sprite_view(0, Vec::new());
        assert_eq!(sprite_types(&view), [(9, "Another actor".to_owned()), (7, "Synthetic actor".to_owned())]);
        let retyped = written(&view, 0, 9);
        assert_eq!(retyped.sprites[0].value.id, 9);
        // An id no actor claims still reads and edits as a number, and names no actor.
        let mut view = view;
        view.snapshot.course.sprites[0].id = 512;
        assert_eq!(shown(&view).actor, None);
        assert_eq!(shown(&view).rows[0].value, Some(512));
    }

    #[test]
    fn rows_stay_at_their_index_and_follow_a_canvas_drag() {
        let flag = SpriteFieldDefinition { checkbox_value: 1, ..field("flag", "Flag", 0, 1) };
        let mut pick = field("pick", "Pick", 4, 2);
        pick.choices = choices(&[(0, "A"), (1, "B")]);
        let mut view = sprite_view(0, vec![flag, pick, field("speed", "Speed", 8, 4)]);
        let rows = shown(&view);
        assert!(matches!(
            (&rows.rows[5].editor, &rows.rows[6].editor, &rows.rows[7].editor),
            (Editor::Boolean, Editor::Choice(_), Editor::Number { .. })
        ));
        // X and Y are still rows 1 and 2, which is what a canvas drag moves.
        view.gesture = Some(Gesture::Move { delta: egui::Vec2::new(16.0, 32.0), submitted: false });
        let rows = shown(&view);
        assert_eq!((rows.rows[1].value, rows.rows[2].value), (Some(11), Some(12)));
        assert_eq!(written(&view, 7, 3).sprites[0].value.settings, 3 << 8);
    }

    #[test]
    fn a_resize_shows_in_the_panel_before_it_is_committed() {
        let mut level = snapshot();
        level.course.views = vec![bigstar_formats::NsmbLevelView { x: 320, y: 320, width: 256, height: 192, ..Default::default() }];
        let mut view = LevelView::new(level);
        view.selection = vec![Item::new(Kind::View, 0)];
        view.sync_panel();
        view.gesture = Some(Gesture::Resize { handle: Handle::BottomRight, delta: egui::Vec2::new(48.0, 48.0), submitted: false });
        let rows = shown(&view);
        assert_eq!((rows.rows[0].value, rows.rows[2].value, rows.rows[3].value), (Some(320), Some(304), Some(240)));
        assert_eq!(view.snapshot.course.views[0].width, 256, "nothing is written until the drag ends");
    }

    #[test]
    fn the_panel_and_the_canvas_selection_follow_each_other() {
        let mut level = snapshot();
        level.objects.push(NsmbLevelObject { object_id: 2, width: 1, height: 1, ..Default::default() });
        let mut view = LevelView::new(level);
        assert_eq!(view.panel.tab, Tab::Course);
        // A canvas pick shows its kind, and makes it what a right-click creates.
        view.selection = vec![Item::new(Kind::Zone, 0)];
        view.sync_panel();
        assert_eq!((view.panel.tab, view.placement.kind), (Tab::Entity(Kind::Zone), Kind::Zone));
        // Picking an item in the panel selects it on the canvas.
        view.panel_tab(Tab::Entity(Kind::Object));
        assert_eq!(view.selection, [Item::new(Kind::Object, 0)]);
        view.panel_item(1);
        assert_eq!(view.selection, [Item::new(Kind::Object, 1)]);
        // The object picked is the one a right-click places next.
        assert_eq!((view.placement.kind, view.placement.object_id), (Kind::Object, 2));
        // A path is picked down to its node.
        view.panel_tab(Tab::Entity(Kind::Path));
        view.panel_node(1);
        assert_eq!(view.selection, [Item::node(Kind::Path, 0, 1)]);
        // So is one picked on the canvas.
        view.placement.set_sprite(5);
        view.selection = vec![Item::new(Kind::Object, 1)];
        view.sync_panel();
        assert_eq!((view.placement.kind, view.placement.object_id), (Kind::Object, 2));
        view.panel_tab(Tab::Entity(Kind::Path));
        view.panel_node(1);
        // A sprite does the same for sprites, even from within a mixed selection, without changing the kind.
        view.snapshot.course.sprites[0].id = 9;
        view.selection = vec![Item::new(Kind::Zone, 0), Item::new(Kind::Sprite, 0)];
        view.sync_panel();
        assert_eq!((view.placement.kind, view.placement.sprite_type, view.placement.object_id), (Kind::Path, 9, 2));
        // A mixed canvas selection leaves the panel where it was; an empty one shows the course.
        view.selection = vec![Item::new(Kind::Object, 0), Item::new(Kind::Zone, 0)];
        view.sync_panel();
        assert_eq!(view.panel.tab, Tab::Entity(Kind::Path));
        view.selection.clear();
        view.sync_panel();
        assert_eq!(view.panel.tab, Tab::Course);
        // The course tab edits the tileset references; the banks name their overlay.
        let Some(LevelEdit::SetTilesetReferences { references }) = apply(&view, &view.panel, &shown(&view), 2, 7) else { panic!() };
        assert_eq!(references.background_screen_id, 7);
        view.panel_tab(Tab::Banks);
        view.snapshot.course.object_banks[0] = 3;
        let mut overlays = [[0; 10]; 16];
        overlays[0][2] = 77;
        assert_eq!(rows(&view, &view.panel, Some(&overlays)).rows[0].label, "Bank ID (overlay 77)");
        assert_eq!(apply(&view, &view.panel, &shown(&view), 0, 4), Some(LevelEdit::SetObjectBank { slot: 0, value: 4 }));
    }

    #[test]
    fn the_list_goes_by_id_and_opens_a_selected_path_onto_its_nodes() {
        let mut level = snapshot();
        level.course.paths.insert(0, NsmbLevelPath { id: 4, nodes: vec![Default::default()], unknown: 0 });
        level.course.entrances =
            vec![NsmbLevelEntrance { id: 2, kind: 3, x: 16, y: 32, ..Default::default() }, NsmbLevelEntrance { kind: 99, ..Default::default() }];
        let mut view = LevelView::new(level);
        // Path 0 is held second and listed first; neither is open until something of it is selected.
        assert_eq!(list_lines(&view, Kind::Path), [Item::new(Kind::Path, 1), Item::new(Kind::Path, 0)]);
        view.selection = vec![Item::node(Kind::Path, 1, 1)];
        let lines = list_lines(&view, Kind::Path);
        assert_eq!(lines, [Item::new(Kind::Path, 1), Item::node(Kind::Path, 1, 0), Item::node(Kind::Path, 1, 1), Item::new(Kind::Path, 0)]);
        let labels: Vec<String> = lines.iter().map(|line| line_label(&view, *line)).collect();
        assert_eq!(labels, ["Path 0 · 2 nodes", "Node 0 (300, 100)", "Node 1 (340, 100)", "Path 4 · 1 node"]);

        let labels: Vec<String> = list_lines(&view, Kind::Entrance).iter().map(|line| line_label(&view, *line)).collect();
        assert_eq!(labels, ["Entrance 0 · Type 99 (0, 0)", "Entrance 2 · Pipe up (16, 32)"]);
        assert_eq!(line_label(&view, Item::new(Kind::Object, 0)), "Object 0:1 (2, 3) 4×1");
        assert_eq!(line_label(&view, Item::new(Kind::Sprite, 0)), "Actor 1 (6, 4)");
        assert_eq!(line_label(&view, Item::new(Kind::Zone, 0)), "Zone 0 (0, 0) 400×200");
    }

    #[test]
    fn an_entrance_names_its_type_and_ticks_its_flags() {
        let mut level = snapshot();
        level.course.entrances = vec![
            NsmbLevelEntrance { kind: 4, flags: 0x88, unknown_11: 3, ..Default::default() },
            NsmbLevelEntrance { kind: 30, flags: 0x08, ..Default::default() },
        ];
        let mut view = LevelView::new(level);
        view.selection = vec![Item::new(Kind::Entrance, 0)];
        view.sync_panel();
        let rows = shown(&view);
        let Editor::Choice(types) = &rows.rows[ENTRANCE_TYPE_ROW].editor else { panic!() };
        assert_eq!((types.len(), types[4].1.as_str(), rows.rows[ENTRANCE_TYPE_ROW].value), (26, "Pipe down", Some(4)));
        let flags: Vec<(&str, Option<i64>)> =
            rows.rows[ENTRANCE_FLAG_ROW..ENTRANCE_FLAG_ROW + 8].iter().map(|r| (r.label.as_str(), r.value)).collect();
        assert_eq!((flags[0], flags[3], flags[7]), (("Bottom screen", Some(0)), ("Connected pipe", Some(1)), ("Exit only", Some(1))));
        assert!(rows.rows[ENTRANCE_FLAG_ROW..ENTRANCE_FLAG_ROW + 8].iter().all(|r| r.editor == Editor::Boolean));
        assert_eq!((rows.rows[22].label.as_str(), rows.rows[22].value, rows.rows.len()), ("Unknown 0x11", Some(3), 23));
        // A tick writes its own bit and nothing beside it.
        assert_eq!(written(&view, ENTRANCE_FLAG_ROW + 7, 0).entrances[0].value.flags, 0x08);
        assert_eq!(written(&view, ENTRANCE_FLAG_ROW, 1).entrances[0].value.flags, 0x89);
        assert_eq!(written(&view, ENTRANCE_TYPE_ROW, 16).entrances[0].value.kind, 16);
        assert_eq!(written(&view, 11, 255).entrances[0].value.view_id, 255);
        // Across a selection a flag reads only where every entrance agrees, and a tick lands on each.
        view.selection.push(Item::new(Kind::Entrance, 1));
        view.sync_panel();
        let rows = shown(&view);
        assert_eq!((rows.rows[ENTRANCE_FLAG_ROW + 3].value, rows.rows[ENTRANCE_FLAG_ROW + 7].value), (Some(1), None));
        assert_eq!(written(&view, ENTRANCE_FLAG_ROW + 7, 1).entrances.iter().map(|u| u.value.flags).collect::<Vec<_>>(), [0x88, 0x88]);
        // A type past the last name is shown as what it is.
        view.selection = vec![Item::new(Kind::Entrance, 1)];
        view.sync_panel();
        assert!(!shown(&view).rows[ENTRANCE_TYPE_ROW].representable);
    }

    #[test]
    fn add_delete_and_nodes_are_named_edits() {
        let mut level = snapshot();
        level.course.paths[0].id = 0;
        level.course.progress_paths = vec![NsmbLevelPath { id: 90, nodes: vec![Default::default()], unknown: 0 }];
        let mut view = LevelView::new(level);
        let Some(LevelEdit::Add { items, name }) = add_edit(&view.snapshot, Tab::Entity(Kind::Path)) else { panic!() };
        assert_eq!((items.paths[0].id, items.paths[0].nodes.len(), name.as_str()), (1, 1, "Add path"));
        let Some(LevelEdit::Add { items, .. }) = add_edit(&view.snapshot, Tab::Entity(Kind::ProgressPath)) else { panic!() };
        assert_eq!(items.progress_paths[0].id, 91, "progress paths count from 90");
        let Some(LevelEdit::Add { items, .. }) = add_edit(&view.snapshot, Tab::Entity(Kind::Zone)) else { panic!() };
        assert_eq!((items.zones[0].id, items.zones[0].width), (1, 256), "zone 0 is taken");
        assert_eq!(add_edit(&view.snapshot, Tab::Course), None);

        view.panel_tab(Tab::Entity(Kind::Path));
        let Some(LevelEdit::Update { updates, name }) = add_node_edit(&view.snapshot, view.panel.tab, &shown(&view)) else { panic!() };
        assert_eq!((updates.paths[0].value.nodes.len(), updates.paths[0].value.nodes[2].x, name.as_str()), (3, 356, "Edit path"));
        view.panel_node(1);
        let Some(LevelEdit::Update { updates, .. }) = delete_node_edit(&view.snapshot, view.panel.tab, &shown(&view)) else { panic!() };
        assert_eq!(updates.paths[0].value.nodes.iter().map(|n| n.x).collect::<Vec<_>>(), [300]);
        let Some(LevelEdit::Remove { selection, name }) = delete_edit(view.panel.tab, &shown(&view)) else { panic!() };
        assert_eq!((selection.paths, name.as_str()), (vec![0], "Delete path"));
    }
}
