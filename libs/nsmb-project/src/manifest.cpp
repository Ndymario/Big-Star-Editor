#include "nsmb/project/manifest.hpp"
#include <fstream>
#include <sstream>
namespace nsmb::project {
static std::string value_after(std::string_view line, std::string_view key) { return line.substr(key.size()).data(); }
core::Result<void> save_manifest(const Project& project, const std::filesystem::path& path) {
    std::error_code ec; std::filesystem::create_directories(path.parent_path(), ec);
    std::ofstream out(path); if (!out) return core::make_error(core::ErrorCode::IoError, "could not open project manifest for writing");
    out << "format=big-star-editor-project\nversion=1\n";
    out << "name=" << project.name() << "\n";
    out << "root=" << project.root_directory().generic_string() << "\n";
    if (project.rom()) out << "extracted_rom=" << project.rom()->extracted_path.generic_string() << "\n";
    for (const auto& file : project.game_files()) out << "file=" << file.path.generic_string() << "|" << file.size << "\n";
    return {};
}
core::Result<Project> load_manifest(const std::filesystem::path& path) {
    std::ifstream in(path); if (!in) return core::make_error(core::ErrorCode::IoError, "could not open project manifest for reading");
    Project project; std::string line; bool saw_format = false;
    while (std::getline(in, line)) {
        if (line == "format=big-star-editor-project") saw_format = true;
        else if (line.rfind("name=", 0) == 0) project.set_name(line.substr(5));
        else if (line.rfind("root=", 0) == 0) project.set_root_directory(line.substr(5));
        else if (line.rfind("extracted_rom=", 0) == 0) project.attach_extracted_rom(line.substr(14));
        else if (line.rfind("file=", 0) == 0) { auto body = line.substr(5); auto sep = body.rfind('|'); GameFile f; f.path = body.substr(0, sep); if (sep != std::string::npos) f.size = static_cast<std::uintmax_t>(std::stoull(body.substr(sep + 1))); project.add_game_file(f); }
    }
    if (!saw_format) return core::make_error(core::ErrorCode::ParseError, "manifest format marker is missing");
    project.mark_clean(); return project;
}
}
