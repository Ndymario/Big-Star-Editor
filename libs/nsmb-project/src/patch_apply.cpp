#include "nsmb/project/patch_apply.hpp"

#include "nsmb/project/manifest.hpp"

#include <system_error>

#include "nsmb/core/file_io.hpp"

namespace nsmb::project {

namespace {

using core::ErrorCode;
using core::make_error;
using core::Result;

// Removing a staging directory is best effort: failing to clean up is not a
// reason to refuse a build that otherwise worked.
void remove_quietly(const std::filesystem::path& path) {
    std::error_code ec;
    std::filesystem::remove_all(path, ec);
}

// std::filesystem::rename fails across devices, which a project on one drive
// and a temp area on another will hit. Falling back to copy-then-remove keeps
// the move working without giving up the atomic path where it exists.
Result<void> move_file(const std::filesystem::path& from, const std::filesystem::path& to) {
    std::error_code ec;
    std::filesystem::remove(to, ec);
    std::filesystem::rename(from, to, ec);
    if (!ec) {
        return {};
    }

    std::filesystem::copy_file(from, to, std::filesystem::copy_options::overwrite_existing, ec);
    if (ec) {
        return make_error(ErrorCode::IoError,
                          "could not move " + from.string() + " to " + to.string() + ": " + ec.message());
    }
    std::filesystem::remove(from, ec);
    return {};
}

} // namespace

std::filesystem::path patch_staging_dir(const std::filesystem::path& project_dir) {
    return project_dir / ".big-star-staging";
}

std::filesystem::path patch_backup_root(const std::filesystem::path& project_dir) {
    return project_dir / ".big-star-backups";
}

std::filesystem::path built_rom_path(const std::filesystem::path& project_dir) {
    return project_dir / built_rom_file_name;
}

Result<PatchApplyResult> apply_patch_transactionally(const PatchApplyRequest& request,
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
    const auto staged_rom = staging / "patched.nds";

    patching::BuildRequest build;
    build.project_dir = request.project_dir;
    build.variant = request.variant;
    build.output_override = staged_rom;
    build.verbose = request.verbose;
    build.cancel = request.cancel;

    patching::NCPatcherClient client(request.ncpatcher_executable);
    auto built = client.build(build, observer);
    if (!built) {
        remove_quietly(staging);
        return built.error();
    }

    result.summary = built.value();
    if (!result.summary.succeeded()) {
        // NCPatcher said no. The project has not been touched, and the staging
        // directory holds nothing worth keeping.
        result.outcome = PatchApplyOutcome::BuildFailed;
        remove_quietly(staging);
        return result;
    }

    // From here the build claims to have worked. What is left is checking that
    // what it produced can actually be used, before the ROM the editor reads is
    // replaced with it.
    auto reject = [&](std::string why) {
        result.outcome = PatchApplyOutcome::OutputRejected;
        result.rejection = std::move(why);
        remove_quietly(staging);
        return result;
    };

    if (!std::filesystem::is_regular_file(staged_rom, ec)) {
        return reject("NCPatcher reported success but wrote no ROM at " + staged_rom.string());
    }
    auto header_bytes = core::read_file_prefix(staged_rom, nds::ds_cartridge_header_size);
    if (!header_bytes) {
        return reject("the built ROM could not be read: " + header_bytes.error().message);
    }
    if (auto header = nds::parse_rom_header(header_bytes.value()); !header) {
        return reject("the built ROM does not parse as an NDS ROM: " + header.error().message);
    }

    // The ROM is sound, so it becomes the one the editor reads. This is the
    // whole of the apply now: one file arriving, nothing unpacked, nothing
    // moved aside. Losing the previous build is not a loss -- NCPatcher rebuilds
    // it from the pristine binaries it keeps, which is what makes rebuilding
    // idempotent in the first place.
    const auto final_rom = built_rom_path(request.project_dir);
    if (auto moved = move_file(staged_rom, final_rom); !moved) {
        return reject("the built ROM was fine, but could not be moved into place: " + moved.error().message);
    }

    remove_quietly(staging);
    result.built_rom = final_rom;
    result.outcome = PatchApplyOutcome::Applied;
    return result;
}

} // namespace nsmb::project
