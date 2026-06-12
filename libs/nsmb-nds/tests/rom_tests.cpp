#include "nsmb/nds/rom.hpp"

#include <algorithm>
#include <cassert>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <iterator>
#include <string>
#include <vector>

namespace {

namespace fs = std::filesystem;

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

void write_bytes(const fs::path& path, const std::vector<std::uint8_t>& bytes) {
    fs::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
}

std::vector<std::uint8_t> read_bytes(const fs::path& path) {
    std::ifstream in(path, std::ios::binary);
    return std::vector<std::uint8_t>(std::istreambuf_iterator<char>(in), {});
}

std::vector<std::uint8_t> make_fnt() {
    std::vector<std::uint8_t> fnt(8, 0);
    put_u32(fnt, 0, 8);
    put_u16(fnt, 4, 0);
    put_u16(fnt, 6, 1);
    const std::string file_name = "course.bin";
    fnt.push_back(static_cast<std::uint8_t>(file_name.size()));
    fnt.insert(fnt.end(), file_name.begin(), file_name.end());
    fnt.push_back(0);
    return fnt;
}

std::vector<std::uint8_t> make_test_rom() {
    constexpr std::uint32_t arm9_offset = 0x4000;
    constexpr std::uint32_t arm9_size = 0x20;
    constexpr std::uint32_t arm7_offset = 0x8000;
    constexpr std::uint32_t arm7_size = 0x20;
    constexpr std::uint32_t fnt_offset = 0x8400;
    constexpr std::uint32_t fat_offset = 0x8500;
    constexpr std::uint32_t file_offset = 0x9000;
    const std::vector<std::uint8_t> file_data{'L', 'E', 'V', 'E', 'L'};
    auto fnt = make_fnt();

    std::vector<std::uint8_t> rom(0x9200, 0xFF);
    std::fill_n(rom.begin(), nsmb::nds::ds_cartridge_header_size, 0);
    const std::string title = "TEST ROM";
    std::copy(title.begin(), title.end(), rom.begin());
    rom[0x00C] = 'T';
    rom[0x00D] = 'E';
    rom[0x00E] = 'S';
    rom[0x00F] = 'T';
    rom[0x010] = '0';
    rom[0x011] = '1';
    rom[0x014] = 7;

    put_u32(rom, 0x020, arm9_offset);
    put_u32(rom, 0x024, 0x02000000);
    put_u32(rom, 0x028, 0x02000000);
    put_u32(rom, 0x02C, arm9_size);
    put_u32(rom, 0x030, arm7_offset);
    put_u32(rom, 0x034, 0x02001000);
    put_u32(rom, 0x038, 0x02001000);
    put_u32(rom, 0x03C, arm7_size);
    put_u32(rom, 0x040, fnt_offset);
    put_u32(rom, 0x044, static_cast<std::uint32_t>(fnt.size()));
    put_u32(rom, 0x048, fat_offset);
    put_u32(rom, 0x04C, 8);
    put_u32(rom, 0x080, static_cast<std::uint32_t>(rom.size()));
    put_u32(rom, 0x084, nsmb::nds::default_header_region_size);

    for (std::uint32_t i = 0; i < arm9_size; ++i) {
        rom[arm9_offset + i] = static_cast<std::uint8_t>(0x90 + i);
    }
    for (std::uint32_t i = 0; i < arm7_size; ++i) {
        rom[arm7_offset + i] = static_cast<std::uint8_t>(0x70 + i);
    }
    std::copy(fnt.begin(), fnt.end(), rom.begin() + fnt_offset);
    put_u32(rom, fat_offset, file_offset);
    put_u32(rom, fat_offset + 4, file_offset + static_cast<std::uint32_t>(file_data.size()));
    std::copy(file_data.begin(), file_data.end(), rom.begin() + file_offset);

    put_u16(rom, 0x06C, nsmb::nds::crc16(std::span<const std::uint8_t>(rom.data() + arm9_offset, 0x8000 - arm9_offset)));
    put_u16(rom, 0x15E, nsmb::nds::crc16(std::span<const std::uint8_t>(rom.data(), 0x15E)));
    return rom;
}

} // namespace

int main() {
    const auto dir = fs::temp_directory_path() / "big-star-nds-rom-test";
    fs::remove_all(dir);
    fs::create_directories(dir);

    const auto rom_path = dir / "test.nds";
    write_bytes(rom_path, make_test_rom());

    auto rom = read_bytes(rom_path);
    auto header = nsmb::nds::parse_rom_header(rom);
    assert(header);
    assert(header.value().game_title == "TEST ROM");
    assert(header.value().game_code == "TEST");
    assert(header.value().arm9.rom_offset == 0x4000);
    assert(header.value().fnt.rom_offset == 0x8400);

    auto fnt = nsmb::nds::parse_fnt(std::span<const std::uint8_t>(rom.data() + header.value().fnt.rom_offset, header.value().fnt.size));
    assert(fnt);
    assert(fnt.value().size() == 1);
    assert(fnt.value()[0].id == 0);
    assert(fnt.value()[0].path == "course.bin");

    nsmb::nds::NitroRomService service;
    const auto extracted_dir = dir / "extracted-rom";
    auto extracted = service.extract(rom_path, extracted_dir);
    assert(extracted);
    assert(read_bytes(extracted_dir / "root" / "course.bin") == std::vector<std::uint8_t>({'L', 'E', 'V', 'E', 'L'}));

    write_bytes(extracted_dir / "root" / "course.bin", std::vector<std::uint8_t>{'P', 'A', 'T', 'C', 'H'});
    const auto rebuilt_path = dir / "rebuilt.nds";
    auto built = service.build(extracted_dir, rebuilt_path);
    assert(built);

    const auto rebuilt_extract_dir = dir / "rebuilt-extract";
    auto rebuilt_extracted = service.extract(rebuilt_path, rebuilt_extract_dir);
    assert(rebuilt_extracted);
    assert(read_bytes(rebuilt_extract_dir / "root" / "course.bin") == std::vector<std::uint8_t>({'P', 'A', 'T', 'C', 'H'}));

    fs::remove_all(dir);
    return 0;
}
