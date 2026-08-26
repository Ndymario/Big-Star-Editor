#include "nsmb/formats/graphics.hpp"

#include <algorithm>
#include <array>
#include <cctype>
#include <limits>
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

core::Result<std::vector<std::uint8_t>> decompress_nitro_lz(std::span<const std::uint8_t> data) {
    if (data.size() < 4 || (data[0] != 0x10 && data[0] != 0x11)) {
        return core::make_error(core::ErrorCode::ParseError, "Nitro LZ stream has an unsupported header");
    }
    std::size_t input = 4;
    std::size_t output_size = static_cast<std::size_t>(data[1]) | (static_cast<std::size_t>(data[2]) << 8U) |
                              (static_cast<std::size_t>(data[3]) << 16U);
    if (output_size == 0 && data[0] == 0x11) {
        if (data.size() < 8) {
            return core::make_error(core::ErrorCode::OutOfBounds, "extended Nitro LZ header is truncated");
        }
        output_size = static_cast<std::size_t>(data[4]) | (static_cast<std::size_t>(data[5]) << 8U) |
                      (static_cast<std::size_t>(data[6]) << 16U) | (static_cast<std::size_t>(data[7]) << 24U);
        input = 8;
    }
    if (output_size > 256U * 1024U * 1024U) {
        return core::make_error(core::ErrorCode::InvalidArgument, "Nitro LZ output exceeds the safety limit");
    }
    std::vector<std::uint8_t> output;
    output.reserve(output_size);
    while (output.size() < output_size) {
        if (input >= data.size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "Nitro LZ flag byte is truncated");
        }
        const auto flags = data[input++];
        for (int bit = 7; bit >= 0 && output.size() < output_size; --bit) {
            if ((flags & (1U << bit)) == 0) {
                if (input >= data.size()) {
                    return core::make_error(core::ErrorCode::OutOfBounds, "Nitro LZ literal is truncated");
                }
                output.push_back(data[input++]);
                continue;
            }
            if (input + 1 >= data.size()) {
                return core::make_error(core::ErrorCode::OutOfBounds, "Nitro LZ back-reference is truncated");
            }
            std::size_t length = 0;
            std::size_t displacement = 0;
            if (data[0] == 0x10) {
                const auto first = data[input++];
                const auto second = data[input++];
                length = (first >> 4U) + 3U;
                displacement = ((static_cast<std::size_t>(first & 0x0FU) << 8U) | second) + 1U;
            } else {
                const auto first = data[input++];
                const auto high = first >> 4U;
                if (high == 0) {
                    if (input + 1 >= data.size()) return core::make_error(core::ErrorCode::OutOfBounds, "Nitro LZ11 token is truncated");
                    const auto second = data[input++];
                    const auto third = data[input++];
                    length = ((static_cast<std::size_t>(first & 0x0FU) << 4U) | (second >> 4U)) + 0x11U;
                    displacement = ((static_cast<std::size_t>(second & 0x0FU) << 8U) | third) + 1U;
                } else if (high == 1) {
                    if (input + 2 >= data.size()) return core::make_error(core::ErrorCode::OutOfBounds, "Nitro LZ11 long token is truncated");
                    const auto second = data[input++];
                    const auto third = data[input++];
                    const auto fourth = data[input++];
                    length = ((static_cast<std::size_t>(first & 0x0FU) << 12U) | (static_cast<std::size_t>(second) << 4U) |
                              (third >> 4U)) +
                             0x111U;
                    displacement = ((static_cast<std::size_t>(third & 0x0FU) << 8U) | fourth) + 1U;
                } else {
                    const auto second = data[input++];
                    length = high + 1U;
                    displacement = ((static_cast<std::size_t>(first & 0x0FU) << 8U) | second) + 1U;
                }
            }
            if (displacement == 0 || displacement > output.size()) {
                return core::make_error(core::ErrorCode::ParseError, "Nitro LZ back-reference precedes the output buffer");
            }
            for (std::size_t i = 0; i < length && output.size() < output_size; ++i) {
                output.push_back(output[output.size() - displacement]);
            }
        }
    }
    return output;
}

core::Result<std::vector<std::uint8_t>> decompress_blz_overlay(std::span<const std::uint8_t> data) {
    if (data.size() < 8) {
        return core::make_error(core::ErrorCode::OutOfBounds, "BLZ overlay footer is truncated");
    }
    const auto footer = data.size() - 8U;
    const std::uint32_t packed = static_cast<std::uint32_t>(data[footer]) | (static_cast<std::uint32_t>(data[footer + 1]) << 8U) |
                                 (static_cast<std::uint32_t>(data[footer + 2]) << 16U) |
                                 (static_cast<std::uint32_t>(data[footer + 3]) << 24U);
    const std::uint32_t extra = static_cast<std::uint32_t>(data[footer + 4]) |
                                (static_cast<std::uint32_t>(data[footer + 5]) << 8U) |
                                (static_cast<std::uint32_t>(data[footer + 6]) << 16U) |
                                (static_cast<std::uint32_t>(data[footer + 7]) << 24U);
    if (extra == 0) {
        return std::vector<std::uint8_t>(data.begin(), data.end());
    }
    const auto header_size = packed >> 24U;
    const auto compressed_size = packed & 0x00FFFFFFU;
    if (header_size < 8 || compressed_size < header_size || compressed_size > data.size() || extra > 256U * 1024U * 1024U) {
        return std::vector<std::uint8_t>(data.begin(), data.end());
    }
    std::vector<std::uint8_t> output(data.begin(), data.end());
    output.resize(data.size() + extra);
    std::size_t source = data.size() - header_size;
    const auto source_limit = data.size() - compressed_size;
    std::size_t destination = output.size();
    while (source > source_limit) {
        const auto flags = output[--source];
        for (int bit = 7; bit >= 0 && source > source_limit; --bit) {
            if ((flags & (1U << bit)) == 0) {
                if (source == 0 || destination == 0) return core::make_error(core::ErrorCode::ParseError, "BLZ literal is invalid");
                output[--destination] = output[--source];
                continue;
            }
            if (source < 2) return core::make_error(core::ErrorCode::OutOfBounds, "BLZ token is truncated");
            const auto high = output[--source];
            const auto low = output[--source];
            const auto displacement = (static_cast<std::size_t>(((high << 8U) | low) & 0x0FFFU)) + 2U;
            const auto length = static_cast<std::size_t>(high >> 4U) + 3U;
            for (std::size_t i = 0; i < length; ++i) {
                if (destination == 0 || destination + displacement >= output.size()) {
                    return core::make_error(core::ErrorCode::ParseError, "BLZ back-reference is invalid");
                }
                output[destination - 1U] = output[destination + displacement];
                --destination;
            }
        }
    }
    return output;
}

core::Result<std::vector<IndexedTile8Bpp>> decode_8bpp_tiles(std::span<const std::uint8_t> data) {
    if (data.size() % 64U != 0) {
        return core::make_error(core::ErrorCode::ParseError, "8bpp tile data must be a multiple of 64 bytes");
    }
    std::vector<IndexedTile8Bpp> tiles(data.size() / 64U);
    for (std::size_t tile = 0; tile < tiles.size(); ++tile) {
        std::copy_n(data.begin() + static_cast<std::ptrdiff_t>(tile * 64U), 64U, tiles[tile].pixels.begin());
    }
    return tiles;
}

core::Result<std::vector<ScreenMapEntry>> decode_screen_map(std::span<const std::uint8_t> data,
                                                            std::uint16_t tile_offset,
                                                            std::uint8_t palette_offset) {
    if (data.size() % 2U != 0) {
        return core::make_error(core::ErrorCode::ParseError, "screen map must contain 16-bit entries");
    }
    core::LittleEndianReader reader(data);
    std::vector<ScreenMapEntry> entries;
    entries.reserve(data.size() / 2U);
    for (std::size_t offset = 0; offset < data.size(); offset += 2U) {
        auto packed = reader.u16(offset);
        if (!packed) return packed.error();
        const auto raw_tile = static_cast<int>(packed.value() & 0x03FFU) - tile_offset;
        const auto raw_palette = static_cast<int>((packed.value() >> 12U) & 0x0FU) - palette_offset;
        entries.push_back({static_cast<std::uint16_t>(std::max(0, raw_tile)),
                           static_cast<std::uint8_t>(std::max(0, raw_palette)),
                           (packed.value() & 0x0400U) != 0,
                           (packed.value() & 0x0800U) != 0});
    }
    return entries;
}

core::Result<std::vector<Map16Tile>> decode_map16(std::span<const std::uint8_t> data,
                                                  std::uint16_t tile_offset,
                                                  std::uint8_t palette_offset) {
    if (data.size() % 8U != 0) {
        return core::make_error(core::ErrorCode::ParseError, "Map16 data must be a multiple of 8 bytes");
    }
    auto entries = decode_screen_map(data, tile_offset, palette_offset);
    if (!entries) return entries.error();
    std::vector<Map16Tile> result(entries.value().size() / 4U);
    for (std::size_t i = 0; i < result.size(); ++i) {
        std::copy_n(entries.value().begin() + static_cast<std::ptrdiff_t>(i * 4U), 4U, result[i].tiles.begin());
    }
    return result;
}

core::Result<RgbaImage> render_screen_map(std::span<const ScreenMapEntry> map,
                                          std::size_t width_tiles,
                                          std::span<const IndexedTile8Bpp> tiles,
                                          std::span<const Bgr555Color> palette) {
    if (width_tiles == 0 || map.size() % width_tiles != 0) {
        return core::make_error(core::ErrorCode::InvalidArgument, "screen map width does not divide its entries");
    }
    RgbaImage image{width_tiles * 8U, (map.size() / width_tiles) * 8U, {}};
    image.pixels.assign(image.width * image.height * 4U, 0);
    for (std::size_t entry_index = 0; entry_index < map.size(); ++entry_index) {
        const auto& entry = map[entry_index];
        if (entry.tile >= tiles.size()) continue;
        for (std::size_t y = 0; y < 8; ++y) {
            for (std::size_t x = 0; x < 8; ++x) {
                const auto source_x = entry.horizontal_flip ? 7U - x : x;
                const auto source_y = entry.vertical_flip ? 7U - y : y;
                const auto color_index = static_cast<std::size_t>(tiles[entry.tile].pixels[source_y * 8U + source_x]) +
                                         static_cast<std::size_t>(entry.palette) * 256U;
                if (color_index >= palette.size()) continue;
                const auto& color = palette[color_index];
                const auto target_x = (entry_index % width_tiles) * 8U + x;
                const auto target_y = (entry_index / width_tiles) * 8U + y;
                const auto pixel = (target_y * image.width + target_x) * 4U;
                image.pixels[pixel] = color.red;
                image.pixels[pixel + 1U] = color.green;
                image.pixels[pixel + 2U] = color.blue;
                image.pixels[pixel + 3U] = color.transparent ? 0 : 255;
            }
        }
    }
    return image;
}

core::Result<std::vector<ObjectLayout>> decode_object_layouts(std::span<const std::uint8_t> definitions,
                                                               std::span<const std::uint8_t> index,
                                                               std::uint8_t tile_page_offset) {
    if (index.size() % 4U != 0) {
        return core::make_error(core::ErrorCode::ParseError, "object layout index must use four-byte entries");
    }
    core::LittleEndianReader reader(index);
    std::vector<ObjectLayout> objects;
    objects.reserve(index.size() / 4U);
    for (std::size_t object = 0; object < index.size() / 4U; ++object) {
        auto offset = reader.u16(object * 4U);
        if (!offset || offset.value() >= definitions.size()) {
            return core::make_error(core::ErrorCode::OutOfBounds, "object layout offset is outside its definition file");
        }
        ObjectLayout layout;
        layout.rows.emplace_back();
        std::size_t cursor = offset.value();
        bool terminated = false;
        while (cursor < definitions.size()) {
            const auto control = definitions[cursor++];
            if (control == 0xFF) {
                terminated = true;
                break;
            }
            if (control == 0xFE) {
                if (!layout.rows.back().empty()) layout.rows.emplace_back();
                continue;
            }
            if ((control & 0x80U) != 0) {
                if (layout.rows.size() == 1U && layout.rows.front().empty()) {
                    layout.slope = true;
                    layout.slope_left = (control & 1U) != 0;
                    layout.slope_down = (control & 2U) != 0;
                }
                continue;
            }
            if (cursor + 1 >= definitions.size()) {
                return core::make_error(core::ErrorCode::OutOfBounds, "object layout tile is truncated");
            }
            const auto low = definitions[cursor++];
            const auto high = definitions[cursor++];
            std::int32_t tile = low == 0 && high == 0 ? -1 : static_cast<std::int32_t>(low) +
                                                                    ((static_cast<std::int32_t>(high) - tile_page_offset) << 8);
            if ((control & 0x40U) != 0) tile += 768;
            layout.rows.back().push_back({tile, control});
        }
        if (!terminated) {
            return core::make_error(core::ErrorCode::OutOfBounds, "object layout definition is missing its terminator");
        }
        while (!layout.rows.empty() && layout.rows.back().empty()) layout.rows.pop_back();
        objects.push_back(std::move(layout));
    }
    return objects;
}

core::Result<std::vector<std::int32_t>> render_object_layout(const ObjectLayout& object,
                                                             std::size_t width,
                                                             std::size_t height) {
    if (width == 0 || height == 0 || object.rows.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "object layout dimensions and rows must be nonzero");
    }
    std::vector<std::int32_t> output(width * height, -1);
    if (object.slope) {
        const auto& row = object.rows.front();
        if (row.empty()) return output;
        const auto steps = std::min(width, height);
        for (std::size_t step = 0; step < steps; ++step) {
            const auto x = object.slope_left ? width - 1U - step : step;
            const auto y = object.slope_down ? step : height - 1U - step;
            output[y * width + x] = row[step % row.size()].tile;
        }
        return output;
    }
    auto choose = [](const std::vector<ObjectLayoutTile>& values, std::size_t position, std::size_t extent) -> const ObjectLayoutTile& {
        auto first_repeat = std::find_if(values.begin(), values.end(), [](const auto& tile) { return (tile.control & 1U) != 0; });
        if (first_repeat == values.end()) return values[position % values.size()];
        auto after_repeat = std::find_if(first_repeat, values.end(), [](const auto& tile) { return (tile.control & 1U) == 0; });
        const auto before = static_cast<std::size_t>(first_repeat - values.begin());
        const auto repeat = static_cast<std::size_t>(after_repeat - first_repeat);
        const auto after = static_cast<std::size_t>(values.end() - after_repeat);
        if (position < before) return values[position];
        if (position >= extent - std::min(extent, after)) return values[values.size() - (extent - position)];
        return *(first_repeat + static_cast<std::ptrdiff_t>((position - before) % std::max<std::size_t>(1, repeat)));
    };
    std::vector<const std::vector<ObjectLayoutTile>*> before_rows;
    std::vector<const std::vector<ObjectLayoutTile>*> repeat_rows;
    std::vector<const std::vector<ObjectLayoutTile>*> after_rows;
    bool repeat_found = false;
    for (const auto& row : object.rows) {
        if (row.empty()) continue;
        if ((row.front().control & 2U) != 0) {
            repeat_found = true;
            repeat_rows.push_back(&row);
        } else {
            (repeat_found ? after_rows : before_rows).push_back(&row);
        }
    }
    if (before_rows.empty() && repeat_rows.empty()) {
        return core::make_error(core::ErrorCode::ParseError, "object layout has no renderable rows");
    }
    for (std::size_t y = 0; y < height; ++y) {
        const std::vector<ObjectLayoutTile>* row = nullptr;
        if (repeat_rows.empty()) row = before_rows[y % before_rows.size()];
        else if (y < before_rows.size()) row = before_rows[y];
        else if (y >= height - std::min(height, after_rows.size())) row = after_rows[after_rows.size() - (height - y)];
        else row = repeat_rows[(y - before_rows.size()) % repeat_rows.size()];
        for (std::size_t x = 0; x < width; ++x) output[y * width + x] = choose(*row, x, width).tile;
    }
    return output;
}

} // namespace nsmb::formats
