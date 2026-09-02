#include "nsmb/formats/graphics.hpp"

#include <algorithm>
#include <array>
#include <cctype>
#include <cstddef>
#include <limits>
#include <map>
#include <numeric>
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

struct LzMatch {
    std::size_t length{};
    std::size_t displacement{};
};

class ForwardMatcher {
public:
    explicit ForwardMatcher(std::span<const std::uint8_t> data)
        : data_(data), head_(65536, -1), previous_(data.size(), -1) {}

    void insert(std::size_t position) {
        if (position + 2U >= data_.size()) return;
        const auto bucket = key(position);
        previous_[position] = head_[bucket];
        head_[bucket] = static_cast<std::ptrdiff_t>(position);
    }

    LzMatch find(std::size_t position, std::size_t maximum) const {
        if (position + 2U >= data_.size()) return {};
        const auto lower = position > 4096U ? position - 4096U : 0U;
        LzMatch best;
        for (auto candidate = head_[key(position)]; candidate >= static_cast<std::ptrdiff_t>(lower);
             candidate = previous_[static_cast<std::size_t>(candidate)]) {
            const auto start = static_cast<std::size_t>(candidate);
            std::size_t length = 0;
            const auto limit = std::min(maximum, data_.size() - position);
            while (length < limit && data_[start + length] == data_[position + length]) ++length;
            if (length > best.length) best = {length, position - start};
            if (best.length == limit) break;
        }
        if (best.length < 3U) return {};
        return best;
    }

private:
    std::size_t key(std::size_t position) const {
        return (static_cast<std::size_t>(data_[position]) * 251U * 251U +
                static_cast<std::size_t>(data_[position + 1U]) * 251U + data_[position + 2U]) & 0xFFFFU;
    }

    std::span<const std::uint8_t> data_;
    std::vector<std::ptrdiff_t> head_;
    std::vector<std::ptrdiff_t> previous_;
};

core::Result<std::vector<std::uint8_t>> compress_nitro_lz(std::span<const std::uint8_t> data, bool lz11) {
    if ((!lz11 && data.size() > 0xFFFFFFU) || data.size() > std::numeric_limits<std::uint32_t>::max()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "input is too large for the selected Nitro LZ header");
    }
    std::vector<std::uint8_t> output;
    output.reserve(data.size() + data.size() / 8U + 8U);
    output.push_back(lz11 ? 0x11 : 0x10);
    if (lz11 && (data.empty() || data.size() > 0xFFFFFFU)) {
        output.insert(output.end(), 3, 0);
        const auto size = static_cast<std::uint32_t>(data.size());
        for (int shift = 0; shift < 32; shift += 8) output.push_back(static_cast<std::uint8_t>(size >> shift));
    } else {
        for (int shift = 0; shift < 24; shift += 8) output.push_back(static_cast<std::uint8_t>(data.size() >> shift));
    }

    ForwardMatcher matcher(data);
    std::size_t position = 0;
    while (position < data.size()) {
        const auto flag_offset = output.size(); output.push_back(0);
        for (int bit = 7; bit >= 0 && position < data.size(); --bit) {
            auto match = matcher.find(position, lz11 ? 0x10110U : 18U);
            const auto consumed = match.length ? match.length : 1U;
            if (!match.length) {
                output.push_back(data[position]);
            } else {
                output[flag_offset] |= static_cast<std::uint8_t>(1U << bit);
                const auto displacement = match.displacement - 1U;
                if (!lz11) {
                    output.push_back(static_cast<std::uint8_t>(((match.length - 3U) << 4U) | (displacement >> 8U)));
                    output.push_back(static_cast<std::uint8_t>(displacement));
                } else if (match.length <= 16U) {
                    output.push_back(static_cast<std::uint8_t>(((match.length - 1U) << 4U) | (displacement >> 8U)));
                    output.push_back(static_cast<std::uint8_t>(displacement));
                } else if (match.length <= 272U) {
                    const auto length = match.length - 0x11U;
                    output.push_back(static_cast<std::uint8_t>(length >> 4U));
                    output.push_back(static_cast<std::uint8_t>((length << 4U) | (displacement >> 8U)));
                    output.push_back(static_cast<std::uint8_t>(displacement));
                } else {
                    const auto length = match.length - 0x111U;
                    output.push_back(static_cast<std::uint8_t>(0x10U | (length >> 12U)));
                    output.push_back(static_cast<std::uint8_t>(length >> 4U));
                    output.push_back(static_cast<std::uint8_t>((length << 4U) | (displacement >> 8U)));
                    output.push_back(static_cast<std::uint8_t>(displacement));
                }
            }
            for (std::size_t i = 0; i < consumed; ++i) matcher.insert(position + i);
            position += consumed;
        }
    }
    return output;
}

std::size_t backward_match(std::span<const std::uint8_t> data, std::size_t remaining, std::size_t& match_position) {
    const auto maximum = std::min<std::size_t>(remaining, 18U);
    const auto available = std::min<std::size_t>(data.size() - remaining, 4098U);
    std::size_t best = 0;
    for (std::size_t i = 0; i < available; ++i) {
        if (data[remaining + i] != data[remaining - 1U]) continue;
        const auto limit = std::min(maximum, i + 1U); std::size_t length = 0;
        while (length < limit && data[remaining - 1U - length] == data[remaining + i - length]) ++length;
        if (length > best) { best = length; match_position = i; }
    }
    return best;
}

void put_u32_le(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    for (int shift = 0; shift < 32; shift += 8) data[offset++] = static_cast<std::uint8_t>(value >> shift);
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
    // BNCL (JNCL) and BNCD (JNCD) used to be claimed here as NCER and NANR. They are not
    // NitroSystem formats at all -- no NitroSDK or NitroSystem header defines them. BNCL
    // holds OAM positions and BNCD holds OAM plus graphics, both parsed by identify_ui_resource
    // and the UI resource editor. parse_ncer and parse_nanr check their magic, so the graphics
    // editor already refused them; the claim only ever produced a wrong name and format kind.
    if (has_magic(data, "RECN")) {
        return GraphicsFormatKind::NitroCellLayout;
    }
    if (has_magic(data, "RNAN")) {
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

void mark_palette_blocks_transparent(std::span<Bgr555Color> palette, std::size_t block) {
    if (!block) return;
    for (std::size_t offset = 0; offset < palette.size(); offset += block) palette[offset].transparent = true;
}

core::Result<std::vector<std::uint8_t>> encode_bgr555_palette(std::span<const Bgr555Color> colors) {
    if (colors.size() > std::numeric_limits<std::size_t>::max() / 2U) return core::make_error(core::ErrorCode::OutOfBounds, "palette is too large");
    std::vector<std::uint8_t> output(colors.size() * 2U);
    const auto shrink = [](std::uint8_t value) { return static_cast<std::uint16_t>((static_cast<unsigned>(value) * 31U + 127U) / 255U); };
    for (std::size_t i = 0; i < colors.size(); ++i) {
        const auto packed = static_cast<std::uint16_t>(shrink(colors[i].red) | (shrink(colors[i].green) << 5U) | (shrink(colors[i].blue) << 10U));
        output[i * 2U] = static_cast<std::uint8_t>(packed);
        output[i * 2U + 1U] = static_cast<std::uint8_t>(packed >> 8U);
    }
    return output;
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

core::Result<std::vector<std::uint8_t>> encode_4bpp_tiles(std::span<const IndexedTile4Bpp> tiles) {
    std::vector<std::uint8_t> output(tiles.size() * 32U);
    for (std::size_t tile = 0; tile < tiles.size(); ++tile) for (std::size_t pixel = 0; pixel < 64U; pixel += 2U) {
        if (tiles[tile].pixels[pixel] > 15U || tiles[tile].pixels[pixel + 1U] > 15U)
            return core::make_error(core::ErrorCode::InvalidArgument, "4bpp tile contains a palette index above 15");
        output[tile * 32U + pixel / 2U] = static_cast<std::uint8_t>(tiles[tile].pixels[pixel] | (tiles[tile].pixels[pixel + 1U] << 4U));
    }
    return output;
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

core::Result<std::vector<std::uint8_t>> compress_nitro_lz10(std::span<const std::uint8_t> data) {
    return compress_nitro_lz(data, false);
}

core::Result<std::vector<std::uint8_t>> compress_nitro_lz11(std::span<const std::uint8_t> data) {
    return compress_nitro_lz(data, true);
}

core::Result<std::vector<std::uint8_t>> decompress_yaz0(std::span<const std::uint8_t> data) {
    if (data.size() < 16U || !has_magic(data, "Yaz0")) return core::make_error(core::ErrorCode::ParseError, "Yaz0 header is missing or truncated");
    const auto output_size = (static_cast<std::size_t>(data[4]) << 24U) | (static_cast<std::size_t>(data[5]) << 16U) |
                             (static_cast<std::size_t>(data[6]) << 8U) | data[7];
    if (output_size > 256U * 1024U * 1024U) return core::make_error(core::ErrorCode::InvalidArgument, "Yaz0 output exceeds the safety limit");
    std::vector<std::uint8_t> output; output.reserve(output_size); std::size_t input = 16U;
    while (output.size() < output_size) {
        if (input >= data.size()) return core::make_error(core::ErrorCode::OutOfBounds, "Yaz0 flag byte is truncated");
        const auto flags = data[input++];
        for (int bit = 7; bit >= 0 && output.size() < output_size; --bit) {
            if ((flags & (1U << bit)) != 0) {
                if (input >= data.size()) return core::make_error(core::ErrorCode::OutOfBounds, "Yaz0 literal is truncated");
                output.push_back(data[input++]); continue;
            }
            if (input + 1U >= data.size()) return core::make_error(core::ErrorCode::OutOfBounds, "Yaz0 back-reference is truncated");
            const auto first = data[input++]; const auto second = data[input++];
            const auto displacement = (static_cast<std::size_t>(first & 0x0FU) << 8U | second) + 1U;
            std::size_t length = first >> 4U;
            if (length == 0) { if (input >= data.size()) return core::make_error(core::ErrorCode::OutOfBounds, "Yaz0 long token is truncated"); length = static_cast<std::size_t>(data[input++]) + 0x12U; }
            else length += 2U;
            if (displacement > output.size()) return core::make_error(core::ErrorCode::ParseError, "Yaz0 back-reference precedes the output buffer");
            for (std::size_t i = 0; i < length && output.size() < output_size; ++i) output.push_back(output[output.size() - displacement]);
        }
    }
    return output;
}

core::Result<std::vector<std::uint8_t>> compress_yaz0(std::span<const std::uint8_t> data) {
    if (data.size() > std::numeric_limits<std::uint32_t>::max()) return core::make_error(core::ErrorCode::InvalidArgument, "input is too large for Yaz0");
    std::vector<std::uint8_t> output{'Y','a','z','0'}; const auto size = static_cast<std::uint32_t>(data.size());
    output.push_back(static_cast<std::uint8_t>(size >> 24U)); output.push_back(static_cast<std::uint8_t>(size >> 16U));
    output.push_back(static_cast<std::uint8_t>(size >> 8U)); output.push_back(static_cast<std::uint8_t>(size)); output.insert(output.end(), 8, 0);
    ForwardMatcher matcher(data); std::size_t position = 0;
    while (position < data.size()) {
        const auto flag_offset = output.size(); output.push_back(0);
        for (int bit = 7; bit >= 0 && position < data.size(); --bit) {
            const auto match = matcher.find(position, 0x111U); const auto consumed = match.length ? match.length : 1U;
            if (!match.length) { output[flag_offset] |= static_cast<std::uint8_t>(1U << bit); output.push_back(data[position]); }
            else { const auto displacement = match.displacement - 1U;
                if (match.length >= 0x12U) { output.push_back(static_cast<std::uint8_t>(displacement >> 8U)); output.push_back(static_cast<std::uint8_t>(displacement)); output.push_back(static_cast<std::uint8_t>(match.length - 0x12U)); }
                else { output.push_back(static_cast<std::uint8_t>(((match.length - 2U) << 4U) | (displacement >> 8U))); output.push_back(static_cast<std::uint8_t>(displacement)); }}
            for (std::size_t i = 0; i < consumed; ++i) matcher.insert(position + i); position += consumed;
        }
    }
    while (output.size() % 4U) output.push_back(0); return output;
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

core::Result<std::vector<std::uint8_t>> compress_blz_overlay(std::span<const std::uint8_t> data) {
    if (data.size() < 16U || data.size() > 0xFFFFFFU) return std::vector<std::uint8_t>{};
    std::vector<std::uint8_t> work(data.size()); std::size_t source = data.size(), target = data.size();
    while (source > 0) {
        if (target == 0) return std::vector<std::uint8_t>{};
        const auto flag_offset = --target; std::uint8_t flags = 0;
        for (int token = 0; token < 8; ++token) {
            flags <<= 1U; if (source == 0) continue;
            std::size_t match_position = 0; const auto length = backward_match(data, source, match_position);
            if (length <= 2U) { if (target == 0) return std::vector<std::uint8_t>{}; work[--target] = data[--source]; }
            else { if (target < 2U) return std::vector<std::uint8_t>{}; source -= length; const auto packed = static_cast<std::uint16_t>((match_position - 2U) & 0x0FFFU) | static_cast<std::uint16_t>((length - 3U) << 12U);
                --target; work[target--] = static_cast<std::uint8_t>(packed >> 8U); work[target] = static_cast<std::uint8_t>(packed); flags |= 1U; }
        }
        work[flag_offset] = flags;
    }
    const auto body_size = data.size() - target; auto total = body_size + 8U; const auto padding = (4U - total % 4U) % 4U; total += padding;
    if (total >= data.size()) return std::vector<std::uint8_t>{};
    std::vector<std::uint8_t> output(total, 0); std::copy(work.begin() + static_cast<std::ptrdiff_t>(target), work.end(), output.begin());
    const auto offset_in = static_cast<std::uint32_t>(total) | (static_cast<std::uint32_t>(8U + padding) << 24U);
    put_u32_le(output, total - 8U, offset_in); put_u32_le(output, total - 4U, static_cast<std::uint32_t>(data.size() - total)); return output;
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

core::Result<std::vector<std::uint8_t>> encode_8bpp_tiles(std::span<const IndexedTile8Bpp> tiles) {
    std::vector<std::uint8_t> output(tiles.size() * 64U);
    for (std::size_t tile = 0; tile < tiles.size(); ++tile)
        std::copy(tiles[tile].pixels.begin(), tiles[tile].pixels.end(), output.begin() + static_cast<std::ptrdiff_t>(tile * 64U));
    return output;
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
        entries.push_back({static_cast<std::int32_t>(packed.value() & 0x03FFU) - tile_offset,
                           static_cast<std::int16_t>(static_cast<int>((packed.value() >> 12U) & 0x0FU) - palette_offset),
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

core::Result<std::vector<std::uint8_t>> encode_screen_map(std::span<const ScreenMapEntry> entries,
                                                          std::uint16_t tile_offset,
                                                          std::uint8_t palette_offset) {
    std::vector<std::uint8_t> output(entries.size() * 2U);
    for (std::size_t i = 0; i < entries.size(); ++i) {
        const auto tile = entries[i].tile + tile_offset;
        const auto palette = entries[i].palette + palette_offset;
        if (tile < 0 || tile > 0x3ff || palette < 0 || palette > 15) return core::make_error(core::ErrorCode::InvalidArgument, "screen-map tile or palette index is not representable");
        const auto packed = static_cast<std::uint16_t>(static_cast<unsigned>(tile) | (entries[i].horizontal_flip ? 0x400U : 0U) |
                                                       (entries[i].vertical_flip ? 0x800U : 0U) | (static_cast<unsigned>(palette) << 12U));
        output[i * 2U] = static_cast<std::uint8_t>(packed);
        output[i * 2U + 1U] = static_cast<std::uint8_t>(packed >> 8U);
    }
    return output;
}

core::Result<std::vector<std::uint8_t>> encode_map16(std::span<const Map16Tile> tiles,
                                                     std::uint16_t tile_offset,
                                                     std::uint8_t palette_offset) {
    std::vector<ScreenMapEntry> entries;
    entries.reserve(tiles.size() * 4U);
    for (const auto& tile : tiles) entries.insert(entries.end(), tile.tiles.begin(), tile.tiles.end());
    return encode_screen_map(entries, tile_offset, palette_offset);
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
        // An entry below the resource's offsets, or past the tiles it carries,
        // names graphics this resource does not hold. Leave it clear rather
        // than drawing whatever tile happens to sit at that index.
        if (entry.tile < 0 || entry.palette < 0 || static_cast<std::size_t>(entry.tile) >= tiles.size()) continue;
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

core::Result<EncodedObjectLayouts> encode_object_layouts(std::span<const ObjectLayout> objects,
                                                          std::uint8_t tile_page_offset,
                                                          std::span<const std::uint8_t> index_template) {
    if (!index_template.empty() && index_template.size() != objects.size() * 4U)
        return core::make_error(core::ErrorCode::InvalidArgument, "object index template has the wrong size");
    EncodedObjectLayouts output;
    output.index.resize(objects.size() * 4U);
    if (!index_template.empty()) output.index.assign(index_template.begin(), index_template.end());
    for (std::size_t object_index = 0; object_index < objects.size(); ++object_index) {
        if (output.definitions.size() > 0xffffU) return core::make_error(core::ErrorCode::OutOfBounds, "object definitions exceed the 16-bit index range");
        const auto offset = static_cast<std::uint16_t>(output.definitions.size());
        output.index[object_index * 4U] = static_cast<std::uint8_t>(offset);
        output.index[object_index * 4U + 1U] = static_cast<std::uint8_t>(offset >> 8U);
        const auto& object = objects[object_index];
        if (object.rows.empty()) return core::make_error(core::ErrorCode::InvalidArgument, "object layout has no rows");
        if (object.slope) output.definitions.push_back(static_cast<std::uint8_t>(0x80U | (object.slope_left ? 1U : 0U) | (object.slope_down ? 2U : 0U)));
        for (std::size_t row_index = 0; row_index < object.rows.size(); ++row_index) {
            if (row_index) output.definitions.push_back(0xfeU);
            for (const auto& entry : object.rows[row_index]) {
                if ((entry.control & 0x80U) != 0 || entry.control == 0xfeU || entry.control == 0xffU)
                    return core::make_error(core::ErrorCode::InvalidArgument, "object tile control collides with a layout marker");
                output.definitions.push_back(entry.control);
                if (entry.tile < 0) { output.definitions.push_back(0); output.definitions.push_back(0); continue; }
                auto tile = entry.tile - ((entry.control & 0x40U) ? 768 : 0);
                if (tile < 0 || tile > 0xffff) return core::make_error(core::ErrorCode::InvalidArgument, "object tile is outside the representable page range");
                const auto page = (tile >> 8) + tile_page_offset;
                if (page < 0 || page > 255 || (tile & 0xff) == 0 && page == 0)
                    return core::make_error(core::ErrorCode::InvalidArgument, "object tile is outside the representable page range");
                output.definitions.push_back(static_cast<std::uint8_t>(tile));
                output.definitions.push_back(static_cast<std::uint8_t>(page));
            }
        }
        output.definitions.push_back(0xffU);
    }
    return output;
}

core::Result<IndexedImage> quantize_bgr555(const RgbaImage& image, std::size_t maximum_colors, std::uint8_t alpha_threshold) {
    if (!image.width || !image.height || image.width > std::numeric_limits<std::size_t>::max() / image.height ||
        image.pixels.size() != image.width * image.height * 4U)
        return core::make_error(core::ErrorCode::InvalidArgument, "quantization needs a complete RGBA image");
    if (maximum_colors < 2U || maximum_colors > 256U) return core::make_error(core::ErrorCode::InvalidArgument, "palette size must be 2 through 256");
    struct Sample { std::uint8_t r{}, g{}, b{}; std::size_t count{}; };
    std::map<std::uint16_t, std::size_t> frequency;
    const auto pack5 = [](std::uint8_t r, std::uint8_t g, std::uint8_t b) { return static_cast<std::uint16_t>(((r * 31U + 127U) / 255U) | (((g * 31U + 127U) / 255U) << 5U) | (((b * 31U + 127U) / 255U) << 10U)); };
    bool transparent = false;
    for (std::size_t i = 0; i < image.width * image.height; ++i) {
        if (image.pixels[i * 4U + 3U] < alpha_threshold) { transparent = true; continue; }
        ++frequency[pack5(image.pixels[i * 4U], image.pixels[i * 4U + 1U], image.pixels[i * 4U + 2U])];
    }
    std::vector<Sample> samples; samples.reserve(frequency.size());
    const auto expand = [](unsigned value) { return static_cast<std::uint8_t>((value << 3U) | (value >> 2U)); };
    for (const auto& [packed, count] : frequency) samples.push_back({expand(packed & 31U), expand((packed >> 5U) & 31U), expand((packed >> 10U) & 31U), count});
    const auto available = maximum_colors - (transparent ? 1U : 0U);
    struct Box { std::vector<std::size_t> values; };
    std::vector<Box> boxes;
    if (!samples.empty()) { boxes.push_back({{}}); boxes.front().values.resize(samples.size()); std::iota(boxes.front().values.begin(), boxes.front().values.end(), 0U); }
    auto range = [&](const Box& box, int channel) { if (box.values.empty()) return 0; auto [low, high] = std::minmax_element(box.values.begin(), box.values.end(), [&](auto a, auto b) { const auto ca = channel == 0 ? samples[a].r : channel == 1 ? samples[a].g : samples[a].b; const auto cb = channel == 0 ? samples[b].r : channel == 1 ? samples[b].g : samples[b].b; return ca < cb; }); const auto value = [&](auto i) { return channel == 0 ? samples[*i].r : channel == 1 ? samples[*i].g : samples[*i].b; }; return static_cast<int>(value(high)) - value(low); };
    while (boxes.size() < available) {
        auto found = std::max_element(boxes.begin(), boxes.end(), [&](const Box& a, const Box& b) { return std::max({range(a,0),range(a,1),range(a,2)}) < std::max({range(b,0),range(b,1),range(b,2)}); });
        if (found == boxes.end() || found->values.size() < 2U) break;
        const std::array ranges{range(*found,0),range(*found,1),range(*found,2)}; const auto channel = static_cast<int>(std::max_element(ranges.begin(), ranges.end()) - ranges.begin());
        std::sort(found->values.begin(), found->values.end(), [&](auto a, auto b) { return channel == 0 ? samples[a].r < samples[b].r : channel == 1 ? samples[a].g < samples[b].g : samples[a].b < samples[b].b; });
        std::size_t total = 0; for (auto value : found->values) total += samples[value].count; std::size_t seen = 0, split = 1;
        for (; split < found->values.size(); ++split) { seen += samples[found->values[split - 1U]].count; if (seen * 2U >= total) break; }
        split = std::min(split, found->values.size() - 1U);
        Box right{{found->values.begin() + static_cast<std::ptrdiff_t>(split), found->values.end()}}; found->values.erase(found->values.begin() + static_cast<std::ptrdiff_t>(split), found->values.end()); boxes.push_back(std::move(right));
    }
    IndexedImage output{image.width, image.height, std::vector<std::uint8_t>(image.width * image.height), {}};
    if (transparent) output.palette.push_back({0,0,0,true});
    for (const auto& box : boxes) {
        std::size_t count = 0, r = 0, g = 0, b = 0; for (auto value : box.values) { count += samples[value].count; r += samples[value].r * samples[value].count; g += samples[value].g * samples[value].count; b += samples[value].b * samples[value].count; }
        if (count) output.palette.push_back({static_cast<std::uint8_t>(r / count), static_cast<std::uint8_t>(g / count), static_cast<std::uint8_t>(b / count), false});
    }
    if (output.palette.empty()) output.palette.push_back({0,0,0,true});
    for (std::size_t i = 0; i < image.width * image.height; ++i) {
        if (transparent && image.pixels[i * 4U + 3U] < alpha_threshold) { output.pixels[i] = 0; continue; }
        std::size_t best = transparent ? 1U : 0U; unsigned best_error = std::numeric_limits<unsigned>::max();
        for (std::size_t candidate = transparent ? 1U : 0U; candidate < output.palette.size(); ++candidate) { const auto dr = static_cast<int>(image.pixels[i*4U])-output.palette[candidate].red, dg = static_cast<int>(image.pixels[i*4U+1U])-output.palette[candidate].green, db = static_cast<int>(image.pixels[i*4U+2U])-output.palette[candidate].blue; const auto error = static_cast<unsigned>(dr*dr+dg*dg+db*db); if (error < best_error) { best_error = error; best = candidate; } }
        output.pixels[i] = static_cast<std::uint8_t>(best);
    }
    return output;
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
