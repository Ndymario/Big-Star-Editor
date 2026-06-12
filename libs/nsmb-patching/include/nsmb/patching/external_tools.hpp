#pragma once

#include <filesystem>
#include <string>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/nds/rom.hpp"

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

class ExternalToolRomBuildService final : public nds::RomExtractionService, public nds::RomBuildService {
public:
    explicit ExternalToolRomBuildService(std::filesystem::path nds_extract = "nds-extract",
                                         std::filesystem::path nds_build = "nds-build");

    [[nodiscard]] core::Result<ExternalToolCommand> build_extract_command(const std::filesystem::path& rom,
                                                                          const std::filesystem::path& output_dir) const;
    [[nodiscard]] core::Result<ExternalToolCommand> build_rom_command(const std::filesystem::path& build_rules,
                                                                      const std::filesystem::path& output_rom) const;
    [[nodiscard]] core::Result<std::filesystem::path> write_fireflower_build_rules(const std::filesystem::path& extracted_dir,
                                                                                   const std::filesystem::path& rules_path) const;

    core::Result<nds::ExtractedRom> extract(const std::filesystem::path& rom, const std::filesystem::path& output_dir) override;
    core::Result<void> build(const std::filesystem::path& extracted_dir, const std::filesystem::path& output_rom) override;

private:
    std::filesystem::path nds_extract_;
    std::filesystem::path nds_build_;
};

std::string command_to_string(const ExternalToolCommand& command);
core::Result<ExternalToolResult> run_external_tool(const ExternalToolCommand& command);

} // namespace nsmb::patching
