#include "nsmb/project/manifest.hpp"

#include "nsmb/core/file_io.hpp"

#include <algorithm>
#include <charconv>
#include <cctype>
#include <fstream>
#include <optional>
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

core::Result<std::string> parse_scalar(std::string value) {
    value = trim(std::move(value));
    if (value.empty() || value.front() != '"') {
        return value;
    }
    if (value.size() < 2 || value.back() != '"') {
        return core::make_error(core::ErrorCode::ParseError, "unterminated quoted manifest value");
    }

    std::string decoded;
    decoded.reserve(value.size() - 2U);
    for (std::size_t i = 1; i + 1U < value.size(); ++i) {
        const auto c = value[i];
        if (c == '"') {
            return core::make_error(core::ErrorCode::ParseError, "unescaped quote in manifest value");
        }
        if (c != '\\') {
            decoded.push_back(c);
            continue;
        }
        if (++i + 1U >= value.size()) {
            return core::make_error(core::ErrorCode::ParseError, "incomplete escape in manifest value");
        }
        switch (value[i]) {
        case '\\': decoded.push_back('\\'); break;
        case '"': decoded.push_back('"'); break;
        case 'n': decoded.push_back('\n'); break;
        case 'r': decoded.push_back('\r'); break;
        case 't': decoded.push_back('\t'); break;
        default:
            return core::make_error(core::ErrorCode::ParseError, "unsupported escape in manifest value");
        }
    }
    return decoded;
}

std::string yaml_scalar(std::string_view key, std::string_view value) {
    std::string escaped;
    escaped.reserve(value.size());
    for (char c : value) {
        switch (c) {
        case '"': escaped += "\\\""; break;
        case '\\': escaped += "\\\\"; break;
        case '\n': escaped += "\\n"; break;
        case '\r': escaped += "\\r"; break;
        case '\t': escaped += "\\t"; break;
        default: escaped.push_back(c); break;
        }
    }
    return std::string(key) + ": \"" + escaped + "\"\n";
}

bool contains_parent_reference(const std::filesystem::path& path) {
    return std::any_of(path.begin(), path.end(), [](const auto& part) { return part == ".."; });
}

std::string path_to_manifest_string(const std::filesystem::path& path) {
    return path.lexically_normal().generic_string();
}

core::Result<std::filesystem::path> parse_manifest_path(std::string value, std::string_view field) {
    auto scalar = parse_scalar(std::move(value));
    if (!scalar) {
        return scalar.error();
    }
    const auto& text = scalar.value();
    const auto has_drive_prefix = text.size() >= 2U && std::isalpha(static_cast<unsigned char>(text[0])) && text[1] == ':';
    auto path = std::filesystem::path(text).lexically_normal();
    if (path.empty() || path == "." || path.is_absolute() || has_drive_prefix || text.find('\\') != std::string::npos ||
        contains_parent_reference(path)) {
        return core::make_error(core::ErrorCode::ParseError, std::string(field) + " must be a project-relative path");
    }
    return path;
}

core::Result<std::uintmax_t> parse_size(std::string value) {
    value = trim(std::move(value));
    std::uintmax_t parsed{};
    const auto [end, error] = std::from_chars(value.data(), value.data() + value.size(), parsed);
    if (error != std::errc{} || end != value.data() + value.size()) {
        return core::make_error(core::ErrorCode::ParseError, "manifest file size is not a valid integer");
    }
    return parsed;
}

} // namespace

core::Result<std::filesystem::path> make_project_relative(const std::filesystem::path& project_dir,
                                                          const std::filesystem::path& path) {
    auto normalized = path.lexically_normal();
    if (normalized.empty() || normalized == ".") {
        return core::make_error(core::ErrorCode::InvalidArgument, "project manifest paths cannot be empty");
    }
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
    std::ostringstream out;

    out << "format: big-star-editor-project\n";
    out << "version: " << manifest_version << "\n";
    out << yaml_scalar("name", project.name());

    if (project.rom()) {
        auto extracted_path = make_project_relative(project_dir, project.rom()->extracted_path);
        if (!extracted_path) {
            return extracted_path.error();
        }
        out << "rom:\n";
        out << "  " << yaml_scalar("extracted_path", path_to_manifest_string(extracted_path.value()));
        if (project.rom()->header) {
            out << "  " << yaml_scalar("game_title", project.rom()->header->game_title);
            out << "  " << yaml_scalar("game_code", project.rom()->header->game_code);
        }
    }

    return core::write_text_file_atomically(path, out.str());
}

core::Result<Project> load_manifest(const std::filesystem::path& path) {
    std::ifstream in(path);
    if (!in) {
        return core::make_error(core::ErrorCode::IoError, "could not open project manifest for reading");
    }

    Project project;
    project.set_directory(path.parent_path());

    bool saw_format = false;
    bool saw_version = false;
    bool saw_name = false;
    bool in_rom = false;
    std::optional<nds::RomHeader> header;
    std::filesystem::path extracted_path;
    std::string line;

    while (std::getline(in, line)) {
        const auto stripped = trim(line);
        if (stripped.empty() || stripped.front() == '#') {
            continue;
        }
        if (stripped == "format: big-star-editor-project") {
            saw_format = true;
            in_rom = false;
        } else if (stripped.rfind("version:", 0) == 0) {
            auto version = parse_size(stripped.substr(8));
            if (!version || version.value() == 0U || version.value() > manifest_version) {
                return core::make_error(core::ErrorCode::ParseError, "unsupported project manifest version");
            }
            saw_version = true;
            in_rom = false;
        } else if (stripped.rfind("name:", 0) == 0) {
            auto name = parse_scalar(stripped.substr(5));
            if (!name) {
                return name.error();
            }
            project.set_name(std::move(name.value()));
            saw_name = true;
            in_rom = false;
        } else if (stripped == "rom:") {
            in_rom = true;
        } else if (stripped == "files:") {
            // Version 1 mirrored the whole extracted tree here. That list is derived from
            // the tree itself and carries no IDs, so it is neither written nor read now --
            // its entries fall through unmatched and the tree is indexed below instead.
            in_rom = false;
        } else if (in_rom && stripped.rfind("extracted_path:", 0) == 0) {
            auto parsed = parse_manifest_path(stripped.substr(15), "rom.extracted_path");
            if (!parsed) {
                return parsed.error();
            }
            extracted_path = std::move(parsed.value());
        } else if (in_rom && stripped.rfind("game_title:", 0) == 0) {
            if (!header) {
                header = nds::RomHeader{};
            }
            auto title = parse_scalar(stripped.substr(11));
            if (!title) {
                return title.error();
            }
            header->game_title = std::move(title.value());
        } else if (in_rom && stripped.rfind("game_code:", 0) == 0) {
            if (!header) {
                header = nds::RomHeader{};
            }
            auto code = parse_scalar(stripped.substr(10));
            if (!code) {
                return code.error();
            }
            header->game_code = std::move(code.value());
        }
    }

    if (!saw_format) {
        return core::make_error(core::ErrorCode::ParseError, "manifest format marker is missing");
    }
    if (!saw_version) {
        return core::make_error(core::ErrorCode::ParseError, "manifest version is missing");
    }
    if (!saw_name || project.name().empty()) {
        return core::make_error(core::ErrorCode::ParseError, "project name is missing");
    }
    if (!extracted_path.empty()) {
        project.attach_extracted_rom(extracted_path, header);
        // The file index is a cache of the extracted tree, so it is rebuilt on open rather
        // than persisted. A project whose ROM has not been extracted yet still loads --
        // it just has an empty index until the next import or NCPatcher build.
        static_cast<void>(index_project_files(project, project.directory() / extracted_path));
    }
    project.mark_clean();
    return project;
}

core::Result<void> index_project_files(Project& project, const std::filesystem::path& extracted_dir) {
    if (project.directory().empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "project directory is required before indexing files");
    }
    if (!project.rom() || project.rom()->extracted_path.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "project needs an extracted ROM before indexing files");
    }
    auto relative_root = make_project_relative(project.directory(), extracted_dir);
    if (!relative_root) {
        return relative_root.error();
    }
    auto configured_root = make_project_relative(project.directory(), project.rom()->extracted_path);
    if (!configured_root) {
        return configured_root.error();
    }
    if (relative_root.value() != configured_root.value()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "indexed directory does not match rom.extracted_path");
    }
    std::error_code ec;
    if (!std::filesystem::is_directory(extracted_dir, ec)) {
        return core::make_error(core::ErrorCode::NotFound, "extracted ROM directory does not exist");
    }

    std::vector<GameFile> files;
    std::filesystem::recursive_directory_iterator current(extracted_dir, ec);
    const std::filesystem::recursive_directory_iterator end;
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not read extracted ROM directory");
    }
    while (current != end) {
        const auto regular = current->is_regular_file(ec);
        if (ec) {
            return core::make_error(core::ErrorCode::IoError, "could not inspect " + current->path().string());
        }
        if (regular) {
            auto relative = make_project_relative(project.directory(), current->path());
            if (!relative) {
                return relative.error();
            }
            const auto size = current->file_size(ec);
            if (ec) {
                return core::make_error(core::ErrorCode::IoError, "could not read file size for " + current->path().string());
            }
            files.push_back({std::move(relative.value()), size});
        }
        current.increment(ec);
        if (ec) {
            return core::make_error(core::ErrorCode::IoError, "could not continue reading extracted ROM directory");
        }
    }
    std::sort(files.begin(), files.end(), [](const auto& lhs, const auto& rhs) {
        return lhs.path.generic_string() < rhs.path.generic_string();
    });
    project.clear_game_files();
    for (auto& file : files) {
        project.add_game_file(std::move(file));
    }
    return {};
}

} // namespace nsmb::project
