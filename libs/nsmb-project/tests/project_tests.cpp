#include "nsmb/project/manifest.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <sstream>

int main() {
    namespace fs = std::filesystem;
    const auto dir = fs::temp_directory_path() / "big-star-project-test";
    fs::remove_all(dir);
    fs::create_directories(dir / "extracted-rom/root/course");

    auto project = nsmb::project::create_empty_project("Test Project", dir).value();
    project.attach_extracted_rom(dir / "extracted-rom");
    project.add_game_file({dir / "extracted-rom/root/course/course1.bin", 12});

    const auto manifest = dir / nsmb::project::manifest_file_name;
    assert(nsmb::project::save_manifest(project, manifest));

    std::ifstream manifest_input(manifest);
    std::stringstream manifest_text;
    manifest_text << manifest_input.rdbuf();
    const auto text = manifest_text.str();
    assert(text.find("format: big-star-editor-project") != std::string::npos);
    assert(text.find("root:") == std::string::npos);
    assert(text.find(dir.string()) == std::string::npos);
    assert(text.find("extracted_path: \"extracted-rom\"") != std::string::npos);

    auto loaded = nsmb::project::load_manifest(manifest);
    assert(loaded);
    assert(loaded.value().name() == "Test Project");
    assert(loaded.value().rom());
    assert(loaded.value().rom()->extracted_path == "extracted-rom");
    assert(loaded.value().game_files().size() == 1);
    assert(loaded.value().game_files()[0].path == "extracted-rom/root/course/course1.bin");
    assert(!loaded.value().dirty());

    auto outside = nsmb::project::make_project_relative(dir, fs::temp_directory_path() / "outside-file.bin");
    assert(!outside);

    fs::remove_all(dir);
    return 0;
}
