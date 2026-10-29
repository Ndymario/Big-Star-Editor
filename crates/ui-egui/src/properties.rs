//! The properties panel as data: which rows a tab shows for the selection, what editor each row wears, what it says
//! about itself, and what turning one writes back. A port of `LevelPropertiesWidget`'s `refresh` and `applyFields`;
//! `properties_ui` only draws these rows.

use bigstar_core::Fx32;
use bigstar_formats::{
    NsmbLevelCamera, NsmbLevelCourse, NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelScreen, NsmbLevelSprite,
    NsmbLevelView, NsmbLevelZone,
};
use bigstar_level::{
    BackgroundLayer, IndexedLevelValue, LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, LevelTilesetReferences, ScreenBlock,
    SpriteFieldDefinition, read_sprite_field, sprite_field_max, sprite_field_min, write_sprite_field,
};
use bigstar_protocol::{ActorInfo, LevelEdit, LevelSnapshot, ObjectBankOverlays};

use crate::level_view::{Gesture, Item, Kind, LevelView, TILE};

/// The panel's side tabs, in the order they are shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tab {
    Course,
    Banks,
    /// The background, tileset and foreground records the views pick from.
    Layers,
    Cameras,
    Entity(Kind),
}

pub const TABS: [(Tab, &str); 11] = [
    (Tab::Course, "Course"),
    (Tab::Banks, "Banks"),
    (Tab::Layers, "Layers"),
    (Tab::Cameras, "Cameras"),
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
    /// The panel shows fields the game ignores or that are best left alone: the Course tab its raw settings and all three
    /// of the header's graphics sets, rather than the first with its palettes; a view its three layer entries, rather than whether it
    /// rotates; and the Layers tab its Add and Delete, and a layer entry its ID, screen, palette and unused fields, with
    /// a tileset's rotation and scroll speeds.
    pub advanced: bool,
    /// Every drop-down a spin box of the number it writes, for values it does not name.
    pub raw: bool,
    /// A field being dragged: shown at this value and written when the drag ends.
    pub dragging: Option<(usize, i64)>,
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
            advanced: false,
            raw: false,
            dragging: None,
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
    /// The values an actor names. They are sparse, so each entry carries the number it stands for. The range is the
    /// field's, which raw input types into.
    Choice {
        choices: Vec<(i64, String)>,
        min: i64,
        max: i64,
        /// The hint of a box above the list that filters it by what is typed, for a list too long to scroll through.
        filter: Option<&'static str>,
    },
    /// On or off, whatever value turning it on writes.
    Boolean,
    /// A value of this many bytes, typed as their hex digits.
    Hex {
        bytes: usize,
    },
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
    /// A title the panel sets apart above the row, opening the rows under it up to the next title. Empty for none.
    pub heading: String,
    /// What the actor says about the field, shown beside it when there is anything to say.
    pub note: String,
    /// Something wrong with the value, shown as a warning sign beside the note. Empty for none.
    pub warning: String,
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
            heading: String::new(),
            note: String::new(),
            warning: String::new(),
            representable: true,
        }
    }

    /// Turns a number row into a drop-down of these, keeping its range for raw input.
    fn choose(&mut self, choices: Vec<(i64, String)>) {
        self.choose_filtered(choices, None);
    }

    /// As [`Row::choose`], with a box above the list that filters it, `filter` its hint.
    fn choose_filtered(&mut self, choices: Vec<(i64, String)>, filter: Option<&'static str>) {
        let (min, max) = match self.editor {
            Editor::Number { min, max } | Editor::Choice { min, max, .. } => (min, max),
            Editor::Boolean => (0, 1),
            Editor::Hex { bytes } => (0, (1i64 << (8 * bytes)) - 1),
        };
        self.editor = Editor::Choice { choices, min, max, filter };
    }
}

pub use crate::controls::format_value;

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
        Tab::Layers => ScreenBlock::ALL.iter().map(|block| block.records(course).len()).sum(),
        Tab::Cameras => course.cameras.len(),
        Tab::Entity(Kind::Object) => snapshot.objects.len(),
        Tab::Entity(Kind::Sprite) => course.sprites.len(),
        Tab::Entity(Kind::Entrance) => course.entrances.len(),
        Tab::Entity(Kind::View) => course.views.len(),
        Tab::Entity(Kind::Zone) => course.zones.len(),
        Tab::Entity(Kind::Path) => course.paths.len(),
        Tab::Entity(Kind::ProgressPath) => course.progress_paths.len(),
    }
}

/// The Layers tab's item as the block it is in and its place there: the backgrounds, then the tilesets, then
/// the foregrounds.
pub fn screen_record(course: &NsmbLevelCourse, item: usize) -> Option<(ScreenBlock, usize)> {
    let mut first = 0;
    for block in ScreenBlock::ALL {
        let count = block.records(course).len();
        if item < first + count {
            return Some((block, item - first));
        }
        first += count;
    }
    None
}

fn paths(snapshot: &LevelSnapshot, kind: Kind) -> &Vec<NsmbLevelPath> {
    if kind == Kind::ProgressPath { &snapshot.course.progress_paths } else { &snapshot.course.paths }
}

/// What each of the game's 76 level tilesets looks like, as NSMB Editor names them. Several look alike; the number
/// tells them apart. Its tables have room for no more.
pub(crate) const TILESET_NAMES: [&str; 76] = [
    "Grassland",
    "Castle",
    "Desert",
    "Underground (with ice)",
    "Sky/mushrooms (World 7 style)",
    "Underwater",
    "Desert",
    "Volcano (brown)",
    "Grassland",
    "Beach (blue)",
    "Beach (blue)",
    "Forest",
    "Grassland",
    "Sky/mushrooms (with flowers and bushes)",
    "Sky/mushrooms (with flowers and bushes)",
    "Volcano (black)",
    "Volcano (black 2) [unused]",
    "Snow (with ice)",
    "Snow",
    "Forest",
    "Oasis in desert",
    "Bonus room [unused]",
    "Volcano (black 2) [unused]",
    "Ghost house",
    "Cliffs",
    "Cliffs",
    "Snow (with ice)",
    "Grassland",
    "Snow (with ice)",
    "Factory (W7-A)",
    "Sewer (W2-3)",
    "Volcano (yellow)",
    "Castle",
    "Sky/mushrooms (with flowers and bushes)",
    "Grassland",
    "Underground",
    "Underground (dark blue) [unused]",
    "Dark world (purple)",
    "Dark world (brown)",
    "Tower",
    "Ghost house exit",
    "Final castle",
    "Toad house 1",
    "Toad house 2",
    "Toad house 3",
    "Final castle",
    "Toad house 4",
    "Bowser Jr. battle",
    "Beach (blue)",
    "Boss battle",
    "Sky/mushrooms (with flowers and bushes)",
    "Forest",
    "Desert",
    "Desert",
    "Forest",
    "Final boss battle",
    "Volcano (black)",
    "Underwater",
    "Light grey castle (W8 castle 1)",
    "Sky/mushrooms (yellowish)",
    "Beach (turquoise, without moss)",
    "Lakithunder and Monty Tank battles",
    "Boss battle",
    "Mummy-Pokey battle",
    "Oasis in desert",
    "Desert",
    "Oasis in desert",
    "Bonus room [unused]",
    "Bonus room [unused]",
    "Bonus room [unused]",
    "Bonus room [unused]",
    "Bonus room [unused]",
    "Bonus room [unused]",
    "Underground (gold) [unused]",
    "Bonus room",
    "Grassland",
];

/// A tileset picked anew brings its own colours, as NSMB Editor has it: the game takes them from the tileset block,
/// which the level must name the tileset in again. Another palette can be chosen after.
pub(crate) fn follow_tileset(references: &mut LevelTilesetReferences, before: LevelTilesetReferences) {
    if references.tileset_id != before.tileset_id {
        references.tileset_palette_id = references.tileset_id;
    }
}

/// The tileset IDs a level can choose, each with its name, after none at 65535, which every load of a tileset's files
/// skips.
pub(crate) fn tileset_choices() -> Vec<(i64, String)> {
    let named = (0i64..).zip(TILESET_NAMES).map(|(id, name)| (id, name.to_owned()));
    std::iter::once((i64::from(u16::MAX), "None".to_owned())).chain(named).collect()
}

/// Why the level's tileset should not be none, though every load of its files skips it. See `docs/no-tileset.md`.
const NO_TILESET: &str = "The game animates the tileset every frame by this number without checking for none, so it calls whatever lies far past its table, which can crash.";

/// What each object bank of a slot holds, as NSMB Editor names them: `BANK_NAMES[slot][bank - 1]`, empty where the
/// bank holds nothing. The game loads banks into slots 0 to 9 and 15.
const BANK_NAMES: [(usize, [&str; 10]); 11] = [
    (0, ["Pipe Enemy Spawner", "Enemy Desert", "Enemy Castle", "Enemy Water", "Kab-omb", "Swooper", "Bowser Jr", "", "", ""]),
    (1, ["Pipe Piranha", "Wiggler", "Whomp, Fence Koopa", "Skeeter", "Snailicorn", "", "Crowber", "", "Boss Common", ""]),
    (2, ["Hammer Bro", "Chain Chomp", "Spike Bass", "Corked Pipe", "Cloud Platform", "", "", "", "", ""]),
    (3, ["Lakitu", "Big Cheep Cheep", "Beetles, Amp", "Blockhopper", "Flame Chomp", "Ghost House", "", "", "", ""]),
    (4, ["Enemy Common", "Final Boss", "", "", "Cheep Chomp", "Dorrie", "Lava Objects", "Screw Mushroom", "Jumping Cheep Cheep", ""]),
    (5, ["", "Squiggler", "Sushi", "Unagi", "Scuttlebug, Money Bag", "Ground Piranha", "Snow Spike", "", "", ""]),
    (6, ["Desert Objects", "Water Objects", "Platforms 1", "Platforms 2", "Mushrooms", "Wavy Goo", "", "", "", ""]),
    (7, ["Platforms 3", "Platforms 4", "Platforms 5", "Bouncing Blocks", "Castle Platforms", "Burner, Ball N Chain", "", "", "", ""]),
    (8, ["Scale Lift", "Rope", "Broozer", "Snake Block", "Bumper", "Expandable Block", "", "", "", ""]),
    (9, ["Snow Common", "Sewer Common", "Ghost House 2", "Giant Wiggler", "Skewer", "Spiked Ball", "", "Toad House", "", ""]),
    (
        15,
        [
            "Stage Objects",
            "Bowser",
            "Mega Goomba",
            "Petey Piranha",
            "Mummipokey",
            "Lakithunder",
            "Cheepskipper",
            "Monty Tank",
            "Versus, Tower Lift",
            "",
        ],
    ),
];

/// The most actors a bank's entry names before it only counts the rest.
const BANK_ACTORS_NAMED: usize = 3;

/// The slots the Banks tab shows, one row each: those the game loads banks into, and any other this level sets.
pub fn bank_slots(object_banks: &[u8]) -> Vec<usize> {
    (0..).zip(object_banks).filter(|&(slot, &bank)| bank != 0 || BANK_NAMES.iter().any(|(named, _)| *named == slot)).map(|(slot, _)| slot).collect()
}

/// The names of the actors that need bank `bank` of slot `slot`, in type order.
fn bank_actors(view: &LevelView, slot: usize, bank: u8) -> Vec<String> {
    (0u16..)
        .zip(&view.art.actors.catalog.sprite_banks)
        .filter(|(_, needs)| usize::from(needs[0]) == slot && needs[1] == bank)
        .map(|(t, _)| {
            view.art.actors.actor(t, None).map(|a| a.definition.name.clone()).filter(|n| !n.is_empty()).unwrap_or_else(|| format!("Actor {t}"))
        })
        .collect()
}

/// A slot's banks to pick from, each named for what it holds, with the overlay it loads and the actors in it.
fn bank_choices(view: &LevelView, slot: usize, overlays: Option<&ObjectBankOverlays>) -> Vec<(i64, String)> {
    let names = BANK_NAMES.iter().find(|(s, _)| *s == slot).map(|(_, names)| names);
    let mut choices = vec![(0, "None".to_owned())];
    for bank in 1..=10u8 {
        let index = usize::from(bank) - 1;
        let mut text = names.and_then(|names| names.get(index).copied()).filter(|name| !name.is_empty()).unwrap_or("Unused").to_owned();
        // A slot with no banks holds 0xFFFFFFFF there: no overlay to name.
        if let Some(overlay) = overlays.and_then(|table| table.get(slot)?.get(index)).filter(|&&overlay| overlay != u32::MAX) {
            text.push_str(&format!(" (overlay {overlay})"));
        }
        let actors = bank_actors(view, slot, bank);
        if !actors.is_empty() {
            let rest = actors.len().saturating_sub(BANK_ACTORS_NAMED);
            let named: Vec<&str> = actors.iter().take(BANK_ACTORS_NAMED).map(String::as_str).collect();
            text.push_str(&format!(": {}", named.join(", ")));
            if rest != 0 {
                text.push_str(&format!(" and {rest} more"));
            }
        }
        choices.push((i64::from(bank), text));
    }
    choices
}

/// The game's music by sequence number, as NSMB Editor names it. The rest are sound effects, unused or a mod's own.
const MUSIC_NAMES: [(u8, &str); 52] = [
    (0, "MvsL Stage"),
    (1, "Tower"),
    (2, "Starman"),
    (3, "Mega Mario"),
    (4, "End of Level"),
    (5, "Death"),
    (6, "Desert"),
    (7, "Boss"),
    (8, "VS Lose"),
    (9, "Underground"),
    (10, "Bonus room"),
    (11, "Underwater"),
    (12, "Lava"),
    (13, "End Credits"),
    (14, "Beach"),
    (15, "Bowser Jr. Battle"),
    (16, "Ghost House"),
    (17, "Castle"),
    (18, "Timer"),
    (19, "End of game fanfare"),
    (20, "Game Over"),
    (21, "Final Boss"),
    (22, "Boss Beaten"),
    (24, "Mushrooms (athletic)"),
    (25, "Toad House"),
    (26, "Grassland"),
    (27, "Title Screen"),
    (28, "SMB End of Level"),
    (29, "Toad House Fanfare"),
    (30, "MvsL Stage 2"),
    (31, "MvsL Title Screen"),
    (32, "MvsL Results Screen"),
    (33, "MvsL Win"),
    (34, "MvsL Lose"),
    (80, "Lava ambient (Castle)"),
    (81, "Desert ambient (Castle)"),
    (82, "Water ambient (Castle)"),
    (83, "Underground ambient (Castle)"),
    (86, "Wind ambient (Castle)"),
    (99, "Early grassland (unused)"),
    (100, "World 1"),
    (101, "World 2"),
    (102, "World 3"),
    (103, "World 4"),
    (104, "World 5"),
    (105, "World 6"),
    (106, "World 7"),
    (107, "World 8"),
    (108, "Ending"),
    (109, "Intro 1"),
    (110, "Intro 2"),
    (111, "Peach Fanfare"),
];

/// Every music number a view can name, each with its name where it has one.
fn music_choices() -> Vec<(i64, String)> {
    (0..=u8::MAX)
        .map(|id| {
            let name = MUSIC_NAMES.iter().find(|(number, _)| *number == id).map_or("unnamed", |(_, name)| name);
            (i64::from(id), name.to_owned())
        })
        .collect()
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

/// The settings bits the Code Reference names, each a checkbox of the Course tab.
const COURSE_SETTINGS: [(u16, &str, &str); 4] = [
    (0x01, "Start as Mini", "Every player starts the level as Mini Mario."),
    (0x02, "Mini physics", "Every player moves as Mini Mario does."),
    (0x08, "Pipe and cannon range", "Changes how far from the players pipes and cannons stay in the level."),
    (0x20, "Wrap around", "The level loops sideways onto itself."),
];

/// The first of the Course tab's rows that is a settings bit.
const COURSE_SETTINGS_ROW: usize = 3;

const MEGA_DROP_CHANCES: [&str; 3] = ["One in three", "One in two", "None"];

/// The block whose first record holds the palette and screen of each of the Course tab's layers, in its order.
const GRAPHICS_BLOCKS: [ScreenBlock; 3] = [ScreenBlock::Tileset, ScreenBlock::Background, ScreenBlock::Foreground];

/// What of a layer's first record does not follow its graphics, which picking the graphics in the Course tab would
/// change: empty when nothing.
fn layer_mismatch(course: &NsmbLevelCourse, block: ScreenBlock, graphics: u16) -> String {
    let Some(first) = block.records(course).first() else {
        return format!("The level has no {} entry, so it has no {} colours. Add one in the Layers tab.", block.name(), block.name());
    };
    let shown = |value: u16| if value == u16::MAX { "none".to_owned() } else { value.to_string() };
    let mut differ = Vec::new();
    if first.palette_id != graphics {
        differ.push(format!("palette {}", shown(first.palette_id)));
    }
    // A tileset record's screen field is its rotation instead.
    if block != ScreenBlock::Tileset && first.screen_id != graphics {
        differ.push(format!("screen {}", shown(first.screen_id)));
    }
    if differ.is_empty() {
        return String::new();
    }
    format!(
        "The first {} entry has {}, not {}. Picking here sets them to match; the advanced mode keeps them apart, here and in the Layers tab.",
        block.name(),
        differ.join(" and "),
        shown(graphics)
    )
}

/// A tileset record's screen when the views naming it turn with the stage (`setupBgParallax`).
pub const ROTATING_TILESET: u16 = 0xff00;

/// A background or foreground's numbers by the names the project's scripts give them, after none at 65535.
fn layer_choices(view: &LevelView, layer: BackgroundLayer) -> Option<Vec<(i64, String)>> {
    let named = view.art.graphics.as_ref().map(|graphics| graphics.backgrounds.layer(layer)).unwrap_or_default();
    if named.is_empty() {
        return None;
    }
    let none = std::iter::once((i64::from(u16::MAX), "None".to_owned()));
    Some(none.chain(named.iter().map(|definition| (i64::from(definition.id), definition.name.clone()))).collect())
}

/// The hint of the box that filters a tileset's list, `None`, or a background or foreground's.
fn layer_filter(layer: Option<BackgroundLayer>) -> &'static str {
    match layer {
        None => "Filter tilesets…",
        Some(BackgroundLayer::Background) => "Filter backgrounds…",
        Some(BackgroundLayer::Foreground) => "Filter foregrounds…",
    }
}

/// The record of a block a view names, and its place in the block: the first when none has the view's ID, as the
/// game's lookups fall back.
pub fn screen_for_view<'a>(course: &'a NsmbLevelCourse, block: ScreenBlock, v: &NsmbLevelView) -> Option<(usize, &'a NsmbLevelScreen)> {
    let id = match block {
        ScreenBlock::Background => v.background_id,
        ScreenBlock::Tileset => v.tileset_id,
        ScreenBlock::Foreground => v.foreground_id,
    };
    let records = block.records(course);
    let at = records.iter().position(|record| record.id == u16::from(id)).unwrap_or(0);
    records.get(at).map(|record| (at, record))
}

/// The camera settings a view names and their place among the level's, falling back to the first as `GetCamera` does.
fn camera_for_view<'a>(course: &'a NsmbLevelCourse, v: &NsmbLevelView) -> Option<(usize, &'a NsmbLevelCamera)> {
    let found = bigstar_level::camera_block(&course.cameras, v.camera_settings_id)?;
    course.cameras.iter().position(|camera| std::ptr::eq(camera, found)).map(|at| (at, found))
}

fn camera_note(course: &NsmbLevelCourse, v: &NsmbLevelView) -> String {
    let lead = "The camera settings this view uses, which are edited in the Cameras tab.";
    match camera_for_view(course, v) {
        None => format!("{lead} The level has none."),
        Some((_, camera)) if camera.id & 0xff == u16::from(v.camera_settings_id) => lead.to_owned(),
        Some((_, camera)) => format!("{lead} None have ID {}, so the game uses the level's first ({}).", v.camera_settings_id, camera.id),
    }
}

/// What a view's background, tileset or foreground number means: the record of that block it names, and what the
/// record shows.
fn layer_note(view: &LevelView, block: ScreenBlock, v: &NsmbLevelView) -> String {
    let course = &view.snapshot.course;
    let what = block.name();
    let lead = format!("The {what} entry this view uses, which is edited in the Layers tab.");
    let Some((_, record)) = screen_for_view(course, block, v) else { return format!("{lead} The level has no {what} entries.") };
    let id = match block {
        ScreenBlock::Background => v.background_id,
        ScreenBlock::Tileset => v.tileset_id,
        ScreenBlock::Foreground => v.foreground_id,
    };
    let fallback = if record.id == u16::from(id) { String::new() } else { format!(" No entry has this ID, so the game uses entry {}.", record.id) };
    if block == ScreenBlock::Tileset {
        // The game loads one tileset for the whole level, from the header and the first record; the record a view names
        // only says whether the view turns.
        let turns = if record.screen_id == ROTATING_TILESET { " It turns with the stage." } else { " It does not turn." };
        return format!("{lead}{fallback}{turns} It does not change the tiles: every view shows the level's one tileset, the Course tab's.");
    }
    let layer = if block == ScreenBlock::Background { BackgroundLayer::Background } else { BackgroundLayer::Foreground };
    if record.screen_id == u16::MAX || record.scroll_y == u16::MAX {
        return format!("{lead}{fallback} That entry shows no screen, so this view has no {what}.");
    }
    match view.art.graphics.as_ref().and_then(|graphics| graphics.backgrounds.name(layer, record.screen_id)) {
        Some(name) => format!("{lead}{fallback} It shows screen {}, {name}.", record.screen_id),
        None => format!("{lead}{fallback} It shows screen {}.", record.screen_id),
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
    note.push_str(" The toolbar's DS menu draws it.");
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
            let h = &course.header;
            let entrance = |id: u8| {
                if course.entrances.iter().any(|e| e.id == id) { String::new() } else { format!("No entrance has ID {id}.") }
            };
            full(rows, "Start entrance", Some(i64::from(h.entrances[0])), 0, 255);
            if let Some(row) = rows.last_mut() {
                row.note = entrance(h.entrances[0]);
            }
            full(rows, "Midway entrance", Some(i64::from(h.entrances[1])), 0, 255);
            if let Some(row) = rows.last_mut() {
                row.note = entrance(h.entrances[1]);
            }
            full(rows, "Time limit", Some(i64::from(h.timer)), 0, 65535);
            for (bit, label, note) in COURSE_SETTINGS {
                rows.push(Row {
                    editor: Editor::Boolean,
                    note: note.to_owned(),
                    ..Row::number(label, Some(i64::from(h.settings & bit != 0)), 0, 1, Span::Full)
                });
            }
            if panel.advanced {
                full(rows, "Settings (raw)", Some(i64::from(h.settings)), 0, 65535);
                if let Some(row) = rows.last_mut() {
                    row.editor = Editor::Hex { bytes: 2 };
                    row.note = "Every settings bit, the named ones above among them.".to_owned();
                }
            }
            full(rows, "Sound group", Some(i64::from(h.sound_group)), 0, 65535);
            full(rows, "Events", Some(i64::from(h.events)), 0, 65535);
            full(rows, "Sprite priority", Some(i64::from(h.sprite_priority)), 0, 65535);
            full(rows, "Mega Mushroom drops", Some(i64::from(h.mega_drop_chance)), 0, 65535);
            if let Some(row) = rows.last_mut() {
                row.choose(MEGA_DROP_CHANCES.iter().zip(0i64..).map(|(name, value)| (value, (*name).to_owned())).collect());
                row.representable = usize::from(h.mega_drop_chance) < MEGA_DROP_CHANCES.len();
            }
            // The header's three of each layer, a tileset, a background and a foreground together for each of the
            // three. The game loads the graphics of the first; the colours and the screens are the records' in the
            // Layers tab.
            let layers = [
                ("Tileset", h.tileset_theme, None),
                ("BG graphics", h.background_theme, Some(BackgroundLayer::Background)),
                ("FG graphics", h.foreground_theme, Some(BackgroundLayer::Foreground)),
            ];
            // The game's tables hold a background's graphics, palette and screen at the same number, so the name the
            // project's scripts give that number names all three.
            let choices = layers.map(|(_, _, layer)| match layer {
                None => Some(tileset_choices()),
                Some(layer) => layer_choices(view, layer),
            });
            if panel.advanced {
                for at in 0..3 {
                    for (i, ((name, values, layer), choices)) in layers.iter().zip(&choices).enumerate() {
                        let value = values.get(at).copied().unwrap_or_default();
                        full(rows, name, Some(i64::from(value)), 0, 65535);
                        let Some(row) = rows.last_mut() else { continue };
                        if i == 0 {
                            row.heading = format!("Graphics set {}", at + 1);
                        }
                        if let Some(choices) = choices {
                            row.representable = choices.iter().any(|(id, _)| *id == i64::from(value));
                            row.choose_filtered(choices.clone(), Some(layer_filter(*layer)));
                        }
                        row.note = match (at, layer) {
                            (0, None) => "Its colours are the first tileset entry's palette, in the Layers tab.".to_owned(),
                            (0, Some(layer)) => format!(
                                "The {} image's tiles. Its colours and screen are the {} entries', in the Layers tab.",
                                layer.name(),
                                layer.name()
                            ),
                            _ => {
                                "The header holds three of each, and the game loads the first. What reads the others has not been traced.".to_owned()
                            }
                        };
                        if at == 0 && layer.is_none() && value == u16::MAX {
                            row.warning = NO_TILESET.to_owned();
                        }
                    }
                }
            } else {
                // The set the game loads, one number for each layer: its graphics, which the header names, and the
                // palette and screen of its first record, which are the whole level's and match the graphics in every
                // level but a few. A level can go without a background or a foreground, but not without its
                // tileset, which only the advanced mode sets to none.
                for (i, ((name, values, layer), (choices, block))) in layers.iter().zip(choices.iter().zip(GRAPHICS_BLOCKS)).enumerate() {
                    let value = values.first().copied().unwrap_or_default();
                    full(rows, name, Some(i64::from(value)), 0, 65535);
                    let Some(row) = rows.last_mut() else { continue };
                    if i == 0 {
                        row.heading = "Graphics".to_owned();
                    }
                    if let Some(choices) = choices {
                        let mut choices = choices.clone();
                        if layer.is_none() {
                            choices.retain(|&(id, _)| id != i64::from(u16::MAX));
                        }
                        row.representable = choices.iter().any(|(id, _)| *id == i64::from(value));
                        row.choose_filtered(choices, Some(layer_filter(*layer)));
                    }
                    row.note = match layer {
                        None => "The level's tileset, and its colours.".to_owned(),
                        Some(layer) => format!("The level's {} image: its tiles, its colours and its screen.", layer.name()),
                    };
                    row.warning = layer_mismatch(course, block, value);
                }
            }
        }
        Tab::Layers => {
            // Outside the advanced mode only entry 0 of each block is edited, the one views name unless they rotate.
            if let Some((block, at)) = screen_record(course, index)
                && let Some(record) = block.records(course).get(at)
                && (panel.advanced || record.id == 0)
            {
                let users: Vec<String> = course
                    .views
                    .iter()
                    .filter(|v| screen_for_view(course, block, v).is_some_and(|(found, _)| found == at))
                    .map(|v| v.id.to_string())
                    .collect();
                let used = if users.is_empty() { "No view uses it.".to_owned() } else { format!("Used by views {}.", users.join(", ")) };
                // Outside the advanced mode the entries keep their IDs, rotation, screen and palette: the Course tab sets
                // the screen and palette, with the graphics they match, and a view's Rotates the rotating tileset entry.
                if panel.advanced {
                    full(rows, "ID", Some(i64::from(record.id)), 0, 65535);
                    if let Some(row) = rows.last_mut() {
                        row.note = format!("Views name a {} entry by this ID; one naming an ID no entry has gets the first. {used}", block.name());
                    }
                }
                if panel.advanced && block == ScreenBlock::Tileset {
                    rows.push(Row {
                        editor: Editor::Boolean,
                        representable: matches!(record.screen_id, 0 | ROTATING_TILESET),
                        note: "0xFF00 here has the views naming this entry turn with the stage, as the rotating rooms of World 8 Final Castle do. No other level in the game uses it."
                            .to_owned(),
                        ..Row::number("Rotates", Some(i64::from(record.screen_id == ROTATING_TILESET)), 0, 1, Span::Full)
                    });
                    // The same field as a number, for what the checkbox cannot say: two of Nintendo's levels hold their
                    // tileset's number here, as a background record holds its screen's, though the game loads no
                    // tileset from it.
                    full(rows, "Rotates (raw)", Some(i64::from(record.screen_id)), 0, 65535);
                    if let Some(row) = rows.last_mut() {
                        row.note = "The field the checkbox above reads. Only 0xFF00 does anything; G04_1 and J07_1 hold their tileset's number here, which the game ignores.".to_owned();
                    }
                } else if panel.advanced {
                    let layer = if block == ScreenBlock::Background { BackgroundLayer::Background } else { BackgroundLayer::Foreground };
                    full(rows, "Screen", Some(i64::from(record.screen_id)), 0, 65535);
                    if let Some(row) = rows.last_mut() {
                        row.note = format!("The {} image the views naming this entry show. 65535 shows none.", layer.name());
                        if let Some(choices) = layer_choices(view, layer) {
                            row.representable = choices.iter().any(|(id, _)| *id == i64::from(record.screen_id));
                            row.choose_filtered(choices, Some(layer_filter(Some(layer))));
                        }
                    }
                }
                if panel.advanced {
                    full(rows, "Palette", Some(i64::from(record.palette_id)), 0, 65535);
                    if let Some(row) = rows.last_mut() {
                        row.note = if at == 0 {
                            format!(
                                "The colours of the whole level's {}: the game loads them from the first {} entry only.",
                                block.name(),
                                block.name()
                            )
                        } else {
                            format!("Unused: the game loads the {}'s colours from the first entry only.", block.name())
                        };
                        // The game's tables hold a background's palette at its graphics' number, so the names are the same.
                        let layer = match block {
                            ScreenBlock::Tileset => None,
                            ScreenBlock::Background => Some(BackgroundLayer::Background),
                            ScreenBlock::Foreground => Some(BackgroundLayer::Foreground),
                        };
                        let choices = match layer {
                            None => Some(tileset_choices()),
                            Some(layer) => layer_choices(view, layer),
                        };
                        if let Some(choices) = choices {
                            row.representable = choices.iter().any(|(id, _)| *id == i64::from(record.palette_id));
                            row.choose_filtered(choices, Some(layer_filter(layer)));
                        }
                    }
                }
                // Read by nothing. Across the game's 578 entries, 0x06 mostly repeats the layer's number in the header's
                // second graphics set (94 of the 108 entries that set it) and 0x08 is always 65535, so both likely
                // survive from an older layout of the level format.
                const UNUSED_06: &str = "Unused, likely left over from a migration of the level format: it mostly holds this layer's graphics \
                                         set 2 number, in the first entry (as in A01_1) or, in J08_1 only, the second.";
                const UNUSED_08: &str =
                    "Unused, likely left over from a migration of the level format, next to 0x06's graphics set 2: 65535 in every level.";
                for (label, value, note) in [
                    ("Unused 0x06", record.unused_06, UNUSED_06),
                    ("Unused 0x08", record.unused_08, UNUSED_08),
                    ("Scroll speed X", record.scroll_x, ""),
                    (
                        "Scroll speed Y",
                        record.scroll_y,
                        "The low nibble is a speed and the next a scroll mode. 65535 hides the layer in the views naming this entry.",
                    ),
                    ("Vertical offset", record.vertical_offset, "Moves the image up or down in steps of eight pixels."),
                    ("Unused 0x10", record.unused_10, ""),
                    ("Unused 0x12", record.unused_12, ""),
                ] {
                    // Only the advanced mode shows the fields nothing reads, and a tileset's scroll speeds, as it scrolls
                    // with the level.
                    if !panel.advanced && (label.starts_with("Unused") || block == ScreenBlock::Tileset && label.starts_with("Scroll")) {
                        continue;
                    }
                    full(rows, label, Some(i64::from(value)), 0, 65535);
                    if let Some(row) = rows.last_mut().filter(|_| block != ScreenBlock::Tileset || [UNUSED_06, UNUSED_08].contains(&note)) {
                        row.note = note.to_owned();
                    }
                }
            }
        }
        Tab::Cameras => {
            if let Some(camera) = course.cameras.get(index) {
                let users: Vec<String> = course
                    .views
                    .iter()
                    .filter(|v| camera_for_view(course, v).is_some_and(|(found, _)| found == index))
                    .map(|v| v.id.to_string())
                    .collect();
                let used = if users.is_empty() { "No view uses them.".to_owned() } else { format!("Used by views {}.", users.join(", ")) };
                full(rows, "ID", Some(i64::from(camera.id)), 0, 65535);
                if let Some(row) = rows.last_mut() {
                    row.note = format!("Views name camera settings by the low byte of this ID; one naming an ID none has gets the first. {used}");
                }
                let lines = "The game moves the camera up or down only as far as keeps the player between two lines down the screen.";
                for (label, value, min, max, note) in [
                    ("Top offset", i64::from(camera.top_offset), i64::from(i32::MIN), i64::from(i32::MAX), format!("{lines} The upper is 64 pixels down plus this.")),
                    ("Bottom offset", i64::from(camera.bottom_offset), i64::from(i32::MIN), i64::from(i32::MAX), format!("{lines} The lower is 128 pixels down plus this.")),
                    (
                        "Top offset (special)",
                        i64::from(camera.top_offset_special),
                        i64::from(i32::MIN),
                        i64::from(i32::MAX),
                        "The upper line in the game's second camera mode. Which of the player's moves switch to it has not been traced.".to_owned(),
                    ),
                    ("Bottom offset (special)", i64::from(camera.bottom_offset_special), i64::from(i32::MIN), i64::from(i32::MAX), "The lower line in that second mode.".to_owned()),
                    ("Scroll height", i64::from(camera.scroll_height), 0, 65535, "15 is none. Anything else has the game note a camera height this many tiles above where it rests on the view's floor; what reads it has not been traced.".to_owned()),
                    ("Unknown", i64::from(camera.unknown), 0, i64::from(u32::MAX), String::new()),
                ] {
                    full(rows, label, Some(value), min, max);
                    if let Some(row) = rows.last_mut() {
                        row.note = note;
                    }
                }
            }
        }
        Tab::Banks => {
            // Every slot at once, as NSMB Editor shows them: the level is set up by looking across them, not one by one.
            for slot in bank_slots(&course.object_banks) {
                let bank = course.object_banks.get(slot).copied().unwrap_or(0);
                full(rows, &format!("Slot {slot}"), Some(i64::from(bank)), 0, 10);
                let Some(row) = rows.last_mut() else { continue };
                row.representable = bank <= 10;
                row.choose(bank_choices(view, slot, overlays));
                // The sprites placed here that this slot's bank leaves out of the game.
                let left_out = (0..course.sprites.len()).filter(|&i| view.missing_bank(i).is_some_and(|(s, _)| usize::from(s) == slot)).count();
                if left_out != 0 {
                    let sprites = if left_out == 1 { "sprite needs" } else { "sprites need" };
                    row.note = format!("{left_out} placed {sprites} another bank in this slot, so the game leaves them out.");
                }
            }
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
                    "Anything but 0 replaces the warp: entering fades out and back in within this area, ignoring the target. What the number means beyond that has not been found.",
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
                        heading: String::new(),
                        note: note.to_owned(),
                        warning: String::new(),
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
                row.choose((0i64..).zip(ENTRANCE_TYPES).map(|(value, name)| (value, name.to_owned())).collect());
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
                    // Outside the advanced mode a view's three layer entries are one checkbox, which adds and removes
                    // the rotating tileset entry for it.
                    if !panel.advanced && matches!(label, "Background" | "Foreground") {
                        continue;
                    }
                    if !panel.advanced && label == "Tileset" {
                        rows.push(Row {
                            editor: Editor::Boolean,
                            note: "The view turns with the stage, as the rotating rooms of World 8 Final Castle do. Checking it adds the \
                                   rotating tileset entry 1 the view names, and unchecking it on the last view to turn removes it."
                                .to_owned(),
                            warning: rotator_conflict(course),
                            ..Row::number("Rotates", Some(i64::from(view_rotates(course, v))), 0, 1, Span::Full)
                        });
                        continue;
                    }
                    full(rows, label, Some(i64::from(value)), 0, 255);
                    let Some(row) = rows.last_mut() else { continue };
                    match label {
                        "BGM" => row.choose(music_choices()),
                        "Camera settings" => row.note = camera_note(course, v),
                        // A view names a record of each layer, and the record says which screen scrolls there and how.
                        "Background" => row.note = layer_note(view, ScreenBlock::Background, v),
                        "Tileset" => row.note = layer_note(view, ScreenBlock::Tileset, v),
                        "Foreground" => row.note = layer_note(view, ScreenBlock::Foreground, v),
                        _ => {}
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
    let (min, max) = (i64::from(sprite_field_min(field)), i64::from(sprite_field_max(field)));
    let (editor, representable) = if field.checkbox_value != 0 {
        (Editor::Boolean, value == 0 || value == 1)
    } else if !field.choices.is_empty() {
        let choices: Vec<(i64, String)> = field.choices.iter().map(|c| (i64::from(c.value), c.label.clone())).collect();
        let named = choices.iter().any(|(v, _)| *v == value);
        (Editor::Choice { choices, min, max, filter: None }, named)
    } else {
        (Editor::Number { min, max }, true)
    };
    let mut note = field.notes.clone();
    if !representable {
        if !note.is_empty() {
            note.push_str("\n\n");
        }
        note.push_str(&format!("This actor does not name the value {value}, which is what these bits hold."));
    }
    Row {
        label: field.label.clone(),
        value: Some(value),
        editor,
        span: Span::Full,
        group: String::new(),
        heading: String::new(),
        note,
        warning: String::new(),
        representable,
    }
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
            let mut h = course.header.clone();
            // Outside the advanced mode the raw settings are hidden, so the rows after them sit one up.
            let changed = if !panel.advanced && changed >= 7 { changed + 1 } else { changed };
            // Without the advanced mode, picking a layer's graphics sets its first record's palette, and screen, to the
            // same number.
            if !panel.advanced
                && let Some(&block) = changed.checked_sub(12).and_then(|at| GRAPHICS_BLOCKS.get(at))
            {
                let field = match block {
                    ScreenBlock::Tileset => &mut h.tileset_theme[0],
                    ScreenBlock::Background => &mut h.background_theme[0],
                    ScreenBlock::Foreground => &mut h.foreground_theme[0],
                };
                *field = narrow(value, *field);
                let graphics = *field;
                let name = "Edit course".to_owned();
                let mut screens = block.records(course).clone();
                let Some(first) = screens.first_mut() else { return Some(LevelEdit::SetHeader { header: h, name }) };
                first.palette_id = graphics;
                if block != ScreenBlock::Tileset {
                    first.screen_id = graphics;
                }
                return Some(LevelEdit::SetLayer { header: h, block, screens, name });
            }
            match changed {
                0 => h.entrances[0] = narrow(value, h.entrances[0]),
                1 => h.entrances[1] = narrow(value, h.entrances[1]),
                row if let Some(&(bit, ..)) = row.checked_sub(COURSE_SETTINGS_ROW).and_then(|at| COURSE_SETTINGS.get(at)) => {
                    h.settings = if value != 0 { h.settings | bit } else { h.settings & !bit };
                }
                row => {
                    let field = match row {
                        2 => &mut h.timer,
                        7 => &mut h.settings,
                        8 => &mut h.sound_group,
                        9 => &mut h.events,
                        10 => &mut h.sprite_priority,
                        11 => &mut h.mega_drop_chance,
                        // A tileset, a background and a foreground for each of the header's three.
                        12..=20 => {
                            let at = (row - 12) / 3;
                            match (row - 12) % 3 {
                                0 => h.tileset_theme.get_mut(at)?,
                                1 => h.background_theme.get_mut(at)?,
                                _ => h.foreground_theme.get_mut(at)?,
                            }
                        }
                        _ => return None,
                    };
                    *field = narrow(value, *field);
                }
            }
            Some(LevelEdit::SetHeader { header: h, name: "Edit course".to_owned() })
        }
        Tab::Layers => {
            let (block, at) = screen_record(course, *rows.indices.first()?)?;
            let mut screens = block.records(course).clone();
            let record = screens.get_mut(at)?;
            let name = format!("Edit {} entry", block.name());
            // A tileset record has its rotation field twice, as a checkbox and as a number, and outside the advanced
            // mode it hides some rows, so each row finds its field by its label.
            let field = match rows.rows.get(changed)?.label.as_str() {
                "Rotates" => {
                    record.screen_id = if value != 0 { ROTATING_TILESET } else { 0 };
                    return Some(LevelEdit::SetScreens { block, screens, name });
                }
                "ID" => &mut record.id,
                "Screen" | "Rotates (raw)" => &mut record.screen_id,
                "Palette" => &mut record.palette_id,
                "Unused 0x06" => &mut record.unused_06,
                "Unused 0x08" => &mut record.unused_08,
                "Scroll speed X" => &mut record.scroll_x,
                "Scroll speed Y" => &mut record.scroll_y,
                "Vertical offset" => &mut record.vertical_offset,
                "Unused 0x10" => &mut record.unused_10,
                "Unused 0x12" => &mut record.unused_12,
                _ => return None,
            };
            *field = narrow(value, *field);
            Some(LevelEdit::SetScreens { block, screens, name })
        }
        Tab::Cameras => {
            let mut cameras = course.cameras.clone();
            let camera = cameras.get_mut(*rows.indices.first()?)?;
            match changed {
                0 => camera.id = narrow(value, camera.id),
                1 => camera.top_offset = narrow(value, camera.top_offset),
                2 => camera.bottom_offset = narrow(value, camera.bottom_offset),
                3 => camera.top_offset_special = narrow(value, camera.top_offset_special),
                4 => camera.bottom_offset_special = narrow(value, camera.bottom_offset_special),
                5 => camera.scroll_height = narrow(value, camera.scroll_height),
                6 => camera.unknown = narrow(value, camera.unknown),
                _ => return None,
            }
            Some(LevelEdit::SetCameras { cameras, name: "Edit camera settings".to_owned() })
        }
        Tab::Banks => {
            let slot = *bank_slots(&course.object_banks).get(changed)?;
            Some(LevelEdit::SetObjectBank { slot, value: narrow(value, 0) })
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
            if rows.rows.get(changed)?.label == "Rotates" {
                return rotates_edit(course, index, value != 0);
            }
            // Outside the advanced mode the three layer rows are one, so the rows after it sit two up.
            let changed = if !panel.advanced && changed > 7 { changed + 2 } else { changed };
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

/// The tileset entry a view names to turn, as H12_1's: a copy of the first with 0xFF00 for its screen.
const ROTATOR_ID: u16 = 1;

fn view_rotates(course: &NsmbLevelCourse, v: &NsmbLevelView) -> bool {
    screen_for_view(course, ScreenBlock::Tileset, v).is_some_and(|(_, record)| record.screen_id == ROTATING_TILESET)
}

/// Why the Rotates checkbox cannot add the rotating entry: the level holds tileset entry 1 as an ordinary one, as J08_1
/// does. Empty when it can.
fn rotator_conflict(course: &NsmbLevelCourse) -> String {
    if !course.tilesets.iter().any(|record| record.id == ROTATOR_ID && record.screen_id != ROTATING_TILESET) {
        return String::new();
    }
    let users: Vec<String> = course.views.iter().filter(|v| u16::from(v.tileset_id) == ROTATOR_ID).map(|v| v.id.to_string()).collect();
    let used = if users.is_empty() { String::new() } else { format!(", which views {} use,", users.join(", ")) };
    format!("Tileset entry 1{used} does not rotate, so this cannot add the rotating one there. The advanced mode edits the entries themselves.")
}

/// The Rotates checkbox of the Views tab: the view names the rotating tileset entry, which is added for the first view to
/// turn and removed with the last, or names entry 0 again.
fn rotates_edit(course: &NsmbLevelCourse, index: usize, rotates: bool) -> Option<LevelEdit> {
    let mut v = course.views.get(index)?.clone();
    if rotates == view_rotates(course, &v) {
        return None;
    }
    let mut screens = course.tilesets.clone();
    let name = if rotates { "Rotate view" } else { "Stop view rotating" }.to_owned();
    let rotator = |record: &NsmbLevelScreen| record.id == ROTATOR_ID && record.screen_id == ROTATING_TILESET;
    let view_only = |v, name| {
        Some(LevelEdit::Update { updates: LevelEntityUpdates { views: vec![IndexedLevelValue { index, value: v }], ..Default::default() }, name })
    };
    if rotates {
        if !rotator_conflict(course).is_empty() {
            return None;
        }
        v.tileset_id = ROTATOR_ID as u8;
        if screens.iter().any(rotator) {
            return view_only(v, name);
        }
        let first = screens.first().cloned().unwrap_or(NsmbLevelScreen {
            unused_06: u16::MAX,
            unused_08: u16::MAX,
            scroll_x: u16::MAX,
            scroll_y: u16::MAX,
            ..Default::default()
        });
        screens.push(NsmbLevelScreen { id: ROTATOR_ID, screen_id: ROTATING_TILESET, ..first });
    } else {
        v.tileset_id = 0;
        let others = course.views.iter().enumerate().any(|(at, other)| at != index && u16::from(other.tileset_id) == ROTATOR_ID);
        if others || !screens.iter().any(rotator) {
            return view_only(v, name);
        }
        screens.retain(|record| !rotator(record));
    }
    Some(LevelEdit::SetViewScreens { index, view: v, block: ScreenBlock::Tileset, screens, name })
}

/// The Add buttons of the Layers tab: a copy of the block's first record, or an empty one, with the first ID
/// nothing in the block uses.
pub fn add_screen_edit(course: &NsmbLevelCourse, block: ScreenBlock) -> Option<LevelEdit> {
    let mut screens = block.records(course).clone();
    let id = first_unused(screens.iter().filter_map(|record| u8::try_from(record.id).ok()), 0);
    screens.push(NsmbLevelScreen { id: u16::from(id), ..screens.first().cloned().unwrap_or_default() });
    Some(LevelEdit::SetScreens { block, screens, name: format!("Add {} entry", block.name()) })
}

/// The Add button of the Cameras tab: a copy of the first settings, or empty ones, under the first ID none use.
pub fn add_camera_edit(course: &NsmbLevelCourse) -> Option<LevelEdit> {
    let mut cameras = course.cameras.clone();
    let id = first_unused(cameras.iter().map(|camera| (camera.id & 0xff) as u8), 0);
    cameras.push(NsmbLevelCamera { id: u16::from(id), ..cameras.first().cloned().unwrap_or_default() });
    Some(LevelEdit::SetCameras { cameras, name: "Add camera settings".to_owned() })
}

/// The Delete button of the Layers and Cameras tabs: the record being looked at.
pub fn delete_record_edit(course: &NsmbLevelCourse, tab: Tab, rows: &Rows) -> Option<LevelEdit> {
    let index = *rows.indices.first()?;
    match tab {
        Tab::Layers => {
            let (block, at) = screen_record(course, index)?;
            let mut screens = block.records(course).clone();
            screens.remove(at);
            Some(LevelEdit::SetScreens { block, screens, name: format!("Delete {} entry", block.name()) })
        }
        Tab::Cameras => {
            let mut cameras = course.cameras.clone();
            (index < cameras.len()).then(|| cameras.remove(index))?;
            Some(LevelEdit::SetCameras { cameras, name: "Delete camera settings".to_owned() })
        }
        _ => None,
    }
}

/// One line for each record of the Layers or Cameras tab.
pub fn record_lines(view: &LevelView, tab: Tab) -> Vec<String> {
    let course = &view.snapshot.course;
    match tab {
        Tab::Layers => ScreenBlock::ALL
            .into_iter()
            .flat_map(|block| block.records(course).iter().map(move |record| (block, record)))
            .map(|(block, record)| {
                let mut name = block.name().to_owned();
                name[..1].make_ascii_uppercase();
                match block {
                    ScreenBlock::Tileset if record.screen_id == ROTATING_TILESET => format!("{name} {} · rotates", record.id),
                    ScreenBlock::Tileset => format!("{name} {}", record.id),
                    _ if record.screen_id == u16::MAX => format!("{name} {} · no screen", record.id),
                    _ => {
                        let layer = if block == ScreenBlock::Background { BackgroundLayer::Background } else { BackgroundLayer::Foreground };
                        match view.art.graphics.as_ref().and_then(|graphics| graphics.backgrounds.name(layer, record.screen_id)) {
                            Some(screen) => format!("{name} {} · {screen}", record.id),
                            None => format!("{name} {} · screen {}", record.id, record.screen_id),
                        }
                    }
                }
            })
            .collect(),
        Tab::Cameras => course.cameras.iter().map(|camera| format!("Camera settings {}", camera.id)).collect(),
        _ => Vec::new(),
    }
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

/// The sprite picker: every type the ROM's actor table maps to a named actor, by number.
pub fn sprite_types(view: &LevelView) -> Vec<(u16, String)> {
    let catalog = &view.art.actors.catalog;
    (0..catalog.sprite_object_ids.len())
        .filter_map(|t| u16::try_from(t).ok())
        .filter_map(|t| view.art.actors.actor(t, None).filter(|a| !a.definition.name.is_empty()).map(|a| (t, a.definition.name.clone())))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bigstar_formats::{NsmbLevelObject, NsmbLevelStageObject};
    use bigstar_level::{SpriteDefinition, SpriteFieldChoice, SpriteFieldDefinition};
    use bigstar_protocol::{ActorCatalog, ActorInfo, LevelEdit};

    use super::*;
    use crate::controls::{format_hex_bytes, parse_hex_bytes};
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
            sprite_banks: Vec::new(),
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
        assert_eq!((format_hex_bytes(0x1234_5678, 4), format_hex_bytes(0x21, 2)), ("12 34 56 78".into(), "00 21".into()));
        assert_eq!((parse_hex_bytes("22 33 44 55", 4), parse_hex_bytes("22 33 44", 4)), (Some(0x2233_4455), None));
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
        assert_eq!(common_settings(&view, &rows), Some(0x1234_5678));
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
    fn camera_settings_are_their_own_records_that_views_name() {
        let mut level = snapshot();
        level.course.views =
            vec![NsmbLevelView { camera_settings_id: 2, ..Default::default() }, NsmbLevelView { id: 1, camera_settings_id: 5, ..Default::default() }];
        level.course.cameras = vec![NsmbLevelCamera { id: 2, top_offset: -8, bottom_offset: 16, ..Default::default() }];
        let mut view = LevelView::new(level);
        view.panel.tab = Tab::Entity(Kind::View);
        let shown = rows(&view, &view.panel, None);
        let row = |label: &str| shown.rows.iter().position(|row| row.label == label).unwrap();
        // A view only names its camera settings; it no longer shows or writes them.
        assert!(shown.rows.iter().all(|row| !row.label.starts_with("Camera ") || row.label == "Camera settings"));
        assert!(shown.rows[row("Camera settings")].note.contains("Cameras tab"));
        // The music is picked by name, and a number nobody named is still there to pick.
        let Editor::Choice { choices: music, .. } = &shown.rows[row("BGM")].editor else { panic!() };
        assert_eq!((music.len(), &music[16], &music[200]), (256, &(16, "Ghost House".to_owned()), &(200, "unnamed".to_owned())));
        // A view naming settings that do not exist reads the first, as the game does.
        view.panel.item = 1;
        assert!(rows(&view, &view.panel, None).rows[row("Camera settings")].note.contains("uses the level's first (2)"));

        view.panel_tab(Tab::Cameras);
        assert_eq!(record_lines(&view, Tab::Cameras), ["Camera settings 2"]);
        let shown = rows(&view, &view.panel, None);
        assert!(shown.rows[0].note.contains("Used by views 0, 1."), "{}", shown.rows[0].note);
        assert_eq!(shown.rows[2].value, Some(16));
        match apply(&view, &view.panel, &shown, 2, -32) {
            Some(LevelEdit::SetCameras { cameras, .. }) => {
                assert_eq!(cameras, [NsmbLevelCamera { id: 2, top_offset: -8, bottom_offset: -32, ..Default::default() }])
            }
            other => panic!("no camera edit: {other:?}"),
        }
        let Some(LevelEdit::SetCameras { cameras, name }) = add_camera_edit(&view.snapshot.course) else { panic!() };
        assert_eq!((cameras[1].id, cameras[1].top_offset, name.as_str()), (0, -8, "Add camera settings"));
        let Some(LevelEdit::SetCameras { cameras, .. }) = delete_record_edit(&view.snapshot.course, Tab::Cameras, &shown) else { panic!() };
        assert!(cameras.is_empty());
    }

    #[test]
    fn the_layers_tab_edits_every_record_of_the_three_blocks() {
        let mut level = snapshot();
        let record = |id: u16, screen_id: u16| NsmbLevelScreen { id, screen_id, palette_id: 41, ..Default::default() };
        level.course.backgrounds = vec![record(0, 41)];
        level.course.tilesets = vec![record(0, 0), record(1, ROTATING_TILESET)];
        level.course.foregrounds = vec![record(0, u16::MAX)];
        level.course.views =
            vec![NsmbLevelView { tileset_id: 1, ..Default::default() }, NsmbLevelView { id: 1, tileset_id: 9, ..Default::default() }];
        let mut view = LevelView::new(level);
        view.panel_tab(Tab::Layers);
        assert_eq!(item_count(&view.snapshot, Tab::Layers), 4);
        assert_eq!(record_lines(&view, Tab::Layers), ["Background 0 · screen 41", "Tileset 0", "Tileset 1 · rotates", "Foreground 0 · no screen"]);
        // The second tileset record: the rotator, which view 0 names and view 1 falls back past.
        view.panel_item(2);
        // Without the advanced mode only entry 0 is edited, and a tileset entry hides its ID, rotation, palette, scroll
        // speeds and unused fields.
        assert!(rows(&view, &view.panel, None).rows.is_empty());
        view.panel_item(1);
        let shown = rows(&view, &view.panel, None);
        let labels: Vec<&str> = shown.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Vertical offset"]);
        let Some(LevelEdit::SetScreens { screens, .. }) = apply(&view, &view.panel, &shown, 0, 5) else { panic!() };
        assert_eq!(screens[0].vertical_offset, 5);
        view.panel_item(2);
        view.panel.advanced = true;
        let shown = rows(&view, &view.panel, None);
        assert!(shown.rows[0].note.contains("Used by views 0."), "{}", shown.rows[0].note);
        assert_eq!((shown.rows[1].label.as_str(), &shown.rows[1].editor, shown.rows[1].value), ("Rotates", &Editor::Boolean, Some(1)));
        assert_eq!((shown.rows[2].label.as_str(), shown.rows[2].value), ("Rotates (raw)", Some(i64::from(ROTATING_TILESET))));
        assert!(shown.rows[3].note.starts_with("Unused"), "only the first entry's colours are loaded");
        let Some(LevelEdit::SetScreens { block, screens, name }) = apply(&view, &view.panel, &shown, 1, 0) else { panic!() };
        assert_eq!((block, screens[1].screen_id, screens[0].screen_id, name.as_str()), (ScreenBlock::Tileset, 0, 0, "Edit tileset entry"));
        // The raw row writes the field as it is, and the rows after it still reach their own fields.
        let Some(LevelEdit::SetScreens { screens, .. }) = apply(&view, &view.panel, &shown, 2, 13) else { panic!() };
        assert_eq!(screens[1].screen_id, 13);
        let Some(LevelEdit::SetScreens { screens, .. }) = apply(&view, &view.panel, &shown, 3, 7) else { panic!() };
        assert_eq!((screens[1].palette_id, screens[1].screen_id), (7, ROTATING_TILESET));
        // A tileset's number there, as G04_1 and J07_1 hold, leaves the checkbox undecided and shows in the raw row.
        view.snapshot.course.tilesets[1].screen_id = 13;
        let shown = rows(&view, &view.panel, None);
        assert_eq!((shown.rows[1].representable, shown.rows[2].value), (false, Some(13)));
        view.snapshot.course.tilesets[1].screen_id = ROTATING_TILESET;
        // The first background record's screen, and its palette, which is the one the level loads.
        view.panel_item(0);
        let shown = rows(&view, &view.panel, None);
        assert!(shown.rows[2].note.starts_with("The colours of the whole level's background"));
        let Some(LevelEdit::SetScreens { block, screens, .. }) = apply(&view, &view.panel, &shown, 1, 7) else { panic!() };
        assert_eq!((block, screens[0].screen_id), (ScreenBlock::Background, 7));
        // Without the advanced mode it hides its ID, the screen and palette, which the Course tab sets, and the fields
        // nothing reads.
        view.panel.advanced = false;
        let shown = rows(&view, &view.panel, None);
        let labels: Vec<&str> = shown.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Scroll speed X", "Scroll speed Y", "Vertical offset"]);
        let Some(LevelEdit::SetScreens { screens, .. }) = apply(&view, &view.panel, &shown, 0, 2) else { panic!() };
        assert_eq!(screens[0].scroll_x, 2);
        let Some(LevelEdit::SetScreens { block, screens, .. }) = add_screen_edit(&view.snapshot.course, ScreenBlock::Tileset) else { panic!() };
        assert_eq!((block, screens.len(), screens[2].id), (ScreenBlock::Tileset, 3, 2));
        view.panel_item(3);
        let Some(LevelEdit::SetScreens { block, screens, .. }) =
            delete_record_edit(&view.snapshot.course, Tab::Layers, &rows(&view, &view.panel, None))
        else {
            panic!()
        };
        assert_eq!((block, screens.len()), (ScreenBlock::Foreground, 0));
        // A view's tileset says which record it gets, and that the record turns.
        view.panel_tab(Tab::Entity(Kind::View));
        view.panel_item(0);
        view.panel.advanced = true;
        let shown = rows(&view, &view.panel, None);
        let tileset = shown.rows.iter().find(|row| row.label == "Tileset").unwrap();
        assert!(tileset.note.contains("It turns with the stage."), "{}", tileset.note);
    }

    #[test]
    fn a_views_rotates_adds_and_removes_the_rotating_tileset_entry() {
        let mut level = snapshot();
        let first = NsmbLevelScreen {
            palette_id: 41,
            unused_06: u16::MAX,
            unused_08: u16::MAX,
            scroll_x: u16::MAX,
            scroll_y: u16::MAX,
            ..Default::default()
        };
        level.course.tilesets = vec![first.clone()];
        level.course.views = vec![NsmbLevelView::default(), NsmbLevelView { id: 1, ..Default::default() }];
        let mut view = LevelView::new(level);
        view.panel_tab(Tab::Entity(Kind::View));
        // Without the advanced mode the three layer rows are one checkbox, and the rows after it still reach their fields.
        let shown = rows(&view, &view.panel, None);
        let labels: Vec<&str> = shown.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(&labels[4..], ["ID", "Camera settings", "BGM", "Rotates", "Light", "Progress path"]);
        assert_eq!(shown.rows[7].value, Some(0));
        let Some(LevelEdit::Update { updates, .. }) = apply(&view, &view.panel, &shown, 8, 3) else { panic!() };
        assert_eq!(updates.views[0].value.light_type, 3);
        // The first view to turn adds entry 1 as H12_1 holds it: a copy of the first with 0xFF00 for its screen.
        let Some(LevelEdit::SetViewScreens { index, view: v, block, screens, .. }) = apply(&view, &view.panel, &shown, 7, 1) else { panic!() };
        assert_eq!((index, v.tileset_id, block), (0, 1, ScreenBlock::Tileset));
        assert_eq!(screens, [first.clone(), NsmbLevelScreen { id: 1, screen_id: ROTATING_TILESET, ..first.clone() }]);
        view.snapshot.course.tilesets = screens;
        view.snapshot.course.views[0] = v;
        // A second one names the same entry.
        view.panel_item(1);
        let shown = rows(&view, &view.panel, None);
        let Some(LevelEdit::Update { updates, .. }) = apply(&view, &view.panel, &shown, 7, 1) else { panic!() };
        view.snapshot.course.views[1] = updates.views[0].value.clone();
        // The entry stays while another view turns, and goes with the last.
        let shown = rows(&view, &view.panel, None);
        assert_eq!(shown.rows[7].value, Some(1));
        let Some(LevelEdit::Update { updates, .. }) = apply(&view, &view.panel, &shown, 7, 0) else { panic!() };
        assert_eq!(updates.views[0].value.tileset_id, 0);
        view.snapshot.course.views[1] = updates.views[0].value.clone();
        view.panel_item(0);
        let shown = rows(&view, &view.panel, None);
        let Some(LevelEdit::SetViewScreens { view: v, screens, .. }) = apply(&view, &view.panel, &shown, 7, 0) else { panic!() };
        assert_eq!((v.tileset_id, screens), (0, vec![first.clone()]));
        view.snapshot.course.views[0] = v;
        // An entry 1 that does not turn, as J08_1's, is warned of and left alone.
        view.snapshot.course.tilesets = vec![first.clone(), NsmbLevelScreen { id: 1, ..first }];
        view.snapshot.course.views[1].tileset_id = 1;
        let shown = rows(&view, &view.panel, None);
        assert!(shown.rows[7].warning.contains("views 1 use"), "{}", shown.rows[7].warning);
        assert_eq!(apply(&view, &view.panel, &shown, 7, 1), None);
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
        let Editor::Choice { choices: named, min, max, .. } = &rows.rows[5].editor else { panic!("{:?}", rows.rows[5].editor) };
        assert_eq!(named, &[(1, "Slow".to_owned()), (2, "Fast".to_owned())]);
        // Raw input types into the three bits, named or not.
        assert_eq!((*min, *max), (0, 7));
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
    fn the_sprite_picker_lists_types_by_number() {
        let view = sprite_view(0, Vec::new());
        assert_eq!(sprite_types(&view), [(7, "Synthetic actor".to_owned()), (9, "Another actor".to_owned())]);
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
            (Editor::Boolean, Editor::Choice { .. }, Editor::Number { .. })
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
        // The course tab edits the header: its flags are checkboxes of their bits, and the rest are its fields.
        view.panel.advanced = true;
        let shown_rows = rows(&view, &view.panel, None);
        let labels: Vec<&str> = shown_rows.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(
            labels[..8],
            [
                "Start entrance",
                "Midway entrance",
                "Time limit",
                "Start as Mini",
                "Mini physics",
                "Pipe and cannon range",
                "Wrap around",
                "Settings (raw)"
            ]
        );
        let Some(LevelEdit::SetHeader { header, name }) = apply(&view, &view.panel, &shown_rows, 3, 1) else { panic!() };
        assert_eq!((header.settings, name.as_str()), (0x01, "Edit course"));
        view.snapshot.course.header.settings = 0x21;
        let shown_rows = rows(&view, &view.panel, None);
        assert_eq!((shown_rows.rows[3].value, shown_rows.rows[4].value, shown_rows.rows[6].value), (Some(1), Some(0), Some(1)));
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 6, 0) else { panic!() };
        assert_eq!(header.settings, 0x01);
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 2, 400) else { panic!() };
        assert_eq!(header.timer, 400);
        // Without the advanced mode the panel shows the set the game loads, one picker for each layer, and the tileset
        // is picked by name, with its number. The raw settings are hidden, and the rows after them still reach their
        // fields.
        view.panel.advanced = false;
        let shown_rows = rows(&view, &view.panel, None);
        assert!(!shown_rows.rows.iter().any(|row| row.label == "Settings (raw)"));
        assert_eq!(shown_rows.rows[7].label, "Sound group");
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 7, 3) else { panic!() };
        assert_eq!(header.sound_group, 3);
        view.snapshot.course.header.tileset_theme[0] = 9;
        view.snapshot.course.tilesets = vec![NsmbLevelScreen { palette_id: 9, ..Default::default() }];
        let shown_rows = rows(&view, &view.panel, None);
        let labels: Vec<&str> = shown_rows.rows[11..].iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels, ["Tileset", "BG graphics", "FG graphics"]);
        assert_eq!(shown_rows.rows[11].heading, "Graphics");
        let tileset = &shown_rows.rows[11];
        let Editor::Choice { choices, .. } = &tileset.editor else { panic!("{:?}", tileset.editor) };
        assert_eq!((tileset.label.as_str(), choices.len(), &choices[1]), ("Tileset", 76, &(1, "Castle".to_owned())));
        assert!(!choices.iter().any(|&(id, _)| id == 65535), "only the advanced mode sets the tileset to none");
        assert_eq!(tileset.warning, "", "the palette follows the graphics");
        // Picking one sets the first record's palette with it, and a background's screen too, as one edit.
        let Some(LevelEdit::SetLayer { header, block, screens, .. }) = apply(&view, &view.panel, &shown_rows, 11, 23) else { panic!() };
        assert_eq!((header.tileset_theme, block, screens[0].palette_id, screens[0].screen_id), ([23, 0, 0], ScreenBlock::Tileset, 23, 0));
        view.snapshot.course.backgrounds =
            vec![NsmbLevelScreen { screen_id: 2, palette_id: 2, ..Default::default() }, NsmbLevelScreen { id: 1, ..Default::default() }];
        view.snapshot.course.header.background_theme[0] = 2;
        let shown_rows = rows(&view, &view.panel, None);
        let Some(LevelEdit::SetLayer { header, block, screens, .. }) = apply(&view, &view.panel, &shown_rows, 12, 5) else { panic!() };
        assert_eq!((header.background_theme[0], block), (5, ScreenBlock::Background));
        assert_eq!((screens[0].palette_id, screens[0].screen_id, &screens[1]), (5, 5, &NsmbLevelScreen { id: 1, ..Default::default() }));
        // A record that does not follow its graphics is warned of, as B03_2's background palette is none.
        view.snapshot.course.backgrounds[0].palette_id = u16::MAX;
        let shown_rows = rows(&view, &view.panel, None);
        assert!(shown_rows.rows[12].warning.contains("palette none, not 2"), "{}", shown_rows.rows[12].warning);
        // A layer with no record has nothing to follow it, so picking writes only the header.
        view.snapshot.course.foregrounds.clear();
        let shown_rows = rows(&view, &view.panel, None);
        assert!(shown_rows.rows[13].warning.contains("no foreground entry"));
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 13, 5) else { panic!() };
        assert_eq!(header.foreground_theme[0], 5);
        // The advanced mode shows all three of the header's sets: a tileset, a background and a foreground together
        // for each, each picked by name.
        view.panel.advanced = true;
        let shown_rows = rows(&view, &view.panel, None);
        let labels: Vec<&str> = shown_rows.rows[12..21].iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels[..4], ["Tileset", "BG graphics", "FG graphics", "Tileset"]);
        assert_eq!(labels[8], "FG graphics");
        let headings: Vec<&str> = shown_rows.rows.iter().map(|row| row.heading.as_str()).filter(|h| !h.is_empty()).collect();
        assert_eq!(headings, ["Graphics set 1", "Graphics set 2", "Graphics set 3"]);
        assert_eq!(shown_rows.rows[15].heading, "Graphics set 2");
        assert!(matches!(shown_rows.rows[18].editor, Editor::Choice { .. }));
        // Only the advanced mode sets the level's tileset to none, and it warns of it; the other two sets' may be none.
        let Editor::Choice { choices, .. } = &shown_rows.rows[12].editor else { panic!() };
        assert_eq!(choices[0], (65535, "None".to_owned()));
        view.snapshot.course.header.tileset_theme = [u16::MAX; 3];
        let none_rows = rows(&view, &view.panel, None);
        assert!(none_rows.rows[12].warning.contains("crash"));
        assert_eq!(none_rows.rows[15].warning, "");
        view.snapshot.course.header.tileset_theme = [9, 0, 0];
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 15, 4) else { panic!() };
        assert_eq!(header.tileset_theme[1], 4);
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 16, 7) else { panic!() };
        assert_eq!(header.background_theme[1], 7);
        let Some(LevelEdit::SetHeader { header, .. }) = apply(&view, &view.panel, &shown_rows, 20, 5) else { panic!() };
        assert_eq!(header.foreground_theme[2], 5);
        view.panel.advanced = false;
        let Editor::Choice { choices: drops, .. } = &shown_rows.rows[11].editor else { panic!() };
        assert_eq!(drops[2], (2, "None".to_owned()));
        view.panel_tab(Tab::Banks);
        view.snapshot.course.object_banks[0] = 3;
        let mut overlays = [[0; 10]; 16];
        overlays[0][2] = 77;
        view.snapshot.course.object_banks[12] = 2;
        // Every slot the game loads banks into shows at once, and any other the level sets, each picked by name.
        let shown_rows = rows(&view, &view.panel, Some(&overlays));
        let labels: Vec<&str> = shown_rows.rows.iter().map(|row| row.label.as_str()).collect();
        assert_eq!(labels[9..], ["Slot 9", "Slot 12", "Slot 15"]);
        let Editor::Choice { choices, .. } = &shown_rows.rows[0].editor else { panic!() };
        assert_eq!((shown_rows.rows[0].value, &choices[3].1), (Some(3), &"Enemy Castle (overlay 77)".to_owned()));
        assert_eq!(apply(&view, &view.panel, &shown(&view), 0, 4), Some(LevelEdit::SetObjectBank { slot: 0, value: 4 }));
        assert_eq!(apply(&view, &view.panel, &shown(&view), 11, 5), Some(LevelEdit::SetObjectBank { slot: 15, value: 5 }));
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
        let Editor::Choice { choices: types, .. } = &rows.rows[ENTRANCE_TYPE_ROW].editor else { panic!() };
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
