#pragma once

#include <chrono>
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

struct ProjectVariantLayers {
    std::string name;
    std::vector<ModuleVariantMapping> mappings;
};

struct ModuleFileOffer {
    std::string rom_path;
    std::filesystem::path source;
    std::string module;
    std::string component;
    std::string from_variant;
};

struct FilesystemEntry {
    std::string path;
    FilesystemStatus status{FilesystemStatus::Vanilla};
    std::string module;
    std::string component;
    std::string variant;
    std::optional<std::int64_t> id;
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

struct FilesystemViewRequest {
    std::filesystem::path extracted_rom;
    std::filesystem::path project_dir;
    patching::ModuleGraph modules;
    std::optional<patching::FileManifest> manifest;
    std::string variant;
    std::vector<ModuleVariantMapping> module_variants;
    std::vector<ModuleFileOffer> explicit_files;
    bool has_current_config{false};
};

core::Result<std::vector<ModuleFileOffer>> resolve_module_files(
    const patching::ModuleGraph& graph,
    std::string_view variant,
    std::span<const ModuleVariantMapping> mappings = {});

core::Result<std::filesystem::path> module_file_target(
    const patching::ModuleInfo& module,
    std::string_view variant,
    std::string_view rom_path,
    std::span<const ModuleVariantMapping> mappings = {});

core::Result<FilesystemView> build_filesystem_view(FilesystemViewRequest request);
core::Result<std::vector<ProjectVariantLayers>> parse_project_variant_layers(std::string_view yaml);
[[nodiscard]] std::string_view filesystem_status_name(FilesystemStatus status);

} // namespace nsmb::project
