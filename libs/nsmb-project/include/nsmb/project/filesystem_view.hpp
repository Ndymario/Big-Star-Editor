#pragma once

#include <cstdint>
#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/patching/ncpatcher.hpp"

namespace nsmb::project {

enum class FilesystemStatus { Vanilla, Replaced, Added, Pending, Orphaned };

struct ModuleVariantMapping {
    std::string module;
    std::string layer;
};

struct FilesystemEntry {
    std::string path;
    FilesystemStatus status{FilesystemStatus::Vanilla};
    std::string module;
    std::string component;
    std::string variant;
    std::optional<std::int64_t> id;

    // The id came from a plan rather than from the ROM, so it is what the next
    // build would assign and not what the game can address today. Only a file
    // the ROM does not hold yet can carry one.
    bool id_is_planned{false};

    std::uintmax_t size{};
    std::filesystem::path source;
    std::filesystem::path extracted;
};

struct FilesystemView {
    std::vector<FilesystemEntry> entries;
    std::vector<std::filesystem::path> watched_directories;

    [[nodiscard]] bool has_pending() const;
    [[nodiscard]] const FilesystemEntry* find(std::string_view path) const;
};

// What the view is a join of. Nothing here is resolved by this library: the
// plan is NCPatcher's answer to "what would a build place", the manifest is the
// last build's own record of what it did place, and the extracted tree is what
// the ROM holds now. Layering, module order, `into:`, the archive-folder
// convention and component subtraction all happen in NCPatcher, once.
struct FilesystemViewRequest {
    std::filesystem::path extracted_rom;
    std::filesystem::path project_dir;

    // Only for the directories to watch and for `module_file_target`. The view
    // never walks a module tree to decide what it produces.
    patching::ModuleGraph modules;

    // `ncpatcher files plan`. Absent when NCPatcher could not be asked, which
    // leaves the view reporting the last build rather than guessing past it.
    std::optional<patching::FileManifest> plan;

    // The last build's `files-dump`. Absent before a first build.
    std::optional<patching::FileManifest> manifest;

    std::string variant;
};

// Where in a module's tree a ROM path is written, which is the one direction no
// build document answers: a plan says where a source goes, not where a new edit
// should be put. Archive addresses (`a.narc!b/c`) become the archive-folder
// spelling (`a_narc/b/c`) NCPatcher sweeps back into the container.
core::Result<std::filesystem::path> module_file_target(
    const patching::ModuleInfo& module,
    std::string_view variant,
    std::string_view rom_path,
    std::span<const ModuleVariantMapping> mappings = {});

core::Result<FilesystemView> build_filesystem_view(FilesystemViewRequest request);
[[nodiscard]] std::string_view filesystem_status_name(FilesystemStatus status);

} // namespace nsmb::project
