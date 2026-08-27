#pragma once

#include <array>
#include <cstdint>
#include <span>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::formats {

struct Bgr555Color {
    std::uint8_t red{};
    std::uint8_t green{};
    std::uint8_t blue{};
    bool transparent{};
    bool operator==(const Bgr555Color&) const = default;
};

struct IndexedTile4Bpp {
    std::array<std::uint8_t, 64> pixels{};
    bool operator==(const IndexedTile4Bpp&) const = default;
};

struct IndexedTile8Bpp {
    std::array<std::uint8_t, 64> pixels{};
    bool operator==(const IndexedTile8Bpp&) const = default;
};

struct ScreenMapEntry {
    std::uint16_t tile{};
    std::uint8_t palette{};
    bool horizontal_flip{};
    bool vertical_flip{};
    bool operator==(const ScreenMapEntry&) const = default;
};

struct Map16Tile {
    std::array<ScreenMapEntry, 4> tiles{};
    bool operator==(const Map16Tile&) const = default;
};

struct RgbaImage {
    std::size_t width{};
    std::size_t height{};
    std::vector<std::uint8_t> pixels;
};

struct IndexedImage {
    std::size_t width{};
    std::size_t height{};
    std::vector<std::uint8_t> pixels;
    std::vector<Bgr555Color> palette;
};

struct EncodedObjectLayouts {
    std::vector<std::uint8_t> definitions;
    std::vector<std::uint8_t> index;
};

struct ObjectLayoutTile {
    std::int32_t tile{-1};
    std::uint8_t control{};
    bool operator==(const ObjectLayoutTile&) const = default;
};

struct ObjectLayout {
    std::vector<std::vector<ObjectLayoutTile>> rows;
    bool slope{};
    bool slope_left{};
    bool slope_down{};
    bool operator==(const ObjectLayout&) const = default;
};

enum class GraphicsFormatKind {
    Unknown,
    NitroCharacterGraphics,
    NitroPalette,
    NitroScreenMap,
    NitroCellLayout,
    NitroCellAnimation,
    NitroTexture,
    NitroModel,
    RawCharacterGraphicsCandidate,
    RawPaletteCandidate,
    RawScreenMapCandidate,
};

struct NitroHeader {
    std::string_view magic;
    std::uint16_t byte_order_mark{};
    std::uint16_t version{};
    std::uint32_t file_size{};
    std::uint16_t header_size{};
    std::uint16_t block_count{};
};

GraphicsFormatKind identify_graphics_format(std::span<const std::uint8_t> data, std::string_view path_hint = {});
std::string_view graphics_format_name(GraphicsFormatKind kind);
core::Result<NitroHeader> parse_nitro_header(std::span<const std::uint8_t> data);
core::Result<std::vector<Bgr555Color>> decode_bgr555_palette(std::span<const std::uint8_t> data, bool first_color_transparent = true);
core::Result<std::vector<std::uint8_t>> encode_bgr555_palette(std::span<const Bgr555Color> colors);
core::Result<std::vector<IndexedTile4Bpp>> decode_4bpp_tiles(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> encode_4bpp_tiles(std::span<const IndexedTile4Bpp> tiles);
core::Result<std::vector<std::uint8_t>> decompress_nitro_lz(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> compress_nitro_lz10(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> compress_nitro_lz11(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> decompress_yaz0(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> compress_yaz0(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> decompress_blz_overlay(std::span<const std::uint8_t> data);
// Returns an empty vector when BLZ would not make the input smaller.
core::Result<std::vector<std::uint8_t>> compress_blz_overlay(std::span<const std::uint8_t> data);
core::Result<std::vector<IndexedTile8Bpp>> decode_8bpp_tiles(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> encode_8bpp_tiles(std::span<const IndexedTile8Bpp> tiles);
core::Result<std::vector<ScreenMapEntry>> decode_screen_map(std::span<const std::uint8_t> data,
                                                            std::uint16_t tile_offset = 0,
                                                            std::uint8_t palette_offset = 0);
core::Result<std::vector<Map16Tile>> decode_map16(std::span<const std::uint8_t> data,
                                                  std::uint16_t tile_offset = 0,
                                                  std::uint8_t palette_offset = 0);
core::Result<std::vector<std::uint8_t>> encode_screen_map(std::span<const ScreenMapEntry> entries,
                                                          std::uint16_t tile_offset = 0,
                                                          std::uint8_t palette_offset = 0);
core::Result<std::vector<std::uint8_t>> encode_map16(std::span<const Map16Tile> tiles,
                                                     std::uint16_t tile_offset = 0,
                                                     std::uint8_t palette_offset = 0);
core::Result<RgbaImage> render_screen_map(std::span<const ScreenMapEntry> map,
                                          std::size_t width_tiles,
                                          std::span<const IndexedTile8Bpp> tiles,
                                          std::span<const Bgr555Color> palette);
core::Result<std::vector<ObjectLayout>> decode_object_layouts(std::span<const std::uint8_t> definitions,
                                                               std::span<const std::uint8_t> index,
                                                               std::uint8_t tile_page_offset = 0);
core::Result<EncodedObjectLayouts> encode_object_layouts(std::span<const ObjectLayout> objects,
                                                          std::uint8_t tile_page_offset = 0,
                                                          std::span<const std::uint8_t> index_template = {});
core::Result<std::vector<std::int32_t>> render_object_layout(const ObjectLayout& object,
                                                             std::size_t width,
                                                             std::size_t height);
core::Result<IndexedImage> quantize_bgr555(const RgbaImage& image,
                                           std::size_t maximum_colors = 256,
                                           std::uint8_t alpha_threshold = 128);

} // namespace nsmb::formats
