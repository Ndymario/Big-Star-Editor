use super::*;

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn words(values: &[u32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

#[test]
fn glue_blocks_round_trip_and_reject_malformed_input() {
    const _: () = assert!(fnv1a_32("coop.canFly") == 0xc8dfeb91);
    const _: () = assert!(fnv1a_32("coop.CoopFlagActor.Default") == 0xf46a44b9);

    let ordered = words(&[0, 0x11111111, 32, 0x22222222, 28, 0, 0, 0xbbbbbbbb, 0xaaaaaaaa]);
    let parsed = parse_glue_block(&ordered).unwrap();
    assert_eq!(parsed.values.len(), 2);
    assert_eq!(parsed.values[0].payload[0], 0xaa);
    assert_eq!(parsed.values[1].payload[0], 0xbb);
    assert_eq!(write_glue_block(&parsed).unwrap(), ordered);

    assert!(parse_glue_block(&[0; 13]).is_err());
    assert!(parse_glue_block(&[0; 8]).is_err());
    assert!(parse_glue_block(&words(&[0xffffffff, 0, 0])).is_err());
    assert!(parse_glue_block(&words(&[0, 1, 12])).is_err());
    assert!(parse_glue_block(&words(&[0, 1, 16, 0, 0, 0])).is_err());
    assert!(parse_glue_block(&words(&[0, 1, 28, 1, 32, 0, 0, 0, 0])).is_err());
    assert!(parse_glue_block(&words(&[0, 1, 28, 2, 28, 0, 0, 0])).is_err());

    let mut invalid = GlueBlock { flags: vec![0], values: Vec::new() };
    assert!(write_glue_block(&invalid).is_err());
    invalid.flags.clear();
    invalid.values = vec![GlueValue { hash: 0, payload: vec![1], payload_order: 0 }];
    assert!(write_glue_block(&invalid).is_err());
    invalid.values = vec![GlueValue { hash: 1, payload: Vec::new(), payload_order: 0 }];
    assert!(write_glue_block(&invalid).is_err());

    let registration = GlueBlock {
        flags: Vec::new(),
        values: vec![GlueValue {
            hash: fnv1a_32("glue.stageObjects"),
            payload: words(&[2, fnv1a_32("coop.CoopFlagActor.Default"), fnv1a_32("coop.CoopFlagActor.Big")]),
            payload_order: 0,
        }],
    };
    let worked = words(&[0, 0x6db68f3c, 0x14, 0, 0, 2, 0xf46a44b9, 0x27e61112]);
    assert_eq!(write_glue_block(&registration).unwrap(), worked);
}

#[test]
fn nitro_lz_round_trips_and_matches_the_reference_bytes() {
    let literal_lz10 = [0x10, 3, 0, 0, 0, b'a', b'b', b'c'];
    assert_eq!(decompress_nitro_lz(&literal_lz10).unwrap(), b"abc");
    assert!(decompress_nitro_lz(&literal_lz10[..6]).is_err());
    let literal_lz11 = [0x11, 3, 0, 0, 0, b'x', b'y', b'z'];
    assert_eq!(decompress_nitro_lz(&literal_lz11).unwrap(), b"xyz");

    let repeated = b"abcabc";
    let packed_lz10 = compress_nitro_lz10(repeated).unwrap();
    assert_eq!(packed_lz10, [0x10, 6, 0, 0, 0x10, b'a', b'b', b'c', 0, 2]);
    assert_eq!(decompress_nitro_lz(&packed_lz10).unwrap(), repeated);
    let packed_lz11 = compress_nitro_lz11(repeated).unwrap();
    assert_eq!(packed_lz11, [0x11, 6, 0, 0, 0x10, b'a', b'b', b'c', 0x20, 2]);
    assert_eq!(decompress_nitro_lz(&packed_lz11).unwrap(), repeated);

    let long_repeated: Vec<u8> = (0..70000).map(|i| (i % 7) as u8).collect();
    let packed_long = compress_nitro_lz11(&long_repeated).unwrap();
    assert_eq!(decompress_nitro_lz(&packed_long).unwrap(), long_repeated);
    assert!(compress_nitro_lz10(&vec![0; 0x1000000]).is_err());

    let empty = compress_nitro_lz11(&[]).unwrap();
    assert_eq!(empty, [0x11, 0, 0, 0, 0, 0, 0, 0]);
    assert!(decompress_nitro_lz(&empty).unwrap().is_empty());
}

#[test]
fn yaz0_round_trips() {
    let long_repeated: Vec<u8> = (0..70000).map(|i| (i % 7) as u8).collect();
    let literal = [b'Y', b'a', b'z', b'0', 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0xE0, b'x', b'y', b'z'];
    assert_eq!(decompress_yaz0(&literal).unwrap(), b"xyz");
    let packed = compress_yaz0(&long_repeated).unwrap();
    assert_eq!(decompress_yaz0(&packed).unwrap(), long_repeated);
    assert!(decompress_yaz0(&literal[..18]).is_err());
}

#[test]
fn blz_overlay_round_trips() {
    let uncompressed = [0u8; 8];
    assert_eq!(decompress_blz_overlay(&uncompressed).unwrap(), uncompressed);
    let overlay: Vec<u8> = (0..4096).map(|i| b"the quick brown fox"[i % 19]).collect();
    let packed = compress_blz_overlay(&overlay).unwrap();
    assert!(!packed.is_empty() && packed.len() < overlay.len() && packed.len().is_multiple_of(4));
    assert_eq!(decompress_blz_overlay(&packed).unwrap(), overlay);
}

#[test]
fn compressors_survive_hostile_input() {
    let noise: Vec<u8> = (0..5000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect();
    for packed in [compress_nitro_lz10(&noise).unwrap(), compress_nitro_lz11(&noise).unwrap()] {
        assert_eq!(decompress_nitro_lz(&packed).unwrap(), noise);
    }
    assert_eq!(decompress_yaz0(&compress_yaz0(&noise).unwrap()).unwrap(), noise);
    let blz = compress_blz_overlay(&noise).unwrap();
    assert!(blz.is_empty() || decompress_blz_overlay(&blz).unwrap() == noise);
    assert!(decompress_nitro_lz(&[]).is_err());
    assert!(decompress_nitro_lz(&[0x12, 0, 0, 0]).is_err());
    assert!(decompress_blz_overlay(&[0; 7]).is_err());
}

#[test]
fn identification_follows_magic_then_name() {
    let ncgr = [b'R', b'G', b'C', b'N', 0xff, 0xfe, 0, 1, 16, 0, 0, 0, 16, 0, 1, 0];
    assert_eq!(identify_graphics_format(&ncgr, ""), GraphicsFormatKind::NitroCharacterGraphics);
    assert_eq!(identify_format(&ncgr, ""), FormatKind::Graphics);
    assert_eq!(identify_format(b"NARC....", ""), FormatKind::NarcArchive);
    assert_eq!(identify_format(&[], "script/course.bmg"), FormatKind::MessageBundle);
    assert_eq!(identify_format(b"MESGbmg1", ""), FormatKind::MessageBundle);
    assert_eq!(identify_format(&[], "a/bgdat/x"), FormatKind::BgdatCandidate);
    assert_eq!(identify_format(&[], "a/course1"), FormatKind::LevelCandidate);
    assert_eq!(identify_format(&[1], "x.bin"), FormatKind::Unknown);
    assert_eq!(identify_graphics_format(&[], "A_NCG.BIN"), GraphicsFormatKind::RawCharacterGraphicsCandidate);
    assert_eq!(format_name(FormatKind::NarcArchive), "NARC archive");
}

#[test]
fn narc_round_trips_and_finds_members() {
    let mut fnt = vec![0u8; 8];
    put_u32(&mut fnt, 0, 8);
    put_u16(&mut fnt, 4, 0);
    put_u16(&mut fnt, 6, 1);
    fnt.extend_from_slice(&[5, b'a', b'.', b'b', b'i', b'n', 0]);
    let archive = NarcArchive { name_table: fnt, files: vec![vec![1, 2, 3], vec![4, 5, 6, 7]] };
    let bytes = write_narc(&archive).unwrap();
    assert!(is_narc(&bytes));
    let parsed = parse_narc(&bytes).unwrap();
    assert_eq!(parsed.files, archive.files);
    assert_eq!(parsed.find("a.bin").unwrap(), 0);
    assert!(parsed.find("missing").is_err());
    assert_eq!(write_narc(&parsed).unwrap(), bytes);

    assert!(parse_narc(&bytes[..bytes.len() - 1]).is_err());
    assert!(parse_narc(b"NARC").is_err());
}

fn append_block(course: &mut Vec<u8>, index: usize, block: &[u8]) {
    let offset = course.len() as u32;
    put_u32(course, index * 8, offset);
    put_u32(course, index * 8 + 4, block.len() as u32);
    course.extend_from_slice(block);
}

#[test]
fn level_course_and_bgdat_parse() {
    let mut course = vec![0u8; NSMB_LEVEL_BLOCK_COUNT * 8];
    for i in 0..NSMB_LEVEL_BLOCK_COUNT {
        let len = course.len() as u32;
        put_u32(&mut course, i * 8, len);
    }

    let mut block0 = vec![0u8; 0x20];
    put_u16(&mut block0, 0x06, 7);
    put_u16(&mut block0, 0x0C, 42);
    put_u16(&mut block0, 0x12, 9);
    append_block(&mut course, 0, &block0);

    let mut block2 = vec![0u8; 20];
    put_u16(&mut block2, 2, 11);
    append_block(&mut course, 2, &block2);

    let mut entrance = vec![0u8; 20];
    put_u16(&mut entrance, 0, 100);
    put_u16(&mut entrance, 2, 120);
    entrance[8] = 3;
    entrance[14] = 2;
    append_block(&mut course, 5, &entrance);

    let mut sprite = vec![0xFFu8; 16];
    put_u16(&mut sprite, 0, 55);
    put_u16(&mut sprite, 2, 12);
    put_u16(&mut sprite, 4, 34);
    sprite[6] = 0xAA;
    append_block(&mut course, 6, &sprite);

    let mut view = vec![0u8; 16];
    put_u16(&mut view, 0, 16);
    put_u16(&mut view, 2, 32);
    put_u16(&mut view, 4, 256);
    put_u16(&mut view, 6, 192);
    view[8] = 1;
    view[9] = 2;
    append_block(&mut course, 7, &view);

    let mut zone = vec![0u8; 12];
    put_u16(&mut zone, 0, 64);
    put_u16(&mut zone, 2, 96);
    put_u16(&mut zone, 4, 320);
    put_u16(&mut zone, 6, 224);
    zone[8] = 4;
    append_block(&mut course, 8, &zone);

    let mut path_header = vec![0u8; 8];
    put_u16(&mut path_header, 0, 6);
    put_u16(&mut path_header, 2, 0);
    put_u16(&mut path_header, 4, 1);
    append_block(&mut course, 10, &path_header);
    let mut path_node = vec![0u8; 16];
    put_u16(&mut path_node, 0, 144);
    put_u16(&mut path_node, 2, 160);
    put_u16(&mut path_node, 4, 99);
    append_block(&mut course, 12, &path_node);

    let parsed = parse_nsmb_level_course(&course).unwrap();
    assert_eq!(parsed.header.tileset_theme[0], 42);
    assert_eq!(parsed.header.background_theme[0], 7);
    assert_eq!(parsed.header.foreground_theme[0], 9);
    assert_eq!(parsed.backgrounds[0].screen_id, 11);
    assert_eq!(parsed.sprites.len(), 1);
    assert_eq!(parsed.sprites[0].id, 55);
    assert_eq!(parsed.sprites[0].events[0], 0xAA);
    assert_eq!(parsed.entrances.len(), 1);
    assert_eq!(parsed.entrances[0].id, 3);
    assert_eq!(parsed.entrances[0].kind, 2);
    assert_eq!(parsed.views.len(), 1);
    assert_eq!(parsed.views[0].width, 256);
    assert_eq!(parsed.zones.len(), 1);
    assert_eq!(parsed.zones[0].id, 4);
    assert_eq!(parsed.paths.len(), 1);
    assert_eq!(parsed.paths[0].id, 6);
    assert_eq!(parsed.paths[0].nodes[0].x, 144);
    assert_eq!(parsed.paths[0].nodes[0].speed.raw(), 99);

    assert!(parse_nsmb_level_course(&course[..100]).is_err());
    let mut bad_path = course.clone();
    // Block 10 holds the path headers; raise the first path's node count past the node block.
    let header_offset = u32::from_le_bytes(bad_path[80..84].try_into().unwrap()) as usize;
    put_u16(&mut bad_path, header_offset + 4, 2);
    assert!(parse_nsmb_level_course(&bad_path).is_err());
    let mut past_end = course.clone();
    put_u32(&mut past_end, 4, 0x10000);
    assert!(parse_nsmb_level_course(&past_end).is_err());

    let mut bgdat = vec![0u8; 10];
    put_u16(&mut bgdat, 0, 0x2000 | 0x0123);
    put_u16(&mut bgdat, 2, (-4i16) as u16);
    put_u16(&mut bgdat, 4, 8);
    put_u16(&mut bgdat, 6, 12);
    put_u16(&mut bgdat, 8, 16);
    let objects = parse_nsmb_bgdat_objects(&bgdat).unwrap();
    assert_eq!(objects.len(), 1);
    assert_eq!(objects[0].object_id, 0x0123);
    assert_eq!(objects[0].tileset_slot, 2);
    assert_eq!(objects[0].x, -4);
    assert_eq!(objects[0].width, 12);
    bgdat.extend_from_slice(&[0xFF, 0xFF]);
    assert!(parse_nsmb_bgdat_objects(&bgdat).is_ok());
    bgdat.push(0);
    assert!(parse_nsmb_bgdat_objects(&bgdat).is_err());
}
