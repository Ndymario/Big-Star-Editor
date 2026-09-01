#pragma once

#include <cstdint>
#include <filesystem>
#include <fstream>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/formats/narc.hpp"
#include "nsmb/nds/rom.hpp"

namespace nsmb::project {

// A built `.nds`, read where it lies.
//
// Nothing is unpacked. A NitroFS file is a window into the ROM read on demand,
// an overlay is decompressed when it is asked for, and an archive is parsed
// only when somebody looks inside it. The two expensive answers -- a
// decompressed overlay and a parsed archive -- are kept in a small cache,
// because opening a level asks for the same overlay every single time.
//
// This lives above `nsmb-nds` rather than in it because it needs both layers:
// `nsmb-nds` parses the container, and `nsmb-formats` -- which already depends
// on `nsmb-nds` for a NARC's name table -- holds the LZ codecs. Every caller
// is at this level or above, so nothing is lost by placing it here.
//
// Reads are not const: the stream position and the caches both move. One reader
// belongs to one thread.
class RomReader {
public:
    static core::Result<RomReader> open(std::filesystem::path rom);

    RomReader(RomReader&&) noexcept = default;
    RomReader& operator=(RomReader&&) noexcept = default;
    RomReader(const RomReader&) = delete;
    RomReader& operator=(const RomReader&) = delete;

    [[nodiscard]] const std::filesystem::path& path() const noexcept { return path_; }
    [[nodiscard]] const nds::RomHeader& header() const noexcept { return header_; }

    // Every NitroFS file the ROM holds, sorted by id, as `parse_fnt` reports it.
    [[nodiscard]] const std::vector<nds::NitroFileEntry>& files() const noexcept { return files_; }

    // The id a ROM path has, or nothing when the ROM does not hold that path.
    [[nodiscard]] std::optional<std::uint16_t> file_id(std::string_view rom_path) const;

    // How big a file is, straight out of the FAT, without reading it.
    [[nodiscard]] std::optional<std::uint32_t> file_size(std::uint16_t file_id) const;

    // Whether the ROM holds an archive at this path -- decided by what the bytes
    // decode to, never by the extension, so a compressed container named `.carc`
    // and a plain one named `.narc` both answer true and `dwc/utility.bin`,
    // which merely starts with 0x10, does not.
    [[nodiscard]] bool is_archive(std::string_view rom_path);

    core::Result<std::vector<std::uint8_t>> read(std::string_view rom_path);
    core::Result<std::vector<std::uint8_t>> read(std::uint16_t file_id);

    // Just the head of a file. Sniffing a format needs a few dozen bytes, and a
    // dock that classified two thousand entries by reading them whole would
    // spend a game's worth of I/O to draw its icons.
    core::Result<std::vector<std::uint8_t>> read_prefix(std::string_view rom_path, std::size_t maximum);

    // Decompressed, whatever the ROM stores. An overlay the ROM keeps raw comes
    // back unchanged, because BLZ marks its own absence.
    core::Result<std::vector<std::uint8_t>> read_overlay(std::uint32_t overlay_id);

    core::Result<std::vector<std::uint8_t>> read_arm9();
    core::Result<std::vector<std::uint8_t>> read_banner();

    // Member names in index order, from the archive's own name table. A member
    // the name table does not name is an empty string rather than a missing
    // row, so a member's position in this list is its index -- which is how the
    // game addresses it and how a build reports it.
    core::Result<std::vector<std::string>> list_archive(std::string_view rom_path);
    core::Result<std::vector<std::uint8_t>> read_member(std::string_view rom_path, std::string_view member);
    core::Result<std::vector<std::uint8_t>> read_member(std::string_view rom_path, std::size_t index);

private:
    RomReader() = default;

    core::Result<std::vector<std::uint8_t>> read_region(std::uint32_t offset, std::uint32_t size, std::string_view what);
    core::Result<const formats::NarcArchive*> archive(std::string_view rom_path);

    std::filesystem::path path_;
    std::ifstream stream_;
    std::uintmax_t size_{};
    nds::RomHeader header_;
    std::vector<nds::FatEntry> fat_;
    std::vector<nds::NitroFileEntry> files_;
    std::vector<nds::OverlayTableEntry> overlays_;

    // Most-recently-used first, and short: an editor works in one level's worth
    // of resources at a time, and holding more decompressed overlays than that
    // costs memory to answer questions nobody asked.
    std::vector<std::pair<std::uint32_t, std::vector<std::uint8_t>>> overlay_cache_;
    std::vector<std::pair<std::string, formats::NarcArchive>> archive_cache_;
};

} // namespace nsmb::project
