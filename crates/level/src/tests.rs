use bigstar_core::Fx32;
use bigstar_formats::*;

use super::*;

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn append_block(course: &mut Vec<u8>, index: usize, block: &[u8]) {
    let (offset, size) = (course.len() as u32, block.len() as u32);
    put_u32(course, index * 8, offset);
    put_u32(course, index * 8 + 4, size);
    course.extend_from_slice(block);
}

/// A course whose blocks are all empty except a zeroed header and one zeroed screen.
fn empty_course() -> Vec<u8> {
    let mut course = vec![0u8; NSMB_LEVEL_BLOCK_COUNT * 8];
    for i in 0..NSMB_LEVEL_BLOCK_COUNT {
        let size = course.len() as u32;
        put_u32(&mut course, i * 8, size);
    }
    append_block(&mut course, 0, &[0; 0x20]);
    append_block(&mut course, 2, &[0; 20]);
    course
}

fn object(id: u16, slot: u8, x: i16, y: i16, width: u16, height: u16) -> NsmbLevelObject {
    NsmbLevelObject { object_id: id, tileset_slot: slot, x, y, width, height }
}

fn sprite(id: u16, x: u16, y: u16, events: [u8; 2], settings: u32) -> NsmbLevelSprite {
    NsmbLevelSprite { id, x, y, events, settings }
}

#[test]
fn module_data_catalogs_parse_and_reject_bad_documents() {
    let level_json = r#"{
      "schema":"nsmbtool.leveldata/1",
      "levelData":[
        {"key":"coop.canFly","module":"coop","dataType":"flag","hash":"0xc8dfeb91"},
        {"key":"glue.stageObjects","module":"glue","dataType":"sizedArray","hash":"0x6db68f3c","typeName":"u32"}
      ]}"#;
    let levels = parse_level_data_catalog(level_json).unwrap();
    assert_eq!(levels.len(), 2);
    assert_eq!(levels[0].kind, LevelDataKind::Flag);
    assert_eq!(levels[1].type_name, "u32");
    assert!(parse_level_data_catalog(r#"{"schema":"nsmbtool.leveldata/2","levelData":[]}"#).is_err());
    assert!(
        parse_level_data_catalog(
            r#"{"schema":"nsmbtool.leveldata/1","levelData":[{"key":"x","module":"x","dataType":"array","hash":"0xfd0c5087","typeName":"u8","arraySize":0}]}"#
        )
        .is_err()
    );

    let objects_json = r#"{
      "schema":"nsmbtool.stageobjects/1","stageObjectIdBase":326,
      "stageObjects":[
        {"name":"coop.CoopFlagActor.Default","hash":"0xf46a44b9","module":"coop","component":"CustomWorldUnlock","object":"CoopFlagActor","variant":"Default","objectId":387,"info":"CoopFlagActor::ObjectInfo_Default","header":"coop_flag.hpp"},
        {"name":"coop.CoopFlagActor.Big","hash":"0x27e61112","module":"coop","component":"CustomWorldUnlock","object":"CoopFlagActor","variant":"Big","objectId":387,"info":"CoopFlagActor::ObjectInfo_Big","header":"coop_flag.hpp"}
      ]}"#;
    let (base, objects) = parse_stage_object_catalog(objects_json).unwrap();
    assert_eq!((base, objects.len()), (326, 2));
    assert_eq!(objects[0].object_id, objects[1].object_id);
    assert!(parse_stage_object_catalog(r#"{"schema":"nsmbtool.stageobjects/2","stageObjectIdBase":326,"stageObjects":[]}"#).is_err());
    assert!(parse_stage_object_catalog(r#"{"schema":"nsmbtool.stageobjects/1","stageObjectIdBase":325,"stageObjects":[]}"#).is_err());

    // Uppercase hex, a wrong hash and an unknown property are all refused.
    assert!(
        parse_level_data_catalog(
            r#"{"schema":"nsmbtool.leveldata/1","levelData":[{"key":"coop.canFly","module":"coop","dataType":"flag","hash":"0xC8DFEB91"}]}"#
        )
        .is_err()
    );
    assert!(
        parse_level_data_catalog(
            r#"{"schema":"nsmbtool.leveldata/1","levelData":[{"key":"coop.canFly","module":"coop","dataType":"flag","hash":"0x00000001"}]}"#
        )
        .is_err()
    );
    assert!(parse_level_data_catalog(r#"{"schema":"nsmbtool.leveldata/1","levelData":[],"extra":1}"#).is_err());
    assert!(parse_level_data_catalog("[]").is_err());
    assert!(parse_level_data_catalog("not json").is_err());
}

#[test]
fn objects_undo_redo_and_history_truncation() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    assert_eq!(level.add_object(object(12, 1, -2, 3, 4, 5)).unwrap(), 0);
    assert_eq!(level.objects()[0].x, -2);
    let mut moved = level.objects()[0].clone();
    moved.x = 24;
    level.update_object(0, moved).unwrap();
    assert_eq!(level.objects()[0].x, 24);
    level.undo().unwrap();
    assert_eq!(level.objects()[0].x, -2);
    level.undo().unwrap();
    assert!(level.objects().is_empty());
    level.redo().unwrap();
    assert_eq!(level.objects().len(), 1);
    let mut moved = level.objects()[0].clone();
    moved.x = 42;
    level.update_object(0, moved).unwrap();
    for _ in 0..32 {
        level.add_object(NsmbLevelObject::default()).unwrap();
    }
    for _ in 0..32 {
        level.undo().unwrap();
    }
    level.undo().unwrap();
    assert_eq!(level.objects()[0].x, -2);

    assert!(level.update_object(5, NsmbLevelObject::default()).is_err());
    assert!(level.remove_object(5).is_err());
}

#[test]
fn every_entity_kind_survives_a_save_and_load() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    level.add_object(object(12, 1, -2, 3, 4, 5)).unwrap();
    assert_eq!(level.add_sprite(sprite(3, 10, 20, [1, 2], 0x06050403)).unwrap(), 0);
    assert_eq!(level.course().sprites[0].id, 3);
    level.add_entrance(NsmbLevelEntrance::default()).unwrap();
    assert_eq!(level.course().entrances.len(), 1);
    level.add_zone(NsmbLevelZone { x: 16, y: 32, width: 256, height: 192, id: 2, ..Default::default() }).unwrap();
    assert_eq!(level.course().zones[0].id, 2);
    let view = NsmbLevelView {
        width: 256,
        height: 192,
        id: 1,
        camera_settings_id: 2,
        bgm_id: 3,
        background_id: 4,
        tileset_id: 5,
        foreground_id: 6,
        light_type: 7,
        progress_path_id: 8,
        ..Default::default()
    };
    level.add_view(view).unwrap();
    assert_eq!(level.course().views[0].background_id, 4);
    level.set_object_bank(15, 3).unwrap();
    assert!(level.set_object_bank(16, 3).is_err());

    // A compound edit is one undo step with its own name.
    let mut moved_object = level.objects()[0].clone();
    moved_object.x = 100;
    let mut moved_zone = level.course().zones[0].clone();
    moved_zone.x = 200;
    let updates = LevelEntityUpdates {
        objects: vec![IndexedLevelValue { index: 0, value: moved_object }],
        zones: vec![IndexedLevelValue { index: 0, value: moved_zone }],
        ..Default::default()
    };
    level.update_entities(updates, "Move selection").unwrap();
    assert_eq!((level.objects()[0].x, level.course().zones[0].x), (100, 200));
    assert_eq!(level.undo_name(), "Move selection");
    level.undo().unwrap();
    assert_eq!((level.objects()[0].x, level.course().zones[0].x), (-2, 16));
    level.redo().unwrap();
    level.undo().unwrap();

    let node = NsmbLevelPathNode { x: 32, y: 48, speed: Fx32::from_raw(1), acceleration: Fx32::from_raw(2), delay: 5, flags: 6 };
    level.add_path(false, NsmbLevelPath { id: 7, nodes: vec![node], unknown: 0 }).unwrap();
    assert_eq!(level.course().paths[0].nodes[0].flags, 6);
    level
        .set_tileset_references(LevelTilesetReferences {
            tileset_id: 9,
            background_graphics_id: 8,
            background_screen_id: 7,
            foreground_graphics_id: 6,
            foreground_screen_id: 5,
            background_palette_id: 4,
            foreground_palette_id: 3,
        })
        .unwrap();
    assert_eq!(level.course().header.tileset_theme[0], 9);
    assert_eq!(level.course().foregrounds[0].screen_id, 5);
    assert!(level.dirty());

    let course = level.serialize_course().unwrap();
    let bgdat = level.serialize_bgdat().unwrap();
    assert_eq!(bgdat.len(), level.objects().len() * NSMB_BGDAT_OBJECT_SIZE + 2);
    assert_eq!(&bgdat[bgdat.len() - 2..], &[0xFF, 0xFF]);
    let round_trip = EditableLevel::load(&course, &bgdat, None).unwrap();
    assert_eq!(round_trip.course().header.tileset_theme[0], 9);
    assert_eq!(round_trip.course().foregrounds[0].screen_id, 5);
    assert_eq!((round_trip.course().backgrounds[0].palette_id, round_trip.course().foregrounds[0].palette_id), (4, 3));
    assert_eq!(round_trip.course().sprites.len(), 1);
    assert_eq!(round_trip.course().sprites[0], sprite(3, 10, 20, [1, 2], 0x06050403));
    assert_eq!(round_trip.course().entrances.len(), 1);
    assert_eq!(round_trip.course().zones.len(), 1);
    assert_eq!((round_trip.course().views.len(), round_trip.course().views[0].progress_path_id), (1, 8));
    assert_eq!(round_trip.course().object_banks[15], 3);
    assert_eq!(round_trip.course().paths[0].nodes[0].flags, 6);
    assert_eq!(round_trip.objects()[0].x, -2);

    level.mark_clean();
    assert!(!level.dirty());
    level.undo().unwrap();
    assert!(level.dirty());
}

#[test]
fn tileset_references_undo_removes_the_screens_they_created() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    assert!(level.course().foregrounds.is_empty());
    level.set_tileset_references(LevelTilesetReferences { foreground_screen_id: 5, ..Default::default() }).unwrap();
    assert_eq!(level.course().foregrounds.len(), 1);
    level.undo().unwrap();
    assert!(level.course().foregrounds.is_empty());
    assert_eq!(level.tileset_references(), LevelTilesetReferences::default());
}

#[test]
fn custom_stage_objects_are_registered_in_glue() {
    let catalog = ModuleDataCatalog {
        stage_objects: vec![
            StageObjectDeclaration { name: "mod.Actor.First".into(), hash: 0x11111111, object_id: 400, ..Default::default() },
            StageObjectDeclaration { name: "mod.Actor.Second".into(), hash: 0x22222222, object_id: 401, ..Default::default() },
        ],
        ..Default::default()
    };
    let mut custom = EditableLevel::load(&empty_course(), &[], Some(&catalog)).unwrap();
    let unknown_glue =
        GlueBlock { flags: vec![0xabcdef01], values: vec![GlueValue { hash: 0xabcdef01, payload: vec![1, 2, 3, 4], payload_order: 0 }] };
    custom.set_glue(unknown_glue.clone(), "Edit level data").unwrap();
    custom.add_sprite(sprite(1, 0, 0, [0; 2], 0)).unwrap();
    custom.add_custom_sprite(0x22222222, sprite(0, 1, 1, [0; 2], 0)).unwrap();
    custom.add_custom_sprite(0x11111111, sprite(0, 2, 2, [0; 2], 0)).unwrap();
    custom.add_custom_sprite(0x22222222, sprite(0, 3, 3, [0; 2], 0)).unwrap();
    assert_eq!(custom.registration_hashes(), [0x22222222, 0x11111111]);
    let vanilla_objects = [100, 101];
    assert_eq!(custom.resolve_sprite_object_id(0, &vanilla_objects, &catalog), Some(101));
    assert_eq!(custom.resolve_sprite_object_id(1, &vanilla_objects, &catalog), Some(401));
    assert_eq!(custom.resolve_sprite_object_id(9, &vanilla_objects, &catalog), None);

    let saved = custom.serialize_course().unwrap();
    let mut loaded = EditableLevel::load(&saved, &[], Some(&catalog)).unwrap();
    assert_eq!(loaded.glue().flags, unknown_glue.flags);
    assert_eq!(loaded.glue().values[0].hash, 0xabcdef01);
    assert_eq!(loaded.glue().values[0].payload, unknown_glue.values[0].payload);
    assert_eq!(loaded.sprite_hash(1), Some(0x22222222));
    assert_eq!(loaded.sprite_hash(2), Some(0x11111111));
    loaded.remove_sprite(3).unwrap();
    loaded.remove_sprite(1).unwrap();
    assert_eq!(loaded.registration_hashes(), [0x11111111]);
    assert_eq!(loaded.course().sprites[1].id, 326);

    // Undo puts the registration back, ids included.
    loaded.undo().unwrap();
    assert_eq!(loaded.registration_hashes(), [0x22222222, 0x11111111]);
    assert_eq!(loaded.course().sprites[1].id, 326);
    assert_eq!(loaded.course().sprites[2].id, 327);

    assert!(custom.add_custom_sprite(0, sprite(0, 0, 0, [0; 2], 0)).is_err());
    let mut registration_edit = loaded.glue().clone();
    registration_edit.values.retain(|value| value.hash != fnv1a_32("glue.stageObjects"));
    assert!(loaded.set_glue(registration_edit, "Edit level data").is_err());
}

#[test]
fn unreadable_glue_is_kept_byte_for_byte() {
    let mut course = vec![0u8; NSMB_LEVEL_BLOCK_COUNT * 8];
    for i in 0..NSMB_LEVEL_BLOCK_COUNT {
        let size = course.len() as u32;
        put_u32(&mut course, i * 8, size);
    }
    let mut header = vec![0u8; 0x20 + 12];
    put_u32(&mut header, 0x20, 0);
    put_u32(&mut header, 0x24, 1);
    put_u32(&mut header, 0x28, 12);
    append_block(&mut course, 0, &header);
    append_block(&mut course, 2, &[0; 20]);
    let mut malformed = EditableLevel::load(&course, &[], None).unwrap();
    assert!(malformed.glue_error().is_some());
    assert!(malformed.set_glue(GlueBlock::default(), "Edit level data").is_err());
    assert!(malformed.add_custom_sprite(1, sprite(0, 0, 0, [0; 2], 0)).is_err());
    let saved = malformed.serialize_course().unwrap();
    let offset = u32::from_le_bytes([saved[0], saved[1], saved[2], saved[3]]) as usize;
    assert_eq!(&saved[offset..offset + header.len()], header.as_slice());
}

#[test]
fn bulk_add_remove_and_reorder_are_single_undo_steps() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    let added = level
        .add_entities(
            LevelEntityInsertions {
                objects: vec![object(1, 0, 0, 0, 1, 1), object(2, 0, 1, 1, 1, 1)],
                sprites: vec![
                    SpriteInsertion { value: sprite(5, 0, 0, [0; 2], 0), hash: None },
                    SpriteInsertion { value: sprite(6, 1, 1, [0; 2], 0), hash: None },
                ],
                zones: vec![NsmbLevelZone::default()],
                ..Default::default()
            },
            "Paste selection",
        )
        .unwrap();
    assert_eq!((added.objects.clone(), added.sprites.clone(), added.zones.clone()), (vec![0, 1], vec![0, 1], vec![0]));
    assert_eq!(level.undo_name(), "Paste selection");
    assert_eq!(level.undo_names().len(), 1);

    assert_eq!(level.reorder_entity(ReorderKind::Sprite, 0, true).unwrap(), 1);
    assert_eq!((level.course().sprites[0].id, level.course().sprites[1].id), (6, 5));
    assert_eq!(level.reorder_entity(ReorderKind::Sprite, 0, false).unwrap(), 0);
    assert_eq!(level.undo_name(), "Bring forward");
    assert!(level.reorder_entity(ReorderKind::Object, 9, true).is_err());
    level.undo().unwrap();
    assert_eq!((level.course().sprites[0].id, level.course().sprites[1].id), (5, 6));

    let selection = LevelEntitySelection { objects: vec![0, 1, 1], sprites: vec![1], ..Default::default() };
    level.remove_entities(selection, "Delete selection").unwrap();
    assert!(level.objects().is_empty());
    assert_eq!(level.course().sprites.len(), 1);
    level.undo().unwrap();
    assert_eq!(level.objects().len(), 2);
    assert_eq!(level.course().sprites.len(), 2);

    // A failing member leaves everything as it was.
    let bad = LevelEntitySelection { objects: vec![0], zones: vec![9], ..Default::default() };
    assert!(level.remove_entities(bad, "Delete selection").is_err());
    assert_eq!(level.objects().len(), 2);
}

#[test]
fn replace_serialized_is_one_undoable_step() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    level.add_object(object(1, 0, 0, 0, 1, 1)).unwrap();
    let mut other = EditableLevel::load(&empty_course(), &[], None).unwrap();
    other.add_zone(NsmbLevelZone { id: 4, ..Default::default() }).unwrap();
    let course = other.serialize_course().unwrap();
    level.replace_serialized(&course, &[], None, "Import level").unwrap();
    assert!(level.objects().is_empty());
    assert_eq!(level.course().zones[0].id, 4);
    assert_eq!(level.undo_name(), "Import level");
    level.undo().unwrap();
    assert_eq!(level.objects().len(), 1);
    assert!(level.course().zones.is_empty());
    assert!(level.replace_serialized(&[1, 2, 3], &[], None, "Import level").is_err());
}

#[test]
fn bgdat_limits_are_enforced_on_save() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    level.add_object(object(0x1000, 0, 0, 0, 1, 1)).unwrap();
    assert!(level.serialize_bgdat().is_err());
    level.undo().unwrap();
    level.add_object(object(1, 16, 0, 0, 1, 1)).unwrap();
    assert!(level.serialize_bgdat().is_err());
}

#[test]
fn sprite_fields_read_and_write_their_bits() {
    let field = SpriteFieldDefinition { key: "mode".into(), label: "Mode".into(), bit_offset: 4, bit_width: 3, ..SpriteFieldDefinition::default() };
    assert_eq!(read_sprite_field(0x70, &field), 7);
    assert_eq!(write_sprite_field(0xffffffff, &field, 2), 0xffffffaf);

    let event =
        SpriteFieldDefinition { key: "event".into(), label: "Event".into(), bit_offset: 40, bit_width: 8, ..SpriteFieldDefinition::default() };
    let sprite_data: u64 = 0x1234_89ab_cdef;
    assert_eq!(read_sprite_field(sprite_data, &event), 0x12);
    assert_eq!(write_sprite_field(sprite_data, &event, 0x56), 0x5634_89ab_cdef);

    let signed_field = SpriteFieldDefinition { bit_width: 4, signed_value: true, value_offset: 2, ..SpriteFieldDefinition::default() };
    assert_eq!(sprite_field_min(&signed_field), -6);
    assert_eq!(sprite_field_max(&signed_field), 9);
    assert_eq!(read_sprite_field(0xf, &signed_field), 1);
    assert_eq!(write_sprite_field(0, &signed_field, 1), 0xf);

    let checkbox = SpriteFieldDefinition { bit_width: 4, checkbox_value: 8, ..SpriteFieldDefinition::default() };
    assert_eq!(sprite_field_max(&checkbox), 1);
    assert_eq!(read_sprite_field(8, &checkbox), 1);
    assert_eq!(write_sprite_field(0, &checkbox, 1), 8);

    // Widths the C++ shifts leave undefined stay defined here.
    let whole = SpriteFieldDefinition { bit_width: 64, ..SpriteFieldDefinition::default() };
    assert_eq!(read_sprite_field(u64::MAX, &whole), -1);
    assert_eq!(write_sprite_field(0, &whole, 5), 5);

    let vanilla = SpriteDefinition { object_id: 83, ..SpriteDefinition::default() };
    let custom = SpriteDefinition { hash: Some(0x12345678), ..SpriteDefinition::default() };
    let catalog = SpriteDefinitionCatalog { definitions: vec![vanilla, custom] };
    assert!(catalog.by_object_id(83).is_some());
    assert!(catalog.by_hash(0x12345678).is_some());
    assert_eq!(SpriteDefinition::default().category, "Other");
}

mod tilemap {
    use bigstar_formats::{NsmbLevelObject, OBJECT_LAYOUT_EMPTY_TILE, ObjectLayout, ObjectLayoutTile};

    use crate::*;

    /// One object whose every cell holds `tile`.
    fn flat(tile: i32) -> ObjectLayout {
        ObjectLayout { rows: vec![vec![ObjectLayoutTile::new(tile, 0)]], ..ObjectLayout::default() }
    }

    fn object(object_id: u16, tileset_slot: u8, x: i16, y: i16, width: u16, height: u16) -> NsmbLevelObject {
        NsmbLevelObject { object_id, tileset_slot, x, y, width, height }
    }

    fn tilesets() -> (DecodedTileset, DecodedTileset) {
        let mut jyotyu = DecodedTileset::default();
        jyotyu.objects.push(flat(40)); // 0: a plain block
        jyotyu.objects.push(flat(OBJECT_LAYOUT_EMPTY_TILE)); // 1: the empty block
        jyotyu.objects.push(flat(3)); // 2: a block the game skips
        jyotyu.objects.push(ObjectLayout { slope: true, rows: vec![vec![ObjectLayoutTile::new(50, 0)]], ..ObjectLayout::default() }); // 3: a slope
        jyotyu.objects.push(flat(41)); // 4: another plain block
        let level_tileset = DecodedTileset { objects: vec![flat(3)], ..DecodedTileset::default() }; // block 259 once based
        (jyotyu, level_tileset)
    }

    #[test]
    fn later_objects_own_the_cells_they_write() {
        let (jyotyu, level_tileset) = tilesets();
        let sets = [Some(&jyotyu), Some(&level_tileset), None];
        let raster = |objects: &[NsmbLevelObject]| rasterize_level_objects(objects, &sets);

        // The object in front wins the cells it covers outright.
        let tilemap = raster(&[object(0, 0, 0, 0, 2, 2), object(4, 0, 1, 1, 2, 2)]);
        assert_eq!(tilemap.cell_at(0, 0).block, 40, "the object behind keeps the cells nothing covers");
        assert!(tilemap.cell_at(1, 1).block == 41 && tilemap.cell_at(1, 1).slot == 0, "the object in front owns the cell it shares");
        assert_eq!(tilemap.cell_at(2, 2).block, 41);

        // An empty block is a block: the eraser leaves the cell holding nothing at all.
        let tilemap = raster(&[object(0, 0, 0, 0, 2, 2), object(1, 0, 1, 1, 2, 2)]);
        assert!(tilemap.cell_at(0, 0).draws());
        assert!(tilemap.cell_at(1, 1).written() && !tilemap.cell_at(1, 1).draws());

        // Blocks 1-6 never reach the array, so a cell naming one keeps what an earlier object put there.
        let tilemap = raster(&[object(0, 0, 0, 0, 2, 2), object(2, 0, 1, 1, 2, 2)]);
        assert_eq!(tilemap.cell_at(1, 1).block, 40);
        assert!(!tilemap.cell_at(2, 2).written());

        // The same block number in the level tileset's slot is 259, which the skip list does not reach.
        let tilemap = raster(&[object(0, 0, 0, 0, 2, 2), object(0, 1, 1, 1, 2, 2)]);
        assert!(tilemap.cell_at(1, 1).block == 3 && tilemap.cell_at(1, 1).slot == 1);

        // A slope fills one diagonal; the cells it never reaches are not its to clear.
        let tilemap = raster(&[object(0, 0, 0, 0, 3, 3), object(3, 0, 0, 0, 3, 3)]);
        assert_eq!(tilemap.cell_at(0, 2).block, 50);
        assert_eq!(tilemap.cell_at(2, 2).block, 40);

        // Cells left of the origin belong to the chunk left of it, and an object far out asks for its own cells only.
        let tilemap = raster(&[object(0, 0, -2, -2, 2, 2), object(0, 0, 300, 300, 1, 1)]);
        assert_eq!(tilemap.cell_at(-1, -1).block, 40);
        assert_eq!(tilemap.chunks.len(), 2);

        // Nothing to draw with is not a crash.
        let tilemap = raster(&[object(0, 2, 0, 0, 2, 2), object(99, 0, 4, 4, 2, 2)]);
        assert!(tilemap.chunks.is_empty());
    }

    #[test]
    fn object_effects_name_the_two_blanks() {
        let (jyotyu, level_tileset) = tilesets();
        assert_eq!(level_object_effect(&jyotyu.objects[0], 0), LevelObjectEffect::Draws);
        assert_eq!(level_object_effect(&jyotyu.objects[1], 0), LevelObjectEffect::Erases);
        assert_eq!(level_object_effect(&jyotyu.objects[2], 0), LevelObjectEffect::Inert);
        assert_eq!(level_object_effect(&level_tileset.objects[0], 1), LevelObjectEffect::Draws);
        let mixed = ObjectLayout {
            rows: vec![vec![ObjectLayoutTile::new(OBJECT_LAYOUT_EMPTY_TILE, 0), ObjectLayoutTile::new(40, 0)]],
            ..ObjectLayout::default()
        };
        assert_eq!(level_object_effect(&mixed, 0), LevelObjectEffect::Draws);
    }

    #[test]
    fn tileset_objects_render_from_map16_images() {
        let red = bigstar_formats::RgbaImage { width: 16, height: 16, pixels: [255, 0, 0, 255].repeat(256) };
        let tileset = DecodedTileset { map16_tiles: vec![red], objects: vec![flat(0), flat(OBJECT_LAYOUT_EMPTY_TILE)], behaviors: Vec::new() };
        let image = render_tileset_object(&tileset, 0, 2, 1).unwrap();
        assert_eq!((image.width, image.height), (32, 16));
        assert_eq!(&image.pixels[..4], &[255, 0, 0, 255]);
        assert_eq!(&image.pixels[31 * 4..32 * 4], &[255, 0, 0, 255]);
        assert_eq!(render_tileset_object(&tileset, 1, 1, 1).unwrap().pixels[3], 0);
        assert!(render_tileset_object(&tileset, 9, 1, 1).is_err());
    }
}

#[test]
fn exchange_files_round_trip() {
    let level = LevelPackage { course_file_id: 12, bgdat_file_id: 13, course: vec![1, 2, 3, 4], bgdat: vec![5, 6] };
    let mut nml = encode_nml(&level).unwrap();
    let decoded = decode_nml(&nml).unwrap();
    assert_eq!(decoded.course, level.course);
    assert_eq!(decoded.bgdat_file_id, 13);
    nml.push(0);
    assert!(decode_nml(&nml).is_err(), "NML trailing bytes rejected");
    let mut old = encode_nml(&level).unwrap();
    old[6] = b'4';
    assert_eq!(decode_nml(&old).unwrap(), level, "an NSMBe4 export still opens");

    let entries = vec![
        NsmbPatchEntry { path: "root/course/A01_1.bin".into(), expected_file_id: 42, contents: vec![1, 1, 1, 2, 3, 5, 8] },
        NsmbPatchEntry { path: "root/course/A01_1_bgdat.bin".into(), expected_file_id: -1, contents: vec![9, 8, 7] },
    ];
    let decoded = decode_nsmbpatch(&encode_nsmbpatch(&entries).unwrap()).unwrap();
    assert_eq!(decoded, entries);
    assert!(encode_nsmbpatch(&[NsmbPatchEntry { path: "../escape".into(), expected_file_id: -1, contents: vec![1] }]).is_err());
    assert!(encode_nsmbpatch(&[NsmbPatchEntry { path: "/abs".into(), ..NsmbPatchEntry::default() }]).is_err());
    assert!(encode_nsmbpatch(&[entries[0].clone(), entries[0].clone()]).is_err(), "duplicate patch path rejected");

    let recovery = LevelRecoverySnapshot {
        course_path: "extracted/root/course/A.bin".into(),
        bgdat_path: "extracted/root/course/A_bgdat.bin".into(),
        package: level.clone(),
    };
    let recovered = decode_level_recovery(&encode_level_recovery(&recovery).unwrap()).unwrap();
    assert_eq!(recovered, recovery);
    assert!(encode_level_recovery(&LevelRecoverySnapshot { course_path: "a/./b".into(), ..recovery }).is_err());
}

#[test]
fn a_view_places_a_layer_by_the_block_it_names() {
    use crate::{LayerPlacement, LayerRepeat, layer_block, layer_placement};
    let block = |id, screen_id, scroll_y, vertical_offset| NsmbLevelScreen { id, screen_id, scroll_y, vertical_offset, ..Default::default() };
    // A view 384 tall whose floor is at 400.
    let (view_y, view_height) = (16, 384);

    // Mode 0 hangs the image from the view's floor, 32 rows of it below; each step of the offset lifts it eight more.
    let blocks = [block(0, 5, 0x02, 0), block(1, 5, 0x02, 2), block(2, 5, 0x02, (-2i16) as u16)];
    let at = |id| layer_placement(&blocks, id, view_y, view_height).unwrap();
    assert_eq!(at(0), LayerPlacement { screen_id: 5, origin_y: 400 - 512 + 32, repeat: LayerRepeat::Wrap });
    assert_eq!((at(1).origin_y, at(2).origin_y), (400 - 512 + 32 - 16, 400 - 512 + 32 + 16));
    // No block has id 9, so the game falls back to the first.
    assert_eq!(layer_block(&blocks, 9).unwrap().id, 0);
    assert_eq!(at(9), at(0));

    // The odd modes hang it from the ceiling instead, and the last two repeat half of it past its end.
    let blocks = [block(0, 5, 0x13, 0), block(1, 5, 0x42, 0), block(2, 5, 0x42, 4), block(3, 5, 0x53, 0)];
    let at = |id| layer_placement(&blocks, id, view_y, view_height).unwrap();
    assert_eq!((at(0).origin_y, at(0).repeat), (16 - 32, LayerRepeat::Wrap));
    assert_eq!((at(1).origin_y, at(1).repeat), (400 - 512 + 32, LayerRepeat::TopHalfAbove));
    assert_eq!(at(2).origin_y, 400 - 512, "mode 4 stops at the image's last row");
    assert_eq!((at(3).origin_y, at(3).repeat), (16 - 32, LayerRepeat::BottomHalfBelow));

    // A view whose block names no screen has no such layer: the ghost houses drop the foreground outside this way.
    let blocks = [block(0, 40, 0x01, 0), block(1, 0xffff, 0xffff, 0)];
    assert!(layer_placement(&blocks, 0, view_y, view_height).is_some());
    assert!(layer_placement(&blocks, 1, view_y, view_height).is_none());
    // A level with no blocks at all is placed as a block of zeros would place it.
    assert_eq!(layer_placement(&[], 0, view_y, view_height).unwrap().origin_y, 400 - 512 + 32);
}

#[test]
fn bands_of_a_layer_move_as_the_scanline_table_moves_them() {
    use crate::{BackgroundCatalog, BackgroundDefinition, BackgroundLayer, BackgroundMotion, BackgroundStrip, strip_offsets};
    let drift = |through_row, speed, channel| BackgroundStrip { through_row, motion: BackgroundMotion::Drift { speed, channel } };
    let scroll = BackgroundStrip { through_row: u16::MAX, motion: BackgroundMotion::Scroll { factor: 4096 } };

    // Nothing to say: the whole image, still.
    assert_eq!(strip_offsets(&[], 512, 0, 100, 2), [(0, 512, 0.0)]);
    // Clouds an eighth of a pixel a frame over hills that only scroll.
    let strips = [drift(344, 512, 2), scroll];
    assert_eq!(strip_offsets(&strips, 512, 0, 0, 2), [(0, 345, 0.0), (345, 512, 0.0)]);
    assert_eq!(strip_offsets(&strips, 512, 0, 80, 2), [(0, 345, 10.0), (345, 512, 0.0)]);
    // Round the image and back where it started.
    assert_eq!(strip_offsets(&strips, 512, 0, 4096, 2), [(0, 345, 0.0), (345, 512, 0.0)]);
    // Two bands on one channel both add to its counter, so both move at the sum.
    let strips = [drift(279, 256, 2), drift(335, 512, 3), drift(364, 1024, 3), scroll];
    let moved: Vec<f32> = strip_offsets(&strips, 512, 0, 16, 2).iter().map(|band| band.2).collect();
    assert_eq!(moved, [1.0, 6.0, 6.0, 0.0]);
    // A negative speed runs the other way, which comes back as the long way round.
    assert_eq!(strip_offsets(&[drift(u16::MAX, -2048, 2)], 512, 0, 2, 2), [(0, 512, 511.0)]);

    // A ripple comes back a few rows at a time, each a little further round.
    let wave = BackgroundStrip { through_row: u16::MAX, motion: BackgroundMotion::Wave { amplitude: 8192, speed: 0, step: 16384 } };
    let bands = strip_offsets(&[wave], 8, 0, 0, 1);
    assert_eq!(bands.len(), 8);
    let moved: Vec<i32> = bands.iter().map(|band| band.2.round() as i32).collect();
    assert_eq!(moved, [0, 2, 0, -2, 0, 2, 0, -2]);

    // Scrolled, a band that only scrolls goes at its own share of the layer's speed and the rest ride on top of it.
    let strips = [drift(344, 512, 2), BackgroundStrip { through_row: u16::MAX, motion: BackgroundMotion::Scroll { factor: 2048 } }];
    assert_eq!(strip_offsets(&strips, 512, 100 * 4096, 80, 2), [(0, 345, 110.0), (345, 512, 50.0)]);
    assert_eq!(strip_offsets(&[], 512, 4096 * 7 + 4095, 0, 2), [(0, 512, 7.0)]);

    // The first definition of a layer and id wins, and an id is found by its low byte, as the game indexes.
    let named = |layer, id, name: &str| BackgroundDefinition { layer, id, name: name.to_owned(), ..Default::default() };
    let catalog = BackgroundCatalog {
        definitions: vec![
            named(BackgroundLayer::Background, 3, "Mine"),
            named(BackgroundLayer::Foreground, 3, "Front"),
            named(BackgroundLayer::Background, 3, "Shipped"),
            named(BackgroundLayer::Background, 1, "Castle"),
        ],
    };
    assert_eq!(catalog.name(BackgroundLayer::Background, 3), Some("Mine"));
    assert_eq!(catalog.name(BackgroundLayer::Background, 0x403), Some("Mine"));
    assert_eq!(catalog.name(BackgroundLayer::Foreground, 3), Some("Front"));
    assert_eq!(catalog.name(BackgroundLayer::Foreground, 1), None);
    assert_eq!(catalog.layer(BackgroundLayer::Background).iter().map(|d| d.id).collect::<Vec<_>>(), [1, 3, 3]);
    assert!(!catalog.definitions[0].animated());
}

#[test]
fn an_entrance_places_the_camera_as_the_game_does() {
    use crate::{ViewFrame, camera_block, entrance_camera, entrance_spawn_point};
    use bigstar_formats::{NsmbLevelCamera, NsmbLevelEntrance};
    let view = ViewFrame { left: 0, top: 0, right: 2000, bottom: 400 };
    let entrance = |x, y, kind, flags| NsmbLevelEntrance { x, y, kind, flags, ..Default::default() };

    // A normal entrance stands the player half a tile right and a tile down; aligned, only down. A pipe facing up
    // puts them a tile right and two down.
    assert_eq!(entrance_spawn_point(&entrance(320, 304, 0, 0)), (328, 320));
    assert_eq!(entrance_spawn_point(&entrance(320, 304, 0, 0x40)), (320, 320));
    assert_eq!(entrance_spawn_point(&entrance(320, 304, 3, 0)), (336, 336));
    assert_eq!(entrance_spawn_point(&entrance(320, 304, 200, 0)), (320, 304));

    // Centred sideways; 128 pixels of screen above the player, until the view's floor stops it.
    assert_eq!(entrance_camera(&entrance(320, 304, 0, 0), view, None, false, None), (200, 192));
    assert_eq!(entrance_camera(&entrance(320, 404, 0, 0), ViewFrame { bottom: 800, ..view }, None, false, None), (200, 292));
    // Kept inside the view at either end, unless the level wraps around and is wide enough to.
    assert_eq!(entrance_camera(&entrance(32, 304, 0, 0), view, None, false, None).0, 0);
    assert_eq!(entrance_camera(&entrance(1990, 304, 0, 0), view, None, false, None).0, 2000 - 256);
    assert_eq!(entrance_camera(&entrance(32, 304, 0, 0), view, None, true, None).0, 0);
    assert_eq!(entrance_camera(&entrance(32, 304, 0, 0), ViewFrame { right: 257 * 16, ..view }, None, true, None).0, 40 - 128);
    // Near the top of the map the player is above the upper line, and sits 64 pixels down the screen instead.
    let tall = ViewFrame { left: 0, top: 0, right: 2000, bottom: 2000 };
    assert_eq!(entrance_camera(&entrance(320, 0, 0, 0), tall, None, false, None).1, 0);
    // The camera settings move both lines, and a camera that already frames the player stays where it was.
    let settings = NsmbLevelCamera { top_offset: 16, bottom_offset: -32, id: 3, ..Default::default() };
    assert_eq!(entrance_camera(&entrance(320, 984, 0, 0), tall, Some(&settings), false, None).1, 1000 - 96);
    assert_eq!(entrance_camera(&entrance(320, 984, 0, 0), tall, None, false, Some(900)).1, 900);
    assert_eq!(entrance_camera(&entrance(320, 984, 0, 0), tall, None, false, Some(1500)).1, 1000 - 64);
    // A view shorter than the screen rests on its floor.
    assert_eq!(entrance_camera(&entrance(320, 50, 0, 0), ViewFrame { bottom: 100, ..view }, None, false, None).1, 100 - 192);

    let cameras = [NsmbLevelCamera { id: 1, ..Default::default() }, settings.clone()];
    assert_eq!(camera_block(&cameras, 3), Some(&settings));
    assert_eq!(camera_block(&cameras, 9).map(|camera| camera.id), Some(1));
    assert!(camera_block(&[], 0).is_none());
}

#[test]
fn a_layer_scrolls_with_the_camera_as_the_game_scrolls_it() {
    use crate::{BackgroundLayer, LayerSampling, ViewFrame, layer_placement, layer_scroll};
    let view = ViewFrame { left: 100, top: 16, right: 2100, bottom: 800 };
    let block =
        |screen_id, scroll_x, scroll_y, vertical_offset| NsmbLevelScreen { screen_id, scroll_x, scroll_y, vertical_offset, ..Default::default() };
    let at = |layer, block: &NsmbLevelScreen, camera| layer_scroll(layer, std::slice::from_ref(block), 0, view, camera, true).unwrap();
    let back = BackgroundLayer::Background;

    // Half speed both ways, anchored to the floor: resting on it, the image's last row is the screen's.
    let half = block(5, 1, 1, 0);
    let rest = at(back, &half, (100, 800 - 192));
    assert_eq!((rest.x, rest.y, rest.sampling), (0, 320 - 32, LayerSampling::Whole));
    // That is where the still picture puts it.
    let still = layer_placement(std::slice::from_ref(&half), 0, view.top, view.bottom - view.top).unwrap();
    assert_eq!(800 - 192 - rest.y, still.origin_y);
    // A hundred pixels along and up, the layer has gone fifty.
    let moved = at(back, &half, (200, 800 - 192 - 100));
    assert_eq!((moved.x, moved.y), (50 * 4096, 320 - 32 - 50));
    // Never before the view's start nor past its width at the layer's speed.
    assert_eq!(at(back, &half, (0, 608)).x, 0);
    assert_eq!(at(back, &half, (9000, 608)).x, 1000 * 4096);
    assert_eq!(layer_scroll(back, std::slice::from_ref(&half), 0, view, (0, 608), false).unwrap().x, -50 * 4096);

    // Anchored to the ceiling, the image's first row starts 32 above it, and an offset lowers the scroll further.
    let top = block(5, 0, 0x10, 2);
    assert_eq!(at(back, &top, (100, 16)).y, 32 + 16);
    assert_eq!(at(back, &top, (100, 116)).y, 100 + 32 + 16);

    // Mode 4 repeats the top half once the screen climbs past the image, and never sinks below its last row.
    let sky = block(5, 0, 0x40, 8);
    assert_eq!((at(back, &sky, (100, 608)).y, at(back, &sky, (100, 608)).sampling), (320, LayerSampling::Whole));
    let high = at(back, &sky, (100, 208));
    assert_eq!((high.y, high.sampling), (320 + 32 - 400, LayerSampling::TopHalf));
    assert_eq!((high.quarter(0, -1), high.quarter(1, 1)), ((0, 0), (1, 0)));

    // Mode 5 repeats the bottom half below: by copying it for the foreground, by stacking its quarters for the
    // background.
    let rock = block(5, 0, 0x50, 0);
    let deep = at(back, &rock, (100, 516));
    assert_eq!((deep.y, deep.sampling), (532, LayerSampling::BottomQuarters));
    assert_eq!((deep.quarter(1, 2), deep.quarter(0, 3)), ((0, 1), (1, 1)));
    assert_eq!(at(BackgroundLayer::Foreground, &rock, (100, 516)).sampling, LayerSampling::BottomHalf);
    assert_eq!(at(back, &rock, (100, 216)).sampling, LayerSampling::Whole);

    // A script's swap takes hold once the layer has scrolled past it, as foreground 8 loses Peach's castle.
    let swaps = [crate::BackgroundSwap { after: 256 * 4096, quarter: (0, 1), shows: (1, 1) }];
    let castle = block(8, 0, 0, 0);
    let front = BackgroundLayer::Foreground;
    assert!(at(front, &castle, (356, 608)).with_swaps(&swaps).swapped.is_empty());
    let past = at(front, &castle, (357, 608)).with_swaps(&swaps);
    assert!(past.quarter(0, 1) == (1, 1) && past.quarter(2, 1) == (1, 1) && past.quarter(0, 0) == (0, 0) && past.quarter(1, 1) == (1, 1));
    assert_eq!(at(front, &castle, (357, 608)).quarter(0, 1), (0, 1));
    // The still picture shows the castle once: from the view's left edge to the last column a screen can show before
    // the swap, and in no repeat of the image after it or before the view.
    let still = |distance: i32| crate::still_quarter(&swaps, (0, 1), distance);
    assert_eq!((still(0), still(511), still(512), still(1024), still(-512)), ((0, 1), (0, 1), (1, 1), (1, 1), (1, 1)));
    assert_eq!(crate::still_quarter(&swaps, (0, 0), 1024), (0, 0));

    // A block with no screen shows nothing.
    assert!(layer_scroll(back, &[block(u16::MAX, 0, 0, 0)], 0, view, (100, 608), true).is_none());
}

#[test]
fn camera_settings_are_edited_saved_and_undone() {
    let mut level = EditableLevel::load(&empty_course(), &[], None).unwrap();
    let cameras = vec![
        NsmbLevelCamera {
            top_offset: -16,
            bottom_offset: 32,
            top_offset_special: 1,
            bottom_offset_special: -2,
            id: 3,
            scroll_height: 15,
            unknown: 7,
        },
        NsmbLevelCamera { id: 4, ..Default::default() },
    ];
    level.set_cameras(cameras.clone(), "Edit camera settings").unwrap();
    assert_eq!(level.course().cameras, cameras);
    let course = level.serialize_course().unwrap();
    let again = EditableLevel::load(&course, &[], None).unwrap();
    assert_eq!(again.course().cameras, cameras);
    // Untouched, a level's camera block is written back byte for byte.
    assert_eq!(again.serialize_course().unwrap(), course);
    level.undo().unwrap();
    assert!(level.course().cameras.is_empty());
    assert!(level.set_cameras(vec![NsmbLevelCamera::default(); 256], "x").is_err());
}
