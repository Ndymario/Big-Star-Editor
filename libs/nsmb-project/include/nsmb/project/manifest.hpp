#pragma once

#include <cstdint>
#include <filesystem>

#include "nsmb/core/result.hpp"
#include "nsmb/project/project.hpp"
#include "nsmb/project/rom_reader.hpp"

namespace nsmb::project {

constexpr const char* manifest_file_name = "big-star-project.yaml";

// 1 carried a `files:` mirror of the extracted ROM; 2 dropped it and indexed the
// extracted tree on open; 3 names the built ROM instead, because there is no
// extracted tree any more. All three load -- an older manifest is rewritten as
// version 3 on the next save, and a version 2 `rom.extracted_path` is read and
// discarded rather than honoured, since the directory it named is derived.
constexpr std::uintmax_t manifest_version = 3;

// What `rom.rom_path` defaults to. A build writes `big-star-build.nds`; before
// the first one the project's own input ROM is the only thing to browse, which
// is better than an editor that shows nothing until something is compiled.
constexpr const char* built_rom_file_name = "big-star-build.nds";
constexpr const char* input_rom_file_name = "rom.nds";

[[nodiscard]] std::filesystem::path default_rom_path(const std::filesystem::path& project_dir);

core::Result<void> save_manifest(const Project& project, const std::filesystem::path& path);
core::Result<Project> load_manifest(const std::filesystem::path& path);
core::Result<std::filesystem::path> make_project_relative(const std::filesystem::path& project_dir,
                                                          const std::filesystem::path& path);
// Fills the project's file list from the ROM's own table, which is the list the
// game addresses. Sizes come from the FAT, so nothing is read to measure it.
core::Result<void> index_rom_files(Project& project, const RomReader& rom);
core::Result<void> index_rom_files(Project& project);

} // namespace nsmb::project
