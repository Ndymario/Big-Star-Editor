#include "nsmb/patching/legacy.hpp"

#include <array>
#include <system_error>
#include <utility>

namespace nsmb::patching {
namespace fs = std::filesystem;

namespace {

core::Result<void> copy_tree(const fs::path& source, const fs::path& target) {
    std::error_code ec;
    fs::copy(source, target,
             fs::copy_options::recursive | fs::copy_options::overwrite_existing,
             ec);
    if (ec) return core::make_error(core::ErrorCode::IoError,
        "could not copy " + source.string() + " to " + target.string() + ": " + ec.message());
    return {};
}

core::Result<void> copy_file_required(const fs::path& source, const fs::path& target) {
    std::error_code ec;
    if (!fs::is_regular_file(source, ec)) return core::make_error(
        core::ErrorCode::NotFound, "legacy patch input is missing " + source.string());
    fs::copy_file(source, target, fs::copy_options::overwrite_existing, ec);
    if (ec) return core::make_error(core::ErrorCode::IoError,
        "could not stage " + source.string() + ": " + ec.message());
    return {};
}

class Cleanup final {
public:
    explicit Cleanup(fs::path path) : path_(std::move(path)) {}
    ~Cleanup() { if (active_) { std::error_code ec; fs::remove_all(path_, ec); } }
    void release() noexcept { active_ = false; }
private:
    fs::path path_;
    bool active_{true};
};

} // namespace

core::Result<void> run_legacy_patch(const LegacyPatchRequest& request,
                                    const LineCallback& on_stdout_line,
                                    const LineCallback& on_stderr_line) {
    if (request.executable.empty() || request.project_dir.empty() ||
        request.extracted_dir.empty() || request.output_dir.empty()) {
        return core::make_error(core::ErrorCode::InvalidArgument,
                                "legacy patch paths must not be empty");
    }
    std::error_code ec;
    if (!fs::is_directory(request.project_dir, ec) || !fs::is_directory(request.extracted_dir, ec)) {
        return core::make_error(core::ErrorCode::NotFound,
                                "legacy project or extracted ROM directory does not exist");
    }
    if (fs::exists(request.output_dir, ec)) return core::make_error(
        core::ErrorCode::InvalidArgument, "legacy output directory already exists");

    const auto workspace = request.project_dir / "__tmp";
    if (fs::exists(workspace, ec)) return core::make_error(
        core::ErrorCode::InvalidArgument,
        "the legacy project already contains __tmp; remove or rename it before building");

    fs::create_directories(workspace / "root", ec);
    if (!ec) fs::create_directories(workspace / "overlay9", ec);
    if (!ec) fs::create_directories(workspace / "overlay7", ec);
    if (ec) return core::make_error(core::ErrorCode::IoError,
        "could not create the legacy __tmp workspace: " + ec.message());
    Cleanup workspace_cleanup(workspace);

    for (const auto* name : std::array{"arm9.bin", "arm9ovt.bin", "arm7.bin", "arm7ovt.bin", "header.bin", "fnt.bin"}) {
        if (auto copied = copy_file_required(request.extracted_dir / name, workspace / name); !copied)
            return copied.error();
    }
    for (const auto* name : std::array{"overlay9", "overlay7"}) {
        const auto source = request.extracted_dir / name;
        if (fs::is_directory(source, ec)) {
            if (auto copied = copy_tree(source, workspace / name); !copied) return copied.error();
        }
    }

    ProcessRequest process;
    process.executable = request.executable;
    process.working_directory = request.project_dir;
    if (request.backend == LegacyPatchBackend::NsmbeWrapper) {
        process.arguments = {"--project", request.project_dir.string(),
                             "--filesystem", workspace.string()};
    }
    auto ran = run_process(process, on_stdout_line, on_stderr_line);
    if (!ran) return ran.error();
    if (ran.value().exit_code != 0) return core::make_error(
        core::ErrorCode::ExternalToolFailed,
        "legacy patcher exited with status " + std::to_string(ran.value().exit_code));

    if (auto copied = copy_tree(request.extracted_dir, request.output_dir); !copied)
        return copied.error();
    Cleanup output_cleanup(request.output_dir);
    for (const auto* name : std::array{"arm9.bin", "arm9ovt.bin", "arm7.bin", "arm7ovt.bin"}) {
        if (auto copied = copy_file_required(workspace / name, request.output_dir / name); !copied)
            return copied.error();
    }
    for (const auto* name : std::array{"overlay9", "overlay7"}) {
        if (auto copied = copy_tree(workspace / name, request.output_dir / name); !copied)
            return copied.error();
    }
    output_cleanup.release();
    return {};
}

} // namespace nsmb::patching
