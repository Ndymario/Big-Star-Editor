#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/identify.hpp"
#include "nsmb/formats/level.hpp"

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

    std::vector<std::uint8_t> course(nsmb_level_block_count * 8U);
    for (std::size_t i = 0; i < nsmb_level_block_count; ++i) {
        put_u32(course, i * 8U, static_cast<std::uint32_t>(course.size()));
    }

    std::vector<std::uint8_t> block0(0x13);
    block0[0x06] = 7;
    block0[0x0C] = 42;
    block0[0x12] = 9;
    append_block(course, 0, block0);

    std::vector<std::uint8_t> block2(3);
    block2[2] = 11;
    append_block(course, 2, block2);

    std::vector<std::uint8_t> entrance(20);
    put_u16(entrance, 0, 100);
    put_u16(entrance, 2, 120);
    entrance[8] = 3;
    entrance[14] = 2;
    append_block(course, 5, entrance);

    std::vector<std::uint8_t> sprite(14);
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
    assert(parsed_course.value().tileset_id == 42);
    assert(parsed_course.value().background_graphics_id == 7);
    assert(parsed_course.value().foreground_graphics_id == 9);
    assert(parsed_course.value().background_screen_id == 11);
    assert(parsed_course.value().sprites.size() == 1);
    assert(parsed_course.value().sprites[0].type == 55);
    assert(parsed_course.value().sprites[0].data[0] == 0xAA);
    assert(parsed_course.value().entrances.size() == 1);
    assert(parsed_course.value().entrances[0].number == 3);
    assert(parsed_course.value().views.size() == 1);
    assert(parsed_course.value().views[0].width == 256);
    assert(parsed_course.value().zones.size() == 1);
    assert(parsed_course.value().zones[0].number == 4);
    assert(parsed_course.value().paths.size() == 1);
    assert(parsed_course.value().paths[0].id == 6);
    assert(parsed_course.value().paths[0].nodes[0].x == 144);
    assert(parsed_course.value().paths[0].nodes[0].values[0] == 99);

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

    assert(identify_graphics_format({}, "UI_O_1P_game_in_d_ncg.bin") == GraphicsFormatKind::RawCharacterGraphicsCandidate);
    assert(identify_graphics_format({}, "enemy/w_cannon.nsbmd") == GraphicsFormatKind::NitroModel);
    return 0;
}
