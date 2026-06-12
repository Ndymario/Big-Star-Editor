#pragma once
#include <filesystem>
#include "nsmb/core/result.hpp"
#include "nsmb/project/project.hpp"
namespace nsmb::project {
constexpr const char* manifest_file_name = "big-star-project.txt";
core::Result<void> save_manifest(const Project& project, const std::filesystem::path& path);
core::Result<Project> load_manifest(const std::filesystem::path& path);
}
