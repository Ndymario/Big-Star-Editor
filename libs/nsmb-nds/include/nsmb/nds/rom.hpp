#pragma once

#include <cstdint>
#include <filesystem>
#include <span>
#include <string>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::nds {

inline constexpr std::uint16_t root_directory_id = 0xF000;
inline constexpr std::size_t ds_cartridge_header_size = 0x200;
inline constexpr std::size_t default_header_region_size = 0x4000;
inline constexpr std::size_t overlay_table_entry_size = 0x20;

struct RomRegion {
    std::uint32_t rom_offset{};
    std::uint32_t size{};
};

struct RomArmBinary {
    std::uint32_t rom_offset{};
    std::uint32_t entry_address{};
    std::uint32_t ram_address{};
    std::uint32_t size{};
};

struct RomHeader {
    std::string game_title;
    std::string game_code;
    std::string maker_code;
    std::uint8_t unit_code{};
    std::uint8_t encryption_seed_select{};
    std::uint8_t device_capacity{};
    std::uint8_t region{};
    std::uint8_t rom_version{};
    std::uint8_t autostart{};
    RomArmBinary arm9;
    RomArmBinary arm7;
    RomRegion fnt;
    RomRegion fat;
    RomRegion arm9_overlay_table;
    RomRegion arm7_overlay_table;
    std::uint32_t normal_card_control{};
    std::uint32_t key1_card_control{};
    std::uint32_t banner_rom_offset{};
    std::uint16_t secure_area_checksum{};
    std::uint16_t secure_area_delay{};
    std::uint32_t arm9_autoload_hook_address{};
    std::uint32_t arm7_autoload_hook_address{};
    std::uint32_t total_used_rom_size{};
    std::uint32_t header_size{};
    std::uint16_t nintendo_logo_checksum{};
    std::uint16_t header_checksum{};

    std::uint32_t arm9_rom_offset{};
    std::uint32_t arm7_rom_offset{};
};

struct FatEntry {
    std::uint32_t start_address{};
    std::uint32_t end_address{};

    [[nodiscard]] bool used() const noexcept;
    [[nodiscard]] std::uint32_t size() const noexcept;
};

struct FntDirectoryEntry {
    std::uint32_t sub_table_offset{};
    std::uint16_t first_file_id{};
    std::uint16_t parent_directory_id{};
    std::uint16_t directory_count{};
};

struct NitroFileEntry {
    std::uint16_t id{};
    std::filesystem::path path;
};

struct OverlayTableEntry {
    std::uint32_t overlay_id{};
    std::uint32_t ram_address{};
    std::uint32_t ram_size{};
    std::uint32_t bss_size{};
    std::uint32_t static_initializer_start_address{};
    std::uint32_t static_initializer_end_address{};
    std::uint32_t file_id{};
    std::uint32_t reserved{};
};

struct FileTreeNode {
    std::string name;
    std::filesystem::path path;
    bool directory{};
    std::vector<FileTreeNode> children;
};

struct ExtractedRom {
    std::filesystem::path root;
    FileTreeNode files;
};

class RomExtractionService {
public:
    virtual ~RomExtractionService() = default;
    virtual core::Result<ExtractedRom> extract(const std::filesystem::path& rom, const std::filesystem::path& output_dir) = 0;
};

class RomBuildService {
public:
    virtual ~RomBuildService() = default;
    virtual core::Result<void> build(const std::filesystem::path& extracted_dir, const std::filesystem::path& output_rom) = 0;
};

class NitroRomService final : public RomExtractionService, public RomBuildService {
public:
    core::Result<ExtractedRom> extract(const std::filesystem::path& rom, const std::filesystem::path& output_dir) override;
    core::Result<void> build(const std::filesystem::path& extracted_dir, const std::filesystem::path& output_rom) override;
};

[[nodiscard]] std::string overlay_file_name(std::uint32_t overlay_id);
[[nodiscard]] std::uint16_t crc16(std::span<const std::uint8_t> data, std::uint16_t initial = 0xFFFF);

core::Result<RomHeader> parse_rom_header(std::span<const std::uint8_t> data);
core::Result<std::vector<FatEntry>> parse_fat(std::span<const std::uint8_t> data);
core::Result<std::vector<NitroFileEntry>> parse_fnt(std::span<const std::uint8_t> data);
core::Result<std::vector<OverlayTableEntry>> parse_overlay_table(std::span<const std::uint8_t> data);
core::Result<FileTreeNode> scan_file_tree(const std::filesystem::path& root);

} // namespace nsmb::nds
