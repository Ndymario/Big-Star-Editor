#include "nsmb/formats/graphics.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/external_tools.hpp"
#include "nsmb/project/manifest.hpp"

#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>
#include <vector>

namespace fs = std::filesystem;

namespace {

void usage() {
    std::cout << "nsmb-cli 0.2.0\n"
              << "Commands:\n"
              << "  --version\n"
              << "  inspect-rom <path>\n"
              << "  import-rom <rom> <project-dir>\n"
              << "  run-patcher <project-dir>\n"
              << "  build-rom <project-dir> <output-rom>\n";
}

nsmb::core::Result<std::vector<std::uint8_t>> read_file_prefix(const fs::path& path, std::size_t max_size) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return nsmb::core::make_error(nsmb::core::ErrorCode::IoError, "could not open file");
    }

    std::vector<std::uint8_t> data(max_size);
    in.read(reinterpret_cast<char*>(data.data()), static_cast<std::streamsize>(data.size()));
    data.resize(static_cast<std::size_t>(in.gcount()));
    return data;
}

void collect_files(nsmb::project::Project& project, const fs::path& project_dir, const fs::path& extracted_dir) {
    std::error_code ec;
    for (const auto& entry : fs::recursive_directory_iterator(extracted_dir, ec)) {
        if (ec) {
            break;
        }
        if (!entry.is_regular_file(ec)) {
            continue;
        }
        auto relative = nsmb::project::make_project_relative(project_dir, entry.path());
        if (!relative) {
            continue;
        }
        project.add_game_file({relative.value(), entry.file_size(ec)});
    }
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
        std::cout << "nsmb-cli 0.2.0\n";
        return 0;
    }

    if (command == "inspect-rom" && argc == 3) {
        auto data = read_file_prefix(argv[2], 0x4000);
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

        auto data = read_file_prefix(rom_path, 0x4000);
        if (!data) {
            std::cerr << data.error().message << '\n';
            return 5;
        }
        auto header = nsmb::nds::parse_rom_header(data.value());
        if (!header) {
            std::cerr << header.error().message << '\n';
            return 6;
        }

        nsmb::patching::ExternalToolRomBuildService rom_service;
        auto extracted = rom_service.extract(rom_path, extracted_dir);
        if (!extracted) {
            std::cerr << extracted.error().message << '\n';
            return 7;
        }

        auto project = nsmb::project::create_empty_project(project_dir.filename().string(), project_dir).value();
        project.attach_extracted_rom("extracted-rom", header.value());
        collect_files(project, project_dir, extracted_dir);

        auto rules = rom_service.write_fireflower_build_rules(extracted_dir, project_dir / "buildrules.txt");
        if (!rules) {
            std::cerr << rules.error().message << '\n';
            return 8;
        }

        auto saved = nsmb::project::save_manifest(project, project_dir / nsmb::project::manifest_file_name);
        if (!saved) {
            std::cerr << saved.error().message << '\n';
            return 9;
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
        nsmb::patching::ExternalToolRomBuildService service;
        auto result = service.build(fs::path(argv[2]) / "extracted-rom", argv[3]);
        if (!result) {
            std::cerr << result.error().message << '\n';
            return 11;
        }
        return 0;
    }

    usage();
    return 1;
}
