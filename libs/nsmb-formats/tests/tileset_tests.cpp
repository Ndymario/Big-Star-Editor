#include "nsmb/formats/tileset.hpp"

#include <cassert>

using namespace nsmb::formats;

int main() {
    IndexedTile8Bpp character;
    character.pixels[0] = 1;
    const auto raw_graphics = encode_8bpp_tiles(std::span{&character, 1}).value();
    const auto compressed_graphics = compress_nitro_lz10(raw_graphics).value();
    const std::array colors{Bgr555Color{0,0,0,true}, Bgr555Color{255,255,255,false}};
    const auto palette = encode_bgr555_palette(colors).value();
    Map16Tile map_tile;
    const auto map16 = encode_map16(std::span{&map_tile, 1}).value();
    TilesetResourceData resources{
        compressed_graphics,
        palette,
        map16,
        {0,0,0,0xff},
        {0,0,7,9},
        {0x34,0x12,0,0},
    };

    auto parsed = parse_tileset(resources);
    assert(parsed);
    assert(parsed.value().graphics.size() == 1);
    assert(parsed.value().palette.size() == 2);
    assert(parsed.value().map16.size() == 1);
    assert(parsed.value().objects.size() == 1);
    assert(parsed.value().objects[0].rows[0][0].tile == -1);
    assert(parsed.value().behaviors == std::vector<std::uint32_t>{0x1234});

    auto untouched = write_tileset(parsed.value());
    assert(untouched);
    assert(untouched.value().graphics == resources.graphics);
    assert(untouched.value().objects == resources.objects);
    assert(untouched.value().object_index == resources.object_index);

    parsed.value().map16[0].tiles[0] = {0,0,true,true};
    parsed.value().objects[0].rows[0][0] = {1,0};
    parsed.value().behaviors[0] = 0x89abcdefU;
    auto changed = write_tileset(parsed.value());
    assert(changed);
    auto changed_round_trip = parse_tileset(changed.value());
    assert(changed_round_trip);
    assert(changed_round_trip.value().map16[0].tiles[0].horizontal_flip);
    assert(changed_round_trip.value().objects[0].rows[0][0].tile == 1);
    assert(changed_round_trip.value().behaviors[0] == 0x89abcdefU);

    auto bundle = write_tileset_bundle(changed.value());
    assert(bundle);
    auto unbundled = parse_tileset_bundle(bundle.value());
    assert(unbundled);
    assert(unbundled.value().graphics == changed.value().graphics);
    assert(unbundled.value().behaviors == changed.value().behaviors);
    auto broken = bundle.value(); broken[8] = 9;
    assert(!parse_tileset_bundle(broken));
    broken = bundle.value();
    broken[24] = broken[12]; broken[25] = broken[13]; broken[26] = broken[14]; broken[27] = broken[15];
    assert(!parse_tileset_bundle(broken));

    TileBehaviorParts parts{0x10122, 5, 7};
    assert(decode_tile_behavior(encode_tile_behavior(parts)).flags == parts.flags);
    assert(decode_tile_behavior(encode_tile_behavior(parts)).subtype == parts.subtype);
    assert(decode_tile_behavior(encode_tile_behavior(parts)).parameter == parts.parameter);
    assert(tile_behavior_parameter_kind({2,0,0}) == TileBehaviorParameterKind::Coin);
    assert(tile_behavior_parameter_name(TileBehaviorParameterKind::Slope, 10) == "Slope edge");

    RgbaImage png{16, 8, std::vector<std::uint8_t>(16U * 8U * 4U)};
    for (std::size_t i = 0; i < 16U * 8U; ++i) { const bool left = i % 16U < 8U; png.pixels[i*4U] = left ? 255 : 0; png.pixels[i*4U+2U] = left ? 0 : 255; png.pixels[i*4U+3U] = 255; }
    EditableTilemap tilemap;
    assert(import_tilemap_image(tilemap, png));
    assert(tilemap.width_tiles == 2 && tilemap.graphics.size() == 2 && tilemap.screen.size() == 2);
    auto rendered = render_tilemap(tilemap);
    assert(rendered && rendered.value().width == 16 && rendered.value().height == 8);
    auto tilemap_files = write_tilemap(tilemap);
    assert(tilemap_files);
    auto tilemap_round_trip = parse_tilemap(tilemap_files.value(), 2);
    assert(tilemap_round_trip && tilemap_round_trip.value().screen.size() == 2);
    auto imported_tileset = parsed.value();
    assert(import_tileset_graphics(imported_tileset, png));
    assert(imported_tileset.palette.front().transparent);
    assert(imported_tileset.graphics.front().pixels.front() != 0);
    return 0;
}
