#include "nsmb/formats/graphics.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/external_tools.hpp"
#include "nsmb/project/manifest.hpp"

#include <filesystem>
#include <iostream>

namespace fs = std::filesystem;

namespace {

void usage() {
    std::cout << "nsmb-cli " BSE_VERSION "\n"
              << "Commands:\n"
              << "  --version\n"
              << "  inspect-rom <path>\n"
              << "  import-rom <rom> <project-dir>\n"
              << "  run-patcher <project-dir>\n"
              << "  build-rom <project-dir> <output-rom>\n";
}

std::size_t count_graphics_candidates(const nsmb::project::Project& project) {
    std::size_t count = 0;
    for (const auto& file : project.game_files()) {
        if (nsmb::formats::identify_graphics_format({}, file.path.generic_string()) != nsmb::formats::GraphicsFormatKind::Unknown) {
            ++count;
        }
    }
    return count;
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        usage();
        return 1;
    }

    const std::string command = argv[1];
    if (command == "--version") {
        std::cout << "nsmb-cli " BSE_VERSION "\n";
        return 0;
    }

    if (command == "inspect-rom" && argc == 3) {
        auto data = nsmb::core::read_file_prefix(argv[2], 0x4000);
        if (!data) {
            std::cerr << data.error().message << '\n';
            return 2;
        }
        auto header = nsmb::nds::parse_rom_header(data.value());
        if (!header) {
            std::cerr << header.error().message << '\n';
            return 3;
        }
        std::cout << "Title: " << header.value().game_title << "\nGame code: " << header.value().game_code << "\n";
        return 0;
    }

    if (command == "import-rom" && argc == 4) {
        const fs::path rom_path = argv[2];
        const fs::path project_dir = argv[3];
        const fs::path extracted_dir = project_dir / "extracted-rom";

        std::error_code ec;
        fs::create_directories(project_dir, ec);
        if (ec) {
            std::cerr << "Could not create project directory: " << ec.message() << '\n';
            return 4;
        }
        const auto manifest_exists = fs::exists(project_dir / nsmb::project::manifest_file_name, ec);
        if (ec) {
            std::cerr << "Could not inspect project directory: " << ec.message() << '\n';
            return 4;
        }
        if (manifest_exists) {
            std::cerr << "Project already exists: " << (project_dir / nsmb::project::manifest_file_name) << '\n';
            return 4;
        }

        auto data = nsmb::core::read_file_prefix(rom_path, 0x4000);
        if (!data) {
            std::cerr << data.error().message << '\n';
            return 5;
        }
        auto header = nsmb::nds::parse_rom_header(data.value());
        if (!header) {
            std::cerr << header.error().message << '\n';
            return 6;
        }

        nsmb::nds::NitroRomService rom_service;
        auto extracted = rom_service.extract(rom_path, extracted_dir);
        if (!extracted) {
            std::cerr << extracted.error().message << '\n';
            return 7;
        }

        auto project_name = project_dir.filename().string();
        if (project_name.empty() || project_name == ".") {
            project_name = header.value().game_title.empty() ? "Imported ROM" : header.value().game_title;
        }
        auto project = nsmb::project::create_empty_project(project_name, project_dir).value();
        project.attach_extracted_rom("extracted-rom", header.value());
        auto indexed = nsmb::project::index_project_files(project, extracted_dir);
        if (!indexed) {
            std::cerr << indexed.error().message << '\n';
            return 8;
        }

        auto saved = nsmb::project::save_manifest(project, project_dir / nsmb::project::manifest_file_name);
        if (!saved) {
            std::cerr << saved.error().message << '\n';
            return 8;
        }

        std::cout << "Imported ROM into " << project_dir << "\n"
                  << "Files indexed: " << project.game_files().size() << "\n"
                  << "Graphics candidates: " << count_graphics_candidates(project) << "\n";
        return 0;
    }

    if (command == "run-patcher" && argc == 3) {
        nsmb::patching::NCPatcherExternalTool patcher;
        auto result = patcher.run({argv[2]});
        if (!result) {
            std::cerr << result.error().message << '\n';
            return 10;
        }
        return result.value().exit_code;
    }

    if (command == "build-rom" && argc == 4) {
        const fs::path project_dir = argv[2];
        auto project = nsmb::project::load_manifest(project_dir / nsmb::project::manifest_file_name);
        if (!project) {
            std::cerr << project.error().message << '\n';
            return 11;
        }
        if (!project.value().rom() || project.value().rom()->extracted_path.empty()) {
            std::cerr << "Project does not contain an imported ROM\n";
            return 11;
        }
        auto extracted_path = project.value().rom()->extracted_path;
        if (!extracted_path.is_absolute()) {
            extracted_path = project_dir / extracted_path;
        }
        nsmb::nds::NitroRomService service;
        auto result = service.build(extracted_path, argv[3]);
        if (!result) {
            std::cerr << result.error().message << '\n';
            return 11;
        }
        return 0;
    }

    usage();
    return 1;
}
