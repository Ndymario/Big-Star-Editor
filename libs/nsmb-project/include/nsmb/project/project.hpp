#pragma once

#include <filesystem>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/nds/rom.hpp"

namespace nsmb::project {

struct TilesetRef {
    std::string id;
    std::filesystem::path file;
};

struct Sprite {
    int id{};
    int x{};
    int y{};
    std::string type;
};

struct Entrance {
    int id{};
    int x{};
    int y{};
};

struct Path {
    std::string name;
    std::vector<std::pair<int, int>> points;
};

struct Zone {
    int id{};
    int x{};
    int y{};
    int width{};
    int height{};
};

struct Area {
    std::string id;
    std::vector<Zone> zones;
    std::vector<Sprite> sprites;
    std::vector<Entrance> entrances;
    std::vector<Path> paths;
};

struct Level {
    std::string id;
    std::string display_name;
    std::vector<Area> areas;
    std::vector<TilesetRef> tilesets;
};

struct GameFile {
    std::filesystem::path path;
    std::uintmax_t size{};
};

struct Rom {
    std::optional<nds::RomHeader> header;
    std::filesystem::path extracted_path;
    std::vector<GameFile> files;
};

class Project {
public:
    Project() = default;
    explicit Project(std::string name);

    [[nodiscard]] const std::string& name() const noexcept;
    void set_name(std::string name);

    [[nodiscard]] const std::filesystem::path& directory() const noexcept;
    void set_directory(std::filesystem::path directory);

    [[nodiscard]] bool dirty() const noexcept;
    void mark_clean() noexcept;

    void attach_extracted_rom(std::filesystem::path relative_path, std::optional<nds::RomHeader> header = std::nullopt);
    [[nodiscard]] const std::optional<Rom>& rom() const noexcept;

    void add_game_file(GameFile file);
    void update_game_file_size(const std::filesystem::path& path, std::uintmax_t size);
    void clear_game_files();
    [[nodiscard]] const std::vector<GameFile>& game_files() const noexcept;

private:
    std::string name_{"Untitled Project"};
    std::filesystem::path directory_;
    std::optional<Rom> rom_;
    bool dirty_{false};
};

core::Result<Project> create_empty_project(std::string name, const std::filesystem::path& directory);

} // namespace nsmb::project
