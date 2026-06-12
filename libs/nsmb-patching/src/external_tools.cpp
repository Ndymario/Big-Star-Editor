#include "nsmb/patching/external_tools.hpp"
#include <cstdlib>
#include <fstream>
#include <sstream>
namespace nsmb::patching {
static std::string quote(const std::filesystem::path& p) { return '"' + p.string() + '"'; }
std::string command_to_string(const ExternalToolCommand& command) { std::ostringstream oss; oss << quote(command.executable); for (const auto& arg : command.arguments) oss << ' ' << quote(arg); return oss.str(); }
core::Result<void> NCPatcherExternalTool::validate_project(const std::filesystem::path& project_dir) const { std::error_code ec; if (!std::filesystem::is_directory(project_dir, ec)) return core::make_error(core::ErrorCode::NotFound, "project directory does not exist"); return {}; }
core::Result<std::filesystem::path> NCPatcherExternalTool::validate_or_create_config(const std::filesystem::path& project_dir) const {
    auto valid = validate_project(project_dir); if (!valid) return valid.error();
    auto config = project_dir / "ncpatcher.json";
    if (!std::filesystem::exists(config)) {
        std::ofstream out(config); if (!out) return core::make_error(core::ErrorCode::IoError, "could not create ncpatcher.json");
        out << "{\n  \"filesystem\": \"__tmp\",\n  \"arm7\": {},\n  \"arm9\": { \"target\": \"arm9.json\", \"build\": \"build\" },\n  \"pre-build\": [],\n  \"post-build\": [],\n  \"thread-count\": 0\n}\n";
    }
    return config;
}
core::Result<ExternalToolCommand> NCPatcherExternalTool::build_command(const PatcherRunRequest& request) const { auto valid = validate_project(request.project_dir); if (!valid) return valid.error(); ExternalToolCommand c{request.ncpatcher_executable, {}, request.project_dir}; if (request.verbose) c.arguments.push_back("-v"); return c; }
core::Result<ExternalToolResult> NCPatcherExternalTool::run(const PatcherRunRequest& request) { auto config = validate_or_create_config(request.project_dir); if (!config) return config.error(); auto cmd = build_command(request); if (!cmd) return cmd.error(); auto old = std::filesystem::current_path(); std::filesystem::current_path(cmd.value().working_directory); int code = std::system(command_to_string(cmd.value()).c_str()); std::filesystem::current_path(old); return ExternalToolResult{code, {}, {}}; }
ExternalToolRomBuildService::ExternalToolRomBuildService(std::filesystem::path nds_extract, std::filesystem::path nds_build) : nds_extract_(std::move(nds_extract)), nds_build_(std::move(nds_build)) {}
core::Result<ExternalToolCommand> ExternalToolRomBuildService::build_extract_command(const std::filesystem::path& rom, const std::filesystem::path& output_dir) const { return ExternalToolCommand{nds_extract_, {rom.string(), output_dir.string()}, output_dir.parent_path()}; }
core::Result<ExternalToolCommand> ExternalToolRomBuildService::build_rom_command(const std::filesystem::path& extracted_dir, const std::filesystem::path& output_rom) const { return ExternalToolCommand{nds_build_, {extracted_dir.string(), output_rom.string()}, extracted_dir}; }
core::Result<nds::ExtractedRom> ExternalToolRomBuildService::extract(const std::filesystem::path& rom, const std::filesystem::path& output_dir) { auto cmd = build_extract_command(rom, output_dir); if (!cmd) return cmd.error(); int code = std::system(command_to_string(cmd.value()).c_str()); if (code != 0) return core::make_error(core::ErrorCode::ExternalToolFailed, "nds-extract failed"); auto tree = nds::scan_file_tree(output_dir); if (!tree) return tree.error(); return nds::ExtractedRom{output_dir, tree.value()}; }
core::Result<void> ExternalToolRomBuildService::build(const std::filesystem::path& extracted_dir, const std::filesystem::path& output_rom) { auto cmd = build_rom_command(extracted_dir, output_rom); if (!cmd) return cmd.error(); int code = std::system(command_to_string(cmd.value()).c_str()); if (code != 0) return core::make_error(core::ErrorCode::ExternalToolFailed, "nds-build failed"); return {}; }
}
