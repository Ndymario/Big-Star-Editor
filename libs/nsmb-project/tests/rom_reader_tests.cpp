#include "nsmb/project/rom_reader.hpp"

#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/narc.hpp"

#include <cassert>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace {

namespace fs = std::filesystem;
using Bytes = std::vector<std::uint8_t>;

void put16(Bytes& data, std::size_t offset, std::uint16_t value) {
    data[offset] = static_cast<std::uint8_t>(value);
    data[offset + 1] = static_cast<std::uint8_t>(value >> 8);
}

void put32(Bytes& data, std::size_t offset, std::uint32_t value) {
    put16(data, offset, static_cast<std::uint16_t>(value));
    put16(data, offset + 2, static_cast<std::uint16_t>(value >> 16));
}

void add16(Bytes& data, std::uint16_t value) { data.push_back(static_cast<std::uint8_t>(value)); data.push_back(static_cast<std::uint8_t>(value >> 8)); }
void add32(Bytes& data, std::uint32_t value) { add16(data, static_cast<std::uint16_t>(value)); add16(data, static_cast<std::uint16_t>(value >> 16)); }

void name_entry(Bytes& sub_table, const std::string& name, bool directory, std::uint16_t directory_id) {
    sub_table.push_back(static_cast<std::uint8_t>(name.size() | (directory ? 0x80 : 0)));
    sub_table.insert(sub_table.end(), name.begin(), name.end());
    if (directory) add16(sub_table, directory_id);
}

// A name table holding `root_files` at the top level and `nested` inside one
// sub-directory -- the same shape a NitroFS and a NARC both use, which is why
// one builder serves the ROM and the archives inside it.
Bytes make_fnt(const std::vector<std::string>& root_files, const std::string& directory, const std::vector<std::string>& nested) {
    const bool has_nested = !nested.empty();
    const std::size_t directories = has_nested ? 2 : 1;
    Bytes root_sub, nested_sub;
    for (const auto& name : root_files) name_entry(root_sub, name, false, 0);
    if (has_nested) {
        name_entry(root_sub, directory, true, 0xF001);
        for (const auto& name : nested) name_entry(nested_sub, name, false, 0);
        nested_sub.push_back(0);
    }
    root_sub.push_back(0);

    Bytes fnt(directories * 8, 0);
    put32(fnt, 0, static_cast<std::uint32_t>(fnt.size()));
    put16(fnt, 4, 0);
    put16(fnt, 6, static_cast<std::uint16_t>(directories));
    if (has_nested) {
        put32(fnt, 8, static_cast<std::uint32_t>(fnt.size() + root_sub.size()));
        put16(fnt, 12, static_cast<std::uint16_t>(root_files.size()));
        put16(fnt, 14, 0xF000);
    }
    fnt.insert(fnt.end(), root_sub.begin(), root_sub.end());
    fnt.insert(fnt.end(), nested_sub.begin(), nested_sub.end());
    return fnt;
}

// Members named at the archive root, then members inside `directory`: a NARC
// name table is an FNT, so a member address nests exactly like a ROM path.
Bytes make_narc(const std::vector<std::pair<std::string, Bytes>>& root_members,
                const std::string& directory,
                const std::vector<std::pair<std::string, Bytes>>& nested_members) {
    nsmb::formats::NarcArchive archive;
    std::vector<std::string> root_names, nested_names;
    for (const auto& [name, data] : root_members) { root_names.push_back(name); archive.files.push_back(data); }
    for (const auto& [name, data] : nested_members) { nested_names.push_back(name); archive.files.push_back(data); }
    archive.name_table = make_fnt(root_names, directory, nested_names);
    auto written = nsmb::formats::write_narc(archive);
    assert(written);
    return written.value();
}

constexpr std::uint32_t arm9_offset = 0x4000, arm9_size = 0x20;
constexpr std::uint32_t arm7_offset = 0x4100, arm7_size = 0x20;
constexpr std::uint32_t banner_offset = 0x4200, banner_size = 0x0A00;
constexpr std::uint32_t fnt_offset = 0x4C00;
constexpr std::uint32_t fat_offset = 0x5000;
constexpr std::uint32_t ovt_offset = 0x5100;
constexpr std::uint32_t data_offset = 0x6000;

// A ROM holding four NitroFS files -- a plain archive, an LZ-wrapped one, a
// loose file under `z_new/`, and an overlay -- which is every shape the reader
// has to answer for.
Bytes make_rom(const std::vector<Bytes>& files, const Bytes& overlay) {
    auto fnt = make_fnt({"plain.narc", "packed.carc"}, "z_new", {"fresh.bin"});

    Bytes rom(data_offset, 0);
    std::vector<std::pair<std::uint32_t, std::uint32_t>> spans;
    auto place = [&](const Bytes& data) {
        const auto start = static_cast<std::uint32_t>(rom.size());
        rom.insert(rom.end(), data.begin(), data.end());
        rom.resize((rom.size() + 3) & ~std::size_t(3), 0xFF);
        spans.emplace_back(start, start + static_cast<std::uint32_t>(data.size()));
    };
    for (const auto& file : files) place(file);
    place(overlay);

    const std::string title = "READER TEST";
    std::copy(title.begin(), title.end(), rom.begin());
    const std::string code = "A2DE";
    std::copy(code.begin(), code.end(), rom.begin() + 0x00C);

    put32(rom, 0x020, arm9_offset);
    put32(rom, 0x02C, arm9_size);
    put32(rom, 0x030, arm7_offset);
    put32(rom, 0x03C, arm7_size);
    put32(rom, 0x040, fnt_offset);
    put32(rom, 0x044, static_cast<std::uint32_t>(fnt.size()));
    put32(rom, 0x048, fat_offset);
    put32(rom, 0x04C, static_cast<std::uint32_t>(spans.size() * 8));
    put32(rom, 0x050, ovt_offset);
    put32(rom, 0x054, nsmb::nds::overlay_table_entry_size);
    put32(rom, 0x068, banner_offset);
    put32(rom, 0x080, static_cast<std::uint32_t>(rom.size()));
    put32(rom, 0x084, nsmb::nds::default_header_region_size);

    for (std::uint32_t i = 0; i < arm9_size; ++i) rom[arm9_offset + i] = static_cast<std::uint8_t>(0x90 + i);
    for (std::uint32_t i = 0; i < arm7_size; ++i) rom[arm7_offset + i] = static_cast<std::uint8_t>(0x70 + i);
    put16(rom, banner_offset, 0x0001);
    rom[banner_offset + 2] = 0xAB;
    std::copy(fnt.begin(), fnt.end(), rom.begin() + fnt_offset);
    for (std::size_t i = 0; i < spans.size(); ++i) {
        put32(rom, fat_offset + i * 8, spans[i].first);
        put32(rom, fat_offset + i * 8 + 4, spans[i].second);
    }
    // One ARM9 overlay, whose file id is the entry after the NitroFS files.
    put32(rom, ovt_offset, 0);
    put32(rom, ovt_offset + 0x18, static_cast<std::uint32_t>(files.size()));
    return rom;
}

void write_bytes(const fs::path& path, const Bytes& bytes) {
    fs::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
}

} // namespace

int main() {
    const auto dir = fs::temp_directory_path() / "big-star-rom-reader-tests";
    fs::remove_all(dir);
    fs::create_directories(dir);

    const Bytes vs_bmg{'M', 'E', 'S', 'G', 1, 2, 3, 4};
    const Bytes hd_bin{'H', 'D', 0, 0};
    const auto plain = make_narc({{"vs.bmg", vs_bmg}}, "menu", {{"hd.bin", hd_bin}});
    const Bytes kart{'K', 'A', 'R', 'T', 0, 0, 0, 0};
    const auto packed_raw = make_narc({{"kart.bin", kart}}, {}, {});
    auto packed = nsmb::formats::compress_nitro_lz10(packed_raw);
    assert(packed);
    const Bytes fresh{'F', 'R', 'E', 'S', 'H'};

    // Compressible on purpose: BLZ declines to grow a file, and an overlay it
    // declined is stored raw, which would not exercise the decompression path.
    Bytes overlay_raw(0x800, 0);
    for (std::size_t i = 0; i < overlay_raw.size(); ++i) overlay_raw[i] = static_cast<std::uint8_t>(i % 7);
    auto overlay = nsmb::formats::compress_blz_overlay(overlay_raw);
    assert(overlay && !overlay.value().empty());

    const auto rom_path = dir / "build.nds";
    write_bytes(rom_path, make_rom({plain, packed.value(), fresh}, overlay.value()));

    auto opened = nsmb::project::RomReader::open(rom_path);
    assert(opened);
    auto& reader = opened.value();

    assert(reader.header().game_code == "A2DE");
    assert(reader.files().size() == 3);

    // By path, and by the id that path resolves to -- the two have to agree, or
    // a resource table that names an id and a dock that names a path disagree.
    assert(reader.file_id("z_new/fresh.bin") == 2);
    assert(!reader.file_id("z_new/missing.bin"));
    assert(reader.read("z_new/fresh.bin").value() == fresh);
    assert(reader.read(std::uint16_t{2}).value() == fresh);
    assert(!reader.read("nothing/here.bin"));

    assert(reader.read_arm9().value().front() == 0x90);
    assert(reader.read_banner().value().size() == banner_size);
    assert(reader.read_banner().value()[2] == 0xAB);

    // The ROM stores the overlay compressed; the reader hands back what the
    // game would run.
    assert(reader.read_overlay(0).value() == overlay_raw);
    assert(!reader.read_overlay(7));

    // A plain container and an LZ-wrapped one are the same thing to a caller.
    assert(reader.is_archive("plain.narc"));
    assert(reader.is_archive("packed.carc"));
    assert(!reader.is_archive("z_new/fresh.bin"));
    assert(reader.list_archive("plain.narc").value() == std::vector<std::string>({"vs.bmg", "menu/hd.bin"}));
    assert(reader.list_archive("packed.carc").value() == std::vector<std::string>({"kart.bin"}));
    assert(reader.read_member("plain.narc", "vs.bmg").value() == vs_bmg);
    assert(reader.read_member("plain.narc", std::size_t{1}).value() == hd_bin);
    assert(reader.read_member("packed.carc", "kart.bin").value() == kart);
    assert(!reader.read_member("plain.narc", "menu/nope.bin"));
    assert(!reader.read_member("plain.narc", std::size_t{9}));

    // Repeat questions are what the cache exists for, and have to answer the
    // same thing as the first one.
    assert(reader.read_overlay(0).value() == overlay_raw);
    assert(reader.read_member("plain.narc", "vs.bmg").value() == vs_bmg);

    assert(!nsmb::project::RomReader::open(dir / "absent.nds"));

    fs::remove_all(dir);
    return 0;
}
