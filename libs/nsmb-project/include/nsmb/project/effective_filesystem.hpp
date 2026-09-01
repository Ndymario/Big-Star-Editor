#pragma once

#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/patching/ncpatcher.hpp"
#include "nsmb/project/rom_reader.hpp"

namespace nsmb::project {

// `a/b.narc!c/d` split in two. `member` is empty for an ordinary file, which is
// the only way to tell a container's own bytes from what is inside it.
struct RomAddress {
    std::string container;
    std::string member;

    [[nodiscard]] bool inside_archive() const noexcept { return !member.empty(); }
};

[[nodiscard]] RomAddress split_rom_address(std::string_view address);

// What the editor sees when it reads a ROM address.
//
// The ROM holds what the last build produced, and a build has not necessarily
// run since the last edit. So a read asks the plan first: when a module
// supplies this destination and that source is on disk, the source is what the
// editor renders. Otherwise a pending edit would stay invisible until a
// rebuild -- which is why the Qt layer used to read a derived tree and then
// correct every path it got back.
//
// Archive addresses resolve the same way and per member, so a module supplying
// one member of an archive does not stop the others coming from the ROM.
class EffectiveFilesystem {
public:
    void set_rom(std::optional<RomReader> rom);
    void set_plan(std::filesystem::path project_dir, std::optional<patching::FileManifest> plan);
    void clear();

    [[nodiscard]] bool has_rom() const noexcept { return rom_.has_value(); }
    [[nodiscard]] RomReader* rom() noexcept { return rom_ ? &*rom_ : nullptr; }
    [[nodiscard]] const RomReader* rom() const noexcept { return rom_ ? &*rom_ : nullptr; }
    [[nodiscard]] const std::filesystem::path& project_dir() const noexcept { return project_dir_; }

    core::Result<std::vector<std::uint8_t>> read(std::string_view address);

    // The head of what `read` would return, for callers that only need to
    // recognise a format. A member still costs its container a parse, because
    // an archive does not hand out a member without one.
    core::Result<std::vector<std::uint8_t>> read_prefix(std::string_view address, std::size_t maximum);

    // The file on disk this address reads from, or empty when the ROM supplies
    // it. This is what a caller watches: the ROM changes only when a build
    // replaces it, and a module source changes whenever somebody saves.
    [[nodiscard]] std::filesystem::path source(std::string_view address) const;

    // The ROM path a NitroFS id names. Resource tables in the game's own code
    // address files by id, so this is how a table entry becomes an address.
    [[nodiscard]] std::optional<std::string> path_for_id(std::uint16_t file_id) const;

private:
    std::optional<RomReader> rom_;
    std::optional<patching::FileManifest> plan_;
    std::filesystem::path project_dir_;
};

} // namespace nsmb::project
