#include "nsmb/project/project.hpp"

#include <utility>

namespace nsmb::project {

Project::Project(std::string name) : name_(std::move(name)) {}

const std::string& Project::name() const noexcept { return name_; }

void Project::set_name(std::string name) {
    name_ = std::move(name);
    dirty_ = true;
}

const std::filesystem::path& Project::directory() const noexcept { return directory_; }

void Project::set_directory(std::filesystem::path directory) {
    directory_ = std::move(directory);
    dirty_ = true;
}

bool Project::dirty() const noexcept { return dirty_; }

void Project::mark_clean() noexcept { dirty_ = false; }

void Project::attach_extracted_rom(std::filesystem::path relative_path, std::optional<nds::RomHeader> header) {
    rom_ = Rom{};
    rom_->extracted_path = std::move(relative_path);
    rom_->header = std::move(header);
    dirty_ = true;
}

const std::optional<Rom>& Project::rom() const noexcept { return rom_; }

void Project::add_game_file(GameFile file) {
    if (!rom_) {
        rom_ = Rom{};
    }
    rom_->files.push_back(std::move(file));
    dirty_ = true;
}

void Project::update_game_file_size(const std::filesystem::path& path, std::uintmax_t size) {
    if (!rom_) {
        return;
    }
    for (auto& file : rom_->files) {
        if (file.path == path) {
            file.size = size;
            dirty_ = true;
            return;
        }
    }
}

void Project::clear_game_files() {
    if (rom_) {
        rom_->files.clear();
        dirty_ = true;
    }
}

const std::vector<GameFile>& Project::game_files() const noexcept {
    static const std::vector<GameFile> empty;
    return rom_ ? rom_->files : empty;
}

core::Result<Project> create_empty_project(std::string name, const std::filesystem::path& directory) {
    Project project(std::move(name));
    project.set_directory(directory);
    project.mark_clean();
    return project;
}

} // namespace nsmb::project
