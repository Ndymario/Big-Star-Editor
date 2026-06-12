#include "nsmb/patching/external_tools.hpp"

#include <array>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>
#include <system_error>

namespace nsmb::patching {

namespace {

std::string shell_quote(const std::filesystem::path& path) {
    std::string input = path.string();
    std::string quoted = "\"";
    for (char c : input) {
        if (c == '"') {
            quoted += "\\\"";
        } else {
            quoted.push_back(c);
        }
    }
    quoted.push_back('"');
    return quoted;
}

std::filesystem::path default_build_rules_path(const std::filesystem::path& extracted_dir) {
    return extracted_dir.parent_path() / "buildrules.txt";
}

} // namespace

std::string command_to_string(const ExternalToolCommand& command) {
    std::ostringstream oss;
    oss << shell_quote(command.executable);
    for (const auto& arg : command.arguments) {
        oss << ' ' << shell_quote(arg);
    }
    return oss.str();
}

core::Result<ExternalToolResult> run_external_tool(const ExternalToolCommand& command) {
    std::error_code ec;
    const auto old_working_directory = std::filesystem::current_path(ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not read current working directory");
    }

    if (!command.working_directory.empty()) {
        std::filesystem::current_path(command.working_directory, ec);
        if (ec) {
            return core::make_error(core::ErrorCode::IoError, "could not change external tool working directory");
        }
    }

    const int exit_code = std::system(command_to_string(command).c_str());

    std::filesystem::current_path(old_working_directory, ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not restore working directory after external tool run");
    }

    return ExternalToolResult{exit_code, {}, {}};
}

core::Result<void> NCPatcherExternalTool::validate_project(const std::filesystem::path& project_dir) const {
    std::error_code ec;
    if (!std::filesystem::is_directory(project_dir, ec)) {
        return core::make_error(core::ErrorCode::NotFound, "project directory does not exist");
    }
    return {};
}

core::Result<std::filesystem::path> NCPatcherExternalTool::validate_or_create_config(const std::filesystem::path& project_dir) const {
    auto valid = validate_project(project_dir);
    if (!valid) {
        return valid.error();
    }

    auto config = project_dir / "ncpatcher.json";
    if (!std::filesystem::exists(config)) {
        std::ofstream out(config);
        if (!out) {
            return core::make_error(core::ErrorCode::IoError, "could not create ncpatcher.json");
        }
        out << "{\n"
            << "  \"filesystem\": \"extracted-rom\",\n"
            << "  \"arm7\": {},\n"
            << "  \"arm9\": { \"target\": \"arm9.json\", \"build\": \"build\" },\n"
            << "  \"pre-build\": [],\n"
            << "  \"post-build\": [],\n"
            << "  \"thread-count\": 0\n"
            << "}\n";
    }
    return config;
}

core::Result<ExternalToolCommand> NCPatcherExternalTool::build_command(const PatcherRunRequest& request) const {
    auto valid = validate_project(request.project_dir);
    if (!valid) {
        return valid.error();
    }

    ExternalToolCommand command{request.ncpatcher_executable, {}, request.project_dir};
    if (request.verbose) {
        command.arguments.push_back("-v");
    }
    return command;
}

core::Result<ExternalToolResult> NCPatcherExternalTool::run(const PatcherRunRequest& request) {
    auto config = validate_or_create_config(request.project_dir);
    if (!config) {
        return config.error();
    }

    auto command = build_command(request);
    if (!command) {
        return command.error();
    }

    auto result = run_external_tool(command.value());
    if (!result) {
        return result.error();
    }
    if (result.value().exit_code != 0) {
        return core::make_error(core::ErrorCode::ExternalToolFailed, "NCPatcher failed");
    }
    return result.value();
}

ExternalToolRomBuildService::ExternalToolRomBuildService(std::filesystem::path nds_extract, std::filesystem::path nds_build)
    : nds_extract_(std::move(nds_extract)), nds_build_(std::move(nds_build)) {}

core::Result<ExternalToolCommand> ExternalToolRomBuildService::build_extract_command(const std::filesystem::path& rom,
                                                                                     const std::filesystem::path& output_dir) const {
    if (rom.empty() || output_dir.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "ROM input and extraction output paths are required");
    }
    return ExternalToolCommand{nds_extract_, {rom.string(), output_dir.string()}, output_dir.parent_path()};
}

core::Result<ExternalToolCommand> ExternalToolRomBuildService::build_rom_command(const std::filesystem::path& build_rules,
                                                                                 const std::filesystem::path& output_rom) const {
    if (build_rules.empty() || output_rom.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument, "build rules and output ROM paths are required");
    }
    return ExternalToolCommand{nds_build_, {build_rules.string(), output_rom.string()}, build_rules.parent_path()};
}

core::Result<std::filesystem::path> ExternalToolRomBuildService::write_fireflower_build_rules(const std::filesystem::path& extracted_dir,
                                                                                              const std::filesystem::path& rules_path) const {
    std::error_code ec;
    if (!std::filesystem::is_directory(extracted_dir, ec)) {
        return core::make_error(core::ErrorCode::NotFound, "extracted ROM directory does not exist");
    }

    std::filesystem::create_directories(rules_path.parent_path(), ec);
    if (ec) {
        return core::make_error(core::ErrorCode::IoError, "could not create build rules directory");
    }

    const auto root = extracted_dir.generic_string();
    std::ofstream out(rules_path);
    if (!out) {
        return core::make_error(core::ErrorCode::IoError, "could not write nds-build rules");
    }

    out << "rom_header " << root << "/header.bin\n"
        << "arm9_entry KEEP\n"
        << "arm9_load KEEP\n"
        << "arm7_entry KEEP\n"
        << "arm7_load KEEP\n"
        << "fnt " << root << "/fnt.bin\n"
        << "file_mode ADJUST\n"
        << "arm9 " << root << "/arm9.bin\n"
        << "arm7 " << root << "/arm7.bin\n"
        << "arm9ovt " << root << "/arm9ovt.bin\n"
        << "arm7ovt " << root << "/arm7ovt.bin\n"
        << "icon " << root << "/banner.bin\n"
        << "rsa_sig " << root << "/rsasig.bin\n"
        << "data " << root << "/root\n"
        << "ovt_repl_flag 0xFF\n"
        << "ov9 " << root << "/overlay9\n"
        << "ov7 " << root << "/overlay7\n";

    return rules_path;
}

core::Result<nds::ExtractedRom> ExternalToolRomBuildService::extract(const std::filesystem::path& rom,
                                                                     const std::filesystem::path& output_dir) {
    auto command = build_extract_command(rom, output_dir);
    if (!command) {
        return command.error();
    }

    auto result = run_external_tool(command.value());
    if (!result) {
        return result.error();
    }
    if (result.value().exit_code != 0) {
        return core::make_error(core::ErrorCode::ExternalToolFailed, "nds-extract failed");
    }

    auto tree = nds::scan_file_tree(output_dir);
    if (!tree) {
        return tree.error();
    }
    return nds::ExtractedRom{output_dir, tree.value()};
}

core::Result<void> ExternalToolRomBuildService::build(const std::filesystem::path& extracted_dir,
                                                      const std::filesystem::path& output_rom) {
    const auto rules_path = default_build_rules_path(extracted_dir);
    auto rules = write_fireflower_build_rules(extracted_dir, rules_path);
    if (!rules) {
        return rules.error();
    }

    auto command = build_rom_command(rules.value(), output_rom);
    if (!command) {
        return command.error();
    }

    auto result = run_external_tool(command.value());
    if (!result) {
        return result.error();
    }
    if (result.value().exit_code != 0) {
        return core::make_error(core::ErrorCode::ExternalToolFailed, "nds-build failed");
    }
    return {};
}

} // namespace nsmb::patching
