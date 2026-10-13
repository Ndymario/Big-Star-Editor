use bigstar_core::{CommandHistory, Error, ErrorCode, Result};
use bigstar_formats::{
    GlueBlock, NSMB_HEADER_SIZE, NsmbLevelCourse, NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelSprite, NsmbLevelView, NsmbLevelZone,
    parse_glue_block, parse_nsmb_bgdat_objects, parse_nsmb_level_course, write_glue_block,
};
use serde::{Deserialize, Serialize};

use crate::commands::{Cmd, Composite, Erase, Field, Insert, Replace, Tileset, Value};
use crate::module_data::{DEFAULT_STAGE_OBJECT_ID_BASE, ModuleDataCatalog};
use crate::state::{LevelState, LevelTilesetReferences, glue_with_registration, read_registration};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexedLevelValue<T> {
    pub index: usize,
    pub value: T,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelEntityUpdates {
    pub objects: Vec<IndexedLevelValue<NsmbLevelObject>>,
    pub sprites: Vec<IndexedLevelValue<NsmbLevelSprite>>,
    pub entrances: Vec<IndexedLevelValue<NsmbLevelEntrance>>,
    pub views: Vec<IndexedLevelValue<NsmbLevelView>>,
    pub zones: Vec<IndexedLevelValue<NsmbLevelZone>>,
    pub paths: Vec<IndexedLevelValue<NsmbLevelPath>>,
    pub progress_paths: Vec<IndexedLevelValue<NsmbLevelPath>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelEntitySelection {
    pub objects: Vec<usize>,
    pub sprites: Vec<usize>,
    pub entrances: Vec<usize>,
    pub views: Vec<usize>,
    pub zones: Vec<usize>,
    pub paths: Vec<usize>,
    pub progress_paths: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpriteInsertion {
    pub value: NsmbLevelSprite,
    pub hash: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelEntityInsertions {
    pub objects: Vec<NsmbLevelObject>,
    pub sprites: Vec<SpriteInsertion>,
    pub entrances: Vec<NsmbLevelEntrance>,
    pub views: Vec<NsmbLevelView>,
    pub zones: Vec<NsmbLevelZone>,
    pub paths: Vec<NsmbLevelPath>,
    pub progress_paths: Vec<NsmbLevelPath>,
}

/// The entity lists whose order is the draw order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReorderKind {
    Object,
    Sprite,
    Entrance,
    Zone,
    Path,
    ProgressPath,
}

pub struct EditableLevel {
    state: LevelState,
    history: CommandHistory<LevelState>,
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidArgument, message)
}

fn out_of_bounds() -> Error {
    Error::new(ErrorCode::OutOfBounds, "item index is outside the collection")
}

fn replacements<V: Clone + Send + 'static>(
    state: &mut LevelState,
    field: Field<Vec<V>>,
    items: Vec<IndexedLevelValue<V>>,
    name: &str,
    commands: &mut Vec<Cmd>,
) -> Result<()> {
    for item in items {
        commands.push(Box::new(Replace::new(state, field, item.index, item.value, name)?));
    }
    Ok(())
}

/// Erase commands for `indices`, highest first so earlier ones do not shift later ones.
fn erasures<V: Clone + Send + 'static>(
    state: &mut LevelState,
    field: Field<Vec<V>>,
    mut indices: Vec<usize>,
    name: &str,
    commands: &mut Vec<Cmd>,
) -> Result<()> {
    indices.sort_unstable_by(|a, b| b.cmp(a));
    indices.dedup();
    for index in indices {
        commands.push(Box::new(Erase::new(state, field, index, name)?));
    }
    Ok(())
}

fn insertions<V: Clone + Send + 'static>(
    state: &mut LevelState,
    field: Field<Vec<V>>,
    values: Vec<V>,
    name: &str,
    commands: &mut Vec<Cmd>,
) -> Vec<usize> {
    let first = field(state).len();
    let mut indices = Vec::with_capacity(values.len());
    for (offset, value) in values.into_iter().enumerate() {
        indices.push(first + offset);
        commands.push(Box::new(Insert::new(field, first + offset, value, name)));
    }
    indices
}

fn objects(state: &mut LevelState) -> &mut Vec<NsmbLevelObject> {
    &mut state.objects
}
fn sprites(state: &mut LevelState) -> &mut Vec<NsmbLevelSprite> {
    &mut state.course.sprites
}
fn sprite_hashes(state: &mut LevelState) -> &mut Vec<Option<u32>> {
    &mut state.sprite_hashes
}
fn entrances(state: &mut LevelState) -> &mut Vec<NsmbLevelEntrance> {
    &mut state.course.entrances
}
fn views(state: &mut LevelState) -> &mut Vec<NsmbLevelView> {
    &mut state.course.views
}
fn zones(state: &mut LevelState) -> &mut Vec<NsmbLevelZone> {
    &mut state.course.zones
}
fn paths(state: &mut LevelState) -> &mut Vec<NsmbLevelPath> {
    &mut state.course.paths
}
fn progress_paths(state: &mut LevelState) -> &mut Vec<NsmbLevelPath> {
    &mut state.course.progress_paths
}

impl EditableLevel {
    pub fn load(course_data: &[u8], bgdat_data: &[u8], catalog: Option<&ModuleDataCatalog>) -> Result<Self> {
        let course = parse_nsmb_level_course(course_data)?;
        let objects = parse_nsmb_bgdat_objects(bgdat_data)?;
        let mut raw_blocks: [Vec<u8>; bigstar_formats::NSMB_LEVEL_BLOCK_COUNT] = Default::default();
        for (slot, block) in raw_blocks.iter_mut().zip(course.blocks.iter()) {
            let (offset, size) = (block.offset as usize, block.size as usize);
            let bytes = offset
                .checked_add(size)
                .and_then(|end| course_data.get(offset..end))
                .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "level block extends past the course file"))?;
            *slot = bytes.to_vec();
        }
        let mut glue = GlueBlock::default();
        let mut glue_error = None;
        let mut registration = Vec::new();
        if let Some(tail) = raw_blocks[0].get(NSMB_HEADER_SIZE..).filter(|tail| !tail.is_empty()) {
            match parse_glue_block(tail) {
                Err(error) => glue_error = Some(error.message),
                Ok(parsed) => {
                    glue = parsed;
                    match read_registration(&glue) {
                        Ok(decoded) => registration = decoded,
                        Err(error) => glue_error = Some(error.message),
                    }
                }
            }
        }
        let base = catalog.map_or(DEFAULT_STAGE_OBJECT_ID_BASE, |catalog| catalog.stage_object_id_base);
        let mut sprite_hashes = vec![None; course.sprites.len()];
        if glue_error.is_none() {
            for (hash, sprite) in sprite_hashes.iter_mut().zip(&course.sprites) {
                if let Some(position) = sprite.id.checked_sub(base) {
                    *hash = registration.get(usize::from(position)).copied();
                }
            }
        }
        let state = LevelState {
            course,
            objects,
            raw_blocks,
            glue,
            glue_error,
            sprite_hashes,
            loaded_registration: registration,
            stage_object_id_base: base,
        };
        Ok(Self { state, history: CommandHistory::new() })
    }

    pub fn course(&self) -> &NsmbLevelCourse {
        &self.state.course
    }

    pub fn objects(&self) -> &[NsmbLevelObject] {
        &self.state.objects
    }

    pub fn tileset_references(&self) -> LevelTilesetReferences {
        self.state.tileset_references()
    }

    pub fn glue(&self) -> &GlueBlock {
        &self.state.glue
    }

    pub fn glue_error(&self) -> Option<&str> {
        self.state.glue_error.as_deref()
    }

    pub fn stage_object_id_base(&self) -> u16 {
        self.state.stage_object_id_base
    }

    pub fn sprite_hash(&self, index: usize) -> Option<u32> {
        self.state.sprite_hash(index)
    }

    pub fn registration_hashes(&self) -> Vec<u32> {
        self.state.registration_hashes()
    }

    /// The vanilla or module object id a sprite placement stands for.
    pub fn resolve_sprite_object_id(&self, index: usize, vanilla_object_ids: &[u16], catalog: &ModuleDataCatalog) -> Option<u32> {
        let sprite = self.state.course.sprites.get(index)?;
        match self.sprite_hash(index) {
            Some(hash) => catalog.stage_object_by_hash(hash).map(|declaration| declaration.object_id),
            None => vanilla_object_ids.get(usize::from(sprite.id)).copied().map(u32::from),
        }
    }

    pub fn serialize_course(&self) -> Result<Vec<u8>> {
        self.state.serialize_course()
    }

    pub fn serialize_bgdat(&self) -> Result<Vec<u8>> {
        self.state.serialize_bgdat()
    }

    fn run(&mut self, command: Cmd) -> Result<()> {
        self.history.execute(&mut self.state, command)
    }

    /// Runs `commands` as one undo step named `name`, then re-derives custom stage-object ids.
    fn run_composite(&mut self, commands: Vec<Cmd>, name: &str) -> Result<()> {
        self.run(Box::new(Composite::new(commands, name)))?;
        self.state.refresh_custom_ids();
        Ok(())
    }

    pub fn set_tileset_references(&mut self, references: LevelTilesetReferences) -> Result<()> {
        let command = Tileset::new(&self.state, references);
        self.run(Box::new(command))
    }

    pub fn replace_serialized(&mut self, course_data: &[u8], bgdat_data: &[u8], catalog: Option<&ModuleDataCatalog>, name: &str) -> Result<()> {
        let replacement = Self::load(course_data, bgdat_data, catalog)?.state;
        let state = &mut self.state;
        let commands: Vec<Cmd> = vec![
            Box::new(Value::new(state, |s| &mut s.course, replacement.course, name)),
            Box::new(Value::new(state, |s| &mut s.objects, replacement.objects, name)),
            Box::new(Value::new(state, |s| &mut s.raw_blocks, replacement.raw_blocks, name)),
            Box::new(Value::new(state, |s| &mut s.glue, replacement.glue, name)),
            Box::new(Value::new(state, |s| &mut s.glue_error, replacement.glue_error, name)),
            Box::new(Value::new(state, |s| &mut s.sprite_hashes, replacement.sprite_hashes, name)),
            Box::new(Value::new(state, |s| &mut s.loaded_registration, replacement.loaded_registration, name)),
            Box::new(Value::new(state, |s| &mut s.stage_object_id_base, replacement.stage_object_id_base, name)),
        ];
        self.run_composite(commands, name)
    }

    pub fn set_glue(&mut self, glue: GlueBlock, name: &str) -> Result<()> {
        if let Some(message) = &self.state.glue_error {
            return Err(Error::new(ErrorCode::ParseError, message.clone()));
        }
        match (read_registration(&self.state.glue), read_registration(&glue)) {
            (Ok(before), Ok(after)) if before == after => {}
            _ => return Err(invalid("stage-object registration is derived and cannot be edited directly")),
        }
        write_glue_block(&glue)?;
        let command = Value::new(&mut self.state, |s| &mut s.glue, glue, name);
        self.run(Box::new(command))
    }

    fn replace_at<V: Clone + Send + 'static>(&mut self, field: Field<Vec<V>>, index: usize, value: V, name: &str) -> Result<()> {
        let command = Replace::new(&mut self.state, field, index, value, name)?;
        self.run(Box::new(command))
    }

    fn append<V: Clone + Send + 'static>(&mut self, field: Field<Vec<V>>, value: V, name: &str) -> Result<usize> {
        let index = field(&mut self.state).len();
        self.run(Box::new(Insert::new(field, index, value, name)))?;
        Ok(index)
    }

    fn erase_at<V: Clone + Send + 'static>(&mut self, field: Field<Vec<V>>, index: usize, name: &str) -> Result<()> {
        let command = Erase::new(&mut self.state, field, index, name)?;
        self.run(Box::new(command))
    }

    pub fn update_object(&mut self, index: usize, object: NsmbLevelObject) -> Result<()> {
        self.replace_at(objects, index, object, "Edit object")
    }

    pub fn add_object(&mut self, object: NsmbLevelObject) -> Result<usize> {
        self.append(objects, object, "Add object")
    }

    pub fn remove_object(&mut self, index: usize) -> Result<()> {
        self.erase_at(objects, index, "Delete object")
    }

    pub fn update_sprite(&mut self, index: usize, sprite: NsmbLevelSprite) -> Result<()> {
        self.replace_at(sprites, index, sprite, "Edit sprite")?;
        self.state.refresh_custom_ids();
        Ok(())
    }

    fn add_sprite_with_hash(&mut self, sprite: NsmbLevelSprite, hash: Option<u32>, name: &str) -> Result<usize> {
        let index = self.state.course.sprites.len();
        let commands: Vec<Cmd> = vec![Box::new(Insert::new(sprites, index, sprite, name)), Box::new(Insert::new(sprite_hashes, index, hash, name))];
        self.run_composite(commands, name)?;
        Ok(index)
    }

    pub fn add_sprite(&mut self, sprite: NsmbLevelSprite) -> Result<usize> {
        self.add_sprite_with_hash(sprite, None, "Add sprite")
    }

    pub fn add_custom_sprite(&mut self, hash: u32, sprite: NsmbLevelSprite) -> Result<usize> {
        if hash == 0 {
            return Err(invalid("custom stage object has the reserved zero hash"));
        }
        if self.state.glue_error.is_some() {
            return Err(invalid("custom stage object cannot be added while glue is unreadable"));
        }
        let mut registration = self.state.registration_hashes();
        if !registration.contains(&hash) {
            registration.push(hash);
        }
        if usize::from(self.state.stage_object_id_base) + registration.len() - 1 > usize::from(u16::MAX) {
            return Err(invalid("custom stage-object IDs exceed the level format"));
        }
        write_glue_block(&glue_with_registration(&self.state.glue, &registration, false))?;
        self.add_sprite_with_hash(sprite, Some(hash), "Add custom sprite")
    }

    pub fn remove_sprite(&mut self, index: usize) -> Result<()> {
        if index >= self.state.course.sprites.len() {
            return Err(out_of_bounds());
        }
        let name = "Delete sprite";
        let commands: Vec<Cmd> =
            vec![Box::new(Erase::new(&mut self.state, sprites, index, name)?), Box::new(Erase::new(&mut self.state, sprite_hashes, index, name)?)];
        self.run_composite(commands, name)
    }

    pub fn update_entrance(&mut self, index: usize, entrance: NsmbLevelEntrance) -> Result<()> {
        self.replace_at(entrances, index, entrance, "Edit entrance")
    }

    pub fn add_entrance(&mut self, entrance: NsmbLevelEntrance) -> Result<usize> {
        self.append(entrances, entrance, "Add entrance")
    }

    pub fn remove_entrance(&mut self, index: usize) -> Result<()> {
        self.erase_at(entrances, index, "Delete entrance")
    }

    pub fn update_view(&mut self, index: usize, view: NsmbLevelView) -> Result<()> {
        self.replace_at(views, index, view, "Edit view")
    }

    pub fn add_view(&mut self, view: NsmbLevelView) -> Result<usize> {
        self.append(views, view, "Add view")
    }

    pub fn remove_view(&mut self, index: usize) -> Result<()> {
        self.erase_at(views, index, "Delete view")
    }

    pub fn set_object_bank(&mut self, slot: usize, value: u8) -> Result<()> {
        let mut banks = self.state.course.object_banks;
        let target = banks.get_mut(slot).ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "object-bank slot is outside the table"))?;
        *target = value;
        let command = Value::new(&mut self.state, |s| &mut s.course.object_banks, banks, "Edit object bank");
        self.run(Box::new(command))
    }

    pub fn update_zone(&mut self, index: usize, zone: NsmbLevelZone) -> Result<()> {
        self.replace_at(zones, index, zone, "Edit zone")
    }

    pub fn add_zone(&mut self, zone: NsmbLevelZone) -> Result<usize> {
        self.append(zones, zone, "Add zone")
    }

    pub fn remove_zone(&mut self, index: usize) -> Result<()> {
        self.erase_at(zones, index, "Delete zone")
    }

    pub fn update_path(&mut self, progress: bool, index: usize, path: NsmbLevelPath) -> Result<()> {
        if progress { self.replace_at(progress_paths, index, path, "Edit progress path") } else { self.replace_at(paths, index, path, "Edit path") }
    }

    pub fn add_path(&mut self, progress: bool, path: NsmbLevelPath) -> Result<usize> {
        if progress { self.append(progress_paths, path, "Add progress path") } else { self.append(paths, path, "Add path") }
    }

    pub fn remove_path(&mut self, progress: bool, index: usize) -> Result<()> {
        if progress { self.erase_at(progress_paths, index, "Delete progress path") } else { self.erase_at(paths, index, "Delete path") }
    }

    pub fn update_entities(&mut self, updates: LevelEntityUpdates, name: &str) -> Result<()> {
        let mut commands = Vec::new();
        let state = &mut self.state;
        replacements(state, objects, updates.objects, "Edit object", &mut commands)?;
        replacements(state, sprites, updates.sprites, "Edit sprite", &mut commands)?;
        replacements(state, entrances, updates.entrances, "Edit entrance", &mut commands)?;
        replacements(state, views, updates.views, "Edit view", &mut commands)?;
        replacements(state, zones, updates.zones, "Edit zone", &mut commands)?;
        replacements(state, paths, updates.paths, "Edit path", &mut commands)?;
        replacements(state, progress_paths, updates.progress_paths, "Edit progress path", &mut commands)?;
        if commands.is_empty() {
            return Ok(());
        }
        self.run_composite(commands, name)
    }

    pub fn remove_entities(&mut self, selection: LevelEntitySelection, name: &str) -> Result<()> {
        let mut commands = Vec::new();
        let state = &mut self.state;
        erasures(state, objects, selection.objects, "Delete object", &mut commands)?;
        erasures(state, sprites, selection.sprites.clone(), "Delete sprite", &mut commands)?;
        erasures(state, sprite_hashes, selection.sprites, "Delete sprite", &mut commands)?;
        erasures(state, entrances, selection.entrances, "Delete entrance", &mut commands)?;
        erasures(state, views, selection.views, "Delete view", &mut commands)?;
        erasures(state, zones, selection.zones, "Delete zone", &mut commands)?;
        erasures(state, paths, selection.paths, "Delete path", &mut commands)?;
        erasures(state, progress_paths, selection.progress_paths, "Delete progress path", &mut commands)?;
        if commands.is_empty() {
            return Ok(());
        }
        self.run_composite(commands, name)
    }

    pub fn add_entities(&mut self, items: LevelEntityInsertions, name: &str) -> Result<LevelEntitySelection> {
        if items.sprites.iter().any(|sprite| sprite.hash == Some(0)) {
            return Err(invalid("custom stage object has the reserved zero hash"));
        }
        if self.state.glue_error.is_some() && items.sprites.iter().any(|sprite| sprite.hash.is_some()) {
            return Err(invalid("custom stage objects cannot be pasted while glue is unreadable"));
        }
        let mut registration = self.state.registration_hashes();
        for hash in items.sprites.iter().filter_map(|sprite| sprite.hash) {
            if !registration.contains(&hash) {
                registration.push(hash);
            }
        }
        if usize::from(self.state.stage_object_id_base) + registration.len() > usize::from(u16::MAX) + 1 {
            return Err(invalid("custom stage-object IDs exceed the level format"));
        }
        let mut commands = Vec::new();
        let state = &mut self.state;
        let mut added =
            LevelEntitySelection { objects: insertions(state, objects, items.objects, "Paste object", &mut commands), ..Default::default() };
        let first = state.course.sprites.len();
        for (offset, sprite) in items.sprites.into_iter().enumerate() {
            added.sprites.push(first + offset);
            commands.push(Box::new(Insert::new(sprites, first + offset, sprite.value, "Paste sprite")));
            commands.push(Box::new(Insert::new(sprite_hashes, first + offset, sprite.hash, "Paste sprite")));
        }
        added.entrances = insertions(state, entrances, items.entrances, "Paste entrance", &mut commands);
        added.views = insertions(state, views, items.views, "Paste view", &mut commands);
        added.zones = insertions(state, zones, items.zones, "Paste zone", &mut commands);
        added.paths = insertions(state, paths, items.paths, "Paste path", &mut commands);
        added.progress_paths = insertions(state, progress_paths, items.progress_paths, "Paste progress path", &mut commands);
        if commands.is_empty() {
            return Ok(added);
        }
        self.run_composite(commands, name)?;
        Ok(added)
    }

    /// Swaps an entity with its neighbour in draw order and returns its new index.
    pub fn reorder_entity(&mut self, kind: ReorderKind, index: usize, forward: bool) -> Result<usize> {
        fn swap<V: Clone + Send + 'static>(
            state: &mut LevelState,
            field: Field<Vec<V>>,
            index: usize,
            forward: bool,
            name: &str,
            commands: &mut Vec<Cmd>,
        ) -> Result<usize> {
            let len = field(state).len();
            if index >= len {
                return Err(out_of_bounds());
            }
            if (!forward && index == 0) || (forward && index + 1 >= len) {
                return Ok(index);
            }
            let other = if forward { index + 1 } else { index - 1 };
            let at_index = field(state).get(index).cloned().ok_or_else(out_of_bounds)?;
            let at_other = field(state).get(other).cloned().ok_or_else(out_of_bounds)?;
            commands.push(Box::new(Replace::new(state, field, index, at_other, name)?));
            commands.push(Box::new(Replace::new(state, field, other, at_index, name)?));
            Ok(other)
        }

        let mut commands = Vec::new();
        let state = &mut self.state;
        let target = match kind {
            ReorderKind::Object => swap(state, objects, index, forward, "Reorder object", &mut commands)?,
            ReorderKind::Sprite => {
                let target = swap(state, sprites, index, forward, "Reorder sprite", &mut commands)?;
                if target != index {
                    swap(state, sprite_hashes, index, forward, "Reorder sprite", &mut commands)?;
                }
                target
            }
            ReorderKind::Entrance => swap(state, entrances, index, forward, "Reorder entrance", &mut commands)?,
            ReorderKind::Zone => swap(state, zones, index, forward, "Reorder zone", &mut commands)?,
            ReorderKind::Path => swap(state, paths, index, forward, "Reorder path", &mut commands)?,
            ReorderKind::ProgressPath => swap(state, progress_paths, index, forward, "Reorder progress path", &mut commands)?,
        };
        if commands.is_empty() {
            return Ok(target);
        }
        self.run_composite(commands, if forward { "Bring forward" } else { "Send backward" })?;
        Ok(target)
    }

    pub fn undo(&mut self) -> Result<()> {
        self.history.undo(&mut self.state)?;
        self.state.refresh_custom_ids();
        Ok(())
    }

    pub fn redo(&mut self) -> Result<()> {
        self.history.redo(&mut self.state)?;
        self.state.refresh_custom_ids();
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn dirty(&self) -> bool {
        self.history.dirty()
    }

    pub fn undo_name(&self) -> String {
        self.history.undo_name()
    }

    pub fn redo_name(&self) -> String {
        self.history.redo_name()
    }

    pub fn undo_names(&self) -> Vec<String> {
        self.history.undo_names()
    }

    pub fn redo_names(&self) -> Vec<String> {
        self.history.redo_names()
    }

    pub fn mark_clean(&mut self) {
        self.history.mark_clean();
    }
}
