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
