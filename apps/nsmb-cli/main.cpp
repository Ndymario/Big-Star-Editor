#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/external_tools.hpp"
#include "nsmb/project/manifest.hpp"
#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <vector>
namespace fs = std::filesystem;
static void usage() { std::cout << "nsmb-cli 0.1.0\nCommands: --version | inspect-rom <path> | import-rom <rom> <project-dir> | run-patcher <project-dir> | build-rom <project-dir> <output-rom>\n"; }
int main(int argc, char** argv) {
    if (argc < 2) { usage(); return 1; }
    std::string cmd = argv[1];
    if (cmd == "--version") { std::cout << "nsmb-cli 0.1.0\n"; return 0; }
    if (cmd == "inspect-rom" && argc == 3) {
        std::ifstream in(argv[2], std::ios::binary); if (!in) { std::cerr << "Could not open ROM\n"; return 2; }
        std::vector<std::uint8_t> data((std::istreambuf_iterator<char>(in)), {});
        auto header = nsmb::nds::parse_rom_header(data); if (!header) { std::cerr << header.error().message << '\n'; return 3; }
        std::cout << "Title: " << header.value().game_title << "\nGame code: " << header.value().game_code << "\n"; return 0;
    }
    if (cmd == "import-rom" && argc == 4) {
        fs::path project_dir = argv[3]; fs::create_directories(project_dir);
        auto project = nsmb::project::create_empty_project(project_dir.filename().string(), project_dir).value();
        project.attach_extracted_rom(project_dir / "extracted-rom");
        auto saved = nsmb::project::save_manifest(project, project_dir / nsmb::project::manifest_file_name);
        if (!saved) { std::cerr << saved.error().message << '\n'; return 4; }
        std::cout << "Created project manifest. ROM extraction is delegated to nds-extract in the next milestone.\n"; return 0;
    }
    if (cmd == "run-patcher" && argc == 3) {
        nsmb::patching::NCPatcherExternalTool patcher; auto command = patcher.build_command({argv[2]});
        if (!command) { std::cerr << command.error().message << '\n'; return 5; }
        std::cout << nsmb::patching::command_to_string(command.value()) << '\n'; return 0;
    }
    if (cmd == "build-rom" && argc == 4) {
        nsmb::patching::ExternalToolRomBuildService service; auto command = service.build_rom_command(fs::path(argv[2]) / "extracted-rom", argv[3]);
        if (!command) { std::cerr << command.error().message << '\n'; return 6; }
        std::cout << nsmb::patching::command_to_string(command.value()) << '\n'; return 0;
    }
    usage(); return 1;
}
