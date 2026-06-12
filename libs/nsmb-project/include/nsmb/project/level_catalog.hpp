#pragma once

#include <optional>
#include <string>
#include <vector>

namespace nsmb::project {

struct LevelCatalogArea {
    std::string display_name;
    std::string source_token;
    std::string course_token;
    std::optional<std::string> narc_name;
};

struct LevelCatalogEntry {
    std::string world;
    std::string display_name;
    std::vector<LevelCatalogArea> areas;
};

[[nodiscard]] const std::vector<LevelCatalogEntry>& nsmb_level_catalog();

} // namespace nsmb::project
