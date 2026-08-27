#pragma once

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::project {

enum class LevelDataKind { Flag, Value, Array, SizedArray };

struct LevelDataDeclaration {
    std::string key, module, type_name;
    LevelDataKind kind{LevelDataKind::Flag};
    std::uint32_t hash{};
    std::optional<std::size_t> array_size;
};

struct StageObjectDeclaration {
    std::string name, module, component, object, variant, info, header;
    std::uint32_t hash{};
    std::uint32_t object_id{};
};

struct ModuleDataCatalog {
    std::vector<LevelDataDeclaration> level_data;
    std::uint16_t stage_object_id_base{326};
    std::vector<StageObjectDeclaration> stage_objects;

    [[nodiscard]] const LevelDataDeclaration* level_data_by_hash(std::uint32_t hash) const;
    [[nodiscard]] const StageObjectDeclaration* stage_object_by_hash(std::uint32_t hash) const;
};

core::Result<std::vector<LevelDataDeclaration>> parse_level_data_catalog(std::string_view json);
core::Result<std::pair<std::uint16_t, std::vector<StageObjectDeclaration>>> parse_stage_object_catalog(std::string_view json);
core::Result<ModuleDataCatalog> load_module_data_catalog(
    const std::filesystem::path& project_dir,
    const std::filesystem::path& generated_dir = "build/generated");

} // namespace nsmb::project
