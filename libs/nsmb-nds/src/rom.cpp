#include "nsmb/nds/rom.hpp"

#include "nsmb/core/endian.hpp"

#include <algorithm>
#include <cstddef>

namespace nsmb::nds {

namespace {

namespace fs = std::filesystem;

namespace header_offset {
constexpr std::size_t game_title = 0x000;
constexpr std::size_t game_code = 0x00C;
constexpr std::size_t maker_code = 0x010;
constexpr std::size_t unit_code = 0x012;
constexpr std::size_t encryption_seed_select = 0x013;
constexpr std::size_t device_capacity = 0x014;
constexpr std::size_t region = 0x01D;
constexpr std::size_t rom_version = 0x01E;
constexpr std::size_t autostart = 0x01F;
constexpr std::size_t arm9_rom_offset = 0x020;
constexpr std::size_t arm9_entry_address = 0x024;
constexpr std::size_t arm9_ram_address = 0x028;
constexpr std::size_t arm9_size = 0x02C;
constexpr std::size_t arm7_rom_offset = 0x030;
constexpr std::size_t arm7_entry_address = 0x034;
constexpr std::size_t arm7_ram_address = 0x038;
constexpr std::size_t arm7_size = 0x03C;
constexpr std::size_t fnt_offset = 0x040;
constexpr std::size_t fnt_size = 0x044;
constexpr std::size_t fat_offset = 0x048;
constexpr std::size_t fat_size = 0x04C;
constexpr std::size_t arm9_overlay_offset = 0x050;
constexpr std::size_t arm9_overlay_size = 0x054;
constexpr std::size_t arm7_overlay_offset = 0x058;
constexpr std::size_t arm7_overlay_size = 0x05C;
constexpr std::size_t normal_card_control = 0x060;
constexpr std::size_t key1_card_control = 0x064;
constexpr std::size_t banner_offset = 0x068;
constexpr std::size_t secure_area_checksum = 0x06C;
constexpr std::size_t secure_area_delay = 0x06E;
constexpr std::size_t arm9_autoload_hook_address = 0x070;
constexpr std::size_t arm7_autoload_hook_address = 0x074;
constexpr std::size_t total_used_rom_size = 0x080;
constexpr std::size_t header_size = 0x084;
constexpr std::size_t nintendo_logo_checksum = 0x15C;
constexpr std::size_t header_checksum = 0x15E;
} // namespace header_offset

std::string ascii_field(std::span<const std::uint8_t> data, std::size_t offset, std::size_t length) {
    std::string out(reinterpret_cast<const char*>(data.data() + offset), length);
    while (!out.empty() && (out.back() == '\0' || out.back() == ' ')) {
        out.pop_back();
    }
    return out;
}

void consider_next_region(std::uint32_t candidate, std::uint32_t banner_offset, std::size_t rom_size, std::size_t& next) {
    if (candidate > banner_offset && static_cast<std::size_t>(candidate) <= rom_size) {
        next = std::min(next, static_cast<std::size_t>(candidate));
    }
}

} // namespace

std::size_t banner_size_from_version(std::span<const std::uint8_t> banner_data) {
    if (banner_data.size() < 2) {
        return 0;
    }
    core::LittleEndianReader reader(banner_data);
    const auto version = reader.u16(0);
    if (!version) {
        return 0;
    }
    switch (version.value()) {
    case 0x0001:
    case 0x0002:
        return 0x0A00;
    case 0x0003:
        return 0x0C00;
    case 0x0103:
        return 0x2400;
    default:
        return 0;
    }
}

// Where the region after the banner starts -- the banner's own header does not
// say how long it is, so the only bound the ROM gives is whatever comes next.
std::size_t banner_region_limit(const RomHeader& header, std::span<const FatEntry> fat, std::size_t rom_size) {
    auto next = rom_size;
    consider_next_region(header.arm9.rom_offset, header.banner_rom_offset, rom_size, next);
    consider_next_region(header.arm7.rom_offset, header.banner_rom_offset, rom_size, next);
    consider_next_region(header.fnt.rom_offset, header.banner_rom_offset, rom_size, next);
    consider_next_region(header.fat.rom_offset, header.banner_rom_offset, rom_size, next);
    consider_next_region(header.arm9_overlay_table.rom_offset, header.banner_rom_offset, rom_size, next);
    consider_next_region(header.arm7_overlay_table.rom_offset, header.banner_rom_offset, rom_size, next);
    consider_next_region(header.total_used_rom_size, header.banner_rom_offset, rom_size, next);
    for (const auto& entry : fat) {
        consider_next_region(entry.start_address, header.banner_rom_offset, rom_size, next);
    }
    return next;
}

namespace {

bool invalid_rom_name(const std::string& name) {
    return name.empty() || name.find('/') != std::string::npos || name.find('\\') != std::string::npos;
}

core::Result<std::vector<FntDirectoryEntry>> parse_fnt_directory_table(std::span<const std::uint8_t> data) {
    if (data.size() < 8) {
        return core::make_error(core::ErrorCode::ParseError, "FNT directory table is missing the root entry");
    }

    core::LittleEndianReader reader(data);
    const auto directory_count = reader.u16(6);
    if (!directory_count) {
        return directory_count.error();
    }
    if (directory_count.value() == 0 || directory_count.value() > 4096) {
        return core::make_error(core::ErrorCode::ParseError, "FNT directory count is invalid");
    }
    if (static_cast<std::size_t>(directory_count.value()) * 8 > data.size()) {
        return core::make_error(core::ErrorCode::ParseError, "FNT directory table extends past the FNT data");
    }

    std::vector<FntDirectoryEntry> directories;
    directories.reserve(directory_count.value());
    for (std::size_t i = 0; i < directory_count.value(); ++i) {
        const auto offset = i * 8;
        auto sub_table_offset = reader.u32(offset);
        auto first_file_id = reader.u16(offset + 4);
        auto third = reader.u16(offset + 6);
        if (!sub_table_offset) {
            return sub_table_offset.error();
        }
        if (!first_file_id) {
            return first_file_id.error();
        }
        if (!third) {
            return third.error();
        }

        FntDirectoryEntry entry;
        entry.sub_table_offset = sub_table_offset.value();
        entry.first_file_id = first_file_id.value();
        if (i == 0) {
            entry.directory_count = third.value();
            entry.parent_directory_id = root_directory_id;
        } else {
            entry.parent_directory_id = third.value();
        }
        directories.push_back(entry);
    }
    return directories;
}

} // namespace

bool FatEntry::used() const noexcept { return start_address != 0 || end_address != 0; }

std::uint32_t FatEntry::size() const noexcept {
    if (!used() || end_address < start_address) {
        return 0;
    }
    return end_address - start_address;
}

std::uint16_t crc16(std::span<const std::uint8_t> data, std::uint16_t initial) {
    auto crc = initial;
    for (const auto byte : data) {
        crc ^= byte;
        for (int bit = 0; bit < 8; ++bit) {
            if ((crc & 1) != 0) {
                crc = static_cast<std::uint16_t>((crc >> 1) ^ 0xA001);
            } else {
                crc = static_cast<std::uint16_t>(crc >> 1);
            }
        }
    }
    return crc;
}

core::Result<RomHeader> parse_rom_header(std::span<const std::uint8_t> data) {
    if (data.size() < ds_cartridge_header_size) {
        return core::make_error(core::ErrorCode::ParseError, "NDS header needs at least 0x200 bytes");
    }

    core::LittleEndianReader reader(data);
    auto read32 = [&](std::size_t offset) -> core::Result<std::uint32_t> { return reader.u32(offset); };
    auto read16 = [&](std::size_t offset) -> core::Result<std::uint16_t> { return reader.u16(offset); };

    RomHeader header;
    header.game_title = ascii_field(data, header_offset::game_title, 12);
    header.game_code = ascii_field(data, header_offset::game_code, 4);
    header.maker_code = ascii_field(data, header_offset::maker_code, 2);
    header.unit_code = data[header_offset::unit_code];
    header.encryption_seed_select = data[header_offset::encryption_seed_select];
    header.device_capacity = data[header_offset::device_capacity];
    header.region = data[header_offset::region];
    header.rom_version = data[header_offset::rom_version];
    header.autostart = data[header_offset::autostart];

    auto arm9_rom_offset = read32(header_offset::arm9_rom_offset);
    auto arm9_entry_address = read32(header_offset::arm9_entry_address);
    auto arm9_ram_address = read32(header_offset::arm9_ram_address);
    auto arm9_size = read32(header_offset::arm9_size);
    auto arm7_rom_offset = read32(header_offset::arm7_rom_offset);
    auto arm7_entry_address = read32(header_offset::arm7_entry_address);
    auto arm7_ram_address = read32(header_offset::arm7_ram_address);
    auto arm7_size = read32(header_offset::arm7_size);
    auto fnt_offset = read32(header_offset::fnt_offset);
    auto fnt_size = read32(header_offset::fnt_size);
    auto fat_offset = read32(header_offset::fat_offset);
    auto fat_size = read32(header_offset::fat_size);
    auto arm9_overlay_offset = read32(header_offset::arm9_overlay_offset);
    auto arm9_overlay_size = read32(header_offset::arm9_overlay_size);
    auto arm7_overlay_offset = read32(header_offset::arm7_overlay_offset);
    auto arm7_overlay_size = read32(header_offset::arm7_overlay_size);
    auto normal_card_control = read32(header_offset::normal_card_control);
    auto key1_card_control = read32(header_offset::key1_card_control);
    auto banner_rom_offset = read32(header_offset::banner_offset);
    auto secure_area_checksum = read16(header_offset::secure_area_checksum);
    auto secure_area_delay = read16(header_offset::secure_area_delay);
    auto arm9_autoload_hook_address = read32(header_offset::arm9_autoload_hook_address);
    auto arm7_autoload_hook_address = read32(header_offset::arm7_autoload_hook_address);
    auto total_used_rom_size = read32(header_offset::total_used_rom_size);
    auto header_size = read32(header_offset::header_size);
    auto nintendo_logo_checksum = read16(header_offset::nintendo_logo_checksum);
    auto header_checksum = read16(header_offset::header_checksum);

    if (!arm9_rom_offset) return arm9_rom_offset.error();
    if (!arm9_entry_address) return arm9_entry_address.error();
    if (!arm9_ram_address) return arm9_ram_address.error();
    if (!arm9_size) return arm9_size.error();
    if (!arm7_rom_offset) return arm7_rom_offset.error();
    if (!arm7_entry_address) return arm7_entry_address.error();
    if (!arm7_ram_address) return arm7_ram_address.error();
    if (!arm7_size) return arm7_size.error();
    if (!fnt_offset) return fnt_offset.error();
    if (!fnt_size) return fnt_size.error();
    if (!fat_offset) return fat_offset.error();
    if (!fat_size) return fat_size.error();
    if (!arm9_overlay_offset) return arm9_overlay_offset.error();
    if (!arm9_overlay_size) return arm9_overlay_size.error();
    if (!arm7_overlay_offset) return arm7_overlay_offset.error();
    if (!arm7_overlay_size) return arm7_overlay_size.error();
    if (!normal_card_control) return normal_card_control.error();
    if (!key1_card_control) return key1_card_control.error();
    if (!banner_rom_offset) return banner_rom_offset.error();
    if (!secure_area_checksum) return secure_area_checksum.error();
    if (!secure_area_delay) return secure_area_delay.error();
    if (!arm9_autoload_hook_address) return arm9_autoload_hook_address.error();
    if (!arm7_autoload_hook_address) return arm7_autoload_hook_address.error();
    if (!total_used_rom_size) return total_used_rom_size.error();
    if (!header_size) return header_size.error();
    if (!nintendo_logo_checksum) return nintendo_logo_checksum.error();
    if (!header_checksum) return header_checksum.error();

    header.arm9 = RomArmBinary{arm9_rom_offset.value(), arm9_entry_address.value(), arm9_ram_address.value(), arm9_size.value()};
    header.arm7 = RomArmBinary{arm7_rom_offset.value(), arm7_entry_address.value(), arm7_ram_address.value(), arm7_size.value()};
    header.fnt = RomRegion{fnt_offset.value(), fnt_size.value()};
    header.fat = RomRegion{fat_offset.value(), fat_size.value()};
    header.arm9_overlay_table = RomRegion{arm9_overlay_offset.value(), arm9_overlay_size.value()};
    header.arm7_overlay_table = RomRegion{arm7_overlay_offset.value(), arm7_overlay_size.value()};
    header.normal_card_control = normal_card_control.value();
    header.key1_card_control = key1_card_control.value();
    header.banner_rom_offset = banner_rom_offset.value();
    header.secure_area_checksum = secure_area_checksum.value();
    header.secure_area_delay = secure_area_delay.value();
    header.arm9_autoload_hook_address = arm9_autoload_hook_address.value();
    header.arm7_autoload_hook_address = arm7_autoload_hook_address.value();
    header.total_used_rom_size = total_used_rom_size.value();
    header.header_size = header_size.value();
    header.nintendo_logo_checksum = nintendo_logo_checksum.value();
    header.header_checksum = header_checksum.value();
    header.arm9_rom_offset = header.arm9.rom_offset;
    header.arm7_rom_offset = header.arm7.rom_offset;
    return header;
}

core::Result<std::vector<FatEntry>> parse_fat(std::span<const std::uint8_t> data) {
    if (data.size() % 8 != 0) {
        return core::make_error(core::ErrorCode::ParseError, "FAT size must be a multiple of 8 bytes");
    }

    core::LittleEndianReader reader(data);
    std::vector<FatEntry> entries;
    entries.reserve(data.size() / 8);
    for (std::size_t offset = 0; offset < data.size(); offset += 8) {
        auto start = reader.u32(offset);
        auto end = reader.u32(offset + 4);
        if (!start) {
            return start.error();
        }
        if (!end) {
            return end.error();
        }
        if (end.value() < start.value()) {
            return core::make_error(core::ErrorCode::ParseError, "FAT entry end address is before its start address");
        }
        entries.push_back(FatEntry{start.value(), end.value()});
    }
    return entries;
}

core::Result<std::vector<NitroFileEntry>> parse_fnt(std::span<const std::uint8_t> data) {
    auto directories = parse_fnt_directory_table(data);
    if (!directories) {
        return directories.error();
    }

    std::vector<NitroFileEntry> files;
    std::vector<bool> visiting(directories.value().size(), false);
    std::vector<bool> visited(directories.value().size(), false);

    auto parse_directory = [&](auto&& self, std::size_t directory_index, const fs::path& base_path) -> core::Result<void> {
        if (directory_index >= directories.value().size()) {
            return core::make_error(core::ErrorCode::ParseError, "FNT sub-directory ID is out of range");
        }
        if (visiting[directory_index]) {
            return core::make_error(core::ErrorCode::ParseError, "FNT directory table contains a cycle");
        }
        if (visited[directory_index]) {
            return {};
        }

        visiting[directory_index] = true;
        const auto& directory = directories.value()[directory_index];
        auto offset = static_cast<std::size_t>(directory.sub_table_offset);
        if (offset >= data.size()) {
            return core::make_error(core::ErrorCode::ParseError, "FNT sub-table offset is out of bounds");
        }

        auto next_file_id = directory.first_file_id;
        while (true) {
            if (offset >= data.size()) {
                return core::make_error(core::ErrorCode::ParseError, "FNT sub-table is missing its terminator");
            }
            const auto type_length = data[offset++];
            if (type_length == 0) {
                break;
            }

            const bool is_directory = (type_length & 0x80) != 0;
            const auto length = static_cast<std::size_t>(type_length & 0x7F);
            if (length == 0 || length > data.size() - offset) {
                return core::make_error(core::ErrorCode::ParseError, "FNT entry name length is invalid");
            }
            const std::string name(reinterpret_cast<const char*>(data.data() + offset), length);
            offset += length;
            if (invalid_rom_name(name)) {
                return core::make_error(core::ErrorCode::ParseError, "FNT entry contains an invalid path name");
            }

            if (is_directory) {
                if (2 > data.size() - offset) {
                    return core::make_error(core::ErrorCode::ParseError, "FNT directory entry is missing its directory ID");
                }
                const auto id = static_cast<std::uint16_t>(data[offset] | (data[offset + 1] << 8));
                offset += 2;
                if (id <= root_directory_id) {
                    return core::make_error(core::ErrorCode::ParseError, "FNT sub-directory ID is invalid");
                }
                auto parsed = self(self, static_cast<std::size_t>(id - root_directory_id), base_path / name);
                if (!parsed) {
                    return parsed.error();
                }
            } else {
                if (next_file_id > 0xEFFF) {
                    return core::make_error(core::ErrorCode::ParseError, "FNT file ID is outside the valid file range");
                }
                files.push_back(NitroFileEntry{next_file_id, base_path / name});
                ++next_file_id;
            }
        }

        visiting[directory_index] = false;
        visited[directory_index] = true;
        return {};
    };

    auto parsed = parse_directory(parse_directory, 0, {});
    if (!parsed) {
        return parsed.error();
    }

    std::sort(files.begin(), files.end(), [](const auto& a, const auto& b) { return a.id < b.id; });
    return files;
}

core::Result<std::vector<OverlayTableEntry>> parse_overlay_table(std::span<const std::uint8_t> data) {
    if (data.size() % overlay_table_entry_size != 0) {
        return core::make_error(core::ErrorCode::ParseError, "overlay table size must be a multiple of 0x20 bytes");
    }

    core::LittleEndianReader reader(data);
    std::vector<OverlayTableEntry> entries;
    entries.reserve(data.size() / overlay_table_entry_size);
    for (std::size_t offset = 0; offset < data.size(); offset += overlay_table_entry_size) {
        auto overlay_id = reader.u32(offset);
        auto ram_address = reader.u32(offset + 0x04);
        auto ram_size = reader.u32(offset + 0x08);
        auto bss_size = reader.u32(offset + 0x0C);
        auto static_initializer_start_address = reader.u32(offset + 0x10);
        auto static_initializer_end_address = reader.u32(offset + 0x14);
        auto file_id = reader.u32(offset + 0x18);
        auto reserved = reader.u32(offset + 0x1C);
        if (!overlay_id) return overlay_id.error();
        if (!ram_address) return ram_address.error();
        if (!ram_size) return ram_size.error();
        if (!bss_size) return bss_size.error();
        if (!static_initializer_start_address) return static_initializer_start_address.error();
        if (!static_initializer_end_address) return static_initializer_end_address.error();
        if (!file_id) return file_id.error();
        if (!reserved) return reserved.error();
        entries.push_back(OverlayTableEntry{overlay_id.value(),
                                            ram_address.value(),
                                            ram_size.value(),
                                            bss_size.value(),
                                            static_initializer_start_address.value(),
                                            static_initializer_end_address.value(),
                                            file_id.value(),
                                            reserved.value()});
    }
    return entries;
}

} // namespace nsmb::nds
