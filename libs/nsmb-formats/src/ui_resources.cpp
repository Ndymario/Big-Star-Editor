#include "nsmb/formats/ui_resources.hpp"

#include "nsmb/core/endian.hpp"
#include "nsmb/nds/rom.hpp"

#include <algorithm>
#include <charconv>
#include <cctype>
#include <filesystem>
#include <limits>
#include <unordered_set>

namespace nsmb::formats {
namespace {

constexpr std::size_t enpg_pixels_size = 256U * 256U;
constexpr std::size_t enpg_palette_size = 256U * 2U;

bool magic(std::span<const std::uint8_t> bytes, std::string_view value) {
    return bytes.size() >= value.size() && std::equal(value.begin(), value.end(), bytes.begin());
}

std::string lower_name(std::string_view path) {
    auto result = std::filesystem::path(path).filename().string();
    std::ranges::transform(result, result.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
    return result;
}

core::Result<std::uint16_t> read16(std::span<const std::uint8_t> bytes, std::size_t offset) {
    return core::LittleEndianReader(bytes).u16(offset);
}

core::Result<std::uint32_t> read32(std::span<const std::uint8_t> bytes, std::size_t offset) {
    return core::LittleEndianReader(bytes).u32(offset);
}

core::Result<void> write16(std::vector<std::uint8_t>& bytes, std::size_t offset, std::uint16_t value) {
    return core::LittleEndianWriter(bytes).u16(offset, value);
}

core::Result<void> write32(std::vector<std::uint8_t>& bytes, std::size_t offset, std::uint32_t value) {
    return core::LittleEndianWriter(bytes).u32(offset, value);
}

core::Result<void> validate_position(std::uint16_t coordinate, std::uint8_t alignment, std::uint8_t reserved) {
    if (coordinate > 0x0fff || alignment > 3 || reserved > 3)
        return core::make_error(core::ErrorCode::InvalidArgument, "UI position or alignment is outside its packed field");
    return {};
}

std::uint16_t packed_position(std::uint16_t coordinate, std::uint8_t alignment, std::uint8_t reserved) {
    return static_cast<std::uint16_t>(coordinate | alignment << 12U | reserved << 14U);
}

std::size_t banner_minimum_size(std::uint16_t version) {
    if (version == 1) return 0x840;
    if (version == 2) return 0x940;
    if (version == 3 || version == 0x0103) return 0xA40;
    return 0;
}

std::size_t banner_title_count(std::uint16_t version) {
    return version == 1 ? 6 : version == 2 ? 7 : 8;
}

void update_banner_crc(std::vector<std::uint8_t>& bytes, std::size_t offset, std::size_t length, std::size_t field) {
    const auto crc = nsmb::nds::crc16(std::span<const std::uint8_t>(bytes).subspan(offset, length));
    (void)write16(bytes, field, crc);
}

std::string_view trim(std::string_view value) {
    while (!value.empty() && (value.front() == ' ' || value.front() == '\t' || value.front() == '\r')) value.remove_prefix(1);
    while (!value.empty() && (value.back() == ' ' || value.back() == '\t' || value.back() == '\r')) value.remove_suffix(1);
    return value;
}

core::Result<std::int32_t> parse_key(std::string_view text) {
    const auto clean = trim(text); std::int32_t result{};
    const auto parsed = std::from_chars(clean.data(), clean.data() + clean.size(), result);
    if (clean.empty() || parsed.ec != std::errc{} || parsed.ptr != clean.data() + clean.size() || result < 0)
        return core::make_error(core::ErrorCode::ParseError, "metadata key must be a non-negative decimal integer");
    return result;
}

core::Result<void> validate_metadata_text(std::string_view text, std::string_view label) {
    if (text.empty() || text.find_first_of("\r\n[]") != std::string_view::npos)
        return core::make_error(core::ErrorCode::InvalidArgument, std::string(label) + " is empty or contains a reserved character");
    if (!std::ranges::all_of(text, [](unsigned char c) { return c >= 0x20 && c <= 0x7e; }))
        return core::make_error(core::ErrorCode::InvalidArgument, std::string(label) + " must contain printable ASCII only");
    return {};
}

} // namespace

UiResourceKind identify_ui_resource(std::span<const std::uint8_t> bytes, std::string_view path) {
    if (magic(bytes, "JNBL")) return UiResourceKind::Bnbl;
    if (magic(bytes, "JNCL")) return UiResourceKind::Bncl;
    if (magic(bytes, "JNCD")) return UiResourceKind::Bncd;
    const auto name = lower_name(path);
    if (name.ends_with(".enpg") && bytes.size() >= enpg_pixels_size + enpg_palette_size) return UiResourceKind::Enpg;
    if (name == "banner.bin" && bytes.size() >= 2 && banner_minimum_size(static_cast<std::uint16_t>(bytes[0] | bytes[1] << 8U))) return UiResourceKind::Banner;
    return UiResourceKind::Unknown;
}

core::Result<Bnbl> parse_bnbl(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 8 || !magic(bytes, "JNBL")) return core::make_error(core::ErrorCode::ParseError, "BNBL header is missing or invalid");
    const auto count = bytes[6];
    if (count > (bytes.size() - 8U) / 6U) return core::make_error(core::ErrorCode::OutOfBounds, "BNBL entry table is truncated");
    Bnbl result; result.original.assign(bytes.begin(), bytes.end()); result.entries.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = 8U + i * 6U; auto x = read16(bytes, offset), y = read16(bytes, offset + 2U);
        if (!x || !y) return core::make_error(core::ErrorCode::OutOfBounds, "BNBL entry is truncated");
        result.entries.push_back({static_cast<std::uint16_t>(x.value() & 0x0fffU), static_cast<std::uint16_t>(y.value() & 0x0fffU),
                                  static_cast<std::uint8_t>((x.value() >> 12U) & 3U), static_cast<std::uint8_t>((y.value() >> 12U) & 3U),
                                  static_cast<std::uint8_t>(x.value() >> 14U), static_cast<std::uint8_t>(y.value() >> 14U), bytes[offset + 4U], bytes[offset + 5U]});
    }
    return result;
}

core::Result<std::vector<std::uint8_t>> write_bnbl(const Bnbl& value) {
    if (value.original.size() < 8 || value.entries.size() > 255) return core::make_error(core::ErrorCode::InvalidArgument, "BNBL header is invalid or has too many entries");
    const auto old_count = value.original[6]; const auto old_end = 8U + static_cast<std::size_t>(old_count) * 6U;
    if (old_end > value.original.size()) return core::make_error(core::ErrorCode::InvalidArgument, "original BNBL table is truncated");
    std::vector<std::uint8_t> result(value.original.begin(), value.original.begin() + 8); result[6] = static_cast<std::uint8_t>(value.entries.size()); result.resize(8U + value.entries.size() * 6U);
    for (std::size_t i = 0; i < value.entries.size(); ++i) {
        const auto& entry = value.entries[i]; if (auto valid = validate_position(entry.x, entry.x_alignment, entry.x_reserved); !valid) return valid.error();
        if (auto valid = validate_position(entry.y, entry.y_alignment, entry.y_reserved); !valid) return valid.error();
        const auto offset = 8U + i * 6U; (void)write16(result, offset, packed_position(entry.x, entry.x_alignment, entry.x_reserved));
        (void)write16(result, offset + 2U, packed_position(entry.y, entry.y_alignment, entry.y_reserved)); result[offset + 4U] = entry.width; result[offset + 5U] = entry.height;
    }
    result.insert(result.end(), value.original.begin() + static_cast<std::ptrdiff_t>(old_end), value.original.end()); return result;
}

core::Result<Bncl> parse_bncl(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 8 || !magic(bytes, "JNCL")) return core::make_error(core::ErrorCode::ParseError, "BNCL header is missing or invalid");
    const auto count = bytes[6]; if (count > (bytes.size() - 8U) / 8U) return core::make_error(core::ErrorCode::OutOfBounds, "BNCL entry table is truncated");
    Bncl result; result.original.assign(bytes.begin(), bytes.end()); result.entries.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = 8U + i * 8U; auto x = read16(bytes, offset), y = read16(bytes, offset + 2U); if (!x || !y) return core::make_error(core::ErrorCode::OutOfBounds, "BNCL entry is truncated");
        result.entries.push_back({static_cast<std::uint16_t>(x.value() & 0x0fffU), static_cast<std::uint16_t>(y.value() & 0x0fffU),
                                  static_cast<std::uint8_t>((x.value() >> 12U) & 3U), static_cast<std::uint8_t>((y.value() >> 12U) & 3U),
                                  static_cast<std::uint8_t>(x.value() >> 14U), static_cast<std::uint8_t>(y.value() >> 14U), bytes[offset + 4U],
                                  {bytes[offset + 5U], bytes[offset + 6U], bytes[offset + 7U]}});
    }
    return result;
}

core::Result<std::vector<std::uint8_t>> write_bncl(const Bncl& value) {
    if (value.original.size() < 8 || value.entries.size() > 255) return core::make_error(core::ErrorCode::InvalidArgument, "BNCL header is invalid or has too many entries");
    const auto old_count = value.original[6]; const auto old_end = 8U + static_cast<std::size_t>(old_count) * 8U;
    if (old_end > value.original.size()) return core::make_error(core::ErrorCode::InvalidArgument, "original BNCL table is truncated");
    std::vector<std::uint8_t> result(value.original.begin(), value.original.begin() + 8); result[6] = static_cast<std::uint8_t>(value.entries.size()); result.resize(8U + value.entries.size() * 8U);
    for (std::size_t i = 0; i < value.entries.size(); ++i) {
        const auto& entry = value.entries[i]; if (auto valid = validate_position(entry.x, entry.x_alignment, entry.x_reserved); !valid) return valid.error();
        if (auto valid = validate_position(entry.y, entry.y_alignment, entry.y_reserved); !valid) return valid.error();
        const auto offset = 8U + i * 8U; (void)write16(result, offset, packed_position(entry.x, entry.x_alignment, entry.x_reserved));
        (void)write16(result, offset + 2U, packed_position(entry.y, entry.y_alignment, entry.y_reserved)); result[offset + 4U] = entry.graphic_id;
        std::copy(entry.reserved.begin(), entry.reserved.end(), result.begin() + static_cast<std::ptrdiff_t>(offset + 5U));
    }
    result.insert(result.end(), value.original.begin() + static_cast<std::ptrdiff_t>(old_end), value.original.end()); return result;
}

core::Result<Bncd> parse_bncd(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 0x1c || !magic(bytes, "JNCD")) return core::make_error(core::ErrorCode::ParseError, "BNCD header is missing or invalid");
    auto count = read16(bytes, 6);
    auto entries = read32(bytes, 8);
    auto subentries = read32(bytes, 12);
    auto texture = read32(bytes, 16);
    auto texture_size = read32(bytes, 20);
    if (!count || !entries || !subentries || !texture || !texture_size) return core::make_error(core::ErrorCode::OutOfBounds, "BNCD header is truncated");
    const auto entry_end = static_cast<std::uint64_t>(entries.value()) + static_cast<std::uint64_t>(count.value()) * 6U;
    const auto texture_end = static_cast<std::uint64_t>(texture.value()) + texture_size.value();
    if (entries.value() < 0x1c || entry_end > subentries.value() || subentries.value() > texture.value() || texture_end > bytes.size())
        return core::make_error(core::ErrorCode::OutOfBounds, "BNCD tables or texture data overlap or leave the file");
    const auto subentry_count = (texture.value() - subentries.value()) / 12U;
    Bncd result; result.original.assign(bytes.begin(), bytes.end()); result.entries_offset = entries.value(); result.subentries_offset = subentries.value(); result.texture_offset = texture.value(); result.texture_size = texture_size.value();
    result.entries.reserve(count.value());
    for (std::size_t i = 0; i < count.value(); ++i) {
        const auto offset = entries.value() + i * 6U; auto start = read16(bytes, offset + 2U), size = read16(bytes, offset + 4U); if (!start || !size) return core::make_error(core::ErrorCode::OutOfBounds, "BNCD entry is truncated");
        if (static_cast<std::uint32_t>(start.value()) + size.value() > subentry_count) return core::make_error(core::ErrorCode::OutOfBounds, "BNCD entry references subentries outside the table");
        result.entries.push_back({bytes[offset], bytes[offset + 1U], start.value(), size.value()});
    }
    result.subentries.reserve(subentry_count);
    for (std::size_t i = 0; i < subentry_count; ++i) {
        const auto offset = subentries.value() + i * 12U;
        auto a0 = read16(bytes, offset);
        auto a1 = read16(bytes, offset + 2U);
        auto unknown = read32(bytes, offset + 4U);
        auto tile = read16(bytes, offset + 8U);
        auto tiles = read16(bytes, offset + 10U);
        if (!a0 || !a1 || !unknown || !tile || !tiles) return core::make_error(core::ErrorCode::OutOfBounds, "BNCD subentry is truncated");
        result.subentries.push_back({a0.value(), a1.value(), unknown.value(), tile.value(), tiles.value()});
    }
    result.texture.assign(bytes.begin() + texture.value(), bytes.begin() + texture.value() + texture_size.value()); return result;
}

core::Result<std::vector<std::uint8_t>> write_bncd(const Bncd& value) {
    if (value.original.size() < 0x1c) return core::make_error(core::ErrorCode::InvalidArgument, "BNCD original data is missing");
    auto count = read16(value.original, 6); if (!count || value.entries.size() != count.value()) return core::make_error(core::ErrorCode::InvalidArgument, "BNCD entry count cannot change");
    const auto entries_end = static_cast<std::uint64_t>(value.entries_offset) + value.entries.size() * 6U;
    const auto subentries_end = static_cast<std::uint64_t>(value.subentries_offset) + value.subentries.size() * 12U;
    const auto texture_end = static_cast<std::uint64_t>(value.texture_offset) + value.texture_size;
    if (value.entries_offset < 0x1c || entries_end > value.subentries_offset ||
        subentries_end > value.texture_offset || texture_end > value.original.size())
        return core::make_error(core::ErrorCode::InvalidArgument, "BNCD table offsets leave the original file");
    if (value.texture.size() != value.texture_size) return core::make_error(core::ErrorCode::InvalidArgument, "BNCD texture size cannot change");
    std::vector<std::uint8_t> result = value.original;
    for (std::size_t i = 0; i < value.entries.size(); ++i) {
        const auto& entry = value.entries[i]; if (static_cast<std::size_t>(entry.subentry_start) + entry.subentry_count > value.subentries.size()) return core::make_error(core::ErrorCode::InvalidArgument, "BNCD entry references unavailable subentries");
        const auto offset = value.entries_offset + i * 6U; result[offset] = entry.width; result[offset + 1U] = entry.height; (void)write16(result, offset + 2U, entry.subentry_start); (void)write16(result, offset + 4U, entry.subentry_count);
    }
    for (std::size_t i = 0; i < value.subentries.size(); ++i) {
        const auto& entry = value.subentries[i]; const auto offset = value.subentries_offset + i * 12U; (void)write16(result, offset, entry.oam_attr0); (void)write16(result, offset + 2U, entry.oam_attr1); (void)write32(result, offset + 4U, entry.unknown); (void)write16(result, offset + 8U, entry.tile_number); (void)write16(result, offset + 10U, entry.tile_count);
    }
    std::copy(value.texture.begin(), value.texture.end(), result.begin() + value.texture_offset); return result;
}

core::Result<Enpg> parse_enpg(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < enpg_pixels_size + enpg_palette_size) return core::make_error(core::ErrorCode::OutOfBounds, "ENPG requires 256x256 indexed pixels and a 256-colour palette");
    Enpg result; result.pixels.assign(bytes.begin(), bytes.begin() + enpg_pixels_size);
    for (std::size_t i = 0; i < result.palette.size(); ++i) result.palette[i] = static_cast<std::uint16_t>(bytes[enpg_pixels_size + i * 2U] | bytes[enpg_pixels_size + i * 2U + 1U] << 8U);
    result.trailing.assign(bytes.begin() + enpg_pixels_size + enpg_palette_size, bytes.end()); return result;
}

core::Result<std::vector<std::uint8_t>> write_enpg(const Enpg& value) {
    if (value.pixels.size() != enpg_pixels_size) return core::make_error(core::ErrorCode::InvalidArgument, "ENPG pixel plane must be exactly 256x256 bytes");
    std::vector<std::uint8_t> result = value.pixels; result.resize(enpg_pixels_size + enpg_palette_size);
    for (std::size_t i = 0; i < value.palette.size(); ++i) { result[enpg_pixels_size + i * 2U] = static_cast<std::uint8_t>(value.palette[i]); result[enpg_pixels_size + i * 2U + 1U] = static_cast<std::uint8_t>(value.palette[i] >> 8U); }
    result.insert(result.end(), value.trailing.begin(), value.trailing.end()); return result;
}

core::Result<DsBanner> parse_ds_banner(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 2) return core::make_error(core::ErrorCode::OutOfBounds, "DS banner header is truncated");
    const auto version = static_cast<std::uint16_t>(bytes[0] | bytes[1] << 8U);
    const auto minimum = banner_minimum_size(version);
    if (!minimum) return core::make_error(core::ErrorCode::Unsupported, "DS banner version is unsupported");
    if (bytes.size() < minimum) return core::make_error(core::ErrorCode::OutOfBounds, "DS banner is truncated");
    DsBanner result; result.original.assign(bytes.begin(), bytes.end()); result.version = version; result.title_count = banner_title_count(version);
    std::copy_n(bytes.begin() + 0x20, result.icon.size(), result.icon.begin());
    for (std::size_t i = 0; i < result.palette.size(); ++i) result.palette[i] = static_cast<std::uint16_t>(bytes[0x220 + i * 2U] | bytes[0x220 + i * 2U + 1U] << 8U);
    for (std::size_t language = 0; language < result.title_count; ++language) for (std::size_t i = 0; i < 128; ++i) {
        const auto offset = 0x240U + language * 0x100U + i * 2U; result.title_slots[language][i] = static_cast<char16_t>(bytes[offset] | bytes[offset + 1U] << 8U);
    }
    return result;
}

core::Result<std::vector<std::uint8_t>> write_ds_banner(const DsBanner& value) {
    const auto minimum = banner_minimum_size(value.version); if (!minimum || value.original.size() < minimum || value.title_count != banner_title_count(value.version)) return core::make_error(core::ErrorCode::InvalidArgument, "DS banner model does not match its version");
    std::vector<std::uint8_t> result = value.original; std::copy(value.icon.begin(), value.icon.end(), result.begin() + 0x20);
    for (std::size_t i = 0; i < value.palette.size(); ++i) { result[0x220 + i * 2U] = static_cast<std::uint8_t>(value.palette[i]); result[0x220 + i * 2U + 1U] = static_cast<std::uint8_t>(value.palette[i] >> 8U); }
    for (std::size_t language = 0; language < value.title_count; ++language) for (std::size_t i = 0; i < 128; ++i) {
        const auto unit = static_cast<std::uint16_t>(value.title_slots[language][i]);
        const auto offset = 0x240U + language * 0x100U + i * 2U;
        result[offset] = static_cast<std::uint8_t>(unit); result[offset + 1U] = static_cast<std::uint8_t>(unit >> 8U);
    }
    update_banner_crc(result, 0x20, 0x820, 2); if (value.version >= 2) update_banner_crc(result, 0x20, 0x920, 4); if (value.version >= 3) update_banner_crc(result, 0x20, 0xA20, 6); return result;
}

std::u16string banner_title(const DsBanner& value, std::size_t index) {
    if (index >= value.title_count) return {}; const auto& slot = value.title_slots[index]; const auto end = std::ranges::find(slot, u'\0'); return {slot.begin(), end};
}

core::Result<void> set_banner_title(DsBanner& value, std::size_t index, std::u16string_view title) {
    if (index >= value.title_count) return core::make_error(core::ErrorCode::OutOfBounds, "banner language index is unavailable");
    if (title.size() > 128) return core::make_error(core::ErrorCode::InvalidArgument, "banner title exceeds 128 UTF-16 code units");
    auto& slot = value.title_slots[index]; slot.fill(0); std::copy(title.begin(), title.end(), slot.begin()); return {};
}

core::Result<RomUserMetadata> parse_rom_user_metadata(std::span<const std::uint8_t> bytes) {
    if (!std::ranges::all_of(bytes, [](std::uint8_t c) { return c == '\n' || c == '\r' || c == '\t' || (c >= 0x20 && c <= 0x7e); })) return core::make_error(core::ErrorCode::ParseError, "ROM user metadata must be ASCII text");
    RomUserMetadata result; result.original.assign(bytes.begin(), bytes.end()); std::string text(bytes.begin(), bytes.end()); std::unordered_set<std::string> section_names; RomUserMetadataSection* current = nullptr;
    for (std::size_t start = 0, line_number = 1; start <= text.size(); ++line_number) {
        const auto end = text.find('\n', start); const auto raw = std::string_view(text).substr(start, end == std::string::npos ? text.size() - start : end - start); const auto line = trim(raw);
        if (!line.empty()) {
            if (line.front() == '[') {
                if (line.size() < 3 || line.back() != ']') return core::make_error(core::ErrorCode::ParseError, "metadata section header is malformed at line " + std::to_string(line_number));
                const auto name = std::string(line.substr(1, line.size() - 2)); if (auto valid = validate_metadata_text(name, "metadata section name"); !valid) return valid.error();
                if (!section_names.insert(name).second) return core::make_error(core::ErrorCode::ParseError, "metadata section is duplicated"); result.sections.push_back({name, {}}); current = &result.sections.back();
            } else {
                if (!current) return core::make_error(core::ErrorCode::ParseError, "metadata entry appears before a section"); const auto separator = line.find('='); if (separator == std::string_view::npos) return core::make_error(core::ErrorCode::ParseError, "metadata entry is missing '=' at line " + std::to_string(line_number));
                auto key = parse_key(line.substr(0, separator)); if (!key) return key.error(); const auto value = std::string(trim(line.substr(separator + 1)));
                if (value.find_first_of("\r\n") != std::string::npos) return core::make_error(core::ErrorCode::ParseError, "metadata value contains a line break");
                if (std::ranges::any_of(current->entries, [&](const auto& entry) { return entry.key == key.value(); })) return core::make_error(core::ErrorCode::ParseError, "metadata key is duplicated within a section"); current->entries.push_back({key.value(), value});
            }
        }
        if (end == std::string::npos) break; start = end + 1;
    }
    result.original_sections = result.sections; return result;
}

core::Result<std::vector<std::uint8_t>> write_rom_user_metadata(const RomUserMetadata& value) {
    if (value.sections == value.original_sections) return value.original;
    std::string text; std::unordered_set<std::string> names;
    for (const auto& section : value.sections) {
        if (auto valid = validate_metadata_text(section.name, "metadata section name"); !valid) return valid.error(); if (!names.insert(section.name).second) return core::make_error(core::ErrorCode::InvalidArgument, "metadata section is duplicated");
        text += '[' + section.name + "]\n"; std::unordered_set<std::int32_t> keys;
        for (const auto& entry : section.entries) { if (entry.key < 0 || !keys.insert(entry.key).second) return core::make_error(core::ErrorCode::InvalidArgument, "metadata key is negative or duplicated"); if (entry.value.find_first_of("\r\n") != std::string::npos || !std::ranges::all_of(entry.value, [](unsigned char c) { return c >= 0x20 && c <= 0x7e; })) return core::make_error(core::ErrorCode::InvalidArgument, "metadata value must be printable ASCII on one line"); text += std::to_string(entry.key) + '=' + entry.value + '\n'; }
    }
    return std::vector<std::uint8_t>(text.begin(), text.end());
}

} // namespace nsmb::formats
