use bigstar_core::{Error, ErrorCode, Fx32, LittleEndianReader, Result};
use serde::{Deserialize, Serialize};

pub const NSMB_LEVEL_BLOCK_COUNT: usize = 14;
pub const NSMB_BGDAT_OBJECT_SIZE: usize = 10;
pub const NSMB_OBJECT_BANK_SLOT_COUNT: usize = 16;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelBlock {
    pub offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelHeader {
    pub entrances: [u8; 2],
    pub settings: u16,
    pub timer: u16,
    pub background_theme: [u16; 3],
    pub tileset_theme: [u16; 3],
    pub foreground_theme: [u16; 3],
    pub events: u16,
    pub sound_group: u16,
    pub sprite_priority: u16,
    pub mega_drop_chance: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelCamera {
    pub top_offset: i32,
    pub bottom_offset: i32,
    pub top_offset_special: i32,
    pub bottom_offset_special: i32,
    pub id: u16,
    pub scroll_height: u16,
    pub unknown: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelScreen {
    pub id: u16,
    pub screen_id: u16,
    pub palette_id: u16,
    pub parallax_x: u16,
    pub parallax_y: u16,
    pub scroll_x: u16,
    pub scroll_y: u16,
    pub vertical_offset: u16,
    pub unused_10: u16,
    pub unused_12: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelObject {
    pub object_id: u16,
    pub tileset_slot: u8,
    pub x: i16,
    pub y: i16,
    pub width: u16,
    pub height: u16,
}

/// A stage object; sprites share this record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelStageObject {
    pub id: u16,
    pub x: u16,
    pub y: u16,
    pub events: [u8; 2],
    pub settings: u32,
}

pub type NsmbLevelSprite = NsmbLevelStageObject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum NsmbEntranceType {
    Normal,
    OverriddenNormal,
    Door,
    PipeUp,
    PipeDown,
    PipeLeft,
    PipeRight,
    Falling,
    GroundPounding,
    Sliding,
    Swimming,
    TransitNormal,
    TransitDoor,
    MarioVsLuigiPipe,
    InvisibleDoorNoSound,
    InvisibleDoor,
    MiniPipeUp,
    MiniPipeDown,
    MiniPipeLeft,
    MiniPipeRight,
    Jumping,
    Vine,
}

pub mod entrance_flags {
    pub const SUB_SCREEN: u8 = 0x01;
    pub const LOCKED_PLAYER_1: u8 = 0x02;
    pub const LOCKED_PLAYER_0: u8 = 0x04;
    pub const CONNECTED_PIPE: u8 = 0x08;
    pub const WIPE_FADE: u8 = 0x10;
    pub const ALIGNED_X: u8 = 0x40;
    pub const EXIT_ONLY: u8 = 0x80;
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelEntrance {
    pub x: u16,
    pub y: u16,
    pub camera_x: u16,
    pub camera_y: u16,
    pub id: u8,
    pub target_stage_area: u8,
    pub target_stage_index: u8,
    pub target_stage_group: u8,
    pub target_entrance: u8,
    pub camera_zoom: u8,
    pub kind: u8,
    pub flags: u8,
    pub unknown_10: u8,
    pub unknown_11: u8,
    pub view_id: u8,
    pub target_worldmap_node: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelView {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub id: u8,
    pub camera_settings_id: u8,
    pub bgm_id: u8,
    pub background_id: u8,
    pub tileset_id: u8,
    pub foreground_id: u8,
    pub light_type: u8,
    pub progress_path_id: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelZone {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
    pub id: u8,
    pub unknown_9: u8,
    pub unknown_a: u8,
    pub unknown_b: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelPathNode {
    pub x: i16,
    pub y: i16,
    pub speed: Fx32,
    pub acceleration: Fx32,
    pub delay: u16,
    pub flags: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelPath {
    pub id: u8,
    pub nodes: Vec<NsmbLevelPathNode>,
    pub unknown: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NsmbLevelCourse {
    pub blocks: [NsmbLevelBlock; NSMB_LEVEL_BLOCK_COUNT],
    pub header: NsmbLevelHeader,
    pub cameras: Vec<NsmbLevelCamera>,
    pub backgrounds: Vec<NsmbLevelScreen>,
    pub tilesets: Vec<NsmbLevelScreen>,
    pub foregrounds: Vec<NsmbLevelScreen>,
    pub sprites: Vec<NsmbLevelStageObject>,
    pub entrances: Vec<NsmbLevelEntrance>,
    pub views: Vec<NsmbLevelView>,
    pub zones: Vec<NsmbLevelZone>,
    pub paths: Vec<NsmbLevelPath>,
    pub progress_paths: Vec<NsmbLevelPath>,
    pub object_banks: [u8; NSMB_OBJECT_BANK_SLOT_COUNT],
}

fn i16_at(r: &LittleEndianReader, o: usize) -> Result<i16> {
    Ok(r.u16(o)? as i16)
}

fn i32_at(r: &LittleEndianReader, o: usize) -> Result<i32> {
    Ok(r.u32(o)? as i32)
}

fn block(data: &[u8], b: NsmbLevelBlock) -> Result<&[u8]> {
    let (offset, size) = (b.offset as usize, b.size as usize);
    offset
        .checked_add(size)
        .and_then(|end| data.get(offset..end))
        .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "level block extends past the course file"))
}

/// Offsets of each record in a block, or an error when the block ends in a partial record.
fn record_offsets(b: &[u8], size: usize, name: &str) -> Result<impl Iterator<Item = usize>> {
    if !b.len().is_multiple_of(size) {
        return Err(Error::new(ErrorCode::ParseError, format!("{name} block contains a partial record")));
    }
    Ok((0..b.len()).step_by(size))
}

fn read_screens(b: &[u8]) -> Result<Vec<NsmbLevelScreen>> {
    let r = LittleEndianReader::new(b);
    record_offsets(b, 20, "screen")?
        .map(|o| {
            Ok(NsmbLevelScreen {
                id: r.u16(o)?,
                screen_id: r.u16(o + 2)?,
                palette_id: r.u16(o + 4)?,
                parallax_x: r.u16(o + 6)?,
                parallax_y: r.u16(o + 8)?,
                scroll_x: r.u16(o + 10)?,
                scroll_y: r.u16(o + 12)?,
                vertical_offset: r.u16(o + 14)?,
                unused_10: r.u16(o + 16)?,
                unused_12: r.u16(o + 18)?,
            })
        })
        .collect()
}

fn read_paths(headers: &[u8], nodes: &[u8]) -> Result<Vec<NsmbLevelPath>> {
    let h = LittleEndianReader::new(headers);
    let n = LittleEndianReader::new(nodes);
    let offsets = record_offsets(headers, 8, "path")?;
    let _ = record_offsets(nodes, 16, "path node")?;
    let available = nodes.len() / 16;
    offsets
        .map(|o| {
            let first = h.u16(o + 2)? as usize;
            let count = h.u16(o + 4)? as usize;
            if first > available || count > available - first {
                return Err(Error::new(ErrorCode::OutOfBounds, "path references nodes outside its node block"));
            }
            let mut path = NsmbLevelPath { id: h.u8(o)?, nodes: Vec::with_capacity(count), unknown: h.u16(o + 6)? };
            for i in 0..count {
                let at = (first + i) * 16;
                path.nodes.push(NsmbLevelPathNode {
                    x: i16_at(&n, at)?,
                    y: i16_at(&n, at + 2)?,
                    speed: Fx32::from_raw(i32_at(&n, at + 4)?),
                    acceleration: Fx32::from_raw(i32_at(&n, at + 8)?),
                    delay: n.u16(at + 12)?,
                    flags: n.u16(at + 14)?,
                });
            }
            Ok(path)
        })
        .collect()
}

pub fn parse_nsmb_level_course(data: &[u8]) -> Result<NsmbLevelCourse> {
    let table_size = NSMB_LEVEL_BLOCK_COUNT * 8;
    if data.len() < table_size {
        return Err(Error::new(ErrorCode::ParseError, "NSMB course file is missing the block table"));
    }
    let r = LittleEndianReader::new(data);
    let mut c = NsmbLevelCourse::default();
    let mut b: Vec<&[u8]> = Vec::with_capacity(NSMB_LEVEL_BLOCK_COUNT);
    for (i, slot) in c.blocks.iter_mut().enumerate() {
        *slot = NsmbLevelBlock { offset: r.u32(i * 8)?, size: r.u32(i * 8 + 4)? };
        b.push(block(data, *slot)?);
    }
    let block_at = |i: usize| -> &[u8] { b.get(i).copied().unwrap_or_default() };

    let header = block_at(0);
    if !header.is_empty() && header.len() < 0x20 {
        return Err(Error::new(ErrorCode::ParseError, "level header is shorter than 0x20 bytes"));
    }
    if header.len() >= 0x20 {
        let h = LittleEndianReader::new(header);
        c.header.entrances = [h.u8(0)?, h.u8(1)?];
        c.header.settings = h.u16(2)?;
        c.header.timer = h.u16(4)?;
        for i in 0..3 {
            if let Some(v) = c.header.background_theme.get_mut(i) {
                *v = h.u16(6 + i * 2)?;
            }
            if let Some(v) = c.header.tileset_theme.get_mut(i) {
                *v = h.u16(12 + i * 2)?;
            }
            if let Some(v) = c.header.foreground_theme.get_mut(i) {
                *v = h.u16(18 + i * 2)?;
            }
        }
        c.header.events = h.u16(24)?;
        c.header.sound_group = h.u16(26)?;
        c.header.sprite_priority = h.u16(28)?;
        c.header.mega_drop_chance = h.u16(30)?;
    }

    let cameras = block_at(1);
    let cr = LittleEndianReader::new(cameras);
    for o in record_offsets(cameras, 24, "camera")? {
        c.cameras.push(NsmbLevelCamera {
            top_offset: i32_at(&cr, o)?,
            bottom_offset: i32_at(&cr, o + 4)?,
            top_offset_special: i32_at(&cr, o + 8)?,
            bottom_offset_special: i32_at(&cr, o + 12)?,
            id: cr.u16(o + 16)?,
            scroll_height: cr.u16(o + 18)?,
            unknown: cr.u32(o + 20)?,
        });
    }

    c.backgrounds = read_screens(block_at(2))?;
    c.tilesets = read_screens(block_at(3))?;
    c.foregrounds = read_screens(block_at(4))?;

    let entrances = block_at(5);
    let er = LittleEndianReader::new(entrances);
    for o in record_offsets(entrances, 20, "entrance")? {
        c.entrances.push(NsmbLevelEntrance {
            x: er.u16(o)?,
            y: er.u16(o + 2)?,
            camera_x: er.u16(o + 4)?,
            camera_y: er.u16(o + 6)?,
            id: er.u8(o + 8)?,
            target_stage_area: er.u8(o + 9)?,
            target_stage_index: er.u8(o + 10)?,
            target_stage_group: er.u8(o + 11)?,
            target_entrance: er.u8(o + 12)?,
            camera_zoom: er.u8(o + 13)?,
            kind: er.u8(o + 14)?,
            flags: er.u8(o + 15)?,
            unknown_10: er.u8(o + 16)?,
            unknown_11: er.u8(o + 17)?,
            view_id: er.u8(o + 18)?,
            target_worldmap_node: er.u8(o + 19)?,
        });
    }

    let objects = block_at(6);
    if !objects.is_empty() && objects.len() < 4 {
        return Err(Error::new(ErrorCode::ParseError, "stage object block is missing its terminator"));
    }
    let object_bytes = objects.len().saturating_sub(4);
    if !object_bytes.is_multiple_of(12) {
        return Err(Error::new(ErrorCode::ParseError, "stage object block contains a partial record"));
    }
    let or = LittleEndianReader::new(objects);
    for o in (0..object_bytes).step_by(12) {
        c.sprites.push(NsmbLevelStageObject {
            id: or.u16(o)?,
            x: or.u16(o + 2)?,
            y: or.u16(o + 4)?,
            events: [or.u8(o + 6)?, or.u8(o + 7)?],
            settings: or.u32(o + 8)?,
        });
    }

    let views = block_at(7);
    let vr = LittleEndianReader::new(views);
    for o in record_offsets(views, 16, "view")? {
        c.views.push(NsmbLevelView {
            x: vr.u16(o)?,
            y: vr.u16(o + 2)?,
            width: vr.u16(o + 4)?,
            height: vr.u16(o + 6)?,
            id: vr.u8(o + 8)?,
            camera_settings_id: vr.u8(o + 9)?,
            bgm_id: vr.u8(o + 10)?,
            background_id: vr.u8(o + 11)?,
            tileset_id: vr.u8(o + 12)?,
            foreground_id: vr.u8(o + 13)?,
            light_type: vr.u8(o + 14)?,
            progress_path_id: vr.u8(o + 15)?,
        });
    }

    let zones = block_at(8);
    let zr = LittleEndianReader::new(zones);
    for o in record_offsets(zones, 12, "zone")? {
        c.zones.push(NsmbLevelZone {
            x: zr.u16(o)?,
            y: zr.u16(o + 2)?,
            width: zr.u16(o + 4)?,
            height: zr.u16(o + 6)?,
            id: zr.u8(o + 8)?,
            unknown_9: zr.u8(o + 9)?,
            unknown_a: zr.u8(o + 10)?,
            unknown_b: zr.u8(o + 11)?,
        });
    }

    c.progress_paths = read_paths(block_at(9), block_at(11))?;
    c.paths = read_paths(block_at(10), block_at(12))?;
    let banks = block_at(13);
    let take = banks.len().min(c.object_banks.len());
    if let (Some(dst), Some(src)) = (c.object_banks.get_mut(..take), banks.get(..take)) {
        dst.copy_from_slice(src);
    }
    Ok(c)
}

pub fn parse_nsmb_bgdat_objects(data: &[u8]) -> Result<Vec<NsmbLevelObject>> {
    let mut data = data;
    if data.len() % NSMB_BGDAT_OBJECT_SIZE == 2 && data.ends_with(&[0xFF, 0xFF]) {
        data = data.get(..data.len() - 2).unwrap_or_default();
    }
    if !data.len().is_multiple_of(NSMB_BGDAT_OBJECT_SIZE) {
        return Err(Error::new(ErrorCode::ParseError, "NSMB BGDat object data must be made of 10-byte records"));
    }
    let r = LittleEndianReader::new(data);
    (0..data.len())
        .step_by(NSMB_BGDAT_OBJECT_SIZE)
        .map(|o| {
            let id = r.u16(o)?;
            Ok(NsmbLevelObject {
                object_id: id & 0xFFF,
                tileset_slot: (id >> 12) as u8,
                x: i16_at(&r, o + 2)?,
                y: i16_at(&r, o + 4)?,
                width: r.u16(o + 6)?,
                height: r.u16(o + 8)?,
            })
        })
        .collect()
}
