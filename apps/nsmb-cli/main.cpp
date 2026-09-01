#include "nsmb/formats/graphics.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/ncpatcher.hpp"
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
              << "  run-patcher <project-dir> [--variant <name>]\n"
              << "  patcher-info <project-dir>\n";
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

        // The ROM is read where it lies rather than unpacked, so importing one
        // is copying it in and reading its table -- and a manifest path has to
        // stay inside the project, so a ROM from elsewhere is copied rather
        // than pointed at.
        const auto imported = project_dir / nsmb::project::input_rom_file_name;
        if (fs::weakly_canonical(rom_path, ec) != fs::weakly_canonical(imported, ec)) {
            fs::copy_file(rom_path, imported, fs::copy_options::overwrite_existing, ec);
            if (ec) {
                std::cerr << "Could not copy the ROM into the project: " << ec.message() << '\n';
                return 7;
            }
        }

        auto project_name = project_dir.filename().string();
        if (project_name.empty() || project_name == ".") {
            project_name = header.value().game_title.empty() ? "Imported ROM" : header.value().game_title;
        }
        auto project = nsmb::project::create_empty_project(project_name, project_dir).value();
        project.attach_rom(nsmb::project::input_rom_file_name, header.value());
        auto indexed = nsmb::project::index_rom_files(project);
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

    if (command == "run-patcher" && (argc == 3 || argc == 5)) {
        nsmb::patching::BuildRequest request;
        request.project_dir = argv[2];
        // `run-patcher <project> --variant <name>`. A project that configures
        // variants requires an explicit one, so NCPatcher cannot silently build
        // the wrong language.
        if (argc == 5) {
            if (std::string(argv[3]) != "--variant") {
                usage();
                return 2;
            }
            request.variant = argv[4];
        }

        nsmb::core::CallbackLogger logger([](nsmb::core::LogLevel level, std::string_view message) {
            std::cerr << '[' << nsmb::core::log_level_name(level) << "] " << message << '\n';
        });
        nsmb::patching::NCPatcherClient client("ncpatcher", logger);

        nsmb::patching::BuildObserver observer;
        observer.on_progress = [](const nsmb::patching::BuildProgress& progress) {
            std::cerr << progress.phase << ' ' << progress.current << '/' << progress.total
                      << "  " << progress.item << '\n';
        };
        observer.on_diagnostic = [](const nsmb::patching::Diagnostic& diagnostic) {
            std::cerr << diagnostic.code << ": " << diagnostic.message << '\n';
        };
        observer.on_log = [](std::string_view line) { std::cerr << line << '\n'; };

        auto result = client.build(request, observer);
        if (!result) {
            std::cerr << result.error().message << '\n';
            return 10;
        }
        const auto& summary = result.value();
        if (!summary.succeeded()) {
            std::cerr << "NCPatcher: " << describe_stage(summary.stage) << " ("
                      << summary.errors << " error(s), " << summary.warnings << " warning(s))\n";
        }
        return summary.exit_code;
    }

    if (command == "patcher-info" && argc == 3) {
        // What the project says about itself, without building it.
        nsmb::patching::NCPatcherClient client;
        const fs::path project_dir = argv[2];

        auto version = client.version();
        if (!version) {
            std::cerr << version.error().message << '\n';
            return 10;
        }
        std::cout << version.value() << '\n';

        auto config = client.dump_config(project_dir);
        if (!config) {
            std::cerr << config.error().message << '\n';
            return 10;
        }
        std::cout << "Config: " << config.value().config_file << '\n'
                  << "ROM: " << config.value().rom_file << '\n'
                  << "Toolchain: " << config.value().toolchain << '\n';
        std::cout << "Variants: ";
        if (config.value().variants.empty()) {
            std::cout << "(this project configures none)";
        } else {
            for (const auto& variant : config.value().variants) {
                std::cout << variant.name << ' ';
            }
        }
        std::cout << '\n';

        auto graph = client.dump_modules(project_dir);
        if (graph) {
            std::cout << "Modules (" << graph.value().modules.size() << "):\n";
            for (const auto& module : graph.value().modules) {
                std::cout << "  " << module.key << (module.enabled ? "" : " (disabled)");
                if (module.nitrofs) {
                    std::cout << " nitrofs=" << module.nitrofs->dir
                              << (module.nitrofs->layered ? " [layered]" : "");
                }
                std::cout << '\n';
            }
        }
        return 0;
    }

    usage();
    return 1;
}
