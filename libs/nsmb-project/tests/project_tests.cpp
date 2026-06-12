#include "nsmb/project/manifest.hpp"
#include <cassert>
#include <filesystem>
int main() {
    namespace fs = std::filesystem;
    auto dir = fs::temp_directory_path() / "big-star-project-test";
    fs::remove_all(dir);
    fs::create_directories(dir);
    auto project = nsmb::project::create_empty_project("Test Project", dir).value();
    project.attach_extracted_rom(dir / "extracted");
    project.add_game_file({"root/course/course1.bin", 12});
    auto manifest = dir / nsmb::project::manifest_file_name;
    assert(nsmb::project::save_manifest(project, manifest));
    auto loaded = nsmb::project::load_manifest(manifest);
    assert(loaded);
    assert(loaded.value().name() == "Test Project");
    assert(loaded.value().rom());
    assert(loaded.value().game_files().size() == 1);
    assert(!loaded.value().dirty());
    fs::remove_all(dir);
    return 0;
}
