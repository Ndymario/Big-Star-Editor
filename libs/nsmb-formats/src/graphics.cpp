#include "nsmb/formats/graphics.hpp"

#include <algorithm>
#include <array>
#include <cctype>
#include <string>

#include "nsmb/core/endian.hpp"

namespace nsmb::formats {

namespace {

bool has_magic(std::span<const std::uint8_t> data, std::string_view magic) {
    return data.size() >= magic.size() && std::equal(magic.begin(), magic.end(), data.begin(), [](char lhs, std::uint8_t rhs) {
        return static_cast<std::uint8_t>(lhs) == rhs;
    });
}

std::string lowercase(std::string_view value) {
    std::string out(value);
    std::transform(out.begin(), out.end(), out.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return out;
}

bool ends_with(std::string_view value, std::string_view suffix) {
    return value.size() >= suffix.size() && value.substr(value.size() - suffix.size()) == suffix;
}

std::uint8_t expand_5_to_8(std::uint16_t value) {
    return static_cast<std::uint8_t>((value << 3U) | (value >> 2U));
}

} // namespace

GraphicsFormatKind identify_graphics_format(std::span<const std::uint8_t> data, std::string_view path_hint) {
    if (has_magic(data, "RGCN")) {
        return GraphicsFormatKind::NitroCharacterGraphics;
    }
    if (has_magic(data, "RLCN")) {
        return GraphicsFormatKind::NitroPalette;
    }
    if (has_magic(data, "RCSN")) {
        return GraphicsFormatKind::NitroScreenMap;
    }
    if (has_magic(data, "JNCL")) {
        return GraphicsFormatKind::NitroCellLayout;
    }
    if (has_magic(data, "JNCD")) {
        return GraphicsFormatKind::NitroCellAnimation;
    }
    if (has_magic(data, "BTX0")) {
        return GraphicsFormatKind::NitroTexture;
    }
    if (has_magic(data, "BMD0")) {
        return GraphicsFormatKind::NitroModel;
    }

    const auto hint = lowercase(path_hint);
    if (ends_with(hint, ".ncgr") || ends_with(hint, "_ncg.bin")) {
        return GraphicsFormatKind::RawCharacterGraphicsCandidate;
    }
    if (ends_with(hint, ".nclr") || ends_with(hint, "_ncl.bin")) {
        return GraphicsFormatKind::RawPaletteCandidate;
    }
    if (ends_with(hint, ".nscr") || ends_with(hint, "_nsc.bin")) {
        return GraphicsFormatKind::RawScreenMapCandidate;
    }
    if (ends_with(hint, ".nsbtx")) {
        return GraphicsFormatKind::NitroTexture;
    }
    if (ends_with(hint, ".nsbmd")) {
        return GraphicsFormatKind::NitroModel;
    }
    if (ends_with(hint, ".bncl")) {
        return GraphicsFormatKind::NitroCellLayout;
    }
    if (ends_with(hint, ".bncd")) {
        return GraphicsFormatKind::NitroCellAnimation;
    }

    return GraphicsFormatKind::Unknown;
}

std::string_view graphics_format_name(GraphicsFormatKind kind) {
    switch (kind) {
    case GraphicsFormatKind::NitroCharacterGraphics:
        return "Nitro character graphics";
    case GraphicsFormatKind::NitroPalette:
        return "Nitro palette";
    case GraphicsFormatKind::NitroScreenMap:
        return "Nitro screen map";
    case GraphicsFormatKind::NitroCellLayout:
        return "Nitro cell layout";
    case GraphicsFormatKind::NitroCellAnimation:
        return "Nitro cell animation";
    case GraphicsFormatKind::NitroTexture:
        return "Nitro texture";
    case GraphicsFormatKind::NitroModel:
        return "Nitro model";
    case GraphicsFormatKind::RawCharacterGraphicsCandidate:
        return "raw character graphics candidate";
    case GraphicsFormatKind::RawPaletteCandidate:
        return "raw palette candidate";
    case GraphicsFormatKind::RawScreenMapCandidate:
        return "raw screen map candidate";
    case GraphicsFormatKind::Unknown:
        return "unknown";
    }
    return "unknown";
}

core::Result<NitroHeader> parse_nitro_header(std::span<const std::uint8_t> data) {
    if (data.size() < 16) {
        return core::make_error(core::ErrorCode::OutOfBounds, "Nitro format header requires at least 16 bytes");
    }

    core::LittleEndianReader reader(data);
    auto bom = reader.u16(4);
    auto version = reader.u16(6);
    auto file_size = reader.u32(8);
    auto header_size = reader.u16(12);
    auto block_count = reader.u16(14);
    if (!bom || !version || !file_size || !header_size || !block_count) {
        return core::make_error(core::ErrorCode::ParseError, "could not read Nitro header fields");
    }
    if (file_size.value() > data.size()) {
        return core::make_error(core::ErrorCode::ParseError, "Nitro header file size exceeds available data");
    }

    const auto* magic = reinterpret_cast<const char*>(data.data());
    return NitroHeader{std::string_view(magic, 4), bom.value(), version.value(), file_size.value(), header_size.value(), block_count.value()};
}

core::Result<std::vector<Bgr555Color>> decode_bgr555_palette(std::span<const std::uint8_t> data, bool first_color_transparent) {
    if ((data.size() % 2U) != 0U) {
        return core::make_error(core::ErrorCode::ParseError, "BGR555 palette data must contain an even number of bytes");
    }

    core::LittleEndianReader reader(data);
    std::vector<Bgr555Color> colors;
    colors.reserve(data.size() / 2U);
    for (std::size_t offset = 0; offset < data.size(); offset += 2U) {
        auto packed = reader.u16(offset);
        if (!packed) {
            return packed.error();
        }
        const auto value = packed.value();
        colors.push_back(Bgr555Color{
            expand_5_to_8(value & 0x1FU),
            expand_5_to_8((value >> 5U) & 0x1FU),
            expand_5_to_8((value >> 10U) & 0x1FU),
            first_color_transparent && offset == 0,
        });
    }
    return colors;
}

core::Result<std::vector<IndexedTile4Bpp>> decode_4bpp_tiles(std::span<const std::uint8_t> data) {
    constexpr std::size_t tile_size = 32;
    constexpr std::size_t tile_width = 8;
    constexpr std::size_t bytes_per_row = 4;

    if ((data.size() % tile_size) != 0U) {
        return core::make_error(core::ErrorCode::ParseError, "4bpp tile data must be a multiple of 32 bytes");
    }

    std::vector<IndexedTile4Bpp> tiles;
    tiles.reserve(data.size() / tile_size);
    for (std::size_t tile_offset = 0; tile_offset < data.size(); tile_offset += tile_size) {
        IndexedTile4Bpp tile;
        for (std::size_t y = 0; y < tile_width; ++y) {
            for (std::size_t byte_x = 0; byte_x < bytes_per_row; ++byte_x) {
                const auto packed = data[tile_offset + y * bytes_per_row + byte_x];
                const auto pixel = y * tile_width + byte_x * 2U;
                tile.pixels[pixel] = static_cast<std::uint8_t>(packed & 0x0FU);
                tile.pixels[pixel + 1U] = static_cast<std::uint8_t>((packed >> 4U) & 0x0FU);
            }
        }
        tiles.push_back(tile);
    }
    return tiles;
}

} // namespace nsmb::formats
