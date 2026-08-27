#pragma once

#include <cstdint>
#include <filesystem>

#include "nsmb/core/result.hpp"
#include "nsmb/project/project.hpp"

namespace nsmb::project {

constexpr const char* manifest_file_name = "big-star-project.yaml";

// 1 carried a `files:` mirror of the extracted ROM; 2 drops it and indexes on open.
// Both load -- a version 1 manifest is rewritten as version 2 on the next save.
constexpr std::uintmax_t manifest_version = 2;

core::Result<void> save_manifest(const Project& project, const std::filesystem::path& path);
core::Result<Project> load_manifest(const std::filesystem::path& path);
core::Result<std::filesystem::path> make_project_relative(const std::filesystem::path& project_dir,
                                                          const std::filesystem::path& path);
core::Result<void> index_project_files(Project& project, const std::filesystem::path& extracted_dir);

} // namespace nsmb::project
