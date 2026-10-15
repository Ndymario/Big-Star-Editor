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

#[test]
fn nitro_header_palettes_and_tiles_decode() {
    let ncgr_header = [b'R', b'G', b'C', b'N', 0xff, 0xfe, 0, 1, 16, 0, 0, 0, 16, 0, 1, 0];
    let header = parse_nitro_header(&ncgr_header).unwrap();
    assert_eq!(&header.magic, b"RGCN");
    assert!(parse_nitro_header(&ncgr_header[..15]).is_err());

    let palette = [0x00, 0x00, 0x1f, 0x00, 0xe0, 0x03, 0x00, 0x7c];
    let colors = decode_bgr555_palette(&palette, true).unwrap();
    assert_eq!(colors.len(), 4);
    assert!(colors[0].transparent);
    assert_eq!(colors[1].red, 255);
    assert_eq!(colors[2].green, 255);
    assert_eq!(colors[3].blue, 255);

    let mut raw_tile = vec![0u8; 32];
    raw_tile[0] = 0x21;
    let tiles = decode_4bpp_tiles(&raw_tile).unwrap();
    assert_eq!(tiles.len(), 1);
    assert_eq!(tiles[0].pixels[0], 1);
    assert_eq!(tiles[0].pixels[1], 2);
    assert!(decode_4bpp_tiles(&raw_tile[..31]).is_err());

    let mut raw_8bpp = vec![0u8; 64];
    raw_8bpp[0] = 3;
    assert_eq!(decode_8bpp_tiles(&raw_8bpp).unwrap()[0].pixels[0], 3);
    assert!(decode_8bpp_tiles(&raw_8bpp[..63]).is_err());
    assert_eq!(encode_8bpp_tiles(&decode_8bpp_tiles(&raw_8bpp).unwrap()).unwrap(), raw_8bpp);

    let palette_source = [Bgr555Color::new(255, 0, 0, false), Bgr555Color::new(0, 255, 0, false)];
    let encoded_palette = encode_bgr555_palette(&palette_source).unwrap();
    assert_eq!(encoded_palette.len(), 4);
    let decoded_palette = decode_bgr555_palette(&encoded_palette, false).unwrap();
    assert_eq!(decoded_palette[0].red, 255);
    assert_eq!(decoded_palette[1].green, 255);

    let mut tile4 = IndexedTile4Bpp::default();
    tile4.pixels[0] = 3;
    tile4.pixels[1] = 12;
    let packed4 = encode_4bpp_tiles(&[tile4]).unwrap();
    assert_eq!(packed4[0], 0xc3);
    assert_eq!(decode_4bpp_tiles(&packed4).unwrap()[0].pixels, tile4.pixels);
    tile4.pixels[2] = 16;
    assert!(encode_4bpp_tiles(&[tile4]).is_err());

    // A tileset palette file is several 256-colour palettes end to end, and colour 0 of each is the transparent
    // one, not only the very first.
    let mut blocks = vec![Bgr555Color::new(1, 2, 3, false); 512];
    mark_palette_blocks_transparent(&mut blocks, 256);
    assert!(blocks[0].transparent && blocks[256].transparent && !blocks[1].transparent && !blocks[255].transparent);
}

#[test]
fn screen_maps_and_map16_round_trip_and_render() {
    let mut map_data = vec![0u8; 8];
    put_u16(&mut map_data, 0, 0);
    put_u16(&mut map_data, 2, 0x0400);
    put_u16(&mut map_data, 4, 0x0800);
    put_u16(&mut map_data, 6, 0x1000);
    let map16 = decode_map16(&map_data, 0, 0).unwrap();
    assert_eq!(map16.len(), 1);
    assert!(map16[0].tiles[1].horizontal_flip);
    assert!(map16[0].tiles[2].vertical_flip);
    assert_eq!(map16[0].tiles[3].palette, 1);
    assert_eq!(encode_map16(&map16, 0, 0).unwrap(), map_data);
    assert!(decode_map16(&map_data[..6], 0, 0).is_err());

    let mut render_tiles = vec![IndexedTile8Bpp::default()];
    render_tiles[0].pixels[0] = 1;
    render_tiles[0].pixels[7] = 2;
    let mut render_palette = vec![Bgr555Color::default(); 256];
    render_palette[1] = Bgr555Color::new(255, 0, 0, false);
    render_palette[2] = Bgr555Color::new(0, 255, 0, false);
    let flipped = [ScreenMapEntry::new(0, 0, true, false)];
    let rendered = render_screen_map(&flipped, 1, &render_tiles, &render_palette).unwrap();
    assert_eq!(rendered.pixels[0], 0);
    assert_eq!(rendered.pixels[1], 255);
    assert!(render_screen_map(&flipped, 0, &render_tiles, &render_palette).is_err());

    let screen_source = [ScreenMapEntry::new(7, 2, true, false), ScreenMapEntry::new(12, 1, false, true)];
    let packed_screen = encode_screen_map(&screen_source, 20, 3).unwrap();
    let unpacked = decode_screen_map(&packed_screen, 20, 3).unwrap();
    assert_eq!(unpacked[0].tile, 7);
    assert_eq!(unpacked[0].palette, 2);
    assert!(unpacked[0].horizontal_flip && unpacked[1].vertical_flip);
    assert!(encode_screen_map(&[ScreenMapEntry::new(0x400, 0, false, false)], 0, 0).is_err());

    // An entry naming a tile or palette below the resource's offsets belongs to graphics another tileset supplies.
    // It has to survive decode as the negative index it is, so that re-encoding reproduces the original bytes and
    // rendering can leave it clear instead of drawing tile zero.
    let below_offset = [0x05, 0x00, 0x00, 0x00];
    let below = decode_screen_map(&below_offset, 192, 2).unwrap();
    assert_eq!(below[0].tile, 5 - 192);
    assert_eq!(below[0].palette, -2);
    assert_eq!(below[1].tile, -192);
    assert_eq!(below[1].palette, -2);
    assert_eq!(encode_screen_map(&below, 192, 2).unwrap(), below_offset);

    let solid = IndexedTile8Bpp { pixels: [1; 64] };
    let clear_check = [Bgr555Color::new(0, 0, 0, true), Bgr555Color::new(255, 255, 255, false)];
    let below_render = render_screen_map(&below, 2, &[solid], &clear_check).unwrap();
    assert_eq!(below_render.pixels[3], 0);
    assert_eq!(below_render.pixels[8 * 4 + 3], 0);
}

#[test]
fn object_layouts_decode_render_and_encode() {
    let object_index = vec![0u8; 4];
    let mut object_definition = vec![0, 5, 0, 1, 6, 0, 0xFE, 2, 7, 0, 0xFF];
    let layouts = decode_object_layouts(&object_definition, &object_index, 0).unwrap();
    assert_eq!(layouts.len(), 1);
    let rendered = render_object_layout(&layouts[0], 4, 3).unwrap();
    assert_eq!(rendered.len(), 12);
    assert_eq!(rendered[0], 5);
    assert_eq!(rendered[1], 6);
    assert_eq!(rendered[3], 6);
    assert_eq!(rendered[4], 7);
    assert_eq!(rendered[11], 7);
    object_definition.pop();
    assert!(decode_object_layouts(&object_definition, &object_index, 0).is_err());
    assert!(decode_object_layouts(&[0, 5], &object_index, 0).is_err());

    let layout = ObjectLayout {
        rows: vec![vec![ObjectLayoutTile::new(1, 0), ObjectLayoutTile::new(2, 1)], vec![ObjectLayoutTile::new(3, 2)]],
        ..ObjectLayout::default()
    };
    let index_template = [0, 0, 9, 8];
    let encoded = encode_object_layouts(std::slice::from_ref(&layout), 0, &index_template).unwrap();
    assert_eq!(encoded.index[2], 9);
    assert_eq!(encoded.index[3], 8);
    let decoded = decode_object_layouts(&encoded.definitions, &encoded.index, 0).unwrap();
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded[0].rows.len(), 2);
    assert_eq!(decoded[0].rows[0][1].tile, 2);
    assert_eq!(decoded[0].rows[1][0].control, 2);
    assert!(encode_object_layouts(&[layout], 0, &[0, 0]).is_err());

    assert_eq!(encoded.definitions, [0, 1, 0, 1, 2, 0, 0xfe, 2, 3, 0, 0xfe, 0xff], "every row ends in a line break");

    // A slope's step is laid along one diagonal and the other cells are left unwritten; an empty definition is
    // block 0, still written.
    let slope = ObjectLayout { rows: vec![vec![ObjectLayoutTile::new(4, 0), ObjectLayoutTile::default()]], slope: true, ..ObjectLayout::default() };
    let cells = render_object_layout(&slope, 4, 2).unwrap();
    let (u, e) = (OBJECT_LAYOUT_UNWRITTEN, OBJECT_LAYOUT_EMPTY_TILE);
    assert_eq!(cells, [u, u, 4, e, 4, e, u, u]);
    assert!(render_object_layout(&slope, 0, 2).is_err());
}

fn slope_cells(definition: &[u8], width: usize, height: usize) -> Vec<Vec<i32>> {
    let layouts = decode_object_layouts(definition, &[0; 4], 0).unwrap();
    let encoded = encode_object_layouts(&layouts, 0, &[]).unwrap();
    assert_eq!(encoded.definitions, definition, "a slope survives being written back");
    render_object_layout(&layouts[0], width, height).unwrap().chunks(width).map(<[i32]>::to_vec).collect()
}

#[test]
fn slopes_are_laid_as_the_game_lays_them() {
    let (u, e) = (OBJECT_LAYOUT_UNWRITTEN, OBJECT_LAYOUT_EMPTY_TILE);

    // Rising to the right, one tile of ground under each step.
    assert_eq!(slope_cells(&[0x80, 0, 0x20, 1, 0xfe, 0x85, 4, 0x21, 1, 0xfe, 0xff], 3, 3), [[u, u, 0x120], [u, 0x120, 0x121], [0x120, 0x121, u]]);
    // Rising to the left, in an object wider than it is tall.
    assert_eq!(slope_cells(&[0x81, 0, 0x23, 1, 0xfe, 0x85, 4, 0x22, 1, 0xfe, 0xff], 3, 2), [[0x122, 0x123, u], [u, 0x122, 0x123]]);
    // A gentle slope: a step two wide, and its ground one tile.
    assert_eq!(
        slope_cells(&[0x80, 0, 0x20, 1, 0, 0x21, 1, 0xfe, 0x85, 4, 0x30, 1, 0xfe, 0xff], 5, 2),
        [[u, u, 0x120, 0x121, 0x130], [0x120, 0x121, 0x130, u, u]]
    );
    // A steep one: a step two tall and nothing under it.
    assert_eq!(slope_cells(&[0x80, 0, 0x0b, 3, 0xfe, 0, 0x0c, 3, 0xfe, 0xff], 2, 4), [[u, 0x30b], [u, 0x30c], [0x30b, u], [0x30c, u]]);
    // Hanging from a ceiling and running left, the fill reads from the step outwards.
    assert_eq!(
        slope_cells(&[0x83, 0, 0x30, 1, 0xfe, 0x85, 4, 0x50, 1, 4, 0x40, 1, 0xfe, 0xff], 4, 3),
        [[u, 0x150, 0x140, 0x130], [0x150, 0x140, 0x130, u], [0x140, 0x130, u, u]]
    );
    // A fill tile that is not marked to be laid once runs to the object's edge -- here the empty block, which
    // clears the first line of each step, over a tile laid once on the second.
    assert_eq!(
        slope_cells(&[0x82, 0, 0xae, 1, 0xfe, 0, 0xbe, 1, 0xfe, 0x85, 0, 0, 0, 0xfe, 4, 0xbf, 1, 0xfe, 0xff], 3, 4),
        [[0x1ae, e, e], [0x1be, 0x1bf, u], [u, 0x1ae, e], [u, 0x1be, 0x1bf]]
    );
    // The block behind the steps, which no retail tileset uses, fills the other side.
    assert_eq!(slope_cells(&[0x80, 0, 1, 1, 0xfe, 0x84, 0, 2, 1, 0xfe, 0xff], 3, 3), [[0x102, 0x102, 0x101], [0x102, 0x101, u], [0x101, u, u]]);
}

#[test]
fn quantization_reserves_a_transparent_colour() {
    let source = RgbaImage { width: 4, height: 1, pixels: vec![255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 0, 0, 0, 0] };
    let quantized = quantize_bgr555(&source, 3, 128).unwrap();
    assert_eq!(quantized.palette.len(), 3);
    assert!(quantized.palette[0].transparent);
    assert_eq!(quantized.pixels[3], 0);
    assert!(quantize_bgr555(&source, 1, 128).is_err());
    assert!(quantize_bgr555(&RgbaImage { width: 2, height: 1, pixels: vec![0; 4] }, 16, 128).is_err());

    assert_eq!(identify_graphics_format(&[], "UI_O_1P_game_in_d_ncg.bin"), GraphicsFormatKind::RawCharacterGraphicsCandidate);
    assert_eq!(identify_graphics_format(&[], "enemy/w_cannon.nsbmd"), GraphicsFormatKind::NitroModel);
}

fn label() -> Nitro2dBlock {
    Nitro2dBlock { magic: *b"LBAL", data: vec![1, 2, 3, 4] }
}

#[test]
fn nitro_2d_files_round_trip() {
    let nclr = Nclr {
        format: 3,
        extended_palette: true,
        palette: vec![0, 0, 0x1f, 0],
        compressed_palette_indices: Some(vec![2, 7]),
        file: Nitro2dFile { blocks: vec![label()], ..Nitro2dFile::default() },
    };
    let nclr_bytes = write_nclr(&nclr).unwrap();
    assert_eq!(identify_graphics_format(&nclr_bytes, ""), GraphicsFormatKind::NitroPalette);
    let parsed_nclr = parse_nclr(&nclr_bytes).unwrap();
    assert_eq!(parsed_nclr.format, 3);
    assert!(parsed_nclr.extended_palette);
    assert_eq!(parsed_nclr.palette, nclr.palette);
    assert_eq!(parsed_nclr.compressed_palette_indices, nclr.compressed_palette_indices);
    assert_eq!(write_nclr(&parsed_nclr).unwrap(), nclr_bytes);

    let ncgr = Ncgr {
        height: 2,
        width: 3,
        pixel_format: 4,
        mapping_type: 1,
        character_format: 0x100,
        characters: vec![0x5a; 64],
        position: Some(NcgrPosition { x: 1, y: 2, width: 3, height: 4 }),
        file: Nitro2dFile { blocks: vec![label()], ..Nitro2dFile::default() },
    };
    let ncgr_bytes = write_ncgr(&ncgr).unwrap();
    let parsed_ncgr = parse_ncgr(&ncgr_bytes).unwrap();
    assert_eq!(parsed_ncgr.height, 2);
    assert_eq!(parsed_ncgr.width, 3);
    assert_eq!(parsed_ncgr.characters, ncgr.characters);
    assert_eq!(parsed_ncgr.position.unwrap().height, 4);
    assert_eq!(write_ncgr(&parsed_ncgr).unwrap(), ncgr_bytes);

    let nscr = Nscr {
        width: 256,
        height: 192,
        color_mode: 1,
        screen_format: 0,
        screen: vec![0, 1, 2, 3],
        file: Nitro2dFile { blocks: vec![label()], ..Nitro2dFile::default() },
    };
    let nscr_bytes = write_nscr(&nscr).unwrap();
    let parsed_nscr = parse_nscr(&nscr_bytes).unwrap();
    assert_eq!(parsed_nscr.width, 256);
    assert_eq!(parsed_nscr.screen, nscr.screen);
    assert_eq!(write_nscr(&parsed_nscr).unwrap(), nscr_bytes);

    let ncer = Ncer {
        mapping_mode: 2,
        cells: vec![
            NcerCell {
                attributes: 5,
                oam: vec![NcerOam { attr0: 1, attr1: 2, attr2: 3 }],
                bounds: Some(NcerBounds { max_x: 8, max_y: 9, min_x: -8, min_y: -9 }),
            },
            NcerCell { attributes: 6, oam: vec![NcerOam { attr0: 4, attr1: 5, attr2: 6 }, NcerOam { attr0: 7, attr1: 8, attr2: 9 }], bounds: None },
        ],
        maximum_transfer_size: 128,
        transfers: vec![NcerTransfer { source_offset: 0, size: 64 }, NcerTransfer { source_offset: 64, size: 128 }],
        file: Nitro2dFile { blocks: vec![label()], ..Nitro2dFile::default() },
        ..Ncer::default()
    };
    let ncer_bytes = write_ncer(&ncer).unwrap();
    assert_eq!(identify_graphics_format(&ncer_bytes, ""), GraphicsFormatKind::NitroCellLayout);
    let parsed_ncer = parse_ncer(&ncer_bytes).unwrap();
    assert_eq!(parsed_ncer.cells.len(), 2);
    assert_eq!(parsed_ncer.cells[0].bounds.unwrap().min_x, -8);
    assert_eq!(parsed_ncer.cells[1].oam[1].attr2, 9);
    assert_eq!(parsed_ncer.transfers[1].size, 128);
    assert_eq!(write_ncer(&parsed_ncer).unwrap(), ncer_bytes);

    let render_cells =
        Ncer { cells: vec![NcerCell { attributes: 0, oam: vec![NcerOam { attr0: 0x2000, attr1: 0, attr2: 0 }], bounds: None }], ..Ncer::default() };
    let mut character = vec![0u8; 64];
    character[0] = 1;
    let mut cell_palette = vec![Bgr555Color::default(); 2];
    cell_palette[1] = Bgr555Color::new(255, 0, 0, false);
    let rendered_cell = render_ncer_cell(&render_cells, 0, &character, &cell_palette).unwrap();
    assert_eq!(rendered_cell.width, 8);
    assert_eq!(rendered_cell.height, 8);
    assert_eq!(rendered_cell.pixels[0], 255);
    assert_eq!(rendered_cell.pixels[3], 255);
    assert!(render_ncer_cell(&render_cells, 1, &character, &cell_palette).is_err());

    let frame = |duration: u16, content: Vec<u8>| NanrFrame { duration, padding: 0xbeef, content };
    let nanr = Nanr {
        sequences: vec![
            NanrSequence { loop_start: 0, animation_type: 0x00010000, play_mode: 1, frames: vec![frame(2, vec![3, 0])] },
            NanrSequence { loop_start: 1, animation_type: 0x00010001, play_mode: 2, frames: vec![frame(4, vec![5; 16])] },
            NanrSequence { loop_start: 0, animation_type: 0x00010002, play_mode: 3, frames: vec![frame(6, vec![7; 8])] },
        ],
        file: Nitro2dFile { blocks: vec![label()], ..Nitro2dFile::default() },
    };
    let nanr_bytes = write_nanr(&nanr).unwrap();
    assert_eq!(identify_graphics_format(&nanr_bytes, ""), GraphicsFormatKind::NitroCellAnimation);
    let parsed_nanr = parse_nanr(&nanr_bytes).unwrap();
    assert_eq!(parsed_nanr.sequences.len(), 3);
    assert_eq!(parsed_nanr.sequences[1].frames[0].content.len(), 16);
    assert_eq!(parsed_nanr.sequences[2].frames[0].duration, 6);
    assert_eq!(write_nanr(&parsed_nanr).unwrap(), nanr_bytes);

    let mut malformed = nclr_bytes.clone();
    malformed[8] = 0xff;
    assert!(parse_nclr(&malformed).is_err());
    let mut malformed = ncer_bytes.clone();
    malformed[28..32].copy_from_slice(&[0xff, 0xff, 0xff, 0x7f]);
    assert!(parse_ncer(&malformed).is_err());
    assert!(parse_ncgr(&nclr_bytes).is_err());
}

#[test]
fn tex0_round_trips_the_reference_module_when_present() {
    // The reference module is a private cross-check and is never committed, so these round trips run only where it
    // happens to be checked out beside the repository, or where `BIGSTAR_COOP_MODULE` names it. The other TEX0 tests
    // are synthetic and always run.
    let root = std::env::var_os("BIGSTAR_COOP_MODULE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deps/nsmb-coop-module"));
    let modules = root.join("modules");
    let fixture = modules.join("coop/nitrofs/en/enemy/koopa_new.nsbmd");
    if !fixture.exists() {
        eprintln!("tex0: reference module absent, skipping ROM-asset round trips");
        return;
    }
    let bytes = std::fs::read(&fixture).unwrap();
    let tex0 = parse_tex0(&bytes).unwrap();
    assert_eq!(tex0.textures.len(), 6);
    assert!(!tex0.palettes.is_empty());
    assert_eq!(tex0.textures[0].name, "koopa_all");
    assert_eq!(write_tex0(&tex0).unwrap(), bytes);
    let mut edited = tex0.clone();
    edited.textures[0].texels[0] ^= 1;
    let written = write_tex0(&edited).unwrap();
    assert_ne!(written, bytes);
    assert_eq!(parse_tex0(&written).unwrap().textures[0].texels[0], edited.textures[0].texels[0]);
    edited.textures[0].width *= 2;
    assert!(write_tex0(&edited).is_err());

    fn walk(dir: &std::path::Path, found: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(&path, found);
            } else if path.extension().is_some_and(|e| e == "nsbmd") {
                found.push(path);
            }
        }
    }
    let mut assets = Vec::new();
    walk(&modules, &mut assets);
    let mut checked = 0;
    for asset in assets {
        let data = std::fs::read(&asset).unwrap();
        if let Ok(parsed) = parse_tex0(&data) {
            assert_eq!(write_tex0(&parsed).unwrap(), data, "{}", asset.display());
            checked += 1;
        }
    }
    assert!(checked >= 10);
}

#[test]
fn tex0_compresses_and_renders_synthetic_textures() {
    let mut image = RgbaImage { width: 8, height: 4, pixels: vec![255; 8 * 4 * 4] };
    for y in 0..4 {
        for x in 0..8 {
            let pixel = (y * 8 + x) * 4;
            image.pixels[pixel] = (x * 31) as u8;
            image.pixels[pixel + 1] = (y * 63) as u8;
            image.pixels[pixel + 2] = ((x + y) * 20) as u8;
            image.pixels[pixel + 3] = if x == 0 && y == 0 { 0 } else { 255 };
        }
    }
    let compressed = compress_tex4x4(&image, 8192).unwrap();
    assert_eq!(compressed.texels.len(), 8);
    assert_eq!(compressed.palette_indices.len(), 4);
    assert!(!compressed.palette.is_empty());
    let texture = Tex0Texture {
        name: "test".into(),
        format: TextureFormat::Compressed4x4,
        width: 8,
        height: 4,
        original_width: 8,
        original_height: 4,
        color_zero_transparent: false,
        texels: compressed.texels.clone(),
        palette_indices: compressed.palette_indices.clone(),
    };
    let palette = Tex0Palette { name: "test".into(), flags: 0, colors: compressed.palette.clone() };
    let rendered = render_tex0_texture(&texture, &palette).unwrap();
    assert_eq!(rendered.width, 8);
    assert_eq!(rendered.pixels[3], 0);
    assert_eq!(rendered.pixels[7 * 4 + 3], 255);

    let mut direct = Tex0Texture {
        name: "direct".into(),
        format: TextureFormat::Direct,
        width: 8,
        height: 8,
        original_width: 1,
        original_height: 1,
        texels: vec![0; 128],
        ..Tex0Texture::default()
    };
    direct.texels[0] = 0x1f;
    direct.texels[1] = 0x80;
    let direct_image = render_tex0_texture(&direct, &Tex0Palette::default()).unwrap();
    assert_eq!(direct_image.pixels[0], 255);
    assert_eq!(direct_image.pixels[3], 255);
    assert!(compress_tex4x4(&RgbaImage { width: 3, height: 4, pixels: vec![0; 48] }, 8192).is_err());
    assert!(parse_tex0(b"NOPE").is_err());
    assert!(write_tex0(&Tex0::default()).is_err());
}

fn bmg_sample() -> Bmg {
    Bmg {
        encoding: BmgEncoding::Utf16,
        file_id: 7,
        attribute_size: 4,
        has_mid1: true,
        messages: vec![
            BmgMessage {
                parts: vec![BmgPart::escape(255, vec![0, 0, 1, 0]), BmgPart::text(" Luigi has appeared!")],
                attributes: vec![1, 2, 3, 4],
                id: Some(100),
            },
            BmgMessage { parts: vec![BmgPart::text("Café ★")], attributes: vec![5, 6, 7, 8], id: Some(101) },
            BmgMessage { parts: vec![], attributes: vec![0, 0, 0, 0], id: Some(102) },
            BmgMessage { parts: vec![BmgPart::text("Café ★")], attributes: vec![9, 0, 0, 0], id: Some(103) },
        ],
        ..Bmg::default()
    }
}

#[test]
fn bmg_round_trips_and_shares_strings() {
    let original = bmg_sample();
    let bytes = write_bmg(&original).unwrap();
    assert!(bytes.len().is_multiple_of(32), "every section is padded to the 32-byte alignment");
    assert!(is_bmg(&bytes, ""));
    assert!(is_bmg(&[], "script/course.bmg"));
    assert!(!is_bmg(&[], "script/.bmg"));
    assert_eq!(identify_format(&bytes, ""), FormatKind::MessageBundle);
    assert_eq!(identify_format(&[], "script/course.bmg"), FormatKind::MessageBundle);

    let parsed = parse_bmg(&bytes).unwrap();
    assert_eq!(parsed.messages, original.messages);
    assert_eq!(parsed.file_id, 7);
    assert_eq!(parsed.attribute_size, 4);
    assert_eq!(write_bmg(&parsed).unwrap(), bytes, "an untouched load/save cycle is byte-identical");

    // Identical strings must share one DAT1 offset the way Nintendo's own files do, and the empty message must fall
    // on the terminator the pool opens with.
    let entry_offset = |index: usize| u32::from_le_bytes(bytes[0x30 + index * 8..0x34 + index * 8].try_into().unwrap());
    assert_eq!(entry_offset(1), entry_offset(3));
    assert_eq!(entry_offset(2), 0);

    let first = &parsed.messages[0];
    assert_eq!(bmg_message_text(first), "[255:00000100] Luigi has appeared!");
    assert_eq!(bmg_message_plain_text(first), " Luigi has appeared!");
    assert_eq!(parse_bmg_message_text(&bmg_message_text(first)), first.parts);
    assert_eq!(parse_bmg_message_text("100% [not:anescape] [1:0z] done")[0].text, "100% [not:anescape] [1:0z] done");
    assert_eq!(parse_bmg_message_text("[1:F]")[0].escape_data, [0x0f]);
    assert_eq!(parse_bmg_message_text("[+1:F]")[0].text, "[+1:F]");

    // Escape payloads the model does not understand still have to survive untouched.
    let mut opaque = original.clone();
    opaque.messages[0].parts = vec![BmgPart::escape(200, vec![0xde, 0xad, 0xbe, 0xef, 0x01])];
    let opaque_parsed = parse_bmg(&write_bmg(&opaque).unwrap()).unwrap();
    assert_eq!(opaque_parsed.messages[0].parts, opaque.messages[0].parts);

    // The 8-bit encodings use a narrower escape header, and CP1252 has its own high range.
    for encoding in [BmgEncoding::Cp1252, BmgEncoding::Utf8] {
        let narrow = Bmg {
            encoding,
            attribute_size: 0,
            messages: vec![BmgMessage { parts: vec![BmgPart::text("Café"), BmgPart::escape(3, vec![0x42])], attributes: vec![], id: None }],
            ..Bmg::default()
        };
        assert_eq!(parse_bmg(&write_bmg(&narrow).unwrap()).unwrap().messages, narrow.messages, "{encoding:?}");
    }

    let unmappable = Bmg {
        encoding: BmgEncoding::Cp1252,
        messages: vec![BmgMessage { parts: vec![BmgPart::text("★")], ..BmgMessage::default() }],
        ..Bmg::default()
    };
    assert!(write_bmg(&unmappable).is_err(), "a character CP1252 cannot hold is refused rather than mangled");

    assert!(parse_bmg(b"NARC").is_err());
    assert!(parse_bmg(&bytes[..0x30]).is_err(), "a file cut off inside its sections is refused");
    let mut shift_jis = bytes.clone();
    shift_jis[0x10] = 3;
    assert!(parse_bmg(&shift_jis).is_err(), "Shift-JIS is refused rather than silently mangled");
}

fn tileset_resources() -> TilesetResourceData {
    let mut character = IndexedTile8Bpp::default();
    character.pixels[0] = 1;
    let raw_graphics = encode_8bpp_tiles(&[character]).unwrap();
    let colors = [Bgr555Color::new(0, 0, 0, true), Bgr555Color::new(255, 255, 255, false)];
    TilesetResourceData {
        graphics: compress_nitro_lz10(&raw_graphics).unwrap(),
        palette: encode_bgr555_palette(&colors).unwrap(),
        map16: encode_map16(&[Map16Tile::default()], 0, 0).unwrap(),
        objects: vec![0, 0, 0, 0xff],
        object_index: vec![0, 0, 7, 9],
        behaviors: vec![0x34, 0x12, 0, 0],
    }
}

#[test]
fn tilesets_round_trip_untouched_and_edited() {
    let resources = tileset_resources();
    let mut parsed = parse_tileset(&resources, 0, 0, 0).unwrap();
    assert_eq!(parsed.graphics.len(), 1);
    assert_eq!(parsed.palette.len(), 2);
    assert_eq!(parsed.map16.len(), 1);
    assert_eq!(parsed.objects.len(), 1);
    assert_eq!(parsed.objects[0].rows[0][0].tile, -1);
    assert_eq!(parsed.behaviors, [0x1234]);

    let untouched = write_tileset(&parsed).unwrap();
    assert_eq!(untouched.graphics, resources.graphics);
    assert_eq!(untouched.objects, resources.objects);
    assert_eq!(untouched.object_index, resources.object_index);

    // Most of a tileset's Map16 names jyotyu tiles, which sit below its own offsets. Opening a tileset and saving it
    // without an edit has to give the file back unchanged, whatever those entries point at.
    let mut offset_resources = resources.clone();
    offset_resources.map16 = vec![0x05, 0x00, 0x00, 0x00, 0xc5, 0x20, 0x00, 0x0c];
    let offset_parsed = parse_tileset(&offset_resources, 192, 2, 1).unwrap();
    assert_eq!(offset_parsed.map16[0].tiles[0].tile, 5 - 192);
    assert_eq!(offset_parsed.map16[0].tiles[2].palette, 0);
    assert_eq!(offset_parsed.map16[0].tiles[2].tile, 5);
    assert!(offset_parsed.map16[0].tiles[3].horizontal_flip && offset_parsed.map16[0].tiles[3].vertical_flip);
    assert_eq!(write_tileset(&offset_parsed).unwrap().map16, offset_resources.map16);

    // Composed against the tileset that supplies tile 5, the below-offset entry draws; alone, it cannot. jyotyu sits
    // at offset 0 and carries the tile.
    let jyotyu = EditableTileset {
        graphics: vec![IndexedTile8Bpp { pixels: [1; 64] }; 6],
        palette: vec![Bgr555Color::new(0, 0, 0, true), Bgr555Color::new(255, 255, 255, false)],
        ..EditableTileset::default()
    };
    let memory = compose_tileset_memory(&[jyotyu, offset_parsed.clone()]);
    assert_eq!(memory.graphics.len(), 193);
    assert_eq!(memory.palette.len(), 2 * 256 + 2);
    assert!(memory.palette[2 * 256].transparent, "the tileset's own block, at its offset");
    assert!(!memory.palette[1].transparent && memory.graphics[5].pixels[0] == 1);

    let alone = render_map16_sheet(&offset_parsed, 1).unwrap();
    let together = render_composed_map16_sheet(&offset_parsed, &memory, 1).unwrap();
    assert_eq!(alone.pixels[3], 0, "entry 0 names tile 5-192: not ours");
    assert_eq!(together.pixels[3], 255, "...which is jyotyu's tile 5");

    parsed.map16[0].tiles[0] = ScreenMapEntry::new(0, 0, true, true);
    parsed.objects[0].rows[0][0] = ObjectLayoutTile::new(1, 0);
    parsed.behaviors[0] = 0x89abcdef;
    let changed = write_tileset(&parsed).unwrap();
    let changed_round_trip = parse_tileset(&changed, 0, 0, 0).unwrap();
    assert!(changed_round_trip.map16[0].tiles[0].horizontal_flip);
    assert_eq!(changed_round_trip.objects[0].rows[0][0].tile, 1);
    assert_eq!(changed_round_trip.behaviors[0], 0x89abcdef);

    let bundle = write_tileset_bundle(&changed).unwrap();
    let unbundled = parse_tileset_bundle(&bundle).unwrap();
    assert_eq!(unbundled.graphics, changed.graphics);
    assert_eq!(unbundled.behaviors, changed.behaviors);
    let mut broken = bundle.clone();
    broken[8] = 9;
    assert!(parse_tileset_bundle(&broken).is_err());
    let mut broken = bundle.clone();
    broken.copy_within(12..16, 24);
    assert!(parse_tileset_bundle(&broken).is_err());

    let parts = TileBehaviorParts { flags: 0x10122, subtype: 5, parameter: 7 };
    assert_eq!(decode_tile_behavior(encode_tile_behavior(parts)), parts);
    assert_eq!(tile_behavior_parameter_kind(TileBehaviorParts { flags: 2, subtype: 0, parameter: 0 }), TileBehaviorParameterKind::Coin);
    assert_eq!(tile_behavior_parameter_kind(TileBehaviorParts { flags: 2, subtype: 4, parameter: 0 }), TileBehaviorParameterKind::Conflict);
    assert_eq!(tile_behavior_parameter_name(TileBehaviorParameterKind::Slope, 10), "Slope edge");
    assert_eq!(tile_behavior_parameter_name(TileBehaviorParameterKind::PipeDoor, 31), "Pipe joint");

    let mut png = RgbaImage { width: 16, height: 8, pixels: vec![0; 16 * 8 * 4] };
    for i in 0..16 * 8 {
        let left = i % 16 < 8;
        png.pixels[i * 4] = if left { 255 } else { 0 };
        png.pixels[i * 4 + 2] = if left { 0 } else { 255 };
        png.pixels[i * 4 + 3] = 255;
    }
    let mut tilemap = EditableTilemap::default();
    import_tilemap_image(&mut tilemap, &png).unwrap();
    assert_eq!(tilemap.width_tiles, 2);
    assert_eq!(tilemap.graphics.len(), 2);
    assert_eq!(tilemap.screen.len(), 2);
    let rendered = render_tilemap(&tilemap).unwrap();
    assert_eq!((rendered.width, rendered.height), (16, 8));
    let tilemap_files = write_tilemap(&tilemap).unwrap();
    assert_eq!(parse_tilemap(&tilemap_files, 2, 0, 0).unwrap().screen.len(), 2);
    let mut imported_tileset = parsed.clone();
    import_tileset_graphics(&mut imported_tileset, &png).unwrap();
    assert!(imported_tileset.palette[0].transparent);
    assert_ne!(imported_tileset.graphics[0].pixels[0], 0);
    assert!(render_tileset_graphics(&imported_tileset, 32).is_ok());
}

#[test]
fn ui_resources_round_trip() {
    let bnbl = vec![b'J', b'N', b'B', b'L', 0x34, 0x12, 1, 0xaa, 0x23, 0xd1, 0x56, 0x72, 8, 9, 0xee];
    let mut touch = parse_bnbl(&bnbl).unwrap();
    assert_eq!(touch.entries[0].x, 0x123);
    assert_eq!(touch.entries[0].x_alignment, 1);
    assert_eq!(touch.entries[0].x_reserved, 3);
    assert_eq!(write_bnbl(&touch).unwrap(), bnbl, "BNBL untouched round trip is byte-identical");
    touch.entries.push(BnblEntry { x: 1, y: 2, width: 3, height: 4, ..BnblEntry::default() });
    let changed = write_bnbl(&touch).unwrap();
    assert_eq!(changed[6], 2);
    assert_eq!(*changed.last().unwrap(), 0xee, "BNBL add preserves trailing bytes");
    assert!(parse_bnbl(&[b'J', b'N', b'B', b'L', 0, 0, 1, 0]).is_err());
    touch.entries[0].x = 0x1000;
    assert!(write_bnbl(&touch).is_err());

    let bncl = vec![b'J', b'N', b'C', b'L', 0, 0, 1, 0x99, 1, 0x20, 2, 0x10, 7, 8, 9, 10, 0xfe];
    let layout = parse_bncl(&bncl).unwrap();
    assert_eq!(write_bncl(&layout).unwrap(), bncl);
    assert!(parse_bncl(&[b'J', b'N', b'B', b'L', 0, 0, 0, 0]).is_err());

    let mut bncd = vec![0u8; 0x40];
    bncd[..4].copy_from_slice(b"JNCD");
    put_u16(&mut bncd, 4, 0x101);
    put_u16(&mut bncd, 6, 1);
    put_u32(&mut bncd, 8, 0x1c);
    put_u32(&mut bncd, 12, 0x24);
    put_u32(&mut bncd, 16, 0x30);
    put_u32(&mut bncd, 20, 0x10);
    bncd[0x1c] = 16;
    bncd[0x1d] = 8;
    put_u16(&mut bncd, 0x20, 1);
    put_u16(&mut bncd, 0x24, 0x4000);
    put_u16(&mut bncd, 0x26, 0x8000);
    put_u32(&mut bncd, 0x28, 0x11223344);
    put_u16(&mut bncd, 0x2c, 2);
    put_u16(&mut bncd, 0x2e, 1);
    let mut cells = parse_bncd(&bncd).unwrap();
    assert_eq!(write_bncd(&cells).unwrap(), bncd);
    assert_eq!(cells.subentries.len(), 1);
    cells.entries[0].subentry_count = 2;
    assert!(write_bncd(&cells).is_err(), "BNCD rejects references outside subentry table");
    cells.texture_offset = 0xffffffff;
    assert!(write_bncd(&cells).is_err(), "BNCD refuses overflowing mutable offsets");

    let enpg: Vec<u8> = (0..0x10203).map(|i| i as u8).collect();
    let image = parse_enpg(&enpg).unwrap();
    assert_eq!(write_enpg(&image).unwrap(), enpg);
    assert!(parse_enpg(&vec![0; 0x101ff]).is_err());
    assert_eq!(identify_ui_resource(&enpg, "a/B.ENPG"), UiResourceKind::Enpg);

    let mut banner = vec![0u8; 0xa00];
    put_u16(&mut banner, 0, 1);
    banner[0x20] = 0x21;
    put_u16(&mut banner, 0x220, 0x7fff);
    for (i, c) in "Test".encode_utf16().enumerate() {
        put_u16(&mut banner, 0x240 + i * 2, c);
    }
    let crc = bigstar_nds::crc16(&banner[0x20..0x840], 0xFFFF);
    put_u16(&mut banner, 2, crc);
    assert_eq!(identify_ui_resource(&banner, "banner.bin"), UiResourceKind::Banner);
    let mut parsed_banner = parse_ds_banner(&banner).unwrap();
    assert_eq!(banner_title(&parsed_banner, 0), "Test".encode_utf16().collect::<Vec<_>>());
    set_banner_title(&mut parsed_banner, 1, &"English".encode_utf16().collect::<Vec<_>>()).unwrap();
    assert!(set_banner_title(&mut parsed_banner, 6, &[]).is_err());
    let output = write_ds_banner(&parsed_banner).unwrap();
    assert_eq!(output.len(), banner.len());
    assert_eq!(output[0x840], 0, "banner preserves padded size");
    assert_eq!(u16::from_le_bytes([output[2], output[3]]), bigstar_nds::crc16(&output[0x20..0x840], 0xFFFF), "banner CRC updates");
    assert!(parse_ds_banner(&[9, 0]).is_err());

    let metadata = "[Music]\r\n0=Overworld\r\n[65535]\r\n1=Breakable block\r\n";
    let mut user = parse_rom_user_metadata(metadata.as_bytes()).unwrap();
    assert_eq!(write_rom_user_metadata(&user).unwrap(), metadata.as_bytes(), "metadata untouched round trip preserves line endings");
    user.sections[0].entries[0].value = "Ground".into();
    assert!(String::from_utf8(write_rom_user_metadata(&user).unwrap()).unwrap().contains("0=Ground"));
    assert!(parse_rom_user_metadata(b"[A]\n0=x\n0=y\n").is_err(), "metadata duplicate key rejected");
    assert!(parse_rom_user_metadata(b"0=x\n").is_err(), "metadata refuses entries before a section");
    assert!(parse_rom_user_metadata(b"[A]\n+1=x\n").is_err());
    assert!(parse_rom_user_metadata(b"[A]\n-0=x\n").is_ok());
}

#[test]
fn a_layers_screen_is_read_from_where_the_game_loads_it() {
    // One 8bpp tile of index 1, two 256-colour palettes, and a background screen naming tile 576 in palette row 11.
    let graphics = [1u8; 64].to_vec();
    let palette: Vec<u8> = (0..512u16).flat_map(|i| (i & 0x7fff).to_le_bytes()).collect();
    let entry = 576u16 | (11 << 12);
    let screen: Vec<u8> = std::iter::repeat_n(entry.to_le_bytes(), 32).flatten().collect();
    let files = TilemapResourceData { graphics, palette, screen };
    let map = parse_tilemap(&files, 0, BACKGROUND_TILE_OFFSET, BACKGROUND_PALETTE_OFFSET).unwrap();
    assert_eq!((map.screen[0].tile, map.screen[0].palette), (0, 1));
    assert!(map.palette[0].transparent && map.palette[256].transparent && !map.palette[1].transparent, "colour 0 of each palette");
    let image = render_tilemap(&map).unwrap();
    assert_eq!(image.pixels[3], 255, "tile 0 in the second palette draws");
    // Untouched, it writes back byte for byte.
    assert_eq!(write_tilemap(&map).unwrap(), files);
    // Read from zero, every entry names a tile and palette the layer does not hold.
    assert_eq!(render_tilemap(&parse_tilemap(&files, 0, 0, 0).unwrap()).unwrap().pixels[3], 0);
}

/// Against a real ROM, named by `BIGSTAR_NSMB_ROM`: every archive in it, plain or LZ-wrapped, is written back byte
/// for byte when nothing in it changed -- the cheapest evidence that repacking one edited member disturbs nothing
/// else. Skipped without one.
#[test]
fn real_rom_archives_write_back_unchanged() {
    let Some(path) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let rom = std::fs::read(path).unwrap();
    let header = bigstar_nds::parse_rom_header(&rom).unwrap();
    let region = |offset: u32, size: u32| &rom[offset as usize..(offset + size) as usize];
    let fat = bigstar_nds::parse_fat(region(header.fat.rom_offset, header.fat.size)).unwrap();
    let files = bigstar_nds::parse_fnt(region(header.fnt.rom_offset, header.fnt.size)).unwrap();
    let mut checked = 0;
    for file in files {
        let entry = fat[usize::from(file.id)];
        let data = region(entry.start_address, entry.size());
        let Some((_, plain)) = unwrap_narc(data) else { continue };
        assert_eq!(write_narc(&parse_narc(&plain).unwrap()).unwrap(), plain, "{} changed on a round trip", file.path);
        checked += 1;
    }
    assert!(checked > 0, "the ROM holds no archives");
}

#[test]
fn an_archive_goes_back_in_the_wrapper_it_came_in() {
    let archive = NarcArchive { name_table: vec![8, 0, 0, 0, 0, 0, 1, 0, 1, b'a', 0], files: vec![b"member".to_vec()] };
    let plain = write_narc(&archive).unwrap();
    assert_eq!(unwrap_narc(&plain), Some((NarcWrapper::None, plain.clone())));
    for (wrapper, first) in [(NarcWrapper::Lz10, 0x10), (NarcWrapper::Lz11, 0x11)] {
        let wrapped = write_wrapped_narc(&archive, wrapper).unwrap();
        assert_eq!(wrapped[0], first);
        assert_eq!(unwrap_narc(&wrapped), Some((wrapper, plain.clone())));
    }
    // Starts like an LZ10 stream, but is not an archive underneath.
    assert_eq!(unwrap_narc(&compress_nitro_lz10(b"not an archive").unwrap()), None);
    assert_eq!(unwrap_narc(b"plain"), None);
}
