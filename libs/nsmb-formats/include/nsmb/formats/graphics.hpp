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
};

struct IndexedTile4Bpp {
    std::array<std::uint8_t, 64> pixels{};
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
core::Result<std::vector<IndexedTile4Bpp>> decode_4bpp_tiles(std::span<const std::uint8_t> data);

} // namespace nsmb::formats
