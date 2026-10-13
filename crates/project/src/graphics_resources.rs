//! Which files a level's graphics come from, read through the effective filesystem, and decoding them. Port of
//! `libs/nsmb-project/src/graphics_resources.cpp`; the tile rasterisation it feeds lives in `bigstar_level`.

use std::path::PathBuf;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::{
    EditableTileset, RgbaImage, TilesetResourceData, compose_tileset_memory, decode_8bpp_tiles, decode_bgr555_palette, decode_screen_map,
    decompress_nitro_lz, mark_palette_blocks_transparent, parse_tileset, render_screen_map,
};
use bigstar_level::{DecodedTileset, LevelTilesetReferences};
use serde::{Deserialize, Serialize};

use crate::EffectiveFilesystem;
use crate::tile_overlays::{TileOverlayCatalog, overlay_map16_images};

/// Where one release of the game keeps its resource tables in overlay 0, and the id its NitroFS numbering starts from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NsmbGraphicsProfile {
    pub game_code: &'static str,
    pub file_id_offset: u16,
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

const fn profile(game_code: &'static str, file_id_offset: u16, tables: [usize; 13]) -> NsmbGraphicsProfile {
    NsmbGraphicsProfile {
        game_code,
        file_id_offset,
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
    }
}

const PROFILES: [NsmbGraphicsProfile; 5] = [
    profile("A2DE", 131, [0x2F8E4, 0x2FA14, 0x2FB44, 0x30EA4, 0x31494, 0x316F4, 0x30D74, 0x31364, 0x31234, 0x30FD4, 0x315C4, 0x31104, 0x29BD8]),
    profile("A2DP", 135, [0x2F0F8, 0x2F228, 0x2F358, 0x306B8, 0x30CA8, 0x30F08, 0x30588, 0x30B78, 0x30A48, 0x307E8, 0x30DD8, 0x30918, 0x290C4]),
    profile("A2DJ", 131, [0x2ECE4, 0x2EE14, 0x2EF44, 0x302A4, 0x30894, 0x30AF4, 0x30174, 0x30764, 0x30634, 0x303D4, 0x309C4, 0x30504, 0x29008]),
    profile("A2DK", 131, [0x2EDA4, 0x2EED4, 0x2F004, 0x30364, 0x30954, 0x30BB4, 0x30234, 0x30824, 0x306F4, 0x30494, 0x30A84, 0x305C4, 0x290E0]),
    profile("A2DC", 131, [0x2EDA4, 0x2EED4, 0x2F004, 0x30364, 0x30954, 0x30BB4, 0x30234, 0x30824, 0x306F4, 0x30494, 0x30A84, 0x305C4, 0x290DC]),
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

fn table_file_id(overlay: &[u8], table: usize, id: u8, base: u16) -> Result<u16> {
    let offset = table + usize::from(id) * 4;
    match (overlay.get(offset), overlay.get(offset + 1)) {
        (Some(&low), Some(&high)) => Ok(u16::from_le_bytes([low, high]).wrapping_add(base)),
        _ => Err(Error::new(ErrorCode::OutOfBounds, "graphics resource table is outside Overlay 0")),
    }
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
        named(files, "d_2d_A_J_jyotyu_ncl.bin"),
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

    level_tileset.tile_offset = 192;
    level_tileset.palette_offset = 2;
    level_tileset.tile_page_offset = 1;
    let id = references.tileset_id;
    let addresses = [
        table(files, profile.tileset_graphics, id),
        table(files, profile.tileset_palette, id),
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

    let backgrounds = [
        (
            profile.background_graphics,
            profile.background_palette,
            profile.background_screen,
            references.background_graphics_id,
            references.background_screen_id,
        ),
        (
            profile.foreground_graphics,
            profile.foreground_palette,
            profile.foreground_screen,
            references.foreground_graphics_id,
            references.foreground_screen_id,
        ),
    ];
    for (target, (graphics_table, palette_table, screen_table, graphics_id, screen_id)) in
        [&mut result.background, &mut result.foreground].into_iter().zip(backgrounds)
    {
        // A level without the layer names it 0xFFFF; truncated, that would index an unrelated file.
        if graphics_id == u16::MAX || screen_id == u16::MAX {
            continue;
        }
        let graphics = table(files, graphics_table, graphics_id);
        fill(files, &mut target.graphics, graphics)?;
        let palette = table(files, palette_table, graphics_id);
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
    result.watched_files.extend(watched);
    result.watched_files.sort();
    result.watched_files.dedup();
    Ok(result)
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

/// A background or foreground layer as one image. Its screen names tiles and palette rows from where the game loads
/// the layer (`tile_offset`, `palette_offset`), and colour 0 of each palette is see-through.
pub fn load_background_graphics(files: &BackgroundResourceFiles, tile_offset: u16, palette_offset: u8) -> Result<RgbaImage> {
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
    render_screen_map(&screen, width, &tiles, &palette)
}
