#pragma once

#include <array>
#include <cstdint>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::formats {

enum class UiResourceKind { Unknown, Bnbl, Bncl, Bncd, Enpg, Banner };

struct BnblEntry {
    std::uint16_t x{}, y{};
    std::uint8_t x_alignment{}, y_alignment{}, x_reserved{}, y_reserved{};
    std::uint8_t width{}, height{};
    bool operator==(const BnblEntry&) const = default;
};

struct Bnbl {
    std::vector<std::uint8_t> original;
    std::vector<BnblEntry> entries;
};

struct BnclEntry {
    std::uint16_t x{}, y{};
    std::uint8_t x_alignment{}, y_alignment{}, x_reserved{}, y_reserved{};
    std::uint8_t graphic_id{};
    std::array<std::uint8_t, 3> reserved{};
    bool operator==(const BnclEntry&) const = default;
};

struct Bncl {
    std::vector<std::uint8_t> original;
    std::vector<BnclEntry> entries;
};

struct BncdEntry {
    std::uint8_t width{}, height{};
    std::uint16_t subentry_start{}, subentry_count{};
    bool operator==(const BncdEntry&) const = default;
};

struct BncdSubentry {
    std::uint16_t oam_attr0{}, oam_attr1{};
    std::uint32_t unknown{};
    std::uint16_t tile_number{}, tile_count{};
    bool operator==(const BncdSubentry&) const = default;
};

struct Bncd {
    std::vector<std::uint8_t> original;
    std::uint32_t entries_offset{}, subentries_offset{}, texture_offset{}, texture_size{};
    std::vector<BncdEntry> entries;
    std::vector<BncdSubentry> subentries;
    std::vector<std::uint8_t> texture;
};

struct Enpg {
    std::vector<std::uint8_t> pixels;
    std::array<std::uint16_t, 256> palette{};
    std::vector<std::uint8_t> trailing;
};

struct DsBanner {
    std::vector<std::uint8_t> original;
    std::uint16_t version{};
    std::array<std::uint8_t, 0x200> icon{};
    std::array<std::uint16_t, 16> palette{};
    std::array<std::array<char16_t, 128>, 8> title_slots{};
    std::size_t title_count{};
};

struct RomUserMetadataEntry {
    std::int32_t key{};
    std::string value;
    bool operator==(const RomUserMetadataEntry&) const = default;
};

struct RomUserMetadataSection {
    std::string name;
    std::vector<RomUserMetadataEntry> entries;
    bool operator==(const RomUserMetadataSection&) const = default;
};

struct RomUserMetadata {
    std::vector<std::uint8_t> original;
    std::vector<RomUserMetadataSection> original_sections;
    std::vector<RomUserMetadataSection> sections;
};

[[nodiscard]] UiResourceKind identify_ui_resource(std::span<const std::uint8_t> bytes, std::string_view path = {});
core::Result<Bnbl> parse_bnbl(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_bnbl(const Bnbl& value);
core::Result<Bncl> parse_bncl(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_bncl(const Bncl& value);
core::Result<Bncd> parse_bncd(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_bncd(const Bncd& value);
core::Result<Enpg> parse_enpg(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_enpg(const Enpg& value);
core::Result<DsBanner> parse_ds_banner(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_ds_banner(const DsBanner& value);
[[nodiscard]] std::u16string banner_title(const DsBanner& value, std::size_t index);
core::Result<void> set_banner_title(DsBanner& value, std::size_t index, std::u16string_view title);
core::Result<RomUserMetadata> parse_rom_user_metadata(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_rom_user_metadata(const RomUserMetadata& value);

} // namespace nsmb::formats
