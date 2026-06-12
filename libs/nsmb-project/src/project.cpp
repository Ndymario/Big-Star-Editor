#include "nsmb/project/project.hpp"
namespace nsmb::project {
Project::Project(std::string name) : name_(std::move(name)) {}
const std::string& Project::name() const noexcept { return name_; }
void Project::set_name(std::string name) { name_ = std::move(name); dirty_ = true; }
const std::filesystem::path& Project::root_directory() const noexcept { return root_directory_; }
void Project::set_root_directory(std::filesystem::path root) { root_directory_ = std::move(root); dirty_ = true; }
bool Project::dirty() const noexcept { return dirty_; }
void Project::mark_clean() noexcept { dirty_ = false; }
void Project::attach_extracted_rom(std::filesystem::path path) { rom_ = Rom{}; rom_->extracted_path = std::move(path); dirty_ = true; }
const std::optional<Rom>& Project::rom() const noexcept { return rom_; }
void Project::add_game_file(GameFile file) { if (!rom_) rom_ = Rom{}; rom_->files.push_back(file); dirty_ = true; }
const std::vector<GameFile>& Project::game_files() const noexcept { static const std::vector<GameFile> empty; return rom_ ? rom_->files : empty; }
core::Result<Project> create_empty_project(std::string name, const std::filesystem::path& root) { Project p(std::move(name)); p.set_root_directory(root); p.mark_clean(); return p; }
}
