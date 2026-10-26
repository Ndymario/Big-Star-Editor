//! A host that lives inside the frontend, with one made-up project and one made-up level. It exists so screenshots and
//! tests have something to show without a ROM. The level is a real `EditableLevel`, so every edit, its undo name and
//! its outcome are the ones a real host gives; the tileset art and the actors are drawn up here.

use std::collections::{BTreeMap, HashMap};

use bigstar_client::{Connection, ConnectionState, HostInfo, Incoming};
use bigstar_core::{Error, ErrorCode};
use bigstar_formats::{
    NSMB_LEVEL_BLOCK_COUNT, NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelStageObject, NsmbLevelView, NsmbLevelZone,
    OBJECT_LAYOUT_EMPTY_TILE, ObjectLayout, ObjectLayoutTile, RgbaImage, fnv1a_32,
};
use bigstar_level::{
    BackgroundCatalog, BackgroundDefinition, BackgroundLayer, EditableLevel, LevelDataDeclaration, LevelDataKind, LevelEntityInsertions,
    ModuleDataCatalog, SpriteBounds, SpriteDefinition, SpriteFieldChoice, SpriteFieldDefinition, SpriteInsertion, StageObjectDeclaration,
};
use bigstar_protocol::{
    ATLAS_COLUMNS, ActorCatalog, ActorDraw, ActorInfo, ActorInput, AreaSummary, DirectoryEntry, DirectoryEntryKind, DirectoryListing, DrawCommand,
    DrawKind, EditOutcome, Event, FileEntry, FileIcon, FileStatus, FilesystemListing, FoundEntity, ImageGraphics, LevelBytes, LevelEdit,
    LevelGraphics, LevelId, LevelSnapshot, LevelSummary, LevelWorld, LogLevel, ModuleChoice, ProjectData, ProjectInfo, ProjectMode, ProjectSummary,
    RecoverableLevel, Request, RequestId, Response, ResponseResult, TextureData, TilesetGraphics,
};

const AREA: &str = "A01_1";
/// A second, smaller area, so there is more than one document to open.
const SECOND_AREA: &str = "A01_2";
const BACKGROUND: &str = "demo-background";
const WALKER: &str = "demo-walker";
/// The demo module's one actor.
pub const SIGNPOST: u32 = 0x5157_0001;

/// One tileset slot's art: a 16x16 image per block and the objects laid out of them.
#[derive(Clone)]
pub struct DemoTileset {
    pub blocks: Vec<RgbaImage>,
    pub objects: Vec<ObjectLayout>,
    /// Each block's behaviour word, or none.
    pub behaviors: Vec<u32>,
}

struct DemoLevel {
    area: &'static str,
    title: &'static str,
    level: EditableLevel,
    revision: u64,
}

pub struct DemoConnection {
    state: ConnectionState,
    next_id: RequestId,
    inbox: Vec<Incoming>,
    /// Events the request being answered raised; they follow its response.
    events: Vec<Incoming>,
    /// The demo's levels; a level's id is its place here plus one. Empty only if they failed to build, which an empty
    /// course never does; every level request then fails.
    levels: Vec<DemoLevel>,
    tilesets: [Option<DemoTileset>; 3],
    background: bool,
    textures: HashMap<String, TextureData>,
    /// Areas the demo pretends an earlier session left autosaved.
    recoverable: Vec<String>,
    /// The demo ROM's other files, and what a module has written over them, by ROM address.
    files: BTreeMap<String, DemoFile>,
    /// The ROM banner, as the configured banner file would hold it.
    banner: Vec<u8>,
    settings: bigstar_protocol::ProjectSettings,
}

struct DemoFile {
    id: Option<i64>,
    member_id: Option<i64>,
    bytes: Vec<u8>,
    /// The module that wrote it, if one did.
    module: Option<String>,
}

/// A few files beside the levels, so the Filesystem dock has a ROM to show: a message file, a palette, an archive.
fn demo_files() -> BTreeMap<String, DemoFile> {
    let file = |id, bytes: Vec<u8>| DemoFile { id: Some(id), member_id: None, bytes, module: None };
    let member = |id, bytes: Vec<u8>| DemoFile { id: None, member_id: Some(id), bytes, module: None };
    let mut files = BTreeMap::new();
    files.insert("00DUMMY".to_owned(), file(0, vec![0; 16]));
    files.insert("BG_ncl/d_2d_A_J_jyotyu_ncl.bin".to_owned(), file(30, [b"RLCN\xff\xfe".to_vec(), vec![0; 26]].concat()));
    files.insert("message/msg.bmg".to_owned(), file(40, demo_messages()));
    // A touch layout of two rectangles, and the ROM's user metadata, which only file 131 is.
    files.insert("uiStudio/demo.bnbl".to_owned(), file(60, b"JNBL\0\0\x02\0\x10\0\x20\0\x40\x30\x80\0\x90\0\x60\x20".to_vec()));
    files.insert("NSMBeUD".to_owned(), file(131, b"[Music]\n0=Overworld\n1=Underground\n[12]\n3=Pipe\n".to_vec()));
    // A headerless tile sheet and the palette beside it: two 4bpp tiles of index 2, and sixteen colours.
    files.insert("BG_ncg/demo_ncg.bin".to_owned(), file(20, vec![0x22; 64]));
    // Colour 1 red, colour 2 green.
    let palette = [[0, 0], 0x001fu16.to_le_bytes(), 0x03e0u16.to_le_bytes()].into_iter().flatten().chain(std::iter::repeat_n(0, 26)).collect();
    files.insert("BG_ncg/demo_ncl.bin".to_owned(), file(21, palette));
    files.insert("dat/demo.narc".to_owned(), file(50, [b"NARC\xfe\xff".to_vec(), vec![0; 26]].concat()));
    // The files behind the levels' tilesets and backgrounds, for the tileset workspace.
    let mut id = 70;
    for (slot, page) in [(0usize, 0u8), (1, 1), (2, 4)] {
        if let Ok(data) = demo_tileset_files(page) {
            let names = [
                "BG_ncg/demo_t{}_ncg.bin",
                "BG_ncl/demo_t{}_ncl.bin",
                "BG_pnl/demo_t{}_pnl.bin",
                "BG_unt/demo_t{}.bin",
                "BG_unt/demo_t{}_hd.bin",
                "BG_chk/demo_t{}_chk.bin",
            ];
            let parts = [data.graphics, data.palette, data.map16, data.objects, data.object_index, data.behaviors];
            for (name, bytes) in names.into_iter().zip(parts) {
                files.insert(name.replace("{}", &slot.to_string()), file(id, bytes));
                id += 1;
            }
        }
    }
    for (name, data) in [("bg", demo_tilemap_files(false)), ("fg", demo_tilemap_files(true))] {
        let Ok(data) = data else { continue };
        for (kind, bytes) in [("ncg", data.graphics), ("ncl", data.palette), ("nsc", data.screen)] {
            files.insert(format!("BG_{kind}/demo_{name}_{kind}.bin"), file(id, bytes));
            id += 1;
        }
    }
    files.insert("dat/demo.narc!a.bin".to_owned(), member(0, vec![1, 2, 3, 4]));
    files.insert("dat/demo.narc!b.bin".to_owned(), member(1, vec![5, 6]));
    files
}

/// The demo's message file: two messages with MID1 IDs and four attribute bytes each, one opening with an escape.
pub(crate) fn demo_messages() -> Vec<u8> {
    use bigstar_formats::{Bmg, BmgMessage, BmgPart, write_bmg};
    let bmg = Bmg {
        attribute_size: 4,
        has_mid1: true,
        messages: vec![
            BmgMessage {
                parts: vec![BmgPart::escape(255, vec![0, 0, 1, 0]), BmgPart::text(" Luigi has appeared!")],
                attributes: vec![1, 0, 0, 0],
                id: Some(40),
            },
            BmgMessage { parts: vec![BmgPart::text("Welcome!\nPress A to start.")], attributes: vec![2, 0, 0, 0], id: Some(41) },
            BmgMessage { parts: vec![BmgPart::text("Game over")], attributes: vec![0, 0, 0, 0], id: Some(42) },
        ],
        ..Bmg::default()
    };
    write_bmg(&bmg).unwrap_or_default()
}

/// A version 1 banner: a round 32×32 icon in three colours and six titles.
fn demo_banner() -> Vec<u8> {
    let mut banner = vec![0u8; 0x840];
    if let Some(version) = banner.get_mut(0..2) {
        version.copy_from_slice(&1u16.to_le_bytes());
    }
    for y in 0..32usize {
        for x in 0..32usize {
            let distance = (x as i32 * 2 - 31).pow(2) + (y as i32 * 2 - 31).pow(2);
            let index: u8 = if distance < 22 * 22 {
                1
            } else if distance < 28 * 28 {
                2
            } else {
                0
            };
            let pixel = ((y / 8) * 4 + x / 8) * 64 + (y % 8) * 8 + x % 8;
            if let Some(byte) = banner.get_mut(0x20 + pixel / 2) {
                *byte |= index << ((pixel & 1) * 4);
            }
        }
    }
    for (i, colour) in [0x7fffu16, 0x027f, 0x001f].into_iter().enumerate() {
        put_u16(&mut banner, 0x220 + i * 2, colour);
    }
    for language in 0..6 {
        for (i, unit) in "Big Star Demo\nNintendo".encode_utf16().enumerate() {
            put_u16(&mut banner, 0x240 + language * 0x100 + i * 2, unit);
        }
    }
    banner
}

/// The sample tileset's blocks as the game stores a tileset: each block four 8×8 tiles of an 8bpp sheet, its colours
/// gathered into one palette, a Map16 entry per block, the sample objects, and solid behaviour for the solid blocks.
fn demo_tileset_files(page: u8) -> Result<bigstar_formats::TilesetResourceData, Error> {
    use bigstar_formats::{Bgr555Color, EditableTileset, IndexedTile8Bpp, Map16Tile, ScreenMapEntry, write_tileset};
    let art = sample_tileset();
    let mut palette = vec![Bgr555Color::new(0, 0, 0, true)];
    let mut index_of = |[r, g, b, a]: [u8; 4]| -> u8 {
        if a == 0 {
            return 0;
        }
        let colour = Bgr555Color::new(r & 0xf8, g & 0xf8, b & 0xf8, false);
        match palette.iter().position(|c| *c == colour) {
            Some(i) => u8::try_from(i).unwrap_or(u8::MAX),
            None if palette.len() < 256 => {
                palette.push(colour);
                u8::try_from(palette.len() - 1).unwrap_or(u8::MAX)
            }
            None => u8::try_from(palette.len() - 1).unwrap_or(u8::MAX),
        }
    };
    let mut graphics = Vec::new();
    let mut map16 = Vec::new();
    for block in &art.blocks {
        let mut tiles = [ScreenMapEntry::default(); 4];
        for (quadrant, entry) in tiles.iter_mut().enumerate() {
            let mut tile = IndexedTile8Bpp::default();
            for (i, pixel) in tile.pixels.iter_mut().enumerate() {
                let (x, y) = ((quadrant % 2) * 8 + i % 8, (quadrant / 2) * 8 + i / 8);
                let at = (y * block.width + x) * 4;
                let rgba = block.pixels.get(at..at + 4).and_then(|p| <[u8; 4]>::try_from(p).ok()).unwrap_or_default();
                *pixel = index_of(rgba);
            }
            *entry = ScreenMapEntry::new(i32::try_from(graphics.len()).unwrap_or(0), 0, false, false);
            graphics.push(tile);
        }
        map16.push(Map16Tile { tiles });
    }
    palette.resize(256, Bgr555Color::new(0, 0, 0, false));
    let behaviors = (0..map16.len()).map(|i| if i >= 7 { 0x0001_0000 } else { 0 }).collect();
    let tileset = EditableTileset { tile_page_offset: page, graphics, palette, map16, objects: art.objects, behaviors, ..EditableTileset::default() };
    write_tileset(&tileset)
}

/// A background of sky and distant hills, or a foreground of bushes over clear tiles, quantised the way an imported
/// PNG is.
fn demo_tilemap_files(foreground: bool) -> Result<bigstar_formats::TilemapResourceData, Error> {
    use bigstar_formats::{EditableTilemap, RgbaImage, import_tilemap_image, write_tilemap};
    // 64×16 tiles: the 1024 a screen holds, 64 across as the game lays backgrounds out.
    let (width, height) = (512usize, 128usize);
    let mut image = RgbaImage { width, height, pixels: Vec::with_capacity(width * height * 4) };
    for y in 0..height {
        for x in 0..width {
            let hill = 80.0 + 22.0 * ((x as f32) / 50.0).sin() + 10.0 * ((x as f32) / 23.0).cos();
            let bush = 104.0 + 12.0 * ((x as f32) / 9.0).sin().abs();
            let pixel = if foreground {
                if (y as f32) > bush { [40, 140, 56, 255] } else { [0, 0, 0, 0] }
            } else if (y as f32) > hill {
                [96, 176, 120, 255]
            } else {
                let band = (y / 16) as u8;
                [96 + band * 12, 160 + band * 9, 232, 255]
            };
            image.pixels.extend(pixel);
        }
    }
    // Numbered from where the game loads the layer, as the game's own screens are.
    let mut map = if foreground {
        EditableTilemap {
            tile_offset: bigstar_formats::FOREGROUND_TILE_OFFSET,
            palette_offset: bigstar_formats::FOREGROUND_PALETTE_OFFSET,
            ..Default::default()
        }
    } else {
        EditableTilemap {
            tile_offset: bigstar_formats::BACKGROUND_TILE_OFFSET,
            palette_offset: bigstar_formats::BACKGROUND_PALETTE_OFFSET,
            ..Default::default()
        }
    };
    import_tilemap_image(&mut map, &image)?;
    write_tilemap(&map)
}

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    if let Some(target) = data.get_mut(offset..offset + 2) {
        target.copy_from_slice(&value.to_le_bytes());
    }
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    if let Some(target) = data.get_mut(offset..offset + 4) {
        target.copy_from_slice(&value.to_le_bytes());
    }
}

/// A course with nothing in it but a zeroed header and one zeroed screen.
fn empty_course() -> Vec<u8> {
    let mut course = vec![0u8; NSMB_LEVEL_BLOCK_COUNT * 8];
    for block in 0..NSMB_LEVEL_BLOCK_COUNT {
        let end = course.len() as u32;
        put_u32(&mut course, block * 8, end);
    }
    put_u32(&mut course, 4, 32);
    course.resize(course.len() + 32, 0);
    let end = course.len() as u32;
    put_u32(&mut course, 16, end);
    put_u32(&mut course, 20, 20);
    course.resize(course.len() + 20, 0);
    course
}

/// A level holding `items` and no history.
fn level_with(items: LevelEntityInsertions) -> Result<EditableLevel, Error> {
    let mut level = EditableLevel::load(&empty_course(), &[0xff, 0xff], None)?;
    level.add_entities(items, "Demo")?;
    EditableLevel::load(&level.serialize_course()?, &level.serialize_bgdat()?, None)
}

fn object(object_id: u16, x: i16, y: i16, width: u16, height: u16) -> NsmbLevelObject {
    NsmbLevelObject { object_id, tileset_slot: 0, x, y, width, height }
}

/// A short stretch of ground with one walker on it.
fn second_items() -> LevelEntityInsertions {
    LevelEntityInsertions {
        objects: vec![object(0, 0, 10, 24, 4), object(1, 8, 6, 4, 1)],
        sprites: vec![SpriteInsertion { value: NsmbLevelStageObject { id: 21, x: 12, y: 9, ..Default::default() }, hash: None }],
        entrances: vec![NsmbLevelEntrance { x: 32, y: 128, ..Default::default() }],
        zones: vec![NsmbLevelZone { width: 384, height: 224, ..Default::default() }],
        ..Default::default()
    }
}

fn sample_items() -> LevelEntityInsertions {
    let sprite = |id, x, y| SpriteInsertion { value: NsmbLevelStageObject { id, x, y, ..Default::default() }, hash: None };
    LevelEntityInsertions {
        objects: vec![
            object(0, 0, 20, 40, 4),
            object(0, 44, 20, 36, 4),
            object(1, 10, 15, 3, 1),
            object(3, 11, 15, 1, 1),
            object(1, 22, 13, 5, 1),
            object(2, 30, 16, 2, 4),
            object(4, 52, 14, 4, 6),
        ],
        sprites: vec![sprite(21, 26, 19), sprite(55, 35, 18), sprite(64, 44, 17), sprite(21, 55, 13)],
        entrances: vec![
            NsmbLevelEntrance { id: 0, x: 48, y: 288, ..Default::default() },
            NsmbLevelEntrance { id: 1, x: 1180, y: 224, ..Default::default() },
        ],
        views: vec![NsmbLevelView { id: 0, x: 0, y: 0, width: 1280, height: 384, ..Default::default() }],
        zones: vec![NsmbLevelZone { id: 0, x: 0, y: 32, width: 1280, height: 352, ..Default::default() }],
        paths: vec![NsmbLevelPath {
            id: 0,
            nodes: [(600, 160), (680, 160), (680, 220)].into_iter().map(|(x, y)| NsmbLevelPathNode { x, y, ..Default::default() }).collect(),
            unknown: 0,
        }],
        ..Default::default()
    }
}

/// A 16x16 block painted by `pixel(x, y)`.
fn block(pixel: impl Fn(usize, usize) -> [u8; 4]) -> RgbaImage {
    let mut pixels = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16 {
        for x in 0..16 {
            pixels.extend_from_slice(&pixel(x, y));
        }
    }
    RgbaImage { width: 16, height: 16, pixels }
}

fn shade([r, g, b]: [u8; 3], by: i16) -> [u8; 4] {
    let channel = |c: u8| (i16::from(c) + by).clamp(0, 255) as u8;
    [channel(r), channel(g), channel(b), 255]
}

fn row(tiles: &[(i32, u8)]) -> Vec<ObjectLayoutTile> {
    tiles.iter().map(|&(tile, control)| ObjectLayoutTile::new(tile, control)).collect()
}

/// Grass, earth, bricks, a question block, a pipe and stone, drawn in code.
pub fn sample_tileset() -> DemoTileset {
    let earth = [176, 112, 64];
    let speckle = |x: usize, y: usize| (x * 7 + y * 13).is_multiple_of(11);
    let mut blocks = vec![block(|_, _| [0, 0, 0, 0]); 7];
    // 7: grass over earth.
    blocks.push(block(|x, y| match y {
        0 => shade([120, 216, 88], 20),
        1..=4 => shade([72, 176, 56], if (x + y) % 5 == 0 { -24 } else { 0 }),
        _ => shade(earth, if speckle(x, y) { -40 } else { 0 }),
    }));
    // 8: earth.
    blocks.push(block(|x, y| shade(earth, if speckle(x, y) { -40 } else { 0 })));
    // 9: bricks.
    blocks.push(block(|x, y| {
        let mortar = y % 8 == 7 || (x + if y < 8 { 0 } else { 8 }) % 16 == 15;
        if mortar { shade([90, 50, 30], 0) } else { shade([200, 96, 48], if y % 8 == 0 { 30 } else { 0 }) }
    }));
    // 10: question block.
    blocks.push(block(|x, y| {
        let border = x == 0 || y == 0 || x == 15 || y == 15;
        let mark = matches!((x, y), (6..=9, 3) | (10, 4..=6) | (8..=9, 7) | (7..=8, 8..=9) | (7..=8, 11..=12));
        if border {
            shade([120, 72, 16], 0)
        } else if mark {
            shade([255, 255, 240], 0)
        } else {
            shade([248, 184, 32], if x == 1 || y == 1 { 30 } else { 0 })
        }
    }));
    let pipe = |x: usize, lip: bool, left: bool| {
        let edge = if left { x == 0 } else { x == 15 };
        let light = if left { (3..6).contains(&x) } else { (10..12).contains(&x) };
        if edge {
            shade([16, 64, 16], 0)
        } else if light {
            shade([152, 232, 120], 0)
        } else {
            shade([48, 168, 48], if lip { 16 } else { 0 })
        }
    };
    // 11-14: a pipe's lip and body, left and right halves.
    blocks.push(block(move |x, y| if y == 0 || y == 15 { shade([16, 64, 16], 0) } else { pipe(x, true, true) }));
    blocks.push(block(move |x, y| if y == 0 || y == 15 { shade([16, 64, 16], 0) } else { pipe(x, true, false) }));
    blocks.push(block(move |x, _| pipe(x, false, true)));
    blocks.push(block(move |x, _| pipe(x, false, false)));
    // 15: stone.
    blocks.push(block(|x, y| {
        let by = if x == 0 || y == 0 {
            40
        } else if x == 15 || y == 15 {
            -50
        } else {
            0
        };
        shade([150, 150, 160], by)
    }));
    let objects = vec![
        ObjectLayout { rows: vec![row(&[(7, 1)]), row(&[(8, 3)])], ..Default::default() },
        ObjectLayout { rows: vec![row(&[(9, 3)])], ..Default::default() },
        ObjectLayout { rows: vec![row(&[(11, 0), (12, 0)]), row(&[(13, 2), (14, 0)])], ..Default::default() },
        ObjectLayout { rows: vec![row(&[(10, 3)])], ..Default::default() },
        ObjectLayout { rows: vec![row(&[(15, 3)])], ..Default::default() },
        ObjectLayout { rows: vec![row(&[(OBJECT_LAYOUT_EMPTY_TILE, 3)])], ..Default::default() },
    ];
    DemoTileset { blocks, objects, behaviors: Vec::new() }
}

/// Every block of a slot packed into one atlas, `ATLAS_COLUMNS` to a row.
fn atlas(id: String, blocks: &[RgbaImage]) -> TextureData {
    let columns = ATLAS_COLUMNS as usize;
    let rows = blocks.len().div_ceil(columns).max(1);
    let (width, height) = (columns * 16, rows * 16);
    let mut rgba = vec![0; width * height * 4];
    for (index, image) in blocks.iter().enumerate() {
        let (bx, by) = ((index % columns) * 16, (index / columns) * 16);
        for y in 0..16usize.min(image.height) {
            let from = y * image.width * 4;
            let to = ((by + y) * width + bx) * 4;
            let length = 16usize.min(image.width) * 4;
            if let (Some(source), Some(target)) = (image.pixels.get(from..from + length), rgba.get_mut(to..to + length)) {
                target.copy_from_slice(source);
            }
        }
    }
    TextureData { id, width: width as u32, height: height as u32, rgba }
}

fn image(id: &str, width: usize, height: usize, pixel: impl Fn(usize, usize) -> [u8; 4]) -> TextureData {
    let mut rgba = Vec::with_capacity(width * height * 4);
    for y in 0..height {
        for x in 0..width {
            rgba.extend_from_slice(&pixel(x, y));
        }
    }
    TextureData { id: id.to_owned(), width: width as u32, height: height as u32, rgba }
}

/// What a project's background scripts would say of the demo's layers: their names, and nothing moving.
fn demo_backgrounds() -> BackgroundCatalog {
    let named = |layer, name: &str| BackgroundDefinition { layer, id: 0, name: name.to_owned(), ..Default::default() };
    BackgroundCatalog { definitions: vec![named(BackgroundLayer::Background, "Demo sky"), named(BackgroundLayer::Foreground, "Demo bushes")] }
}

/// A sky, darker at the top.
fn background() -> TextureData {
    image(BACKGROUND, 64, 64, |_, y| {
        let t = y as f32 / 63.0;
        let mix = |a: f32, b: f32| (a + (b - a) * t) as u8;
        [mix(96.0, 200.0), mix(160.0, 232.0), mix(232.0, 255.0), 255]
    })
}

/// A round walker with two eyes.
fn walker() -> TextureData {
    image(WALKER, 16, 16, |x, y| {
        let (dx, dy) = (x as f32 - 7.5, y as f32 - 8.5);
        let body = dx * dx / 64.0 + dy * dy / 56.0 <= 1.0;
        let eye = (y == 6 || y == 7) && (x == 5 || x == 10);
        let feet = y >= 14 && ((2..6).contains(&x) || (10..14).contains(&x));
        if eye {
            [255, 255, 255, 255]
        } else if feet {
            [40, 24, 16, 255]
        } else if body {
            [150, 84, 40, 255]
        } else {
            [0, 0, 0, 0]
        }
    })
}

fn definition(object_id: u32, name: &str, category: &str) -> SpriteDefinition {
    SpriteDefinition { object_id, name: name.to_owned(), category: category.to_owned(), ..Default::default() }
}

impl DemoConnection {
    pub fn new() -> Self {
        Self::with_level(sample_items())
    }

    /// The demo host as a connection with no workspace sees it: one let in to edit a ROM it uploads.
    pub fn without_workspace() -> Self {
        let mut connection = Self::new();
        connection.state =
            ConnectionState::Connected(HostInfo { host_version: "demo".to_owned(), workspace_name: String::new(), has_workspace: false });
        connection
    }

    /// The demo project as one that has no clean ROM to build from yet, so opening it asks for one.
    pub fn without_clean_rom() -> Self {
        let mut connection = Self::new();
        connection.settings.clean_rom_problem = Some("This project has no clean ROM to build from yet.".to_owned());
        connection
    }

    /// The demo host with its art and a level holding just `items`.
    pub fn with_level(items: LevelEntityInsertions) -> Self {
        let first = level_with(items).or_else(|_| level_with(LevelEntityInsertions::default()));
        let second = level_with(second_items());
        let levels = match (first, second) {
            (Ok(first), Ok(second)) => vec![
                DemoLevel { area: AREA, title: "1-1", level: first, revision: 1 },
                DemoLevel { area: SECOND_AREA, title: "1-1 Area 2", level: second, revision: 1 },
            ],
            _ => Vec::new(),
        };
        let mut connection = Self {
            state: ConnectionState::Connected(HostInfo {
                host_version: "demo".to_owned(),
                workspace_name: "Demo workspace".to_owned(),
                has_workspace: true,
            }),
            next_id: 1,
            inbox: Vec::new(),
            events: Vec::new(),
            levels,
            tilesets: [Some(sample_tileset()), None, None],
            background: true,
            textures: HashMap::new(),
            recoverable: Vec::new(),
            files: demo_files(),
            banner: demo_banner(),
            settings: bigstar_protocol::ProjectSettings {
                active_module: "demo".to_owned(),
                generated_path: "build/generated".to_owned(),
                variants: vec!["en".to_owned(), "fr".to_owned()],
                modules: vec!["demo".to_owned()],
                autosave_minutes: 1,
                backups_kept: 10,
                ..Default::default()
            },
        };
        connection.rebuild_textures();
        connection
    }

    /// Replaces the art of one tileset slot, or takes it away.
    pub fn with_tileset(mut self, slot: usize, tileset: Option<DemoTileset>) -> Self {
        if let Some(target) = self.tilesets.get_mut(slot) {
            *target = tileset;
        }
        self.rebuild_textures();
        self
    }

    /// Offers the demo level as recoverable when the project opens.
    pub fn with_recovery(mut self) -> Self {
        self.recoverable = vec![AREA.to_owned()];
        self
    }

    pub fn without_background(mut self) -> Self {
        self.background = false;
        self.rebuild_textures();
        self
    }

    fn rebuild_textures(&mut self) {
        self.textures.clear();
        for (slot, tileset) in self.tilesets.iter().enumerate() {
            if let Some(tileset) = tileset {
                let texture = atlas(format!("demo-tileset-{slot}"), &tileset.blocks);
                self.textures.insert(texture.id.clone(), texture);
            }
        }
        if self.background {
            self.textures.insert(BACKGROUND.to_owned(), background());
        }
        self.textures.insert(WALKER.to_owned(), walker());
    }

    fn entry(&mut self, id: LevelId) -> Result<&mut DemoLevel, Error> {
        let index = usize::try_from(id).ok().and_then(|id| id.checked_sub(1));
        index.and_then(|index| self.levels.get_mut(index)).ok_or_else(|| Error::new(ErrorCode::NotFound, "there is no such level session"))
    }

    fn level(&mut self, id: LevelId) -> Result<&mut EditableLevel, Error> {
        self.entry(id).map(|entry| &mut entry.level)
    }

    fn snapshot(&mut self, id: LevelId) -> Result<LevelSnapshot, Error> {
        let entry = self.entry(id)?;
        let level = &entry.level;
        Ok(LevelSnapshot {
            level: id,
            area: entry.area.to_owned(),
            title: entry.title.to_owned(),
            revision: entry.revision,
            course: level.course().clone(),
            objects: level.objects().to_vec(),
            tileset_references: level.tileset_references(),
            glue_error: level.glue_error().map(str::to_owned),
            glue: level.glue().clone(),
            stage_object_id_base: level.stage_object_id_base(),
            registration_hashes: level.registration_hashes(),
            sprite_hashes: (0..level.course().sprites.len()).map(|index| level.sprite_hash(index)).collect(),
            dirty: level.dirty(),
            can_undo: level.can_undo(),
            can_redo: level.can_redo(),
            undo_names: level.undo_names(),
            redo_names: level.redo_names(),
        })
    }

    fn edited(&mut self, id: LevelId, outcome: EditOutcome) -> ResponseResult {
        self.entry(id)?.revision += 1;
        let snapshot = self.snapshot(id)?;
        self.events.push(Incoming::Event(Event::LevelChanged { level: id, revision: snapshot.revision, dirty: snapshot.dirty }));
        Ok(Response::Edited { snapshot: Box::new(snapshot), outcome })
    }

    /// A level's file, as saved: `course/<area>.bin` or `course/<area>_bgdat.bin`.
    fn level_file(&mut self, path: &str) -> Result<Vec<u8>, Error> {
        let missing = || Error::new(ErrorCode::NotFound, format!("the demo ROM has no {path}"));
        let name = path.strip_prefix("course/").ok_or_else(missing)?;
        let (area, bgdat) = match name.strip_suffix("_bgdat.bin") {
            Some(area) => (area, true),
            None => (name.strip_suffix(".bin").ok_or_else(missing)?, false),
        };
        let index = self.levels.iter().position(|entry| entry.area == area).ok_or_else(missing)?;
        let level = &self.levels.get(index).ok_or_else(missing)?.level;
        if bgdat { level.serialize_bgdat() } else { level.serialize_course() }
    }

    fn row(path: &str, file: &DemoFile) -> FileEntry {
        let status = match (&file.module, file.id) {
            (Some(_), Some(_)) => FileStatus::Pending,
            (Some(_), None) => FileStatus::Pending,
            (None, _) => FileStatus::Vanilla,
        };
        let icon = if path.ends_with(".narc") {
            FileIcon::Archive
        } else if path.ends_with(".bmg") {
            FileIcon::Text
        } else if path.ends_with(".bnbl") {
            FileIcon::TouchHitbox
        } else if path.contains("ncl") {
            FileIcon::Palette
        } else if path.contains("ncg") {
            FileIcon::Image
        } else {
            FileIcon::File
        };
        FileEntry {
            path: path.to_owned(),
            status,
            module: file.module.clone().unwrap_or_default(),
            id: file.id,
            id_is_planned: file.module.is_some() && file.id.is_none() && file.member_id.is_none(),
            member_id: file.member_id,
            size: file.bytes.len() as u64,
            from_module: file.module.is_some(),
            icon,
            ..FileEntry::default()
        }
    }

    fn listing(&mut self) -> Result<FilesystemListing, Error> {
        let mut entries: Vec<FileEntry> =
            self.files.iter().filter(|(path, _)| !path.contains('!')).map(|(path, file)| Self::row(path, file)).collect();
        for (index, area) in [AREA, SECOND_AREA].into_iter().enumerate() {
            for (offset, suffix) in [(0, ".bin"), (1, "_bgdat.bin")] {
                let path = format!("course/{area}{suffix}");
                if self.files.contains_key(&path) {
                    continue;
                }
                let size = self.level_file(&path)?.len() as u64;
                let id = Some(100 + 2 * index as i64 + offset);
                entries.push(FileEntry { path, id, size, icon: FileIcon::Level, ..FileEntry::default() });
            }
        }
        let pending = entries.iter().any(|e| e.status == FileStatus::Pending);
        Ok(FilesystemListing {
            entries,
            pending,
            modules: vec![ModuleChoice { key: "demo".to_owned(), name: "Demo module".to_owned() }],
            active_module: "demo".to_owned(),
            active_variant: String::new(),
            error: None,
        })
    }

    fn graphics(&self, level: LevelId) -> LevelGraphics {
        let tilesets = std::array::from_fn(|slot| match self.tilesets.get(slot).and_then(Option::as_ref) {
            Some(tileset) => TilesetGraphics {
                atlas: Some(format!("demo-tileset-{slot}")),
                blocks: tileset.blocks.len() as u32,
                objects: tileset.objects.clone(),
                behaviors: tileset.behaviors.clone(),
                error: None,
            },
            None => TilesetGraphics::default(),
        });
        let background = if self.background {
            ImageGraphics { texture: Some(BACKGROUND.to_owned()), error: None, frames: Vec::new() }
        } else {
            ImageGraphics { texture: None, error: Some("the demo has no background".to_owned()), frames: Vec::new() }
        };
        LevelGraphics {
            level,
            generation: 0,
            tilesets,
            background,
            foreground: ImageGraphics::default(),
            backgrounds: demo_backgrounds(),
            error: None,
        }
    }

    fn catalog() -> ActorCatalog {
        let actor = |definition, has_renderer| ActorInfo { definition, has_renderer, init_error: None };
        let mut walker = definition(21, "Walker", "Enemies");
        walker.render_bounds = SpriteBounds { x: 0, y: 0, width: 16, height: 16 };
        walker.notes = "Walks until it meets a wall, then turns round.".to_owned();
        let field = |key: &str, label: &str, bit_offset, bit_width| SpriteFieldDefinition {
            key: key.to_owned(),
            label: label.to_owned(),
            bit_offset,
            bit_width,
            ..Default::default()
        };
        let choice = |value, label: &str| SpriteFieldChoice { value, label: label.to_owned() };
        walker.fields = vec![
            SpriteFieldDefinition {
                choices: vec![choice(0, "Left"), choice(1, "Right")],
                notes: "The way it sets off.".to_owned(),
                ..field("facing", "Facing", 0, 1)
            },
            SpriteFieldDefinition { notes: "Pixels a frame, in sixteenths.".to_owned(), ..field("speed", "Speed", 4, 4) },
            SpriteFieldDefinition { checkbox_value: 1, ..field("winged", "Winged", 8, 1) },
        ];
        ActorCatalog {
            generation: 0,
            actors: vec![actor(walker, true), actor(definition(55, "Shell walker", "Enemies"), false), actor(definition(64, "Coin", "Items"), true)],
            sprite_object_ids: (0..400).collect(),
            sprite_banks: Vec::new(),
        }
    }

    /// One module with one actor and one level-data key, so the custom-actor tree and the level-data tab have something
    /// to show.
    fn project_data() -> ProjectData {
        let modules = ModuleDataCatalog {
            level_data: vec![LevelDataDeclaration {
                key: "demo.lowGravity".to_owned(),
                module: "demo".to_owned(),
                kind: LevelDataKind::Flag,
                hash: fnv1a_32("demo.lowGravity"),
                ..Default::default()
            }],
            stage_objects: vec![StageObjectDeclaration {
                name: "Signpost".to_owned(),
                module: "demo".to_owned(),
                component: "props".to_owned(),
                hash: SIGNPOST,
                object_id: 400,
                ..Default::default()
            }],
            ..Default::default()
        };
        ProjectData { modules, object_bank_overlays: None }
    }

    fn render(input: &ActorInput) -> ActorDraw {
        let at = |x: f64, y: f64, width: f64, height: f64| DrawCommand {
            x: input.pixel_x + x,
            y: input.pixel_y + y,
            width,
            height,
            opacity: 1.0,
            ..Default::default()
        };
        let commands = match (input.hash, input.sprite_type) {
            (None, 21) => {
                vec![DrawCommand { kind: DrawKind::Image, texture: Some(WALKER.to_owned()), source_size: [16, 16], ..at(0.0, 0.0, 16.0, 16.0) }]
            }
            (None, 64) => vec![
                DrawCommand { kind: DrawKind::FillEllipse, color: 0xf8c8_38ff, ..at(2.0, 0.0, 12.0, 16.0) },
                DrawCommand { kind: DrawKind::FillRect, color: 0xfff0_a0ff, ..at(7.0, 3.0, 2.0, 10.0) },
            ],
            (None, 55) => return ActorDraw { found: true, ..Default::default() },
            _ => return ActorDraw::default(),
        };
        ActorDraw { found: true, commands, custom_rendered: true, ..Default::default() }
    }

    fn edit(&mut self, id: LevelId, edit: LevelEdit) -> Result<EditOutcome, Error> {
        let level = self.level(id)?;
        match edit {
            LevelEdit::Update { updates, name } => level.update_entities(updates, &name).map(|()| EditOutcome::None),
            LevelEdit::Add { items, name } => level.add_entities(items, &name).map(EditOutcome::Added),
            LevelEdit::Remove { selection, name } => level.remove_entities(selection, &name).map(|()| EditOutcome::None),
            LevelEdit::Reorder { kind, index, forward } => level.reorder_entity(kind, index, forward).map(EditOutcome::Moved),
            LevelEdit::SetObjectBank { slot, value } => level.set_object_bank(slot, value).map(|()| EditOutcome::None),
            LevelEdit::SetTilesetReferences { references } => level.set_tileset_references(references).map(|()| EditOutcome::None),
            LevelEdit::SetCameras { cameras, name } => level.set_cameras(cameras, &name).map(|()| EditOutcome::None),
            LevelEdit::SetGlue { glue, name } => level.set_glue(glue, &name).map(|()| EditOutcome::None),
            LevelEdit::ReplaceSerialized { course, bgdat, name } => {
                level.replace_serialized(&course, &bgdat, None, &name).map(|()| EditOutcome::None)
            }
        }
    }

    /// A project, or a ROM edited directly from `path` (the uploaded ones' name).
    fn project_info(mode: ProjectMode, path: &str) -> ProjectInfo {
        let name = path.rsplit('/').next().unwrap_or(path).to_owned();
        ProjectInfo {
            path: path.to_owned(),
            name: if mode == ProjectMode::NcPatcher { "Demo".to_owned() } else { name },
            rom_path: (mode != ProjectMode::Uploaded).then(|| "rom.nds".to_owned()),
            game_title: Some("NEW SUPER MARIO BROS".to_owned()),
            game_code: Some("A2DE".to_owned()),
            file_count: 1204,
            mode,
        }
    }

    fn answer(&mut self, request: Request) -> ResponseResult {
        match request {
            Request::ListProjects => Ok(Response::Projects(vec![ProjectSummary { path: "demo".to_owned(), name: "Demo".to_owned() }])),
            Request::OpenProject { .. } => {
                let mut info = Self::project_info(ProjectMode::NcPatcher, "demo");
                // Without a clean ROM, and never built, there is no ROM to read.
                if self.settings.clean_rom_problem.is_some() {
                    info.rom_path = None;
                }
                Ok(Response::Project(info))
            }
            Request::OpenRom { path } | Request::SaveRomAs { path } => Ok(Response::Project(Self::project_info(ProjectMode::Rom, &path))),
            Request::UploadRom { name, offset, total, bytes } => {
                let received = offset + bytes.len() as u64;
                if received < total {
                    Ok(Response::Uploaded { received })
                } else {
                    Ok(Response::Project(Self::project_info(ProjectMode::Uploaded, &name)))
                }
            }
            // The demo's ROM is a handful of bytes, sent in two pieces as a large one would be in many.
            Request::DownloadRom { offset } => {
                let rom = b"DEMO ROM BYTES";
                let start = (offset as usize).min(rom.len());
                let end = (start + 8).min(rom.len());
                Ok(Response::RomChunk {
                    name: "rom.nds".to_owned(),
                    total: rom.len() as u64,
                    offset,
                    bytes: rom.get(start..end).unwrap_or_default().to_vec(),
                })
            }
            Request::CloseProject => Ok(Response::Done),
            Request::ListLevels => {
                let area = |token: &str, display: &str, available: bool| AreaSummary {
                    token: token.to_owned(),
                    display_name: display.to_owned(),
                    available,
                    course_path: if available { format!("course/{token}.bin") } else { String::new() },
                    bgdat_path: if available { format!("course/{token}_bgdat.bin") } else { String::new() },
                };
                let level = |name: &str, areas| LevelSummary { display_name: name.to_owned(), areas };
                Ok(Response::Levels(vec![LevelWorld {
                    name: "World 1".to_owned(),
                    levels: vec![
                        level("1-1", vec![area(AREA, "Area 1", true), area("A01_2", "Area 2", true)]),
                        level("1-2", vec![area("A02_1", "Area 1", false)]),
                    ],
                }]))
            }
            Request::OpenLevel { area } => {
                let index = self.levels.iter().position(|entry| entry.area == area);
                let id = index.ok_or_else(|| Error::new(ErrorCode::NotFound, format!("the demo ROM does not hold {area}")))? as LevelId + 1;
                Ok(Response::Level(Box::new(self.snapshot(id)?)))
            }
            Request::EditLevel { level, base_revision, edit } => {
                if base_revision != self.entry(level)?.revision {
                    return Err(Error::new(ErrorCode::InvalidArgument, "the level changed since it was last seen"));
                }
                let outcome = self.edit(level, edit)?;
                self.edited(level, outcome)
            }
            Request::UndoLevel { level } => {
                self.level(level)?.undo()?;
                self.edited(level, EditOutcome::None)
            }
            Request::RedoLevel { level } => {
                self.level(level)?.redo()?;
                self.edited(level, EditOutcome::None)
            }
            Request::SaveLevel { level } => {
                self.level(level)?.mark_clean();
                let title = self.entry(level)?.title;
                self.events.push(Incoming::Event(Event::Log { level: LogLevel::Info, message: format!("Saved {title}") }));
                self.edited(level, EditOutcome::None)
            }
            // The host drops a closed level's unsaved edits; here they are undone back to the last save.
            Request::CloseLevel { level } => {
                let entry = self.entry(level)?;
                while entry.level.dirty() && entry.level.can_undo() {
                    entry.level.undo()?;
                    entry.revision += 1;
                }
                Ok(Response::Done)
            }
            Request::LevelGraphics { level } => Ok(Response::LevelGraphics(Box::new(self.graphics(level)))),
            Request::ActorCatalog => Ok(Response::ActorCatalog(Box::new(Self::catalog()))),
            Request::RenderActors { inputs } => Ok(Response::ActorDraws { generation: 0, draws: inputs.iter().map(Self::render).collect() }),
            Request::ProjectData => Ok(Response::ProjectData(Box::new(Self::project_data()))),
            Request::LevelBytes { level } => {
                let level = self.level(level)?;
                let bytes = LevelBytes {
                    course: level.serialize_course()?,
                    bgdat: level.serialize_bgdat()?,
                    course_file_id: Some(300),
                    bgdat_file_id: Some(301),
                };
                Ok(Response::LevelBytes(Box::new(bytes)))
            }
            Request::FindEntities { sprites, id } => {
                let mut entities = Vec::new();
                for entry in &self.levels {
                    let level = &entry.level;
                    let found = |index, x: i32, y: i32, settings| FoundEntity {
                        area: entry.area.to_owned(),
                        title: entry.title.to_owned(),
                        index,
                        x,
                        y,
                        settings,
                    };
                    if sprites {
                        let matches = level.course().sprites.iter().enumerate().filter(|(_, s)| s.id == id);
                        entities.extend(matches.map(|(i, s)| found(i, i32::from(s.x), i32::from(s.y), Some(s.settings))));
                    } else {
                        let matches = level.objects().iter().enumerate().filter(|(_, o)| o.object_id == id);
                        entities.extend(matches.map(|(i, o)| found(i, i32::from(o.x), i32::from(o.y), None)));
                    }
                }
                Ok(Response::Found(entities))
            }
            Request::Recovery => {
                let offered = self.recoverable.iter().map(|area| RecoverableLevel { area: area.clone(), title: "1-1".to_owned() }).collect();
                Ok(Response::Recovery(offered))
            }
            Request::Recover { accept } => {
                let areas = std::mem::take(&mut self.recoverable);
                if !accept {
                    return Ok(Response::Recovered(Vec::new()));
                }
                // The "autosave" is the level with its first object gone.
                if !self.level(1)?.objects().is_empty() {
                    let selection = bigstar_level::LevelEntitySelection { objects: vec![0], ..Default::default() };
                    self.level(1)?.remove_entities(selection, "Recover autosave")?;
                    self.entry(1)?.revision += 1;
                }
                Ok(Response::Recovered(areas))
            }
            Request::Filesystem => Ok(Response::Filesystem(Box::new(self.listing()?))),
            Request::ListArchive { path } => {
                let prefix = format!("{path}!");
                let members = self.files.iter().filter(|(p, _)| p.starts_with(&prefix)).map(|(p, f)| Self::row(p, f)).collect();
                Ok(Response::Files(members))
            }
            Request::ReadFile { path } => {
                let bytes = match self.files.get(&path) {
                    Some(file) => file.bytes.clone(),
                    None => self.level_file(&path)?,
                };
                Ok(Response::FileBytes { path, bytes })
            }
            Request::WriteFile { path, bytes } => {
                let known = self.files.contains_key(&path) || self.level_file(&path).is_ok();
                if !known && !path.starts_with("z_new/") {
                    return Err(Error::new(ErrorCode::InvalidArgument, "New NitroFS files must be under z_new/."));
                }
                let id = self.files.get(&path).and_then(|f| f.id);
                self.files.insert(path, DemoFile { id, member_id: None, bytes, module: Some("demo".to_owned()) });
                Ok(Response::Done)
            }
            Request::FileTargets { path } => {
                let held = self.files.get(&path).is_some_and(|f| f.module.is_some());
                Ok(Response::FileTargets(if held { vec![String::new()] } else { Vec::new() }))
            }
            Request::RemoveFile { path } => match self.files.get(&path) {
                Some(file) if file.module.is_some() && file.id.is_none() => {
                    self.files.remove(&path);
                    Ok(Response::Done)
                }
                Some(file) if file.module.is_some() => {
                    // The ROM's copy shows again.
                    let restored = demo_files().remove(&path).map_or_else(Vec::new, |f| f.bytes);
                    self.files.insert(path, DemoFile { id: file.id, member_id: None, bytes: restored, module: None });
                    Ok(Response::Done)
                }
                _ => Err(Error::new(ErrorCode::InvalidArgument, "only a file a module supplies can be removed")),
            },
            Request::TilesetResources { level } => {
                let references = self.entry(level)?.level.tileset_references();
                let get = |path: String| bigstar_protocol::ResourceBytes {
                    bytes: self.files.get(&path).map(|f| f.bytes.clone()).unwrap_or_default(),
                    rom_path: path,
                };
                let tilesets = [(0u8, 0usize), (1, 1), (4, 2)]
                    .into_iter()
                    .map(|(page, slot)| bigstar_protocol::TilesetSlot {
                        graphics: get(format!("BG_ncg/demo_t{slot}_ncg.bin")),
                        palette: get(format!("BG_ncl/demo_t{slot}_ncl.bin")),
                        map16: get(format!("BG_pnl/demo_t{slot}_pnl.bin")),
                        objects: get(format!("BG_unt/demo_t{slot}.bin")),
                        object_index: get(format!("BG_unt/demo_t{slot}_hd.bin")),
                        behaviors: get(format!("BG_chk/demo_t{slot}_chk.bin")),
                        tile_page_offset: page,
                        ..Default::default()
                    })
                    .collect();
                let map = |name: &str| bigstar_protocol::TilemapSlot {
                    graphics: get(format!("BG_ncg/demo_{name}_ncg.bin")),
                    palette: get(format!("BG_ncl/demo_{name}_ncl.bin")),
                    screen: get(format!("BG_nsc/demo_{name}_nsc.bin")),
                };
                let resources = bigstar_protocol::TilesetResources {
                    level,
                    tilesets,
                    background: map("bg"),
                    foreground: map("fg"),
                    references,
                    overlays: Vec::new(),
                };
                Ok(Response::TilesetResources(Box::new(resources)))
            }
            Request::WriteFiles { files } => {
                for write in files {
                    self.answer(Request::WriteFile { path: write.path, bytes: write.bytes })?;
                }
                Ok(Response::Done)
            }
            Request::ProjectSettings => Ok(Response::Settings(self.settings.clone())),
            // The workspace holds the demo project and its clean ROM.
            Request::BrowseDirectory { path } => {
                let (kind, name) = if path.is_empty() { (DirectoryEntryKind::Directory, "demo") } else { (DirectoryEntryKind::File, "clean.nds") };
                let entries = vec![DirectoryEntry { name: name.to_owned(), kind, size: 0 }];
                Ok(Response::Directory(DirectoryListing { path, entries }))
            }
            Request::SetCleanRom { path } => {
                let info = ProjectInfo { rom_path: Some(path.clone()), ..Self::project_info(ProjectMode::NcPatcher, "demo") };
                self.settings.clean_rom = path;
                self.settings.clean_rom_problem = None;
                self.events.push(Incoming::Event(Event::ProjectChanged(info)));
                self.events.push(Incoming::Event(Event::SettingsChanged(self.settings.clone())));
                Ok(Response::Settings(self.settings.clone()))
            }
            Request::SetActiveVariant { variant } => {
                self.settings.active_variant = variant;
                self.events.push(Incoming::Event(Event::SettingsChanged(self.settings.clone())));
                Ok(Response::Settings(self.settings.clone()))
            }
            Request::SetPreferences { autosave_minutes, backups_kept } => {
                self.settings.autosave_minutes = autosave_minutes.clamp(1, 60);
                self.settings.backups_kept = backups_kept.clamp(1, 100);
                Ok(Response::Settings(self.settings.clone()))
            }
            Request::BuildConfig => Ok(Response::BuildConfig(bigstar_protocol::BuildConfigKind::Yaml)),
            Request::MigrateConfig | Request::CancelBuild | Request::ExtractRom { .. } => Ok(Response::Done),
            // A build that compiles two files, warns once and applies, all at once.
            Request::Build { .. } => {
                use bigstar_protocol::{BuildDiagnostic, BuildEvent, BuildOutcome, DiagnosticLevel};
                let events = [
                    BuildEvent::Started { variant: self.settings.active_variant.clone() },
                    BuildEvent::Progress { phase: "compile".to_owned(), current: 1, total: 2, item: "source/main.cpp".to_owned() },
                    BuildEvent::Log("[arm9] compiling source/main.cpp".to_owned()),
                    BuildEvent::Diagnostic(BuildDiagnostic {
                        level: DiagnosticLevel::Warning,
                        code: "NCP0204".to_owned(),
                        message: "unused variable 'speed'\n  3 | int speed = 4;".to_owned(),
                        file: "source/main.cpp".to_owned(),
                        line: 3,
                        column: 9,
                    }),
                    BuildEvent::Progress { phase: "link".to_owned(), current: 2, total: 2, item: String::new() },
                    BuildEvent::Artifact("arm9 (patched)".to_owned()),
                    BuildEvent::Finished(BuildOutcome { ok: true, stage: "succeeded".to_owned(), warnings: 1, applied: true, ..Default::default() }),
                ];
                for event in events {
                    self.events.push(Incoming::Event(Event::Build(event)));
                }
                self.events.push(Incoming::Event(Event::SettingsChanged(self.settings.clone())));
                Ok(Response::Done)
            }
            Request::Banner => Ok(Response::Banner { bytes: self.banner.clone(), target: "assets/banner.bin".to_owned() }),
            Request::WriteBanner { bytes } => {
                self.banner = bytes;
                Ok(Response::Done)
            }
            Request::GetTextures { ids } => Ok(Response::Textures(ids.iter().filter_map(|id| self.textures.get(id).cloned()).collect())),
            _ => Err(Error::new(ErrorCode::Unsupported, "the demo host does not do that")),
        }
    }
}

impl Default for DemoConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl Connection for DemoConnection {
    fn send(&mut self, request: Request) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        let result = self.answer(request);
        self.inbox.push(Incoming::Response { id, result });
        self.inbox.append(&mut self.events);
        id
    }

    fn poll(&mut self) -> Vec<Incoming> {
        std::mem::take(&mut self.inbox)
    }

    fn state(&self) -> &ConnectionState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_demo_tilesets_and_backgrounds_parse() {
        for page in [0, 1, 4] {
            let data = super::demo_tileset_files(page).unwrap();
            let tileset = bigstar_formats::parse_tileset(&data, 0, 0, page).unwrap();
            assert_eq!(tileset.map16.len(), super::sample_tileset().blocks.len());
        }
        for foreground in [false, true] {
            let (tiles, palettes) = if foreground {
                (bigstar_formats::FOREGROUND_TILE_OFFSET, bigstar_formats::FOREGROUND_PALETTE_OFFSET)
            } else {
                (bigstar_formats::BACKGROUND_TILE_OFFSET, bigstar_formats::BACKGROUND_PALETTE_OFFSET)
            };
            let map = bigstar_formats::parse_tilemap(&super::demo_tilemap_files(foreground).unwrap(), 0, tiles, palettes).unwrap();
            assert!(map.screen.iter().all(|entry| entry.tile >= 0 && entry.palette == 0), "numbered from the layer's slots");
            assert_eq!((map.width_tiles, map.screen.len()), (64, 1024));
        }
    }
}
