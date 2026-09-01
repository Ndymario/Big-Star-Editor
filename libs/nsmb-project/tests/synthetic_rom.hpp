#pragma once

// A hand-built `.nds`: small enough to reason about, complete enough to read.
// Give it ROM paths and bytes and it lays out a header, a name table, a FAT, an
// overlay table and a banner around them. Shared by every test that needs a ROM
// to read, because they all have to be told the same story about one.

#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/narc.hpp"
#include "nsmb/nds/rom.hpp"

#include <algorithm>
#include <cassert>
#include <cstdint>
#include <filesystem>
#include <fstream>
#include <map>
#include <string>
#include <vector>

namespace synthetic {

namespace fs = std::filesystem;
using Bytes = std::vector<std::uint8_t>;

struct RomFile {
    std::string path;
    Bytes data;
};

inline void put16(Bytes& data, std::size_t offset, std::uint16_t value) {
    data[offset] = static_cast<std::uint8_t>(value);
    data[offset + 1] = static_cast<std::uint8_t>(value >> 8);
}

inline void put32(Bytes& data, std::size_t offset, std::uint32_t value) {
    put16(data, offset, static_cast<std::uint16_t>(value));
    put16(data, offset + 2, static_cast<std::uint16_t>(value >> 16));
}

inline void add16(Bytes& data, std::uint16_t value) {
    data.push_back(static_cast<std::uint8_t>(value));
    data.push_back(static_cast<std::uint8_t>(value >> 8));
}

inline void add32(Bytes& data, std::uint32_t value) {
    add16(data, static_cast<std::uint16_t>(value));
    add16(data, static_cast<std::uint16_t>(value >> 16));
}

// A directory in the name table being built. Ids follow creation order, and
// files are numbered directory by directory in that same order, which is what
// makes a path's id predictable enough to assert on.
struct Directory {
    std::string name;
    std::size_t parent{};
    std::vector<std::string> files;
    std::vector<std::size_t> children;
    std::uint16_t first_file_id{};
};

struct NameTable {
    Bytes bytes;
    std::map<std::string, std::uint16_t> ids;
};

inline NameTable make_name_table(const std::vector<std::string>& paths) {
    std::vector<Directory> directories{Directory{}};
    std::map<std::string, std::size_t> by_path{{"", 0}};

    auto directory_for = [&](const std::string& path) {
        std::size_t current = 0;
        std::string prefix;
        std::size_t start = 0;
        while (true) {
            const auto slash = path.find('/', start);
            if (slash == path.npos) return current;
            const auto part = path.substr(start, slash - start);
            prefix += (prefix.empty() ? "" : "/") + part;
            const auto found = by_path.find(prefix);
            if (found != by_path.end()) {
                current = found->second;
            } else {
                directories.push_back(Directory{part, current, {}, {}, 0});
                directories[current].children.push_back(directories.size() - 1);
                current = directories.size() - 1;
                by_path.emplace(prefix, current);
            }
            start = slash + 1;
        }
    };
    for (const auto& path : paths) directories[directory_for(path)].files.push_back(path);

    NameTable table;
    std::uint16_t next_id = 0;
    std::vector<Bytes> sub_tables(directories.size());
    for (std::size_t i = 0; i < directories.size(); ++i) {
        directories[i].first_file_id = next_id;
        for (const auto& path : directories[i].files) {
            const auto name = path.substr(path.rfind('/') == path.npos ? 0 : path.rfind('/') + 1);
            sub_tables[i].push_back(static_cast<std::uint8_t>(name.size()));
            sub_tables[i].insert(sub_tables[i].end(), name.begin(), name.end());
            table.ids.emplace(path, next_id++);
        }
        for (const auto child : directories[i].children) {
            const auto& name = directories[child].name;
            sub_tables[i].push_back(static_cast<std::uint8_t>(name.size() | 0x80));
            sub_tables[i].insert(sub_tables[i].end(), name.begin(), name.end());
            add16(sub_tables[i], static_cast<std::uint16_t>(nsmb::nds::root_directory_id + child));
        }
        sub_tables[i].push_back(0);
    }

    table.bytes.assign(directories.size() * 8, 0);
    std::size_t offset = table.bytes.size();
    for (std::size_t i = 0; i < directories.size(); ++i) {
        put32(table.bytes, i * 8, static_cast<std::uint32_t>(offset));
        put16(table.bytes, i * 8 + 4, directories[i].first_file_id);
        put16(table.bytes, i * 8 + 6,
              i == 0 ? static_cast<std::uint16_t>(directories.size())
                     : static_cast<std::uint16_t>(nsmb::nds::root_directory_id + directories[i].parent));
        offset += sub_tables[i].size();
    }
    for (const auto& sub_table : sub_tables)
        table.bytes.insert(table.bytes.end(), sub_table.begin(), sub_table.end());
    return table;
}

// Members named at the archive root, then members inside `directory`: a NARC
// name table is an FNT, so a member address nests exactly like a ROM path.
inline Bytes make_narc(const std::vector<std::pair<std::string, Bytes>>& members) {
    nsmb::formats::NarcArchive archive;
    std::vector<std::string> paths;
    for (const auto& [path, data] : members) paths.push_back(path);
    auto table = make_name_table(paths);
    archive.files.resize(members.size());
    for (const auto& [path, data] : members) archive.files[table.ids.at(path)] = data;
    archive.name_table = std::move(table.bytes);
    auto written = nsmb::formats::write_narc(archive);
    assert(written);
    return written.value();
}

constexpr std::uint32_t arm9_offset = 0x4000, arm9_size = 0x20;
constexpr std::uint32_t arm7_offset = 0x4100, arm7_size = 0x20;
constexpr std::uint32_t banner_offset = 0x4200, banner_size = 0x0A00;
constexpr std::uint32_t fnt_offset = 0x4C00;

inline std::uint32_t align_to(std::uint32_t value, std::uint32_t alignment) {
    return (value + alignment - 1U) / alignment * alignment;
}

// The ROM. `overlay` becomes ARM9 overlay 0, whose file id follows the NitroFS
// files, which is where a real ROM puts its overlays too.
inline Bytes make_rom(const std::vector<RomFile>& files, const Bytes& overlay = {}) {
    std::vector<std::string> paths;
    for (const auto& file : files) paths.push_back(file.path);
    auto table = make_name_table(paths);

    const auto fat_offset = align_to(fnt_offset + static_cast<std::uint32_t>(table.bytes.size()), 4);
    const auto entries = static_cast<std::uint32_t>(files.size() + (overlay.empty() ? 0U : 1U));
    const auto ovt_offset = fat_offset + entries * 8U;
    const auto data_offset = align_to(ovt_offset + nsmb::nds::overlay_table_entry_size, 0x200);

    Bytes rom(data_offset, 0);
    std::vector<std::pair<std::uint32_t, std::uint32_t>> spans(entries);
    auto place = [&](std::size_t id, const Bytes& data) {
        const auto start = static_cast<std::uint32_t>(rom.size());
        rom.insert(rom.end(), data.begin(), data.end());
        rom.resize((rom.size() + 3) & ~std::size_t(3), 0xFF);
        spans[id] = {start, start + static_cast<std::uint32_t>(data.size())};
    };
    for (const auto& file : files) place(table.ids.at(file.path), file.data);
    if (!overlay.empty()) place(files.size(), overlay);

    const std::string title = "READER TEST";
    std::copy(title.begin(), title.end(), rom.begin());
    const std::string code = "A2DE";
    std::copy(code.begin(), code.end(), rom.begin() + 0x00C);

    put32(rom, 0x020, arm9_offset);
    put32(rom, 0x02C, arm9_size);
    put32(rom, 0x030, arm7_offset);
    put32(rom, 0x03C, arm7_size);
    put32(rom, 0x040, fnt_offset);
    put32(rom, 0x044, static_cast<std::uint32_t>(table.bytes.size()));
    put32(rom, 0x048, fat_offset);
    put32(rom, 0x04C, entries * 8U);
    put32(rom, 0x050, ovt_offset);
    put32(rom, 0x054, overlay.empty() ? 0U : nsmb::nds::overlay_table_entry_size);
    put32(rom, 0x068, banner_offset);
    put32(rom, 0x080, static_cast<std::uint32_t>(rom.size()));
    put32(rom, 0x084, nsmb::nds::default_header_region_size);

    for (std::uint32_t i = 0; i < arm9_size; ++i) rom[arm9_offset + i] = static_cast<std::uint8_t>(0x90 + i);
    for (std::uint32_t i = 0; i < arm7_size; ++i) rom[arm7_offset + i] = static_cast<std::uint8_t>(0x70 + i);
    put16(rom, banner_offset, 0x0001);
    rom[banner_offset + 2] = 0xAB;
    std::copy(table.bytes.begin(), table.bytes.end(), rom.begin() + fnt_offset);
    for (std::size_t i = 0; i < spans.size(); ++i) {
        put32(rom, fat_offset + i * 8, spans[i].first);
        put32(rom, fat_offset + i * 8 + 4, spans[i].second);
    }
    if (!overlay.empty()) {
        put32(rom, ovt_offset, 0);
        put32(rom, ovt_offset + 0x18, static_cast<std::uint32_t>(files.size()));
    }
    return rom;
}

inline void write_bytes(const fs::path& path, const Bytes& bytes) {
    fs::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary);
    out.write(reinterpret_cast<const char*>(bytes.data()), static_cast<std::streamsize>(bytes.size()));
}

inline Bytes text(std::string_view value) { return Bytes(value.begin(), value.end()); }

} // namespace synthetic
