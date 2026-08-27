#include "nsmb/project/patch_apply.hpp"

#include <chrono>
#include <sstream>
#include <system_error>

#include "nsmb/core/file_io.hpp"

namespace nsmb::project {

namespace {

using core::ErrorCode;
using core::make_error;
using core::Result;

std::string timestamp_suffix() {
    const auto now = std::chrono::system_clock::now();
    const auto seconds =
        std::chrono::duration_cast<std::chrono::seconds>(now.time_since_epoch()).count();
    return std::to_string(seconds);
}

// Removing a staging directory is best effort: failing to clean up is not a
// reason to refuse a build that otherwise worked.
void remove_quietly(const std::filesystem::path& path) {
    std::error_code ec;
    std::filesystem::remove_all(path, ec);
}

// std::filesystem::rename fails across devices, which a project on one drive
// and a temp area on another will hit. Falling back to copy-then-remove keeps
// the swap working without giving up the atomic path where it exists.
Result<void> move_directory(const std::filesystem::path& from, const std::filesystem::path& to) {
    std::error_code ec;
    std::filesystem::rename(from, to, ec);
    if (!ec) {
        return {};
    }

    std::filesystem::copy(from, to,
                          std::filesystem::copy_options::recursive |
                              std::filesystem::copy_options::copy_symlinks,
                          ec);
    if (ec) {
        return make_error(ErrorCode::IoError,
                          "could not move " + from.string() + " to " + to.string() + ": " +
                              ec.message());
    }
    std::filesystem::remove_all(from, ec);
    return {};
}

} // namespace

std::filesystem::path patch_staging_dir(const std::filesystem::path& project_dir) {
    return project_dir / ".big-star-staging";
}

std::filesystem::path patch_backup_root(const std::filesystem::path& project_dir) {
    return project_dir / ".big-star-backups";
}

Result<PatchApplyResult> apply_patch_transactionally(const PatchApplyRequest& request,
                                                     nds::RomExtractionService& extraction,
                                                     const patching::BuildObserver& observer) {
    std::error_code ec;
    if (!std::filesystem::is_directory(request.project_dir, ec)) {
        return make_error(ErrorCode::NotFound,
                          "project directory does not exist: " + request.project_dir.string());
    }

    const auto config = patching::detect_project_config(request.project_dir);
    if (config.kind == patching::ProjectConfigKind::None) {
        return make_error(ErrorCode::NotFound,
                          "no ncpatcher.yaml in " + request.project_dir.string());
    }
    if (config.kind == patching::ProjectConfigKind::V1Json) {
        return make_error(ErrorCode::Unsupported,
                          "this project still uses the legacy ncpatcher.json; convert it with "
                          "NCPatcher's migrate step first");
    }

    const auto staging = patch_staging_dir(request.project_dir);
    // A staging directory left behind by an interrupted run is stale by
    // definition, and reusing it would apply a mix of two builds.
    remove_quietly(staging);
    std::filesystem::create_directories(staging, ec);
    if (ec) {
        return make_error(ErrorCode::IoError,
                          "could not create the staging directory " + staging.string());
    }

    PatchApplyResult result;
    result.built_rom = staging / "patched.nds";

    patching::BuildRequest build;
    build.project_dir = request.project_dir;
    build.variant = request.variant;
    build.output_override = result.built_rom;
    build.verbose = request.verbose;

    patching::NCPatcherClient client(request.ncpatcher_executable);
    auto built = client.build(build, observer);
    if (!built) {
        remove_quietly(staging);
        return built.error();
    }

    result.summary = built.value();
    if (!result.summary.succeeded()) {
        // NCPatcher said no. The live tree has not been touched, and the
        // staging directory holds nothing worth keeping.
        result.outcome = PatchApplyOutcome::BuildFailed;
        remove_quietly(staging);
        result.built_rom.clear();
        return result;
    }

    // From here the build claims to have worked. Everything below is checking
    // whether what it produced can actually be used, before anything the editor
    // depends on is disturbed.
    if (!std::filesystem::is_regular_file(result.built_rom, ec)) {
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = "NCPatcher reported success but wrote no ROM at " +
                           result.built_rom.string();
        remove_quietly(staging);
        result.built_rom.clear();
        return result;
    }

    auto header_bytes = core::read_file_prefix(result.built_rom, 0x200);
    if (!header_bytes) {
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = "the built ROM could not be read: " + header_bytes.error().message;
        remove_quietly(staging);
        result.built_rom.clear();
        return result;
    }
    auto header = nds::parse_rom_header(header_bytes.value());
    if (!header) {
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = "the built ROM does not parse as an NDS ROM: " + header.error().message;
        remove_quietly(staging);
        result.built_rom.clear();
        return result;
    }

    const auto staged_tree = staging / "extracted";
    auto extracted = extraction.extract(result.built_rom, staged_tree);
    if (!extracted) {
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = "the built ROM could not be extracted: " + extracted.error().message;
        remove_quietly(staging);
        result.built_rom.clear();
        return result;
    }

    // The new tree is complete and on disk. Only now is the old one moved, and
    // it is moved rather than deleted so a bad build is recoverable by hand.
    std::filesystem::create_directories(patch_backup_root(request.project_dir), ec);
    const auto backup =
        patch_backup_root(request.project_dir) /
        (request.extracted_dir.filename().string() + "-" + timestamp_suffix());

    if (std::filesystem::exists(request.extracted_dir, ec)) {
        if (auto moved = move_directory(request.extracted_dir, backup); !moved) {
            result.outcome = PatchApplyOutcome::OutputRejected;
            result.rejection =
                "the built ROM was fine, but the existing extracted tree could not be moved "
                "aside: " +
                moved.error().message;
            remove_quietly(staging);
            result.built_rom.clear();
            return result;
        }
        result.backup_dir = backup;
    }

    if (auto moved = move_directory(staged_tree, request.extracted_dir); !moved) {
        // The old tree is already aside. Put it back rather than leaving the
        // project with no extracted ROM at all.
        if (!result.backup_dir.empty()) {
            (void)move_directory(result.backup_dir, request.extracted_dir);
            result.backup_dir.clear();
        }
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = "the new extracted tree could not be moved into place: " +
                           moved.error().message;
        remove_quietly(staging);
        result.built_rom.clear();
        return result;
    }

    // The staged ROM itself is kept: it is the thing to run in an emulator, and
    // rebuilding just to get it back would be wasteful.
    const auto kept_rom = request.project_dir / "big-star-build.nds";
    std::filesystem::remove(kept_rom, ec);
    std::filesystem::rename(result.built_rom, kept_rom, ec);
    if (!ec) {
        result.built_rom = kept_rom;
    }

    remove_quietly(staging);
    result.outcome = PatchApplyOutcome::Applied;
    return result;
}

} // namespace nsmb::project
