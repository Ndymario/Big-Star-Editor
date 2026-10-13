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

    // A slope fills one diagonal and leaves the other cells unwritten; an empty definition is block 0, still written.
    let slope = ObjectLayout { rows: vec![vec![ObjectLayoutTile::new(4, 0), ObjectLayoutTile::default()]], slope: true, ..ObjectLayout::default() };
    let cells = render_object_layout(&slope, 2, 2).unwrap();
    assert_eq!(cells, [OBJECT_LAYOUT_UNWRITTEN, OBJECT_LAYOUT_EMPTY_TILE, 4, OBJECT_LAYOUT_UNWRITTEN]);
    assert!(render_object_layout(&slope, 0, 2).is_err());
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
