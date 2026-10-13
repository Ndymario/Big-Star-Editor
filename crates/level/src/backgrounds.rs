//! A level's two scrolling layers as the game treats them: what a background script says about one, and where a view
//! puts it. Pure data and arithmetic; the scripts are loaded in `bigstar_project` and the contract is
//! `docs/lua-backgrounds.md`.
//!
//! The level format calls block 2 the background and block 4 the foreground, and so does this. On the hardware the
//! background is BG1 and the foreground BG3, both behind the level, sharing one block of 8bpp tiles: the foreground's
//! graphics load at tile 256 and the background's at 576.

use bigstar_formats::{NsmbLevelCamera, NsmbLevelEntrance, NsmbLevelScreen, NsmbLevelView};
use serde::{Deserialize, Serialize};

/// The game's fixed point: 4096 is 1.0.
pub const BACKGROUND_FX_ONE: i32 = 4096;
/// How many of each layer the game's tables hold.
pub const BACKGROUND_SLOT_COUNT: u16 = 76;
/// A layer's image is a 64x64 screen of 8-pixel tiles.
pub const BACKGROUND_IMAGE_SIZE: i32 = 512;
/// What the game shows of a level at once.
pub const DS_SCREEN_WIDTH: i32 = 256;
pub const DS_SCREEN_HEIGHT: i32 = 192;
/// The bit of the level header's settings that joins the level's two ends.
pub const LEVEL_WRAPS_AROUND: u16 = 0x20;

/// `bgScrollSpeeds`: what the low nibble of a block's `scroll_x` or `scroll_y` multiplies the camera's travel by.
const SCROLL_SPEEDS: [i32; 16] = [4096, 2048, 1024, 512, 0x1333, 3072, 256, 384, 832, 4096, 4096, 4096, 4096, 8192, 16384, 32768];

/// `Entrance::entranceInfoTable`, in pixels: how far right of an entrance and how far below it each type puts the
/// player, and how much further right when the entrance is not aligned (flag 0x40). The game reads past the table for
/// the types after Vine; they are placed here as an entry of zeros would place them.
const ENTRANCE_SPAWNS: [(i32, i32, i32); 22] = [
    (0, 16, 8),
    (0, 0, 8),
    (16, 16, 0),
    (16, 32, 0),
    (16, 16, 0),
    (16, 32, 0),
    (0, 32, 0),
    (0, 0, 8),
    (0, 0, 8),
    (0, 0, 8),
    (0, 0, 0),
    (0, 0, 0),
    (0, 0, 8),
    (0, 0, 0),
    (8, 16, 0),
    (8, 16, 0),
    (8, 16, 0),
    (8, 16, 0),
    (8, 16, 0),
    (8, 16, 0),
    (0, 0, 8),
    (0, 0, 8),
];
const ENTRANCE_ALIGNED: u8 = 0x40;

/// The game's fixed-point product, rounded as `FX_Mul` rounds.
fn fx_mul(a: i32, b: i32) -> i32 {
    ((i64::from(a) * i64::from(b) + 0x800) >> 12) as i32
}

/// The speed a block's `scroll_x` or `scroll_y` names, in fixed point.
pub fn background_scroll_speed(setting: u16) -> i32 {
    SCROLL_SPEEDS.get(usize::from(setting & 0xf)).copied().unwrap_or(BACKGROUND_FX_ONE)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BackgroundLayer {
    #[default]
    Background,
    Foreground,
}

impl BackgroundLayer {
    pub fn name(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Foreground => "foreground",
        }
    }
}

/// How one band of a layer's rows moves sideways. The game rewrites the layer's horizontal offset on every scanline
/// from a table of these (`StageLayout::fillBgParallaxData`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackgroundMotion {
    /// Follows the layer's own scroll, times this.
    Scroll { factor: i32 },
    /// Moves on its own, this many pixels a frame. Bands on one channel share a counter that every one of them adds
    /// to, so they all move at the sum of their speeds.
    Drift { speed: i32, channel: u8 },
    /// Ripples: each row is offset by a sine `step` further round than the row above, turning by `speed` a frame. A
    /// full turn is 65536.
    Wave { amplitude: i32, speed: u16, step: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundStrip {
    /// The last row of the image this band covers; the final band runs to the bottom whatever it says.
    pub through_row: u16,
    pub motion: BackgroundMotion,
}

/// `count` tiles copied from the animation graphics, `from` tiles past the step's own, over tile `to` of a layer's
/// graphics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTileCopy {
    pub layer: BackgroundLayer,
    pub to: u16,
    pub count: u16,
    pub from: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTileStep {
    pub tile: u16,
    pub ticks: u16,
}

/// Tiles the game overwrites as it runs: the lava, torches and flags of a layer.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundTileAnimation {
    pub file: String,
    pub steps: Vec<BackgroundTileStep>,
    pub copies: Vec<BackgroundTileCopy>,
}

/// Colours the game overwrites as it runs: `count` of them from `to`, each frame `stride` colours into the file after
/// the one before.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundPaletteAnimation {
    pub file: String,
    pub ticks: u16,
    pub frames: u16,
    pub to: u16,
    pub count: u16,
    pub stride: u16,
}

/// A quarter of the image that shows another once the layer has scrolled more than `after` (fixed point) sideways:
/// the game rewrites the layer's own map as it runs. Quarters are `(column, row)`, each 0 or 1.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundSwap {
    pub after: i32,
    pub quarter: (u8, u8),
    pub shows: (u8, u8),
}

impl BackgroundSwap {
    /// How many columns from the image's first a still picture goes on showing the quarter as its own: the last a
    /// screen can show before the layer has scrolled past `after`.
    pub fn still_reach(&self) -> i32 {
        (self.after >> 12) + DS_SCREEN_WIDTH
    }
}

/// The quarter a still picture of a layer shows for `quarter`, `distance` pixels right of the view's left edge, where
/// the image's first column lands. The picture is every scroll at once, so a swap holds wherever the game could only
/// show the layer after it: beyond its reach, and left of the view, where the game shows nothing. Peach's castle then
/// stands once at the start of its view instead of in every repeat of the image.
pub fn still_quarter(swaps: &[BackgroundSwap], quarter: (u8, u8), distance: i32) -> (u8, u8) {
    let passed = |swap: &&BackgroundSwap| swap.quarter == quarter && !(0..swap.still_reach()).contains(&distance);
    swaps.iter().rev().find(passed).map_or(quarter, |swap| swap.shows)
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundDefinition {
    pub layer: BackgroundLayer,
    pub id: u16,
    pub name: String,
    /// Top to bottom. Empty when the whole layer scrolls as one.
    pub strips: Vec<BackgroundStrip>,
    pub tiles: Option<BackgroundTileAnimation>,
    pub palette: Option<BackgroundPaletteAnimation>,
    /// In order; where two name one quarter, the later wins.
    pub swaps: Vec<BackgroundSwap>,
}

impl BackgroundDefinition {
    pub fn animated(&self) -> bool {
        self.tiles.is_some() || self.palette.is_some() || self.strips.iter().any(|strip| !matches!(strip.motion, BackgroundMotion::Scroll { .. }))
    }
}

/// Definitions in layer order with the first for a layer and id winning, so a module renames or reanimates one
/// background without restating the rest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundCatalog {
    pub definitions: Vec<BackgroundDefinition>,
}

impl BackgroundCatalog {
    /// The game indexes its tables with the low byte of a screen id, so that is what names a definition.
    pub fn find(&self, layer: BackgroundLayer, screen_id: u16) -> Option<&BackgroundDefinition> {
        let id = screen_id & 0xff;
        self.definitions.iter().find(|definition| definition.layer == layer && definition.id == id)
    }

    pub fn name(&self, layer: BackgroundLayer, screen_id: u16) -> Option<&str> {
        self.find(layer, screen_id).map(|definition| definition.name.as_str())
    }

    /// One layer's definitions in id order.
    pub fn layer(&self, layer: BackgroundLayer) -> Vec<&BackgroundDefinition> {
        let mut found: Vec<&BackgroundDefinition> = self.definitions.iter().filter(|definition| definition.layer == layer).collect();
        found.sort_by_key(|definition| definition.id);
        found
    }
}

/// How a layer's image continues past its own 512 rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerRepeat {
    /// The whole image again.
    Wrap,
    /// Its top half again above it, as a sky that goes on up (vertical mode 4).
    TopHalfAbove,
    /// Its bottom half again below it (vertical mode 5).
    BottomHalfBelow,
}

/// Where a view shows a layer, with the camera resting at the edge the layer is anchored to. A layer that scrolls
/// slower than the level sits somewhere else once the camera moves, which a still picture cannot show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerPlacement {
    pub screen_id: u16,
    /// The level row the image's first row lands on. Its first column lands on the view's left edge.
    pub origin_y: i32,
    pub repeat: LayerRepeat,
}

/// The block a view names, or the first when none has that id, which is what the game's own lookup falls back to.
pub fn layer_block(screens: &[NsmbLevelScreen], block_id: u8) -> Option<&NsmbLevelScreen> {
    screens.iter().find(|screen| screen.id == u16::from(block_id)).or_else(|| screens.first())
}

/// A port of the vertical half of `StageLayout::setupStuffForPlayer` and the six scroll modes it dispatches to: a
/// block's `scroll_y` holds a speed in its low nibble and a mode in the next, and its `vertical_offset` moves the
/// image in steps of eight pixels. `None` when the view shows no such layer, which is how the ghost houses drop the
/// foreground outside. A level with no block at all for the layer is placed as a block of zeros would place it.
pub fn layer_placement(screens: &[NsmbLevelScreen], block_id: u8, view_y: i32, view_height: i32) -> Option<LayerPlacement> {
    let none = NsmbLevelScreen::default();
    let block = layer_block(screens, block_id).unwrap_or(&none);
    if block.screen_id == u16::MAX || block.scroll_y == u16::MAX {
        return None;
    }
    let mode = (block.scroll_y >> 4) & 0xf;
    let top_anchored = mode & 1 != 0;
    let mut base = i32::from(block.vertical_offset as i16) * 8 + if top_anchored { 32 } else { -32 };
    // Mode 4 never scrolls past the image's last row, so an offset that would lift it off the view's floor does not.
    if mode == 4 {
        base = base.min(0);
    }
    let origin_y = if top_anchored { view_y - base } else { view_y + view_height - BACKGROUND_IMAGE_SIZE - base };
    let repeat = match mode {
        4 => LayerRepeat::TopHalfAbove,
        5 => LayerRepeat::BottomHalfBelow,
        _ => LayerRepeat::Wrap,
    };
    Some(LayerPlacement { screen_id: block.screen_id, origin_y, repeat })
}

/// How far each band of `strips` has moved after `ticks` frames with the layer scrolled `scroll` (fixed point)
/// sideways, as `(first row, one past the last row, pixels the band has moved left)`, top to bottom over an image
/// `height` rows tall. A waving band comes back as one entry per `wave_rows` rows. Whole pixels, as the hardware
/// scrolls.
pub fn strip_offsets(strips: &[BackgroundStrip], height: u16, scroll: i32, ticks: u64, wave_rows: u16) -> Vec<(u16, u16, f32)> {
    let whole = |fixed: i64| (fixed >> 12) as f32;
    let mut result = Vec::new();
    let mut first = 0u16;
    for (index, strip) in strips.iter().enumerate() {
        if first >= height {
            break;
        }
        let end = if index + 1 == strips.len() { height } else { strip.through_row.saturating_add(1).min(height) };
        if end <= first {
            continue;
        }
        match strip.motion {
            BackgroundMotion::Scroll { factor } => result.push((first, end, whole(i64::from(fx_mul(scroll, factor))))),
            BackgroundMotion::Drift { channel, .. } => {
                let speed: i64 = strips
                    .iter()
                    .filter_map(|other| match other.motion {
                        BackgroundMotion::Drift { speed, channel: theirs } if theirs == channel => Some(i64::from(speed)),
                        _ => None,
                    })
                    .sum();
                let moved = (speed * (ticks % (1 << 32)) as i64).rem_euclid(i64::from(BACKGROUND_IMAGE_SIZE) * i64::from(BACKGROUND_FX_ONE));
                result.push((first, end, whole(moved + i64::from(scroll))));
            }
            BackgroundMotion::Wave { amplitude, speed, step } => {
                let rows = wave_rows.max(1);
                let mut row = first;
                while row < end {
                    let next = row.saturating_add(rows).min(end);
                    let angle = (u64::from(speed) * ticks + u64::from(step) * u64::from(row - first)) % 65536;
                    let turn = angle as f32 / 65536.0 * std::f32::consts::TAU;
                    result.push((row, next, whole(i64::from(scroll)) + whole((f64::from(turn.sin()) * f64::from(amplitude)).round() as i64)));
                    row = next;
                }
            }
        }
        first = end;
    }
    if result.is_empty() {
        result.push((0, height, whole(i64::from(scroll))));
    }
    result
}

/// A view's edges in level pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ViewFrame {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl ViewFrame {
    pub fn of(view: &NsmbLevelView) -> Self {
        let (left, top) = (i32::from(view.x), i32::from(view.y));
        Self { left, top, right: left + i32::from(view.width), bottom: top + i32::from(view.height) }
    }
}

/// Where the player stands on coming in through an entrance (`Entrance::accessEntrance`): the point the camera is
/// placed by.
pub fn entrance_spawn_point(entrance: &NsmbLevelEntrance) -> (i32, i32) {
    let (right, down, unaligned) = ENTRANCE_SPAWNS.get(usize::from(entrance.kind)).copied().unwrap_or_default();
    let x = i32::from(entrance.x) + right + if entrance.flags & ENTRANCE_ALIGNED == 0 { unaligned } else { 0 };
    (x, i32::from(entrance.y) + down)
}

/// The camera settings a view names, or the first when none has that id, as `GetCamera` falls back.
pub fn camera_block(cameras: &[NsmbLevelCamera], id: u8) -> Option<&NsmbLevelCamera> {
    cameras.iter().find(|camera| camera.id & 0xff == u16::from(id)).or_else(|| cameras.first())
}

/// The top left corner of the screen when the player comes in through `entrance` into `view`, at the normal zoom.
///
/// Sideways the player is centred and the camera then kept inside the view, unless the level wraps around and is at
/// least 257 tiles wide. Upright the game moves the camera only as far as brings the player between two lines, 64
/// pixels plus the camera settings' top offset and 128 plus their bottom offset below the screen's top, so the answer
/// depends on where the camera was: `previous_y` is that, and `None` is a level that has just loaded, whose camera
/// starts at the top of the map. The view's floor then wins over its ceiling.
pub fn entrance_camera(
    entrance: &NsmbLevelEntrance,
    view: ViewFrame,
    camera: Option<&NsmbLevelCamera>,
    wraps: bool,
    previous_y: Option<i32>,
) -> (i32, i32) {
    let (player_x, player_y) = entrance_spawn_point(entrance);
    let wanted = player_x - DS_SCREEN_WIDTH / 2;
    let mut x = wanted;
    if !wraps || (view.right - view.left) / 16 - 1 < 0x100 {
        if wanted <= view.left {
            x += view.left - wanted;
        }
        if view.right < wanted + DS_SCREEN_WIDTH {
            x -= wanted + DS_SCREEN_WIDTH - view.right;
        }
    }

    let (top_offset, bottom_offset) = camera.map_or((0, 0), |camera| (camera.top_offset, camera.bottom_offset));
    let previous = previous_y.unwrap_or(0);
    let (upper, lower) = (previous + 64 + top_offset, previous + 128 + bottom_offset);
    let mut y =
        if upper < player_y { if player_y < lower { previous } else { player_y - (128 + bottom_offset) } } else { player_y - (64 + top_offset) };
    let floor = view.bottom - DS_SCREEN_HEIGHT;
    if y >= floor {
        y = floor;
    } else if y < view.top {
        y = view.top;
    }
    (x, y)
}

/// Which quarters of a layer's image the hardware is showing. The game swaps them as a layer scrolls, which is how one
/// image stands for a sky that goes on up or rock that goes on down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerSampling {
    /// All four, repeating.
    Whole,
    /// The top half over and over (vertical mode 4, scrolled above the image).
    TopHalf,
    /// The bottom half over and over (the foreground in vertical mode 5, scrolled below the image).
    BottomHalf,
    /// The bottom left quarter above the bottom right one, 256 pixels wide: what the background's mode 5 really
    /// shows, which is the bottom half again only for an image that repeats every 256 pixels.
    BottomQuarters,
}

/// A layer exactly as the game scrolls it with the camera somewhere in a view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayerScroll {
    pub screen_id: u16,
    /// How far the layer has scrolled sideways, in fixed point; the hardware takes its whole pixels, band by band.
    pub x: i32,
    /// The image row at the top of the screen. Negative above the image.
    pub y: i32,
    pub sampling: LayerSampling,
    /// The quarters showing another, as `(quarter, what it shows)`: the layer's swaps that the scroll has passed.
    pub swapped: Vec<((u8, u8), (u8, u8))>,
}

impl LayerScroll {
    /// The quarter of the image, as `(column, row)` each 0 or 1, that shows `column` 256-pixel blocks right and `row`
    /// blocks down of the image's own top left corner.
    pub fn quarter(&self, column: i32, row: i32) -> (u8, u8) {
        let (column, row) = (column.rem_euclid(2) as u8, row.rem_euclid(2) as u8);
        let quarter = match self.sampling {
            LayerSampling::Whole => (column, row),
            LayerSampling::TopHalf => (column, 0),
            LayerSampling::BottomHalf => (column, 1),
            LayerSampling::BottomQuarters => (row, 1),
        };
        self.swapped.iter().rev().find(|swap| swap.0 == quarter).map_or(quarter, |swap| swap.1)
    }

    /// With the swaps of the layer's script that this scroll has passed.
    #[must_use]
    pub fn with_swaps(mut self, swaps: &[BackgroundSwap]) -> Self {
        self.swapped = swaps.iter().filter(|swap| self.x > swap.after).map(|swap| (swap.quarter, swap.shows)).collect();
        self
    }
}

/// A port of what `StageLayout::doSomeBgCameraThings` works out for one layer each frame: `computeBottomBgScrolling`
/// or `computeTopBgScrolling` sideways, and the six vertical modes. `camera` is the screen's top left corner in level
/// pixels. `clamped` is false in a level that wraps around, where the layer scrolls on past the view's width. `None`
/// when the view shows no such layer.
///
/// A layer's script may swap quarters of the image once it has scrolled far enough; `LayerScroll::with_swaps` adds
/// those.
///
/// Left out: a `scroll_x` of 0x100 or more also drifts while the level is being cleared, and the zoom some entrances
/// set.
pub fn layer_scroll(
    layer: BackgroundLayer,
    screens: &[NsmbLevelScreen],
    block_id: u8,
    view: ViewFrame,
    camera: (i32, i32),
    clamped: bool,
) -> Option<LayerScroll> {
    let none = NsmbLevelScreen::default();
    let block = layer_block(screens, block_id).unwrap_or(&none);
    if block.screen_id == u16::MAX || block.scroll_y == u16::MAX {
        return None;
    }
    let pixels = |value: i32| value.saturating_mul(BACKGROUND_FX_ONE);

    let speed_x = background_scroll_speed(block.scroll_x);
    let mut x = fx_mul(pixels(camera.0 - view.left), speed_x);
    if clamped {
        x = x.clamp(0, fx_mul(pixels(view.right - view.left), speed_x).max(0));
    }

    let mode = (block.scroll_y >> 4) & 0xf;
    let top_anchored = mode & 1 != 0;
    let speed_y = background_scroll_speed(block.scroll_y);
    let base = pixels(i32::from(block.vertical_offset as i16) * 8 + if top_anchored { 32 } else { -32 });
    let mut y = if top_anchored {
        fx_mul(pixels(camera.1 - view.top), speed_y) + base
    } else {
        pixels(320) - fx_mul(pixels(view.bottom - DS_SCREEN_HEIGHT - camera.1), speed_y) + base
    };
    let mut sampling = LayerSampling::Whole;
    match mode {
        4 => {
            if y < pixels(16) {
                sampling = LayerSampling::TopHalf;
            }
            y = y.min(pixels(320));
        }
        5 if y + pixels(DS_SCREEN_HEIGHT) > pixels(496) => {
            sampling = if layer == BackgroundLayer::Background { LayerSampling::BottomQuarters } else { LayerSampling::BottomHalf };
        }
        _ => {}
    }
    Some(LayerScroll { screen_id: block.screen_id, x, y: y >> 12, sampling, swapped: Vec::new() })
}
