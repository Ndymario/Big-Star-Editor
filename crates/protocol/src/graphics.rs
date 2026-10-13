//! Graphics as they cross the wire: the host decodes, the frontend paints. Pixels travel once per content hash and the
//! frontend keeps them; everything else names them by [`TextureId`].

use bigstar_formats::ObjectLayout;
use bigstar_level::{SpriteBounds, SpriteDefinition};
use serde::{Deserialize, Serialize};

/// A texture's content hash. The same pixels always have the same id, so a frontend that holds one never asks for it
/// again, whichever level or actor named it.
pub type TextureId = String;

/// Straight RGBA, row by row, `width * height * 4` bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureData {
    pub id: TextureId,
    pub width: u32,
    pub height: u32,
    #[serde(with = "serde_bytes")]
    pub rgba: Vec<u8>,
}

/// How many Map16 blocks a tileset atlas holds per row; each block is 16x16 pixels.
pub const ATLAS_COLUMNS: u32 = 32;

/// One tileset slot of a level: every Map16 block packed into one atlas, block `i` at column `i % ATLAS_COLUMNS` and
/// row `i / ATLAS_COLUMNS`, and the object layouts the frontend rasterises the tilemap from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilesetGraphics {
    pub atlas: Option<TextureId>,
    pub blocks: u32,
    pub objects: Vec<ObjectLayout>,
    /// Why this slot did not decode. The other slots still render.
    pub error: Option<String>,
}

/// A decoded image or why there is none.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageGraphics {
    pub texture: Option<TextureId>,
    pub error: Option<String>,
}

/// Everything the canvas draws a level's tiles from. `generation` grows whenever a watched file changes; an answer
/// from an older generation than one the frontend has already seen is stale and is dropped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelGraphics {
    pub level: crate::LevelId,
    pub generation: u64,
    pub tilesets: [TilesetGraphics; 3],
    pub background: ImageGraphics,
    pub foreground: ImageGraphics,
    /// Set when the level's resources could not be found at all; nothing else is filled then.
    pub error: Option<String>,
}

/// One entry of the actor picker and the properties panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorInfo {
    pub definition: SpriteDefinition,
    pub has_renderer: bool,
    /// Why `init` failed, if it did; the actor keeps its metadata and draws as the stock marker.
    pub init_error: Option<String>,
}

/// The actors the open project knows, and the map from sprite type to the class id they are keyed by.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorCatalog {
    pub generation: u64,
    pub actors: Vec<ActorInfo>,
    /// `sprite_object_ids[type]` is the runtime class id of sprite type `type`, read from the ROM's own table. Empty
    /// when the ROM has no table the host knows.
    pub sprite_object_ids: Vec<u32>,
}

/// One sprite as an actor script sees it. The host finds the actor: by `hash` for a module sprite, otherwise by the
/// class id its type maps to.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActorInput {
    pub sprite_type: u16,
    pub hash: Option<u32>,
    pub x: i32,
    pub y: i32,
    pub pixel_x: f64,
    pub pixel_y: f64,
    pub settings: u32,
    /// The target and trigger events, `events[0]` and `events[1]` of the level format.
    pub events: [u8; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DrawKind {
    #[default]
    Image,
    Text,
    FillRect,
    StrokeRect,
    FillEllipse,
    Line,
}

/// `LuaActorDrawCommand` with its image replaced by a texture id.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DrawCommand {
    pub kind: DrawKind,
    pub texture: Option<TextureId>,
    pub text: String,
    pub source: [i32; 2],
    pub source_size: [u32; 2],
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub opacity: f64,
    pub x2: f64,
    pub y2: f64,
    pub stroke_width: f64,
    /// Non-premultiplied 0xRRGGBBAA.
    pub color: u32,
    pub flip_x: bool,
    pub flip_y: bool,
}

/// What one sprite draws and where it can be grabbed. `found` is false when no actor speaks for the sprite, in which
/// case the frontend draws the stock marker at the tile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ActorDraw {
    pub found: bool,
    pub commands: Vec<DrawCommand>,
    /// False means the script wants the stock numbered marker drawn over whatever it drew.
    pub custom_rendered: bool,
    pub grab: Option<SpriteBounds>,
    pub render: Option<SpriteBounds>,
    pub error: Option<String>,
}

/// What the host remembers about a project between sessions, in the project directory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSettings {
    /// The build variant whose module layers the editor reads and writes. Empty for the base layer.
    pub active_variant: String,
    /// The module a new edit lands in.
    pub active_module: String,
    /// Where a build leaves its generated module data, relative to the project.
    pub generated_path: String,
    /// The variants NCPatcher reports the project configures, in declaration order. Empty when it configures none
    /// or NCPatcher could not be asked; `patcher_error` says which.
    pub variants: Vec<String>,
    /// The enabled modules, in `modules.enabled` order.
    pub modules: Vec<String>,
    pub patcher_error: Option<String>,
    /// How often unsaved level edits are written to a recovery snapshot, in minutes (1 to 60).
    pub autosave_minutes: u32,
    /// How many level-save and patch-import backups of each kind are kept (1 to 100).
    pub backups_kept: u32,
}
