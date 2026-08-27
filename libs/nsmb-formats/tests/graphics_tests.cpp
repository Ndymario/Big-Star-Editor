#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/identify.hpp"
#include "nsmb/formats/level.hpp"
#include "nsmb/formats/narc.hpp"

#include <cassert>
#include <cstddef>
#include <cstdint>
#include <vector>

namespace {

void put_u16(std::vector<std::uint8_t>& data, std::size_t offset, std::uint16_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFFU);
    data[offset + 1U] = static_cast<std::uint8_t>((value >> 8U) & 0xFFU);
}

void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFFU);
    data[offset + 1U] = static_cast<std::uint8_t>((value >> 8U) & 0xFFU);
    data[offset + 2U] = static_cast<std::uint8_t>((value >> 16U) & 0xFFU);
    data[offset + 3U] = static_cast<std::uint8_t>((value >> 24U) & 0xFFU);
}

std::size_t append_block(std::vector<std::uint8_t>& course, std::size_t block_index, std::vector<std::uint8_t> block) {
    const auto offset = course.size();
    put_u32(course, block_index * 8U, static_cast<std::uint32_t>(offset));
    put_u32(course, block_index * 8U + 4U, static_cast<std::uint32_t>(block.size()));
    course.insert(course.end(), block.begin(), block.end());
    return offset;
}

} // namespace

int main() {
    using namespace nsmb::formats;

    std::vector<std::uint8_t> ncgr_header = {'R', 'G', 'C', 'N', 0xff, 0xfe, 0, 1, 16, 0, 0, 0, 16, 0, 1, 0};
    auto header = parse_nitro_header(ncgr_header);
    assert(header);
    assert(header.value().magic == "RGCN");
    assert(identify_graphics_format(ncgr_header, {}) == GraphicsFormatKind::NitroCharacterGraphics);
    assert(identify_format(ncgr_header, {}) == FormatKind::Graphics);

    std::vector<std::uint8_t> palette = {0x00, 0x00, 0x1f, 0x00, 0xe0, 0x03, 0x00, 0x7c};
    auto colors = decode_bgr555_palette(palette);
    assert(colors);
    assert(colors.value().size() == 4);
    assert(colors.value()[0].transparent);
    assert(colors.value()[1].red == 255);
    assert(colors.value()[2].green == 255);
    assert(colors.value()[3].blue == 255);

    std::vector<std::uint8_t> raw_tile(32);
    raw_tile[0] = 0x21;
    auto tiles = decode_4bpp_tiles(raw_tile);
    assert(tiles);
    assert(tiles.value().size() == 1);
    assert(tiles.value()[0].pixels[0] == 1);
    assert(tiles.value()[0].pixels[1] == 2);
    assert(!decode_4bpp_tiles({raw_tile.data(), 31}));

    std::vector<std::uint8_t> raw_8bpp(64);
    raw_8bpp[0] = 3;
    auto tiles_8bpp = decode_8bpp_tiles(raw_8bpp);
    assert(tiles_8bpp && tiles_8bpp.value()[0].pixels[0] == 3);
    assert(!decode_8bpp_tiles({raw_8bpp.data(), 63}));

    const std::vector<std::uint8_t> literal_lz10 = {0x10, 3, 0, 0, 0, 'a', 'b', 'c'};
    auto lz10 = decompress_nitro_lz(literal_lz10);
    assert(lz10 && std::string(lz10.value().begin(), lz10.value().end()) == "abc");
    assert(!decompress_nitro_lz({literal_lz10.data(), 6}));
    const std::vector<std::uint8_t> literal_lz11 = {0x11, 3, 0, 0, 0, 'x', 'y', 'z'};
    auto lz11 = decompress_nitro_lz(literal_lz11);
    assert(lz11 && std::string(lz11.value().begin(), lz11.value().end()) == "xyz");
    const std::vector<std::uint8_t> repeated = {'a','b','c','a','b','c'};
    auto packed_lz10 = compress_nitro_lz10(repeated);
    assert(packed_lz10 && packed_lz10.value() == std::vector<std::uint8_t>({0x10,6,0,0,0x10,'a','b','c',0,2}));
    assert(decompress_nitro_lz(packed_lz10.value()).value() == repeated);
    auto packed_lz11 = compress_nitro_lz11(repeated);
    assert(packed_lz11 && packed_lz11.value() == std::vector<std::uint8_t>({0x11,6,0,0,0x10,'a','b','c',0x20,2}));
    assert(decompress_nitro_lz(packed_lz11.value()).value() == repeated);
    std::vector<std::uint8_t> long_repeated(70000); for (std::size_t i=0;i<long_repeated.size();++i) long_repeated[i]=static_cast<std::uint8_t>(i%7);
    auto packed_long_lz11=compress_nitro_lz11(long_repeated);assert(packed_long_lz11&&decompress_nitro_lz(packed_long_lz11.value()).value()==long_repeated);
    const std::vector<std::uint8_t> too_large_for_lz10(0x1000000U);assert(!compress_nitro_lz10(too_large_for_lz10));

    const std::vector<std::uint8_t> literal_yaz0 = {'Y','a','z','0',0,0,0,3,0,0,0,0,0,0,0,0,0xE0,'x','y','z'};
    auto yaz_literal=decompress_yaz0(literal_yaz0);assert(yaz_literal&&std::string(yaz_literal.value().begin(),yaz_literal.value().end())=="xyz");
    auto yaz0=compress_yaz0(long_repeated);assert(yaz0&&decompress_yaz0(yaz0.value()).value()==long_repeated);
    assert(!decompress_yaz0(std::span<const std::uint8_t>(literal_yaz0.data(),18)));
    const std::vector<std::uint8_t> uncompressed_overlay(8);
    auto blz = decompress_blz_overlay(uncompressed_overlay);
    assert(blz && blz.value() == uncompressed_overlay);
    std::vector<std::uint8_t> overlay(4096);for(std::size_t i=0;i<overlay.size();++i)overlay[i]=static_cast<std::uint8_t>("the quick brown fox"[i%19]);
    auto packed_blz=compress_blz_overlay(overlay);assert(packed_blz&&!packed_blz.value().empty()&&packed_blz.value().size()<overlay.size()&&packed_blz.value().size()%4==0);
    assert(decompress_blz_overlay(packed_blz.value()).value()==overlay);

    std::vector<std::uint8_t> map_data(8);
    put_u16(map_data, 0, 0);
    put_u16(map_data, 2, 0x0400);
    put_u16(map_data, 4, 0x0800);
    put_u16(map_data, 6, 0x1000);
    auto map16 = decode_map16(map_data);
    assert(map16 && map16.value().size() == 1);
    assert(map16.value()[0].tiles[1].horizontal_flip);
    assert(map16.value()[0].tiles[2].vertical_flip);
    assert(map16.value()[0].tiles[3].palette == 1);

    std::vector<IndexedTile8Bpp> render_tiles(1);
    render_tiles[0].pixels[0] = 1;
    render_tiles[0].pixels[7] = 2;
    std::vector<Bgr555Color> render_palette(256);
    render_palette[1] = {255, 0, 0, false};
    render_palette[2] = {0, 255, 0, false};
    const std::array<ScreenMapEntry, 1> flipped{{{0, 0, true, false}}};
    auto rendered_map = render_screen_map(flipped, 1, render_tiles, render_palette);
    assert(rendered_map && rendered_map.value().pixels[0] == 0 && rendered_map.value().pixels[1] == 255);

    std::vector<std::uint8_t> object_index(4);
    std::vector<std::uint8_t> object_definition = {0, 5, 0, 1, 6, 0, 0xFE, 2, 7, 0, 0xFF};
    auto layouts = decode_object_layouts(object_definition, object_index);
    assert(layouts && layouts.value().size() == 1);
    auto rendered_layout = render_object_layout(layouts.value()[0], 4, 3);
    assert(rendered_layout && rendered_layout.value().size() == 12);
    assert(rendered_layout.value()[0] == 5);
    assert(rendered_layout.value()[1] == 6 && rendered_layout.value()[3] == 6);
    assert(rendered_layout.value()[4] == 7 && rendered_layout.value()[11] == 7);
    object_definition.pop_back();
    assert(!decode_object_layouts(object_definition, object_index));

    assert(nsmb::core::Fx32::from_raw(6144).value() == 1.5);
    std::vector<std::uint8_t> fnt(8);put_u32(fnt,0,8);put_u16(fnt,4,0);put_u16(fnt,6,1);
    fnt.insert(fnt.end(),{5,'a','.','b','i','n',0});
    NarcArchive archive{fnt,{{1,2,3},{4,5,6,7}}};
    auto narc_bytes=write_narc(archive);assert(narc_bytes&&is_narc(narc_bytes.value()));
    auto narc=parse_narc(narc_bytes.value());assert(narc&&narc.value().files==archive.files);
    auto member=narc.value().find("a.bin");assert(member&&member.value()==0);
    assert(write_narc(narc.value()).value()==narc_bytes.value());

    std::vector<std::uint8_t> course(nsmb_level_block_count * 8U);
    for (std::size_t i = 0; i < nsmb_level_block_count; ++i) {
        put_u32(course, i * 8U, static_cast<std::uint32_t>(course.size()));
    }

    std::vector<std::uint8_t> block0(0x20);
    put_u16(block0,0x06,7);put_u16(block0,0x0C,42);put_u16(block0,0x12,9);
    append_block(course, 0, block0);

    std::vector<std::uint8_t> block2(20);
    put_u16(block2,2,11);
    append_block(course, 2, block2);

    std::vector<std::uint8_t> entrance(20);
    put_u16(entrance, 0, 100);
    put_u16(entrance, 2, 120);
    entrance[8] = 3;
    entrance[14] = 2;
    append_block(course, 5, entrance);

    std::vector<std::uint8_t> sprite(16,0xFF);
    put_u16(sprite, 0, 55);
    put_u16(sprite, 2, 12);
    put_u16(sprite, 4, 34);
    sprite[6] = 0xAA;
    append_block(course, 6, sprite);

    std::vector<std::uint8_t> view(16);
    put_u16(view, 0, 16);
    put_u16(view, 2, 32);
    put_u16(view, 4, 256);
    put_u16(view, 6, 192);
    view[8] = 1;
    view[9] = 2;
    append_block(course, 7, view);

    std::vector<std::uint8_t> zone(12);
    put_u16(zone, 0, 64);
    put_u16(zone, 2, 96);
    put_u16(zone, 4, 320);
    put_u16(zone, 6, 224);
    zone[8] = 4;
    append_block(course, 8, zone);

    std::vector<std::uint8_t> path_header(8);
    put_u16(path_header, 0, 6);
    put_u16(path_header, 2, 0);
    put_u16(path_header, 4, 1);
    append_block(course, 10, path_header);
    std::vector<std::uint8_t> path_node(16);
    put_u16(path_node, 0, 144);
    put_u16(path_node, 2, 160);
    put_u16(path_node, 4, 99);
    append_block(course, 12, path_node);

    auto parsed_course = parse_nsmb_level_course(course);
    assert(parsed_course);
    assert(parsed_course.value().header.tileset_theme[0] == 42);
    assert(parsed_course.value().header.background_theme[0] == 7);
    assert(parsed_course.value().header.foreground_theme[0] == 9);
    assert(parsed_course.value().backgrounds[0].screen_id == 11);
    assert(parsed_course.value().sprites.size() == 1);
    assert(parsed_course.value().sprites[0].id == 55);
    assert(parsed_course.value().sprites[0].events[0] == 0xAA);
    assert(parsed_course.value().entrances.size() == 1);
    assert(parsed_course.value().entrances[0].id == 3);
    assert(parsed_course.value().views.size() == 1);
    assert(parsed_course.value().views[0].width == 256);
    assert(parsed_course.value().zones.size() == 1);
    assert(parsed_course.value().zones[0].id == 4);
    assert(parsed_course.value().paths.size() == 1);
    assert(parsed_course.value().paths[0].id == 6);
    assert(parsed_course.value().paths[0].nodes[0].x == 144);
    assert(parsed_course.value().paths[0].nodes[0].speed.raw() == 99);

    std::vector<std::uint8_t> bgdat(10);
    put_u16(bgdat, 0, static_cast<std::uint16_t>(0x2000U | 0x0123U));
    put_u16(bgdat, 2, static_cast<std::uint16_t>(-4));
    put_u16(bgdat, 4, 8);
    put_u16(bgdat, 6, 12);
    put_u16(bgdat, 8, 16);
    auto objects = parse_nsmb_bgdat_objects(bgdat);
    assert(objects);
    assert(objects.value().size() == 1);
    assert(objects.value()[0].object_id == 0x0123);
    assert(objects.value()[0].tileset_slot == 2);
    assert(objects.value()[0].x == -4);
    assert(objects.value()[0].width == 12);
    bgdat.push_back(0xFF);
    bgdat.push_back(0xFF);
    assert(parse_nsmb_bgdat_objects(bgdat));
    bgdat.push_back(0);
    assert(!parse_nsmb_bgdat_objects(bgdat));

    const std::array palette_source{Bgr555Color{255, 0, 0, false}, Bgr555Color{0, 255, 0, false}};
    auto encoded_palette = encode_bgr555_palette(palette_source);
    assert(encoded_palette && encoded_palette.value().size() == 4);
    auto decoded_palette = decode_bgr555_palette(encoded_palette.value(), false);
    assert(decoded_palette && decoded_palette.value()[0].red == 255 && decoded_palette.value()[1].green == 255);

    IndexedTile4Bpp tile4;
    tile4.pixels[0] = 3; tile4.pixels[1] = 12;
    auto packed4 = encode_4bpp_tiles(std::span{&tile4, 1});
    assert(packed4 && packed4.value()[0] == 0xc3);
    assert(decode_4bpp_tiles(packed4.value()).value()[0].pixels == tile4.pixels);
    tile4.pixels[2] = 16;
    assert(!encode_4bpp_tiles(std::span{&tile4, 1}));

    const std::array screen_source{ScreenMapEntry{7, 2, true, false}, ScreenMapEntry{12, 1, false, true}};
    auto packed_screen = encode_screen_map(screen_source, 20, 3);
    assert(packed_screen);
    auto unpacked_screen = decode_screen_map(packed_screen.value(), 20, 3);
    assert(unpacked_screen && unpacked_screen.value()[0].tile == 7 && unpacked_screen.value()[0].palette == 2);
    assert(unpacked_screen.value()[0].horizontal_flip && unpacked_screen.value()[1].vertical_flip);

    ObjectLayout layout;
    layout.rows = {{{1, 0}, {2, 1}}, {{3, 2}}};
    std::array<std::uint8_t, 4> index_template{0, 0, 9, 8};
    auto encoded_layout = encode_object_layouts(std::span{&layout, 1}, 0, index_template);
    assert(encoded_layout && encoded_layout.value().index[2] == 9 && encoded_layout.value().index[3] == 8);
    auto decoded_layout = decode_object_layouts(encoded_layout.value().definitions, encoded_layout.value().index);
    assert(decoded_layout && decoded_layout.value().size() == 1 && decoded_layout.value()[0].rows.size() == 2);
    assert(decoded_layout.value()[0].rows[0][1].tile == 2 && decoded_layout.value()[0].rows[1][0].control == 2);

    RgbaImage quantize_source{4, 1, {255,0,0,255, 0,255,0,255, 0,0,255,255, 0,0,0,0}};
    auto quantized = quantize_bgr555(quantize_source, 3);
    assert(quantized && quantized.value().palette.size() == 3 && quantized.value().palette.front().transparent);
    assert(quantized.value().pixels[3] == 0);

    assert(identify_graphics_format({}, "UI_O_1P_game_in_d_ncg.bin") == GraphicsFormatKind::RawCharacterGraphicsCandidate);
    assert(identify_graphics_format({}, "enemy/w_cannon.nsbmd") == GraphicsFormatKind::NitroModel);
    return 0;
}
