use std::collections::HashSet;

use bigstar_core::{Error, ErrorCode, LittleEndianReader, LittleEndianWriter, Result};
use bigstar_formats::{
    GlueBlock, GlueValue, NSMB_BGDAT_OBJECT_SIZE, NSMB_HEADER_SIZE, NSMB_LEVEL_BLOCK_COUNT, NSMB_OBJECT_BANK_SLOT_COUNT, NsmbLevelCourse,
    NsmbLevelObject, NsmbLevelPath, fnv1a_32, write_glue_block,
};
use serde::{Deserialize, Serialize};

pub(crate) const STAGE_OBJECTS_HASH: u32 = fnv1a_32("glue.stageObjects");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelTilesetReferences {
    pub tileset_id: u16,
    pub background_graphics_id: u16,
    pub background_screen_id: u16,
    pub foreground_graphics_id: u16,
    pub foreground_screen_id: u16,
}

/// Everything the history edits.
#[derive(Debug, Clone)]
pub(crate) struct LevelState {
    pub course: NsmbLevelCourse,
    pub objects: Vec<NsmbLevelObject>,
    pub raw_blocks: [Vec<u8>; NSMB_LEVEL_BLOCK_COUNT],
    pub glue: GlueBlock,
    pub glue_error: Option<String>,
    pub sprite_hashes: Vec<Option<u32>>,
    pub loaded_registration: Vec<u32>,
    pub stage_object_id_base: u16,
}

fn error(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

pub(crate) fn read_registration(glue: &GlueBlock) -> Result<Vec<u32>> {
    let Some(found) = glue.values.iter().find(|value| value.hash == STAGE_OBJECTS_HASH) else {
        return Ok(Vec::new());
    };
    let reader = LittleEndianReader::new(&found.payload);
    let count = reader.u32(0).map_err(|_| error(ErrorCode::ParseError, "glue.stageObjects is missing its count"))?;
    if 4 + u64::from(count) * 4 != found.payload.len() as u64 {
        return Err(error(ErrorCode::ParseError, "glue.stageObjects size does not match its count"));
    }
    (0..count as usize).map(|i| reader.u32(4 + i * 4)).collect()
}

pub(crate) fn write_registration(hashes: &[u32]) -> Vec<u8> {
    let mut payload = Vec::with_capacity(4 + hashes.len() * 4);
    payload.extend_from_slice(&(hashes.len() as u32).to_le_bytes());
    for hash in hashes {
        payload.extend_from_slice(&hash.to_le_bytes());
    }
    payload
}

/// Stores `registration` in `glue`, replacing the existing entry or appending one after the others.
fn store_registration(glue: &mut GlueBlock, registration: &[u32]) {
    if let Some(existing) = glue.values.iter_mut().find(|value| value.hash == STAGE_OBJECTS_HASH) {
        existing.payload = write_registration(registration);
    } else {
        let order = glue.values.iter().map(|value| value.payload_order + 1).max().unwrap_or(0);
        glue.values.push(GlueValue { hash: STAGE_OBJECTS_HASH, payload: write_registration(registration), payload_order: order });
    }
}

/// `glue` ready to hold `registration`, or without the entry when nothing is registered.
pub(crate) fn glue_with_registration(glue: &GlueBlock, registration: &[u32], drop_when_empty: bool) -> GlueBlock {
    let mut glue = glue.clone();
    if registration.is_empty() && drop_when_empty {
        glue.values.retain(|value| value.hash != STAGE_OBJECTS_HASH);
    } else {
        store_registration(&mut glue, registration);
    }
    glue
}

fn put16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn encode_paths(paths: &[NsmbLevelPath]) -> Result<(Vec<u8>, Vec<u8>)> {
    let mut node_count = 0usize;
    for path in paths {
        if path.nodes.len() > usize::from(u16::MAX) || node_count > usize::from(u16::MAX) - path.nodes.len() {
            return Err(error(ErrorCode::InvalidArgument, "path node table exceeds the NSMB format limit"));
        }
        node_count += path.nodes.len();
    }
    let mut headers = Vec::with_capacity(paths.len() * 8);
    let mut nodes = Vec::with_capacity(node_count * 16);
    let mut first_node = 0usize;
    for path in paths {
        headers.push(path.id);
        headers.push(0);
        put16(&mut headers, first_node as u16);
        put16(&mut headers, path.nodes.len() as u16);
        put16(&mut headers, path.unknown);
        for node in &path.nodes {
            first_node += 1;
            put16(&mut nodes, node.x as u16);
            put16(&mut nodes, node.y as u16);
            put32(&mut nodes, node.speed.raw() as u32);
            put32(&mut nodes, node.acceleration.raw() as u32);
            put16(&mut nodes, node.delay);
            put16(&mut nodes, node.flags);
        }
    }
    Ok((headers, nodes))
}

fn patch_screen_id(block: &mut Vec<u8>, screen_id: u16) -> Result<()> {
    if block.len() < 20 {
        block.resize(20, 0);
    }
    LittleEndianWriter::new(block).u16(2, screen_id)
}

impl LevelState {
    pub(crate) fn tileset_references(&self) -> LevelTilesetReferences {
        LevelTilesetReferences {
            tileset_id: self.course.header.tileset_theme[0],
            background_graphics_id: self.course.header.background_theme[0],
            background_screen_id: self.course.backgrounds.first().map_or(0, |screen| screen.screen_id),
            foreground_graphics_id: self.course.header.foreground_theme[0],
            foreground_screen_id: self.course.foregrounds.first().map_or(0, |screen| screen.screen_id),
        }
    }

    pub(crate) fn sprite_hash(&self, index: usize) -> Option<u32> {
        self.sprite_hashes.get(index).copied().flatten()
    }

    /// Hashes in registration order: the ones the level loaded with that are still used, then new ones in sprite order.
    pub(crate) fn registration_hashes(&self) -> Vec<u32> {
        let mut used: HashSet<u32> = self.sprite_hashes.iter().flatten().copied().collect();
        let mut result = Vec::new();
        for hash in &self.loaded_registration {
            if used.remove(hash) {
                result.push(*hash);
            }
        }
        for hash in self.sprite_hashes.iter().flatten() {
            if used.remove(hash) {
                result.push(*hash);
            }
        }
        result
    }

    fn registered_id(&self, registration: &[u32], hash: u32) -> Option<u16> {
        let position = registration.iter().position(|candidate| *candidate == hash)?;
        u16::try_from(usize::from(self.stage_object_id_base) + position).ok()
    }

    /// Re-derives the id of every custom placement from the current registration.
    pub(crate) fn refresh_custom_ids(&mut self) {
        let registration = self.registration_hashes();
        for index in 0..self.course.sprites.len() {
            let Some(hash) = self.sprite_hash(index) else { continue };
            let Some(id) = self.registered_id(&registration, hash) else { continue };
            if let Some(sprite) = self.course.sprites.get_mut(index) {
                sprite.id = id;
            }
        }
    }

    pub(crate) fn serialize_course(&self) -> Result<Vec<u8>> {
        let mut blocks = self.raw_blocks.clone();
        if blocks[0].len() < NSMB_HEADER_SIZE {
            return Err(error(ErrorCode::ParseError, "level header is too small to save"));
        }
        let mut sprites = self.course.sprites.clone();
        if self.glue_error.is_none() {
            let registration = self.registration_hashes();
            let base = self.stage_object_id_base;
            let unresolved = sprites.iter().enumerate().any(|(i, sprite)| sprite.id >= base && self.sprite_hash(i).is_none());
            if unresolved && registration != self.loaded_registration {
                return Err(error(
                    ErrorCode::InvalidArgument,
                    "custom stage-object registration changed while an unresolved placement still uses it",
                ));
            }
            for (i, sprite) in sprites.iter_mut().enumerate() {
                let Some(hash) = self.sprite_hash(i) else { continue };
                if !registration.contains(&hash) {
                    return Err(error(ErrorCode::InvalidArgument, "custom stage-object placement has no registration"));
                }
                sprite.id = self
                    .registered_id(&registration, hash)
                    .ok_or_else(|| error(ErrorCode::InvalidArgument, "custom stage-object IDs exceed the level format"))?;
            }
            let glue = glue_with_registration(&self.glue, &registration, true);
            let tail = write_glue_block(&glue)?;
            let h = &self.course.header;
            let mut header = vec![h.entrances[0], h.entrances[1]];
            put16(&mut header, h.settings);
            put16(&mut header, h.timer);
            for value in h.background_theme {
                put16(&mut header, value);
            }
            for value in h.tileset_theme {
                put16(&mut header, value);
            }
            for value in h.foreground_theme {
                put16(&mut header, value);
            }
            for value in [h.events, h.sound_group, h.sprite_priority, h.mega_drop_chance] {
                put16(&mut header, value);
            }
            header.extend_from_slice(&tail);
            blocks[0] = header;
        }
        if let Some(screen) = self.course.backgrounds.first() {
            patch_screen_id(&mut blocks[2], screen.screen_id)?;
        }
        if let Some(screen) = self.course.foregrounds.first() {
            patch_screen_id(&mut blocks[4], screen.screen_id)?;
        }

        blocks[5].clear();
        for e in &self.course.entrances {
            for value in [e.x, e.y, e.camera_x, e.camera_y] {
                put16(&mut blocks[5], value);
            }
            blocks[5].extend_from_slice(&[
                e.id,
                e.target_stage_area,
                e.target_stage_index,
                e.target_stage_group,
                e.target_entrance,
                e.camera_zoom,
                e.kind,
                e.flags,
                e.unknown_10,
                e.unknown_11,
                e.view_id,
                e.target_worldmap_node,
            ]);
        }

        blocks[6].clear();
        for s in &sprites {
            for value in [s.id, s.x, s.y] {
                put16(&mut blocks[6], value);
            }
            blocks[6].extend_from_slice(&s.events);
            put32(&mut blocks[6], s.settings);
        }
        blocks[6].extend_from_slice(&[0xFF; 4]);

        blocks[7].clear();
        for v in &self.course.views {
            for value in [v.x, v.y, v.width, v.height] {
                put16(&mut blocks[7], value);
            }
            blocks[7].extend_from_slice(&[
                v.id,
                v.camera_settings_id,
                v.bgm_id,
                v.background_id,
                v.tileset_id,
                v.foreground_id,
                v.light_type,
                v.progress_path_id,
            ]);
        }

        blocks[8].clear();
        for z in &self.course.zones {
            for value in [z.x, z.y, z.width, z.height] {
                put16(&mut blocks[8], value);
            }
            blocks[8].extend_from_slice(&[z.id, z.unknown_9, z.unknown_a, z.unknown_b]);
        }

        (blocks[10], blocks[12]) = encode_paths(&self.course.paths)?;
        (blocks[9], blocks[11]) = encode_paths(&self.course.progress_paths)?;
        if blocks[13].len() < NSMB_OBJECT_BANK_SLOT_COUNT {
            blocks[13].resize(NSMB_OBJECT_BANK_SLOT_COUNT, 0);
        }
        blocks[13]
            .get_mut(..NSMB_OBJECT_BANK_SLOT_COUNT)
            .ok_or_else(|| error(ErrorCode::OutOfBounds, "object bank table is too small"))?
            .copy_from_slice(&self.course.object_banks);

        let too_large = || error(ErrorCode::InvalidArgument, "course file exceeds the NSMB format limit");
        let mut table = Vec::with_capacity(NSMB_LEVEL_BLOCK_COUNT * 8);
        let mut offset = u32::try_from(NSMB_LEVEL_BLOCK_COUNT * 8).map_err(|_| too_large())?;
        for block in &blocks {
            let size = u32::try_from(block.len()).map_err(|_| too_large())?;
            put32(&mut table, offset);
            put32(&mut table, size);
            offset = offset.checked_add(size).ok_or_else(too_large)?;
        }
        for block in &blocks {
            table.extend_from_slice(block);
        }
        Ok(table)
    }

    pub(crate) fn serialize_bgdat(&self) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(self.objects.len() * NSMB_BGDAT_OBJECT_SIZE + 2);
        for object in &self.objects {
            if object.object_id > 0x0FFF || object.tileset_slot > 0x0F {
                return Err(error(ErrorCode::InvalidArgument, "object ID or tileset slot exceeds the BGDat format limit"));
            }
            put16(&mut data, object.object_id | (u16::from(object.tileset_slot) << 12));
            put16(&mut data, object.x as u16);
            put16(&mut data, object.y as u16);
            put16(&mut data, object.width);
            put16(&mut data, object.height);
        }
        data.extend_from_slice(&[0xFF, 0xFF]);
        Ok(data)
    }
}
