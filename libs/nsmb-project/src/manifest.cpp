#include "nsmb/project/manifest.hpp"

#include <algorithm>
#include <fstream>
#include <sstream>
#include <string>
#include <vector>

namespace nsmb::project {

namespace {

std::string trim(std::string value) {
    const auto first = value.find_first_not_of(" \t\r\n");
    if (first == std::string::npos) {
        return {};
    }
    const auto last = value.find_last_not_of(" \t\r\n");
    return value.substr(first, last - first + 1);
}

std::string unquote(std::string value) {
    value = trim(std::move(value));
    if (value.size() >= 2 && value.front() == '"' && value.back() == '"') {
        value = value.substr(1, value.size() - 2);
    }
    return value;
}

std::string yaml_scalar(std::string_view key, std::string_view value) {
    std::string escaped;
    escaped.reserve(value.size());
    for (char c : value) {
        if (c == '"' || c == '\\') {
            escaped.push_back('\\');
        }
        escaped.push_back(c);
    }
    return std::string(key) + ": \"" + escaped + "\"\n";
}

bool contains_parent_reference(const std::filesystem::path& path) {
    return std::any_of(path.begin(), path.end(), [](const auto& part) { return part == ".."; });
}

std::string path_to_manifest_string(const std::filesystem::path& path) {
    return path.lexically_normal().generic_string();
}

} // namespace

core::Result<std::filesystem::path> make_project_relative(const std::filesystem::path& project_dir,
                                                          const std::filesystem::path& path) {
    auto normalized = path.lexically_normal();
    if (!normalized.is_absolute()) {
        if (contains_parent_reference(normalized)) {
            return core::make_error(core::ErrorCode::InvalidArgument, "project manifest paths must stay inside the project directory");
        }
        return normalized;
    }

    std::error_code ec;
    auto relative = std::filesystem::relative(normalized, project_dir, ec);
    if (ec || relative.empty() || relative.is_absolute() || contains_parent_reference(relative)) {
        return core::make_error(core::ErrorCode::InvalidArgument, "absolute path is outside the project directory");
    }
    return relative.lexically_normal();
}

core::Result<void> save_manifest(const Project& project, const std::filesystem::path& path) {
    const auto project_dir = path.parent_path();
    std::error_code ec;
    std::filesystem::create_directories(project_dir, ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not create project directory");
    }

    std::ofstream out(path);
    if (!out) {
        return core::make_error(core::ErrorCode::IoError, "could not open project manifest for writing");
    }

    out << "format: big-star-editor-project\n";
    out << "version: 1\n";
    out << yaml_scalar("name", project.name());

    if (project.rom()) {
        auto extracted_path = make_project_relative(project_dir, project.rom()->extracted_path);
        if (!extracted_path) {
            return extracted_path.error();
        }
        out << "rom:\n";
        out << "  extracted_path: \"" << path_to_manifest_string(extracted_path.value()) << "\"\n";
        if (project.rom()->header) {
            out << "  game_title: \"" << project.rom()->header->game_title << "\"\n";
            out << "  game_code: \"" << project.rom()->header->game_code << "\"\n";
        }
        out << "files:\n";
        for (const auto& file : project.game_files()) {
            auto relative = make_project_relative(project_dir, file.path);
            if (!relative) {
                return relative.error();
            }
            out << "  - path: \"" << path_to_manifest_string(relative.value()) << "\"\n";
            out << "    size: " << file.size << "\n";
        }
    }

    return {};
}

core::Result<Project> load_manifest(const std::filesystem::path& path) {
    std::ifstream in(path);
    if (!in) {
        return core::make_error(core::ErrorCode::IoError, "could not open project manifest for reading");
    }

    Project project;
    project.set_directory(path.parent_path());

    bool saw_format = false;
    bool in_rom = false;
    bool in_files = false;
    std::optional<nds::RomHeader> header;
    std::filesystem::path extracted_path;
    std::optional<GameFile> pending_file;
    std::vector<GameFile> files;
    std::string line;

    auto flush_pending_file = [&files, &pending_file]() {
        if (pending_file) {
            files.push_back(*pending_file);
            pending_file.reset();
        }
    };

    while (std::getline(in, line)) {
        const auto stripped = trim(line);
        if (stripped.empty() || stripped.front() == '#') {
            continue;
        }
        if (stripped == "format: big-star-editor-project") {
            saw_format = true;
            in_rom = false;
            in_files = false;
        } else if (stripped.rfind("name:", 0) == 0) {
            project.set_name(unquote(stripped.substr(5)));
            in_rom = false;
            in_files = false;
        } else if (stripped == "rom:") {
            in_rom = true;
            in_files = false;
        } else if (stripped == "files:") {
            in_rom = false;
            in_files = true;
        } else if (in_rom && stripped.rfind("extracted_path:", 0) == 0) {
            extracted_path = unquote(stripped.substr(15));
        } else if (in_rom && stripped.rfind("game_title:", 0) == 0) {
            if (!header) {
                header = nds::RomHeader{};
            }
            header->game_title = unquote(stripped.substr(11));
        } else if (in_rom && stripped.rfind("game_code:", 0) == 0) {
            if (!header) {
                header = nds::RomHeader{};
            }
            header->game_code = unquote(stripped.substr(10));
        } else if (in_files && stripped.rfind("- path:", 0) == 0) {
            flush_pending_file();
            pending_file = GameFile{};
            pending_file->path = unquote(stripped.substr(7));
        } else if (in_files && stripped.rfind("size:", 0) == 0 && pending_file) {
            pending_file->size = static_cast<std::uintmax_t>(std::stoull(trim(stripped.substr(5))));
        }
    }
    flush_pending_file();

    if (!saw_format) {
        return core::make_error(core::ErrorCode::ParseError, "manifest format marker is missing");
    }
    if (!extracted_path.empty()) {
        project.attach_extracted_rom(extracted_path, header);
    }
    for (const auto& file : files) {
        project.add_game_file(file);
    }
    project.mark_clean();
    return project;
}

} // namespace nsmb::project
