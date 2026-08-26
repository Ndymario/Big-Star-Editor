#include "nsmb/nds/rom.hpp"

#include "nsmb/core/endian.hpp"
#include "nsmb/core/file_io.hpp"

#include <algorithm>
#include <cstddef>
#include <fstream>
#include <iomanip>
#include <limits>
#include <optional>
#include <sstream>
#include <system_error>

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

struct GeneratedFnt {
    std::vector<std::uint8_t> bytes;
    std::vector<NitroFileEntry> files;
};

struct BuildDirectory {
    fs::path disk_path;
    fs::path rom_path;
    std::string name;
    std::uint16_t parent_id{root_directory_id};
    std::uint16_t first_file_id{};
    std::vector<std::string> files;
    std::vector<std::size_t> children;
};

std::string ascii_field(std::span<const std::uint8_t> data, std::size_t offset, std::size_t length) {
    std::string out(reinterpret_cast<const char*>(data.data() + offset), length);
    while (!out.empty() && (out.back() == '\0' || out.back() == ' ')) {
        out.pop_back();
    }
    return out;
}

std::size_t align_up(std::size_t value, std::size_t alignment) {
    if (alignment == 0) {
        return value;
    }
    const auto remainder = value % alignment;
    return remainder == 0 ? value : value + (alignment - remainder);
}

void append_u16(std::vector<std::uint8_t>& data, std::uint16_t value) {
    data.push_back(static_cast<std::uint8_t>(value & 0xFF));
    data.push_back(static_cast<std::uint8_t>((value >> 8) & 0xFF));
}

void put_u16(std::vector<std::uint8_t>& data, std::size_t offset, std::uint16_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFF);
    data[offset + 1] = static_cast<std::uint8_t>((value >> 8) & 0xFF);
}

void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset] = static_cast<std::uint8_t>(value & 0xFF);
    data[offset + 1] = static_cast<std::uint8_t>((value >> 8) & 0xFF);
    data[offset + 2] = static_cast<std::uint8_t>((value >> 16) & 0xFF);
    data[offset + 3] = static_cast<std::uint8_t>((value >> 24) & 0xFF);
}

core::Result<std::vector<std::uint8_t>> read_file(const fs::path& path) {
    return core::read_file(path);
}

core::Result<std::vector<std::uint8_t>> read_optional_file(const fs::path& path) {
    std::error_code ec;
    if (!fs::exists(path, ec)) {
        return std::vector<std::uint8_t>{};
    }
    return read_file(path);
}

core::Result<void> write_file(const fs::path& path, std::span<const std::uint8_t> data) {
    if (data.size() > static_cast<std::size_t>(std::numeric_limits<std::streamsize>::max())) {
        return core::make_error(core::ErrorCode::InvalidArgument, "file is too large to write");
    }
    std::error_code ec;
    const auto parent = path.parent_path();
    if (!parent.empty()) {
        fs::create_directories(parent, ec);
        if (ec) {
            return core::make_error(core::ErrorCode::IoError, "could not create directory for " + path.string());
        }
    }

    std::ofstream out(path, std::ios::binary);
    if (!out) {
        return core::make_error(core::ErrorCode::IoError, "could not write " + path.string());
    }
    if (!data.empty()) {
        out.write(reinterpret_cast<const char*>(data.data()), static_cast<std::streamsize>(data.size()));
    }
    out.flush();
    if (!out) {
        return core::make_error(core::ErrorCode::IoError, "could not finish writing " + path.string());
    }
    return {};
}

core::Result<std::span<const std::uint8_t>> checked_region(std::span<const std::uint8_t> rom,
                                                           std::uint32_t offset,
                                                           std::uint32_t size,
                                                           const std::string& name) {
    const auto start = static_cast<std::size_t>(offset);
    const auto length = static_cast<std::size_t>(size);
    if (start > rom.size() || length > rom.size() - start) {
        return core::make_error(core::ErrorCode::OutOfBounds, name + " extends past the end of the ROM");
    }
    return std::span<const std::uint8_t>(rom.data() + start, length);
}

std::size_t normalized_header_region_size(const RomHeader& header, std::size_t rom_size) {
    auto size = static_cast<std::size_t>(header.header_size);
    if (size < ds_cartridge_header_size || size > rom_size) {
        size = std::min(default_header_region_size, rom_size);
    }
    return std::max<std::size_t>(size, ds_cartridge_header_size);
}

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

void consider_next_region(std::uint32_t candidate, std::uint32_t banner_offset, std::size_t rom_size, std::size_t& next) {
    if (candidate > banner_offset && static_cast<std::size_t>(candidate) <= rom_size) {
        next = std::min(next, static_cast<std::size_t>(candidate));
    }
}

std::size_t infer_banner_size(std::span<const std::uint8_t> rom, const RomHeader& header, const std::vector<FatEntry>& fat) {
    if (header.banner_rom_offset == 0 || static_cast<std::size_t>(header.banner_rom_offset) >= rom.size()) {
        return 0;
    }

    auto next = rom.size();
    consider_next_region(header.arm9.rom_offset, header.banner_rom_offset, rom.size(), next);
    consider_next_region(header.arm7.rom_offset, header.banner_rom_offset, rom.size(), next);
    consider_next_region(header.fnt.rom_offset, header.banner_rom_offset, rom.size(), next);
    consider_next_region(header.fat.rom_offset, header.banner_rom_offset, rom.size(), next);
    consider_next_region(header.arm9_overlay_table.rom_offset, header.banner_rom_offset, rom.size(), next);
    consider_next_region(header.arm7_overlay_table.rom_offset, header.banner_rom_offset, rom.size(), next);
    consider_next_region(header.total_used_rom_size, header.banner_rom_offset, rom.size(), next);
    for (const auto& entry : fat) {
        consider_next_region(entry.start_address, header.banner_rom_offset, rom.size(), next);
    }

    const auto available = next - static_cast<std::size_t>(header.banner_rom_offset);
    const auto banner_data = std::span<const std::uint8_t>(rom.data() + header.banner_rom_offset, rom.size() - header.banner_rom_offset);
    const auto implied = banner_size_from_version(banner_data);
    if (implied != 0 && implied <= available) {
        return implied;
    }
    return available;
}

bool invalid_rom_name(const std::string& name) {
    return name.empty() || name.find('/') != std::string::npos || name.find('\\') != std::string::npos;
}

bool path_starts_with(const fs::path& path, const fs::path& prefix) {
    auto path_part = path.begin();
    for (auto prefix_part = prefix.begin(); prefix_part != prefix.end(); ++prefix_part, ++path_part) {
        if (path_part == path.end() || *path_part != *prefix_part) {
            return false;
        }
    }
    return true;
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

core::Result<void> collect_build_directory(std::vector<BuildDirectory>& directories, std::size_t index) {
    std::error_code ec;
    std::vector<fs::directory_entry> entries;
    for (const auto& entry : fs::directory_iterator(directories[index].disk_path, ec)) {
        if (ec) {
            return core::make_error(core::ErrorCode::IoError, "could not read " + directories[index].disk_path.string());
        }
        entries.push_back(entry);
    }
    std::sort(entries.begin(), entries.end(), [](const auto& a, const auto& b) {
        return a.path().filename().string() < b.path().filename().string();
    });

    for (const auto& entry : entries) {
        const auto name = entry.path().filename().string();
        if (invalid_rom_name(name)) {
            return core::make_error(core::ErrorCode::InvalidArgument, "invalid NitroFS name: " + name);
        }
        if (entry.is_directory(ec)) {
            BuildDirectory child;
            child.disk_path = entry.path();
            child.rom_path = directories[index].rom_path / name;
            child.name = name;
            child.parent_id = static_cast<std::uint16_t>(root_directory_id + index);
            const auto child_index = directories.size();
            directories[index].children.push_back(child_index);
            directories.push_back(std::move(child));
            auto collected = collect_build_directory(directories, child_index);
            if (!collected) {
                return collected.error();
            }
        } else if (entry.is_regular_file(ec)) {
            directories[index].files.push_back(name);
        }
    }

    return {};
}

core::Result<GeneratedFnt> generate_fnt_from_root(const fs::path& root) {
    std::error_code ec;
    if (!fs::is_directory(root, ec)) {
        return core::make_error(core::ErrorCode::NotFound, "NitroFS root directory does not exist");
    }

    std::vector<BuildDirectory> directories;
    directories.push_back(BuildDirectory{root, {}, {}, root_directory_id, 0, {}, {}});
    auto collected = collect_build_directory(directories, 0);
    if (!collected) {
        return collected.error();
    }
    if (directories.size() > 4096) {
        return core::make_error(core::ErrorCode::InvalidArgument, "NitroFS contains more than 4096 directories");
    }

    const auto table_size = directories.size() * 8;
    std::vector<std::uint8_t> fnt(table_size, 0);
    std::vector<NitroFileEntry> files;
    std::uint16_t next_file_id = 0;

    for (std::size_t i = 0; i < directories.size(); ++i) {
        auto& directory = directories[i];
        directory.first_file_id = next_file_id;
        const auto sub_table_offset = fnt.size();

        for (const auto& name : directory.files) {
            if (name.size() > 127) {
                return core::make_error(core::ErrorCode::InvalidArgument, "NitroFS file name is longer than 127 bytes");
            }
            if (next_file_id > 0xEFFF) {
                return core::make_error(core::ErrorCode::InvalidArgument, "NitroFS contains more than 61440 files");
            }
            fnt.push_back(static_cast<std::uint8_t>(name.size()));
            fnt.insert(fnt.end(), name.begin(), name.end());
            files.push_back(NitroFileEntry{next_file_id, directory.rom_path / name});
            ++next_file_id;
        }

        for (const auto child_index : directory.children) {
            const auto& child = directories[child_index];
            if (child.name.size() > 127) {
                return core::make_error(core::ErrorCode::InvalidArgument, "NitroFS directory name is longer than 127 bytes");
            }
            fnt.push_back(static_cast<std::uint8_t>(0x80 | child.name.size()));
            fnt.insert(fnt.end(), child.name.begin(), child.name.end());
            append_u16(fnt, static_cast<std::uint16_t>(root_directory_id + child_index));
        }
        fnt.push_back(0);

        put_u32(fnt, i * 8, static_cast<std::uint32_t>(sub_table_offset));
        put_u16(fnt, i * 8 + 4, directory.first_file_id);
        put_u16(fnt, i * 8 + 6, i == 0 ? static_cast<std::uint16_t>(directories.size()) : directory.parent_id);
    }

    std::sort(files.begin(), files.end(), [](const auto& a, const auto& b) { return a.id < b.id; });
    return GeneratedFnt{std::move(fnt), std::move(files)};
}

std::uint32_t append_aligned(std::vector<std::uint8_t>& output, std::span<const std::uint8_t> data, std::size_t alignment) {
    const auto offset = align_up(output.size(), alignment);
    output.resize(offset, 0xFF);
    output.insert(output.end(), data.begin(), data.end());
    return static_cast<std::uint32_t>(offset);
}

core::Result<void> write_fat_entry(std::vector<std::uint8_t>& fat, std::size_t index, const FatEntry& entry) {
    const auto offset = index * 8;
    if (offset > fat.size() || 8 > fat.size() - offset) {
        return core::make_error(core::ErrorCode::OutOfBounds, "FAT write is out of bounds");
    }
    put_u32(fat, offset, entry.start_address);
    put_u32(fat, offset + 4, entry.end_address);
    return {};
}

core::Result<void> set_header_u16(core::LittleEndianWriter& writer, std::size_t offset, std::uint16_t value) {
    auto written = writer.u16(offset, value);
    if (!written) {
        return written.error();
    }
    return {};
}

core::Result<void> set_header_u32(core::LittleEndianWriter& writer, std::size_t offset, std::uint32_t value) {
    auto written = writer.u32(offset, value);
    if (!written) {
        return written.error();
    }
    return {};
}

} // namespace

bool FatEntry::used() const noexcept { return start_address != 0 || end_address != 0; }

std::uint32_t FatEntry::size() const noexcept {
    if (!used() || end_address < start_address) {
        return 0;
    }
    return end_address - start_address;
}

std::string overlay_file_name(std::uint32_t overlay_id) {
    std::ostringstream out;
    out << "overlay_" << std::setw(4) << std::setfill('0') << overlay_id << ".bin";
    return out.str();
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

core::Result<ExtractedRom> NitroRomService::extract(const fs::path& rom, const fs::path& output_dir) {
    if (rom.empty() || output_dir.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "ROM input and extraction output paths are required");
    }
    std::error_code ec;
    if (fs::exists(output_dir, ec)) {
        if (!fs::is_directory(output_dir, ec)) {
            return core::make_error(core::ErrorCode::InvalidArgument, "ROM extraction output must be a directory");
        }
        if (!fs::is_empty(output_dir, ec)) {
            return core::make_error(core::ErrorCode::InvalidArgument, "ROM extraction output directory must be empty");
        }
    }
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not inspect ROM extraction output directory");
    }

    auto rom_data = read_file(rom);
    if (!rom_data) {
        return rom_data.error();
    }
    auto header = parse_rom_header(rom_data.value());
    if (!header) {
        return header.error();
    }

    const auto header_size = normalized_header_region_size(header.value(), rom_data.value().size());
    auto header_region = checked_region(rom_data.value(), 0, static_cast<std::uint32_t>(header_size), "header");
    if (!header_region) {
        return header_region.error();
    }
    auto written = write_file(output_dir / "header.bin", header_region.value());
    if (!written) {
        return written.error();
    }

    auto arm9 = checked_region(rom_data.value(), header.value().arm9.rom_offset, header.value().arm9.size, "ARM9 binary");
    if (!arm9) {
        return arm9.error();
    }
    written = write_file(output_dir / "arm9.bin", arm9.value());
    if (!written) {
        return written.error();
    }

    auto arm7 = checked_region(rom_data.value(), header.value().arm7.rom_offset, header.value().arm7.size, "ARM7 binary");
    if (!arm7) {
        return arm7.error();
    }
    written = write_file(output_dir / "arm7.bin", arm7.value());
    if (!written) {
        return written.error();
    }

    auto fnt_region = checked_region(rom_data.value(), header.value().fnt.rom_offset, header.value().fnt.size, "FNT");
    if (!fnt_region) {
        return fnt_region.error();
    }
    written = write_file(output_dir / "fnt.bin", fnt_region.value());
    if (!written) {
        return written.error();
    }

    auto fat_region = checked_region(rom_data.value(), header.value().fat.rom_offset, header.value().fat.size, "FAT");
    if (!fat_region) {
        return fat_region.error();
    }
    written = write_file(output_dir / "fat.bin", fat_region.value());
    if (!written) {
        return written.error();
    }

    auto fat = parse_fat(fat_region.value());
    if (!fat) {
        return fat.error();
    }
    auto files = parse_fnt(fnt_region.value());
    if (!files) {
        return files.error();
    }
    for (const auto& file : files.value()) {
        if (file.id >= fat.value().size()) {
            return core::make_error(core::ErrorCode::ParseError, "FNT file ID has no FAT entry");
        }
        const auto& fat_entry = fat.value()[file.id];
        std::span<const std::uint8_t> file_data;
        if (fat_entry.used()) {
            auto region = checked_region(rom_data.value(), fat_entry.start_address, fat_entry.size(), "NitroFS file");
            if (!region) {
                return region.error();
            }
            file_data = region.value();
        }
        written = write_file(output_dir / "root" / file.path, file_data);
        if (!written) {
            return written.error();
        }
    }

    auto arm9_ovt = checked_region(rom_data.value(),
                                   header.value().arm9_overlay_table.rom_offset,
                                   header.value().arm9_overlay_table.size,
                                   "ARM9 overlay table");
    if (!arm9_ovt) {
        return arm9_ovt.error();
    }
    written = write_file(output_dir / "arm9ovt.bin", arm9_ovt.value());
    if (!written) {
        return written.error();
    }

    auto arm7_ovt = checked_region(rom_data.value(),
                                   header.value().arm7_overlay_table.rom_offset,
                                   header.value().arm7_overlay_table.size,
                                   "ARM7 overlay table");
    if (!arm7_ovt) {
        return arm7_ovt.error();
    }
    written = write_file(output_dir / "arm7ovt.bin", arm7_ovt.value());
    if (!written) {
        return written.error();
    }

    auto write_overlays = [&](std::span<const std::uint8_t> table, const fs::path& directory) -> core::Result<void> {
        auto overlays = parse_overlay_table(table);
        if (!overlays) {
            return overlays.error();
        }
        for (const auto& overlay : overlays.value()) {
            if (overlay.file_id > 0xEFFF || overlay.file_id >= fat.value().size()) {
                return core::make_error(core::ErrorCode::ParseError, "overlay table file ID has no FAT entry");
            }
            const auto& fat_entry = fat.value()[static_cast<std::size_t>(overlay.file_id)];
            if (!fat_entry.used()) {
                continue;
            }
            auto region = checked_region(rom_data.value(), fat_entry.start_address, fat_entry.size(), "overlay file");
            if (!region) {
                return region.error();
            }
            auto overlay_written = write_file(output_dir / directory / overlay_file_name(overlay.overlay_id), region.value());
            if (!overlay_written) {
                return overlay_written.error();
            }
        }
        return {};
    };

    auto overlays_written = write_overlays(arm9_ovt.value(), "overlay9");
    if (!overlays_written) {
        return overlays_written.error();
    }
    overlays_written = write_overlays(arm7_ovt.value(), "overlay7");
    if (!overlays_written) {
        return overlays_written.error();
    }

    const auto banner_size = infer_banner_size(rom_data.value(), header.value(), fat.value());
    if (banner_size != 0) {
        auto banner = checked_region(rom_data.value(), header.value().banner_rom_offset, static_cast<std::uint32_t>(banner_size), "banner");
        if (!banner) {
            return banner.error();
        }
        written = write_file(output_dir / "banner.bin", banner.value());
        if (!written) {
            return written.error();
        }
    }

    auto tree = scan_file_tree(output_dir);
    if (!tree) {
        return tree.error();
    }
    return ExtractedRom{output_dir, tree.value()};
}

core::Result<void> NitroRomService::build(const fs::path& extracted_dir, const fs::path& output_rom) {
    if (extracted_dir.empty() || output_rom.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "extracted ROM input and output ROM paths are required");
    }
    std::error_code ec;
    if (!fs::is_directory(extracted_dir, ec)) {
        return core::make_error(core::ErrorCode::NotFound, "extracted ROM directory does not exist");
    }
    const auto canonical_input = fs::weakly_canonical(extracted_dir, ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not resolve extracted ROM directory");
    }
    const auto canonical_output = fs::weakly_canonical(output_rom, ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not resolve ROM output path");
    }
    if (path_starts_with(canonical_output, canonical_input)) {
        return core::make_error(core::ErrorCode::InvalidArgument, "ROM output must be outside the extracted ROM directory");
    }

    auto header_bytes = read_file(extracted_dir / "header.bin");
    if (!header_bytes) {
        return header_bytes.error();
    }
    if (header_bytes.value().size() < ds_cartridge_header_size) {
        return core::make_error(core::ErrorCode::ParseError, "header.bin needs at least 0x200 bytes");
    }
    auto header = parse_rom_header(header_bytes.value());
    if (!header) {
        return header.error();
    }

    auto arm9 = read_file(extracted_dir / "arm9.bin");
    if (!arm9) {
        return arm9.error();
    }
    auto arm7 = read_file(extracted_dir / "arm7.bin");
    if (!arm7) {
        return arm7.error();
    }

    std::vector<std::uint8_t> fnt_data;
    std::vector<NitroFileEntry> files;
    if (fs::exists(extracted_dir / "fnt.bin", ec)) {
        auto fnt = read_file(extracted_dir / "fnt.bin");
        if (!fnt) {
            return fnt.error();
        }
        fnt_data = std::move(fnt.value());
        auto parsed_files = parse_fnt(fnt_data);
        if (!parsed_files) {
            return parsed_files.error();
        }
        files = std::move(parsed_files.value());
    } else {
        auto generated = generate_fnt_from_root(extracted_dir / "root");
        if (!generated) {
            return generated.error();
        }
        fnt_data = std::move(generated.value().bytes);
        files = std::move(generated.value().files);
    }

    auto existing_fat_data = read_optional_file(extracted_dir / "fat.bin");
    if (!existing_fat_data) {
        return existing_fat_data.error();
    }
    std::size_t fat_entry_count = 0;
    if (!existing_fat_data.value().empty()) {
        auto existing_fat = parse_fat(existing_fat_data.value());
        if (!existing_fat) {
            return existing_fat.error();
        }
        fat_entry_count = existing_fat.value().size();
    }

    auto arm9_ovt = read_optional_file(extracted_dir / "arm9ovt.bin");
    if (!arm9_ovt) {
        return arm9_ovt.error();
    }
    auto arm7_ovt = read_optional_file(extracted_dir / "arm7ovt.bin");
    if (!arm7_ovt) {
        return arm7_ovt.error();
    }
    auto arm9_overlays = parse_overlay_table(arm9_ovt.value());
    if (!arm9_overlays) {
        return arm9_overlays.error();
    }
    auto arm7_overlays = parse_overlay_table(arm7_ovt.value());
    if (!arm7_overlays) {
        return arm7_overlays.error();
    }

    std::uint32_t max_file_id = 0;
    bool has_file = false;
    for (const auto& file : files) {
        max_file_id = std::max<std::uint32_t>(max_file_id, file.id);
        has_file = true;
    }
    for (const auto& overlay : arm9_overlays.value()) {
        if (overlay.file_id > 0xEFFF) {
            return core::make_error(core::ErrorCode::ParseError, "ARM9 overlay file ID is outside the valid file range");
        }
        max_file_id = std::max(max_file_id, overlay.file_id);
        has_file = true;
    }
    for (const auto& overlay : arm7_overlays.value()) {
        if (overlay.file_id > 0xEFFF) {
            return core::make_error(core::ErrorCode::ParseError, "ARM7 overlay file ID is outside the valid file range");
        }
        max_file_id = std::max(max_file_id, overlay.file_id);
        has_file = true;
    }
    if (has_file) {
        fat_entry_count = std::max<std::size_t>(fat_entry_count, static_cast<std::size_t>(max_file_id) + 1);
    }

    std::vector<std::optional<std::vector<std::uint8_t>>> file_data(fat_entry_count);
    for (const auto& file : files) {
        auto data = read_file(extracted_dir / "root" / file.path);
        if (!data) {
            return data.error();
        }
        file_data[file.id] = std::move(data.value());
    }

    auto apply_overlay_data = [&](const std::vector<OverlayTableEntry>& overlays, const fs::path& directory) -> core::Result<void> {
        for (const auto& overlay : overlays) {
            auto path = extracted_dir / directory / overlay_file_name(overlay.overlay_id);
            if (fs::exists(path, ec)) {
                auto data = read_file(path);
                if (!data) {
                    return data.error();
                }
                file_data[static_cast<std::size_t>(overlay.file_id)] = std::move(data.value());
            } else if (!file_data[static_cast<std::size_t>(overlay.file_id)]) {
                return core::make_error(core::ErrorCode::NotFound, "overlay file is missing: " + path.string());
            }
        }
        return {};
    };

    auto overlays_applied = apply_overlay_data(arm9_overlays.value(), "overlay9");
    if (!overlays_applied) {
        return overlays_applied.error();
    }
    overlays_applied = apply_overlay_data(arm7_overlays.value(), "overlay7");
    if (!overlays_applied) {
        return overlays_applied.error();
    }

    auto banner = read_optional_file(extracted_dir / "banner.bin");
    if (!banner) {
        return banner.error();
    }

    auto header_region_size = static_cast<std::size_t>(header.value().header_size);
    if (header_region_size < ds_cartridge_header_size) {
        header_region_size = default_header_region_size;
    }
    header_region_size = std::max(header_region_size, header_bytes.value().size());
    header_bytes.value().resize(header_region_size, 0xFF);

    std::vector<std::uint8_t> output(header_region_size, 0xFF);
    const auto arm9_offset = append_aligned(output, arm9.value(), 0x1000);
    const auto arm7_offset = append_aligned(output, arm7.value(), 0x200);
    const auto fnt_offset = append_aligned(output, fnt_data, 0x200);
    const auto fat_offset = append_aligned(output, std::span<const std::uint8_t>{}, 0x200);
    output.resize(output.size() + fat_entry_count * 8, 0xFF);
    const auto arm9_ovt_offset = append_aligned(output, arm9_ovt.value(), 0x200);
    const auto arm7_ovt_offset = append_aligned(output, arm7_ovt.value(), 0x200);
    std::uint32_t banner_offset = 0;
    if (!banner.value().empty()) {
        banner_offset = append_aligned(output, banner.value(), 0x200);
    }

    std::vector<FatEntry> fat_entries(fat_entry_count);
    for (std::size_t i = 0; i < file_data.size(); ++i) {
        if (!file_data[i]) {
            continue;
        }
        const auto offset = align_up(output.size(), 0x200);
        output.resize(offset, 0xFF);
        const auto start = static_cast<std::uint32_t>(output.size());
        output.insert(output.end(), file_data[i]->begin(), file_data[i]->end());
        const auto end = static_cast<std::uint32_t>(output.size());
        fat_entries[i] = FatEntry{start, end};
    }

    std::vector<std::uint8_t> fat_data(fat_entry_count * 8, 0);
    for (std::size_t i = 0; i < fat_entries.size(); ++i) {
        auto written = write_fat_entry(fat_data, i, fat_entries[i]);
        if (!written) {
            return written.error();
        }
    }
    std::copy(fat_data.begin(), fat_data.end(), output.begin() + static_cast<std::ptrdiff_t>(fat_offset));

    if (arm9_offset < 0x8000) {
        output.resize(std::max<std::size_t>(output.size(), 0x8000), 0xFF);
    }
    output.resize(align_up(output.size(), 0x200), 0xFF);

    core::LittleEndianWriter writer(header_bytes.value());
    auto header_written = set_header_u32(writer, header_offset::arm9_rom_offset, arm9_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm9_size, static_cast<std::uint32_t>(arm9.value().size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm7_rom_offset, arm7_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm7_size, static_cast<std::uint32_t>(arm7.value().size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::fnt_offset, fnt_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::fnt_size, static_cast<std::uint32_t>(fnt_data.size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::fat_offset, fat_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::fat_size, static_cast<std::uint32_t>(fat_data.size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm9_overlay_offset, arm9_ovt_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm9_overlay_size, static_cast<std::uint32_t>(arm9_ovt.value().size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm7_overlay_offset, arm7_ovt_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::arm7_overlay_size, static_cast<std::uint32_t>(arm7_ovt.value().size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::banner_offset, banner_offset);
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::total_used_rom_size, static_cast<std::uint32_t>(output.size()));
    if (!header_written) return header_written.error();
    header_written = set_header_u32(writer, header_offset::header_size, static_cast<std::uint32_t>(header_region_size));
    if (!header_written) return header_written.error();

    std::uint16_t secure_checksum = 0;
    if (arm9_offset < 0x8000) {
        secure_checksum = crc16(std::span<const std::uint8_t>(output.data() + arm9_offset, 0x8000 - arm9_offset));
    }
    header_written = set_header_u16(writer, header_offset::secure_area_checksum, secure_checksum);
    if (!header_written) return header_written.error();
    const auto checksum = crc16(std::span<const std::uint8_t>(header_bytes.value().data(), header_offset::header_checksum));
    header_written = set_header_u16(writer, header_offset::header_checksum, checksum);
    if (!header_written) return header_written.error();

    std::copy(header_bytes.value().begin(), header_bytes.value().end(), output.begin());
    return core::write_file_atomically(output_rom, output);
}

core::Result<FileTreeNode> scan_file_tree(const std::filesystem::path& root) {
    std::error_code ec;
    const auto exists = std::filesystem::exists(root, ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not inspect file tree root");
    }
    if (!exists) {
        return core::make_error(core::ErrorCode::NotFound, "file tree root does not exist");
    }
    const auto directory = std::filesystem::is_directory(root, ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not inspect " + root.string());
    }
    FileTreeNode node{root.filename().string(), root, directory, {}};
    if (node.directory) {
        std::filesystem::directory_iterator current(root, ec);
        const std::filesystem::directory_iterator end;
        if (ec) {
            return core::make_error(core::ErrorCode::IoError, "could not read " + root.string());
        }
        while (current != end) {
            auto child = scan_file_tree(current->path());
            if (!child) {
                return child.error();
            }
            node.children.push_back(child.value());
            current.increment(ec);
            if (ec) {
                return core::make_error(core::ErrorCode::IoError, "could not continue reading " + root.string());
            }
        }
        std::sort(node.children.begin(), node.children.end(), [](const auto& a, const auto& b) { return a.name < b.name; });
    }
    return node;
}

} // namespace nsmb::nds
