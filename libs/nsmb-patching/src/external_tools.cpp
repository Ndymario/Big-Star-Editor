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
#ifdef _WIN32
    std::string quoted = "\"";
    for (char c : input) {
        if (c == '"') {
            quoted += "\\\"";
        } else {
            quoted.push_back(c);
        }
    }
    quoted.push_back('"');
#else
    std::string quoted = "'";
    for (char c : input) {
        if (c == '\'') {
            quoted += "'\\''";
        } else {
            quoted.push_back(c);
        }
    }
    quoted.push_back('\'');
#endif
    return quoted;
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

} // namespace nsmb::patching
