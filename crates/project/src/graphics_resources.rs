//! Which files a level's graphics come from, read through the effective filesystem, and decoding them. Port of
//! `libs/nsmb-project/src/graphics_resources.cpp`; the tile rasterisation it feeds lives in `bigstar_level`.

use std::path::PathBuf;

use bigstar_core::{Error, ErrorCode, Result};
use std::collections::HashMap;

use bigstar_formats::{
    Bgr555Color, EditableTileset, IndexedTile8Bpp, RgbaImage, ScreenMapEntry, TilesetResourceData, compose_tileset_memory, decode_8bpp_tiles,
    decode_bgr555_palette, decode_screen_map, decompress_nitro_lz, mark_palette_blocks_transparent, parse_tileset, render_screen_map,
};
use bigstar_level::{BackgroundLayer, BackgroundPaletteAnimation, BackgroundTileAnimation, DecodedTileset, LevelTilesetReferences};
use serde::{Deserialize, Serialize};

use crate::EffectiveFilesystem;
use crate::tile_overlays::{TileOverlayCatalog, overlay_map16_images};

/// Where one release of the game keeps its resource tables in overlay 0, and the id its NitroFS numbering starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NsmbGraphicsProfile {
    pub game_code: &'static str,
    pub file_id_offset: u16,
    /// What the tables hold where a slot has no file: the release's file count, one past its last file. The game
    /// checks for it before every load and skips the resource.
    pub no_file: u16,
    pub tileset_index: usize,
    pub tileset_objects: usize,
    pub tileset_behaviors: usize,
    pub tileset_graphics: usize,
    pub tileset_palette: usize,
    pub tileset_map16: usize,
    pub background_graphics: usize,
    pub background_palette: usize,
    pub background_screen: usize,
    pub foreground_graphics: usize,
    pub foreground_palette: usize,
    pub foreground_screen: usize,
    pub sprite_object_ids: usize,
    /// The shared tileset's behaviour table, held in the overlay itself rather than in a file: 256 words, one a block.
    pub shared_behaviors: usize,
    /// Which object bank each sprite type needs: a tileset slot and a bank number for each, 0 for none.
    pub sprite_banks: usize,
    /// Which of the shared tileset's four palettes each background loads, a byte for each.
    pub shared_palettes: usize,
}

/// One resource, named the way the game names it and carrying what the editor would read there now -- the module's
/// copy when a build has not caught up with an edit yet, and the ROM's otherwise.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceFile {
    pub rom_path: String,
    pub data: Vec<u8>,
}

impl ResourceFile {
    pub fn is_empty(&self) -> bool {
        self.rom_path.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilesetResourceFiles {
    pub graphics: ResourceFile,
    pub palette: ResourceFile,
    pub map16: ResourceFile,
    pub objects: ResourceFile,
    pub object_index: ResourceFile,
    pub behaviors: ResourceFile,
    pub tile_offset: u16,
    pub palette_offset: u8,
    pub tile_page_offset: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackgroundResourceFiles {
    pub graphics: ResourceFile,
    pub palette: ResourceFile,
    pub screen: ResourceFile,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorResourceFiles {
    pub r#type: u16,
    pub object_id: u32,
    pub hash: Option<u32>,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelGraphicsResources {
    pub tilesets: [TilesetResourceFiles; 3],
    pub background: BackgroundResourceFiles,
    pub foreground: BackgroundResourceFiles,
    pub actors: Vec<ActorResourceFiles>,
    /// `sprite_banks[type]` is the tileset slot and object bank sprite type `type` needs, bank 0 for none. Empty when
    /// the ROM's table is not where this release keeps it.
    pub sprite_banks: Vec<[u8; 2]>,
    /// Files on disk that, if they change, change these resources: the module sources supplying them, and the ROM
    /// everything else comes from. There is nothing to watch inside the ROM, so it is watched whole.
    pub watched_files: Vec<PathBuf>,
}

pub type ObjectBankOverlayTable = [[u32; 10]; 16];

const EXPECTED_BANKS: ObjectBankOverlayTable = [
    [22, 23, 24, 25, 26, 27, 28, 29, 30, 31],
    [32, 33, 34, 35, 36, 37, 38, 39, 40, 41],
    [56, 57, 58, 59, 60, 61, 62, 63, 64, 65],
    [66, 67, 68, 69, 70, 71, 72, 73, 74, 75],
    [42, 43, 44, 45, 46, 47, 48, 49, 50, 51],
    [76, 77, 78, 79, 80, 81, 82, 83, 84, 85],
    [86, 87, 88, 89, 90, 91, 92, 93, 94, 95],
    [96, 97, 98, 99, 100, 101, 102, 103, 104, 105],
    [106, 107, 108, 109, 110, 111, 112, 113, 114, 115],
    [116, 117, 118, 119, 120, 121, 122, 123, 124, 125],
    [0; 10],
    [0; 10],
    [0; 10],
    [0; 10],
    [0; 10],
    [12, 13, 14, 15, 16, 17, 18, 19, 20, 21],
];

/// Finds the table of which overlay holds each object bank by searching ARM9 for its retail contents, and returns
/// the table as the ROM has it.
///
/// A slot with no banks is `0xFFFFFFFF` in a retail ARM9. The C++ looked for zeros there, so it never found the table
/// in a real ROM; either is accepted here.
pub fn find_object_bank_overlays(arm9: &[u8]) -> Result<ObjectBankOverlayTable> {
    let words = EXPECTED_BANKS.len() * 10;
    let read = |offset: usize, index: usize| -> Option<u32> {
        let at = offset.checked_add(index.checked_mul(4)?)?;
        let bytes: [u8; 4] = arm9.get(at..at + 4)?.try_into().ok()?;
        Some(u32::from_le_bytes(bytes))
    };
    let mut offset = 0;
    while offset + words * 4 <= arm9.len() {
        let mut found: ObjectBankOverlayTable = [[0; 10]; 16];
        let matched = EXPECTED_BANKS.iter().flatten().enumerate().all(|(index, &expected)| {
            let Some(value) = read(offset, index) else { return false };
            if let Some(cell) = found.get_mut(index / 10).and_then(|row| row.get_mut(index % 10)) {
                *cell = value;
            }
            value == expected || (expected == 0 && value == u32::MAX)
        });
        if matched {
            return Ok(found);
        }
        offset += 4;
    }
    Err(Error::new(ErrorCode::NotFound, "could not locate the object-bank overlay table in ARM9"))
}

const fn profile(game_code: &'static str, file_id_offset: u16, no_file: u16, tables: [usize; 16]) -> NsmbGraphicsProfile {
    NsmbGraphicsProfile {
        game_code,
        file_id_offset,
        no_file,
        tileset_index: tables[0],
        tileset_objects: tables[1],
        tileset_behaviors: tables[2],
        tileset_graphics: tables[3],
        tileset_palette: tables[4],
        tileset_map16: tables[5],
        background_graphics: tables[6],
        background_palette: tables[7],
        background_screen: tables[8],
        foreground_graphics: tables[9],
        foreground_palette: tables[10],
        foreground_screen: tables[11],
        sprite_object_ids: tables[12],
        shared_behaviors: tables[13],
        sprite_banks: tables[14],
        shared_palettes: tables[15],
    }
}

// The shared behaviours sit right after the level tilesets' animation table, two 76-entry tables past their
// behaviour table, in every release; NSMB Editor's Korean and Chinese offsets for them do not, and are not used.
const PROFILES: [NsmbGraphicsProfile; 5] = [
    profile(
        "A2DE",
        131,
        0x7a5,
        [
            0x2F8E4, 0x2FA14, 0x2FB44, 0x30EA4, 0x31494, 0x316F4, 0x30D74, 0x31364, 0x31234, 0x30FD4, 0x315C4, 0x31104, 0x29BD8, 0x2FDA4, 0x2C930,
            0x30CD8,
        ],
    ),
    profile(
        "A2DP",
        135,
        0x90a,
        [
            0x2F0F8, 0x2F228, 0x2F358, 0x306B8, 0x30CA8, 0x30F08, 0x30588, 0x30B78, 0x30A48, 0x307E8, 0x30DD8, 0x30918, 0x290C4, 0x2F5B8, 0x2BDF0,
            0x304EC,
        ],
    ),
    profile(
        "A2DJ",
        131,
        0x7a6,
        [
            0x2ECE4, 0x2EE14, 0x2EF44, 0x302A4, 0x30894, 0x30AF4, 0x30174, 0x30764, 0x30634, 0x303D4, 0x309C4, 0x30504, 0x29008, 0x2F1A4, 0x2BD30,
            0x300D8,
        ],
    ),
    profile(
        "A2DK",
        131,
        0x798,
        [
            0x2EDA4, 0x2EED4, 0x2F004, 0x30364, 0x30954, 0x30BB4, 0x30234, 0x30824, 0x306F4, 0x30494, 0x30A84, 0x305C4, 0x290E0, 0x2F264, 0x2BDF0,
            0x30198,
        ],
    ),
    profile(
        "A2DC",
        131,
        0x79a,
        [
            0x2EDA4, 0x2EED4, 0x2F004, 0x30364, 0x30954, 0x30BB4, 0x30234, 0x30824, 0x306F4, 0x30494, 0x30A84, 0x305C4, 0x290DC, 0x2F264, 0x2BDF0,
            0x30198,
        ],
    ),
];

pub fn nsmb_graphics_profile(game_code: &str) -> Option<&'static NsmbGraphicsProfile> {
    PROFILES.iter().find(|profile| profile.game_code == game_code)
}

/// A graphics resource may be stored LZ-wrapped whether it comes from the ROM or from a module tree, so the unwrap
/// belongs to the bytes rather than to where they were read.
fn unwrap_resource(data: &[u8]) -> Result<Vec<u8>> {
    match data.first() {
        Some(0x10 | 0x11) => decompress_nitro_lz(data),
        _ => Ok(data.to_vec()),
    }
}

/// The shared tileset's palette the game loads under background `background_screen_id`, as NSMB Editor picks it:
/// ghost houses grey its blocks, castles blue them, and so on. A background past the table's 76 gets the usual one.
fn shared_palette(overlay: &[u8], profile: &NsmbGraphicsProfile, background_screen_id: u16) -> &'static str {
    let id = usize::from(background_screen_id);
    let choice = if id < 76 { overlay.get(profile.shared_palettes + id) } else { None };
    match choice {
        Some(1) => "d_2d_A_J_jyotyu_B_ncl.bin",
        Some(2) => "d_2d_A_J_jyotyu_R_ncl.bin",
        Some(3) => "d_2d_A_J_jyotyu_W_ncl.bin",
        _ => "d_2d_A_J_jyotyu_ncl.bin",
    }
}

/// The shared tileset's behaviour table: a word for each of its 256 blocks.
const SHARED_BEHAVIORS_SIZE: usize = 256 * 4;

fn table_entry(overlay: &[u8], table: usize, id: u8) -> Result<u16> {
    let offset = table + usize::from(id) * 4;
    match (overlay.get(offset), overlay.get(offset + 1)) {
        (Some(&low), Some(&high)) => Ok(u16::from_le_bytes([low, high])),
        _ => Err(Error::new(ErrorCode::OutOfBounds, "graphics resource table is outside Overlay 0")),
    }
}

fn table_file_id(overlay: &[u8], table: usize, id: u8, base: u16) -> Result<u16> {
    table_entry(overlay, table, id).map(|entry| entry.wrapping_add(base))
}

/// What the level's graphics are, read through `files` rather than off a tree somebody unpacked earlier. The game's
/// own resource tables address files by id, so this walks the ROM's file table to turn each id into an address, and
/// then asks `files` what that address holds -- which is how a module's pending edit reaches the canvas without a
/// rebuild.
pub fn resolve_level_graphics(files: &mut EffectiveFilesystem, references: LevelTilesetReferences) -> Result<LevelGraphicsResources> {
    let rom = files.rom_mut().ok_or_else(|| Error::new(ErrorCode::InvalidArgument, "the project has no built ROM to read graphics from"))?;
    let profile = nsmb_graphics_profile(&rom.header().game_code)
        .ok_or_else(|| Error::new(ErrorCode::Unsupported, "ROM game code does not have a graphics resource profile"))?;
    let overlay = rom.read_overlay(0)?;
    let rom_path = rom.path().to_path_buf();

    // Where each resource is addressed from. The game's tables hold file ids, and two of the three tilesets are named
    // outright rather than indexed. The C++ passes the 16-bit ids as `uint8_t`, so they truncate the same way here.
    let table = |files: &EffectiveFilesystem, offset: usize, id: u16| -> Result<String> {
        let file_id = table_file_id(&overlay, offset, id as u8, profile.file_id_offset)?;
        files.path_for_id(file_id).ok_or_else(|| Error::new(ErrorCode::NotFound, format!("graphics resource file ID {file_id} is not in the ROM")))
    };
    let named = |files: &EffectiveFilesystem, name: &str| -> Result<String> {
        files
            .rom()
            .and_then(|rom| rom.files().iter().find(|file| file.path.rsplit('/').next() == Some(name)))
            .map(|file| file.path.clone())
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("required ROM graphics file is missing: {name}")))
    };

    let mut result = LevelGraphicsResources::default();
    // The ROM is watched whole, because a build replaces it whole and nothing inside it changes on its own.
    result.watched_files.push(rom_path);
    let mut watched = Vec::new();
    let mut fill = |files: &mut EffectiveFilesystem, target: &mut ResourceFile, address: Result<String>| -> Result<()> {
        let address = address?;
        let data = files.read(&address)?;
        // A module supplying this resource is the file to watch; when the ROM supplies it there is nothing to add.
        if let Some(source) = files.source(&address) {
            watched.push(source);
        }
        *target = ResourceFile { rom_path: address, data };
        Ok(())
    };

    let [jyotyu, level_tileset, nohara] = &mut result.tilesets;
    let addresses = [
        named(files, "d_2d_A_J_jyotyu_ncg.bin"),
        named(files, shared_palette(&overlay, profile, references.background_screen_id)),
        named(files, "d_2d_PA_A_J_jyotyu.bin"),
        named(files, "A_J_jyotyu.bin"),
        named(files, "A_J_jyotyu_hd.bin"),
    ];
    let [graphics, palette, map16, objects, index] = addresses;
    fill(files, &mut jyotyu.graphics, graphics)?;
    fill(files, &mut jyotyu.palette, palette)?;
    fill(files, &mut jyotyu.map16, map16)?;
    fill(files, &mut jyotyu.objects, objects)?;
    fill(files, &mut jyotyu.object_index, index)?;
    // Its behaviours are in the overlay, so they come with no file to write them back to.
    if let Some(table) = overlay.get(profile.shared_behaviors..profile.shared_behaviors + SHARED_BEHAVIORS_SIZE) {
        jyotyu.behaviors = ResourceFile { rom_path: String::new(), data: table.to_vec() };
    }

    level_tileset.tile_offset = 192;
    level_tileset.palette_offset = 2;
    level_tileset.tile_page_offset = 1;
    let id = references.tileset_id;
    // A level with no tileset names it 0xFFFF, and the game loads none of its files (`loadCharacterGraphics`,
    // `LoadTileBehaviors`); truncated, the number would index an unrelated file. The slot stays empty and the
    // backgrounds still load. See `docs/no-tileset.md`.
    if id != u16::MAX {
        let addresses = [
            table(files, profile.tileset_graphics, id),
            // The game takes the tileset's colours from the tileset block, which nearly always names the same tileset.
            table(files, profile.tileset_palette, references.tileset_palette_id),
            table(files, profile.tileset_map16, id),
            table(files, profile.tileset_objects, id),
            table(files, profile.tileset_index, id),
        ];
        let behaviors = table(files, profile.tileset_behaviors, id);
        let [graphics, palette, map16, objects, index] = addresses;
        fill(files, &mut level_tileset.graphics, graphics)?;
        fill(files, &mut level_tileset.palette, palette)?;
        fill(files, &mut level_tileset.map16, map16)?;
        fill(files, &mut level_tileset.objects, objects)?;
        fill(files, &mut level_tileset.object_index, index)?;
        // A behaviour table the overlay cannot resolve is skipped rather than failing the level, as the C++ does.
        if let Ok(behaviors) = behaviors
            && !behaviors.is_empty()
        {
            fill(files, &mut level_tileset.behaviors, Ok(behaviors))?;
        }
    }

    nohara.tile_offset = 640;
    nohara.palette_offset = 6;
    nohara.tile_page_offset = 4;
    let addresses = [
        named(files, "d_2d_I_S_tikei_nohara_ncg.bin"),
        named(files, "d_2d_I_S_tikei_nohara_ncl.bin"),
        named(files, "d_2d_PA_I_S_nohara.bin"),
        named(files, "I_S_nohara.bin"),
        named(files, "I_S_nohara_hd.bin"),
    ];
    let behaviors = named(files, "NoHaRaSubUnitChangeData.bin");
    let [graphics, palette, map16, objects, index] = addresses;
    fill(files, &mut nohara.graphics, graphics)?;
    fill(files, &mut nohara.palette, palette)?;
    fill(files, &mut nohara.map16, map16)?;
    fill(files, &mut nohara.objects, objects)?;
    fill(files, &mut nohara.object_index, index)?;
    if let Ok(behaviors) = behaviors
        && !behaviors.is_empty()
    {
        fill(files, &mut nohara.behaviors, Ok(behaviors))?;
    }

    // `StageLayout::loadCharacterGraphics`, `loadPalettes` and `loadTilemaps`: the graphics come from the header, the
    // palette and the screen from the layer's own block, and each is its own table.
    let backgrounds = [
        (
            profile.background_graphics,
            profile.background_palette,
            profile.background_screen,
            references.background_graphics_id,
            references.background_palette_id,
            references.background_screen_id,
        ),
        (
            profile.foreground_graphics,
            profile.foreground_palette,
            profile.foreground_screen,
            references.foreground_graphics_id,
            references.foreground_palette_id,
            references.foreground_screen_id,
        ),
    ];
    for (target, (graphics_table, palette_table, screen_table, graphics_id, palette_id, screen_id)) in
        [&mut result.background, &mut result.foreground].into_iter().zip(backgrounds)
    {
        // A level without the layer names it 0xFFFF; truncated, that would index an unrelated file.
        if graphics_id == u16::MAX || palette_id == u16::MAX || screen_id == u16::MAX {
            continue;
        }
        // A slot the game has no file for loads nothing, so the layer is not there to draw.
        let slots = [(graphics_table, graphics_id), (palette_table, palette_id), (screen_table, screen_id)];
        if slots.iter().any(|&(offset, id)| table_entry(&overlay, offset, id as u8).is_ok_and(|entry| entry == profile.no_file)) {
            continue;
        }
        let graphics = table(files, graphics_table, graphics_id);
        fill(files, &mut target.graphics, graphics)?;
        let palette = table(files, palette_table, palette_id);
        fill(files, &mut target.palette, palette)?;
        let screen = table(files, screen_table, screen_id);
        fill(files, &mut target.screen, screen)?;
    }

    const ACTOR_TYPE_COUNT: u16 = 326;
    for actor_type in 0..ACTOR_TYPE_COUNT {
        let class_offset = profile.sprite_object_ids + usize::from(actor_type) * 2;
        let (Some(&low), Some(&high)) = (overlay.get(class_offset), overlay.get(class_offset + 1)) else { continue };
        let class_id = u16::from_le_bytes([low, high]);
        result.actors.push(ActorResourceFiles {
            r#type: actor_type,
            object_id: u32::from(class_id),
            hash: None,
            name: format!("Actor {actor_type} (class {class_id})"),
        });
    }
    result.sprite_banks = sprite_banks(&overlay, profile.sprite_banks, usize::from(ACTOR_TYPE_COUNT));
    result.watched_files.extend(watched);
    result.watched_files.sort();
    result.watched_files.dedup();
    Ok(result)
}

/// The table of which object bank each sprite type needs, as NSMB Editor reads it. A table that names a slot the game
/// has no banks for, or a bank past the ten a slot has, is not the table, and none is returned.
fn sprite_banks(overlay: &[u8], offset: usize, count: usize) -> Vec<[u8; 2]> {
    let Some(table) = overlay.get(offset..offset + count * 2) else { return Vec::new() };
    let banks: Vec<[u8; 2]> = table.as_chunks::<2>().0.to_vec();
    let plausible = banks.iter().all(|&[slot, bank]| bank == 0 || ((slot <= 9 || slot == 15) && bank <= 10));
    if plausible { banks } else { Vec::new() }
}

fn parse_resource_tileset(files: &TilesetResourceFiles) -> Result<EditableTileset> {
    let resources = TilesetResourceData {
        graphics: files.graphics.data.clone(),
        palette: files.palette.data.clone(),
        map16: files.map16.data.clone(),
        objects: files.objects.data.clone(),
        object_index: files.object_index.data.clone(),
        behaviors: files.behaviors.data.clone(),
    };
    let mut parsed = parse_tileset(&resources, files.tile_offset, files.palette_offset, files.tile_page_offset)?;
    mark_palette_blocks_transparent(&mut parsed.palette, 256);
    Ok(parsed)
}

/// Every tileset the level loads, decoded together. A slot that fails to decode leaves its entry empty and its reason
/// in `errors`; the others still render. See `docs/tileset-memory.md`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTilesets {
    pub tilesets: [DecodedTileset; 3],
    pub errors: [String; 3],
}

/// `tileset_id` and `overlays` are what turns a block the game never draws into the texture it actually shows. Pass
/// no catalog and the Map16 is drawn exactly as the files describe it. See `docs/lua-tiles.md`.
pub fn load_level_tilesets(files: &[TilesetResourceFiles; 3], tileset_id: u16, overlays: Option<&TileOverlayCatalog>) -> LevelTilesets {
    let mut result = LevelTilesets::default();
    let mut parsed: [Option<EditableTileset>; 3] = Default::default();
    for ((file, slot), error) in files.iter().zip(&mut parsed).zip(&mut result.errors) {
        // A slot with no files is a level without that tileset, which is not an error.
        if file.graphics.data.is_empty() && file.map16.data.is_empty() {
            continue;
        }
        match parse_resource_tileset(file) {
            Ok(tileset) => *slot = Some(tileset),
            Err(e) => *error = e.message,
        }
    }
    // Compose from whatever decoded. A slot that failed contributes nothing, so blocks that needed it stay clear
    // instead of taking the whole level down.
    let present: Vec<EditableTileset> = parsed.iter().flatten().cloned().collect();
    let memory = compose_tileset_memory(&present);
    for ((tileset, target), error) in parsed.into_iter().zip(&mut result.tilesets).zip(&mut result.errors) {
        let Some(tileset) = tileset else { continue };
        target.objects = tileset.objects;
        target.behaviors = tileset.behaviors;
        target.map16_tiles.reserve(tileset.map16.len());
        for block in &tileset.map16 {
            let mut absolute = block.tiles;
            for quadrant in &mut absolute {
                quadrant.tile += i32::from(tileset.tile_offset);
                quadrant.palette = quadrant.palette.wrapping_add(i16::from(tileset.palette_offset));
            }
            match render_screen_map(&absolute, 2, &memory.graphics, &memory.palette) {
                Ok(image) => target.map16_tiles.push(image),
                Err(e) => {
                    *error = e.message;
                    *target = DecodedTileset::default();
                    break;
                }
            }
        }
        if let Some(overlays) = overlays
            && !target.map16_tiles.is_empty()
        {
            overlay_map16_images(&mut target.map16_tiles, tileset_id, u16::from(tileset.tile_page_offset) * 256, overlays);
        }
    }
    result
}

/// A background or foreground layer decoded but not yet drawn. Its screen names tiles and palette rows from where the
/// game loads the layer (`tile_offset`, `palette_offset`), and colour 0 of each palette is see-through.
struct DecodedLayer {
    tiles: Vec<IndexedTile8Bpp>,
    palette: Vec<Bgr555Color>,
    screen: Vec<ScreenMapEntry>,
    width: usize,
}

impl DecodedLayer {
    fn decode(files: &BackgroundResourceFiles, tile_offset: u16, palette_offset: u8) -> Result<Self> {
        let graphics = unwrap_resource(&files.graphics.data);
        let palette_data = unwrap_resource(&files.palette.data);
        let screen_data = unwrap_resource(&files.screen.data);
        let (graphics, palette_data, screen_data) = (graphics?, palette_data?, screen_data?);
        let tiles = decode_8bpp_tiles(&graphics);
        let palette = decode_bgr555_palette(&palette_data, false).map(|mut palette| {
            mark_palette_blocks_transparent(&mut palette, 256);
            palette
        });
        let screen = decode_screen_map(&screen_data, tile_offset, palette_offset);
        let (tiles, palette, screen) = (tiles?, palette?, screen?);
        let width = if screen.len().is_multiple_of(64) { 64 } else { 32 };
        Ok(Self { tiles, palette, screen, width })
    }

    fn render(&self) -> Result<RgbaImage> {
        render_screen_map(&self.screen, self.width, &self.tiles, &self.palette)
    }
}

/// A background or foreground layer as one image, as the files hold it.
pub fn load_background_graphics(files: &BackgroundResourceFiles, tile_offset: u16, palette_offset: u8) -> Result<RgbaImage> {
    DecodedLayer::decode(files, tile_offset, palette_offset)?.render()
}

/// What one step of an animation overwrites in a layer, and for how long it shows. Steps that write the same thing
/// share a `key`, so the picture they make is drawn once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundAnimationStep {
    pub key: u32,
    pub ticks: u32,
    tiles: Vec<(usize, IndexedTile8Bpp)>,
    colors: Vec<(usize, Bgr555Color)>,
}

/// One thing animating a layer, as the steps it cycles through.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundAnimator {
    pub steps: Vec<BackgroundAnimationStep>,
}

/// The part of a tile animation that lands in `layer`'s graphics, read from the animation's own graphics file.
/// `None` when it writes nothing there: a castle's background animates tiles of its foreground.
pub fn background_tile_animator(animation: &BackgroundTileAnimation, layer: BackgroundLayer, data: &[u8]) -> Result<Option<BackgroundAnimator>> {
    if animation.copies.iter().all(|copy| copy.layer != layer) {
        return Ok(None);
    }
    let source = decode_8bpp_tiles(&unwrap_resource(data)?)?;
    let steps = animation
        .steps
        .iter()
        .map(|step| {
            let mut tiles = Vec::new();
            for copy in animation.copies.iter().filter(|copy| copy.layer == layer) {
                let first = usize::from(step.tile) + usize::from(copy.from);
                for offset in 0..usize::from(copy.count) {
                    // A copy that runs off the file writes what it has; the game would write whatever follows.
                    if let Some(tile) = source.get(first + offset) {
                        tiles.push((usize::from(copy.to) + offset, *tile));
                    }
                }
            }
            BackgroundAnimationStep { key: u32::from(step.tile), ticks: u32::from(step.ticks), tiles, colors: Vec::new() }
        })
        .collect();
    Ok(Some(BackgroundAnimator { steps }))
}

/// A palette animation's frames, read from its own palette file.
pub fn background_palette_animator(animation: &BackgroundPaletteAnimation, data: &[u8]) -> Result<BackgroundAnimator> {
    let source = decode_bgr555_palette(&unwrap_resource(data)?, false)?;
    let steps = (0..usize::from(animation.frames))
        .map(|frame| {
            let first = frame * usize::from(animation.stride);
            let colors = (0..usize::from(animation.count))
                .filter_map(|offset| Some((usize::from(animation.to) + offset, *source.get(first + offset)?)))
                .collect();
            BackgroundAnimationStep { key: frame as u32, ticks: u32::from(animation.ticks), tiles: Vec::new(), colors }
        })
        .collect();
    Ok(BackgroundAnimator { steps })
}

/// A layer's animation as pictures and the order they show in: `frames` holds an index into `images` and how many
/// frames of the game it stays up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackgroundFrames {
    pub images: Vec<RgbaImage>,
    pub frames: Vec<(usize, u32)>,
}

/// Longer than any loop the game has; an animation whose parts never line up again is cut here.
const MAX_ANIMATION_TICKS: u32 = 3600;
const MAX_ANIMATION_IMAGES: usize = 64;

/// Plays `animators` over a layer until they are all back at their first step, drawing each distinct picture once.
/// Empty when nothing animates the layer.
pub fn load_background_frames(
    files: &BackgroundResourceFiles,
    tile_offset: u16,
    palette_offset: u8,
    animators: &[BackgroundAnimator],
) -> Result<BackgroundFrames> {
    let animators: Vec<&BackgroundAnimator> =
        animators.iter().filter(|animator| animator.steps.len() > 1 && animator.steps.iter().all(|step| step.ticks > 0)).collect();
    let mut result = BackgroundFrames::default();
    if animators.is_empty() {
        return Ok(result);
    }
    let base = DecodedLayer::decode(files, tile_offset, palette_offset)?;
    let mut drawn: HashMap<Vec<u32>, usize> = HashMap::new();
    // Where each animator is: its step, and how long that step has left.
    let mut at: Vec<(usize, u32)> = animators.iter().map(|animator| (0, animator.steps.first().map_or(1, |step| step.ticks))).collect();
    let mut elapsed = 0;
    loop {
        let steps: Vec<&BackgroundAnimationStep> = animators.iter().zip(&at).filter_map(|(animator, &(step, _))| animator.steps.get(step)).collect();
        let keys: Vec<u32> = steps.iter().map(|step| step.key).collect();
        let image = match drawn.get(&keys) {
            Some(&image) => image,
            None => {
                if result.images.len() >= MAX_ANIMATION_IMAGES {
                    break;
                }
                let mut layer = DecodedLayer { tiles: base.tiles.clone(), palette: base.palette.clone(), screen: Vec::new(), width: base.width };
                for step in &steps {
                    for (index, tile) in &step.tiles {
                        if let Some(target) = layer.tiles.get_mut(*index) {
                            *target = *tile;
                        }
                    }
                    for (index, color) in &step.colors {
                        // The slot keeps whether it is see-through: that is its place in the palette, not its colour.
                        if let Some(target) = layer.palette.get_mut(*index) {
                            *target = Bgr555Color { transparent: target.transparent, ..*color };
                        }
                    }
                }
                result.images.push(render_screen_map(&base.screen, base.width, &layer.tiles, &layer.palette)?);
                drawn.insert(keys, result.images.len() - 1);
                result.images.len() - 1
            }
        };
        let ticks = at.iter().map(|&(_, left)| left).min().unwrap_or(1);
        result.frames.push((image, ticks));
        elapsed += ticks;
        for (animator, (step, left)) in animators.iter().zip(&mut at) {
            *left -= ticks;
            if *left == 0 {
                *step = (*step + 1) % animator.steps.len();
                *left = animator.steps.get(*step).map_or(1, |step| step.ticks);
            }
        }
        let restarted =
            animators.iter().zip(&at).all(|(animator, &(step, left))| step == 0 && animator.steps.first().is_some_and(|first| first.ticks == left));
        if restarted || elapsed >= MAX_ANIMATION_TICKS {
            break;
        }
    }
    Ok(result)
}
