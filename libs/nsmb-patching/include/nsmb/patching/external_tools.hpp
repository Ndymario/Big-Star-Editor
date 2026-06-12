#pragma once

#include <filesystem>
#include <string>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::patching {

struct ExternalToolCommand {
    std::filesystem::path executable;
    std::vector<std::string> arguments;
    std::filesystem::path working_directory;
};

struct ExternalToolResult {
    int exit_code{};
    std::string stdout_text;
    std::string stderr_text;
};

struct PatcherRunRequest {
    std::filesystem::path project_dir;
    std::filesystem::path ncpatcher_executable{"ncpatcher"};
    bool verbose{};
};

class PatcherService {
public:
    virtual ~PatcherService() = default;
    virtual core::Result<ExternalToolResult> run(const PatcherRunRequest& request) = 0;
};

class NCPatcherExternalTool final : public PatcherService {
public:
    [[nodiscard]] core::Result<ExternalToolCommand> build_command(const PatcherRunRequest& request) const;
    [[nodiscard]] core::Result<void> validate_project(const std::filesystem::path& project_dir) const;
    [[nodiscard]] core::Result<std::filesystem::path> validate_or_create_config(const std::filesystem::path& project_dir) const;
    core::Result<ExternalToolResult> run(const PatcherRunRequest& request) override;
};

std::string command_to_string(const ExternalToolCommand& command);
core::Result<ExternalToolResult> run_external_tool(const ExternalToolCommand& command);

} // namespace nsmb::patching
