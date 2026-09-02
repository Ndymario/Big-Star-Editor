#pragma once

#include <filesystem>
#include <string>

#include "nsmb/core/result.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/ncpatcher.hpp"

namespace nsmb::project {

// Applying an NCPatcher build to a project without ever leaving it broken.
//
// The project directory is also the NCPatcher project: it holds ncpatcher.yaml,
// the pristine input ROM, and the modules. The editor reads the built `.nds`
// directly, so applying a build is one file arriving -- there is no derived
// tree to unpack, back up, or swap.
//
// A build is still not applied in place. It runs with `--out` to a staging
// path, the ROM it produced is validated there, and only a ROM that parses is
// moved in. A failed build, or one that writes something unreadable, leaves the
// project exactly as it was.
struct PatchApplyRequest {
    // The project directory, which is also the NCPatcher project directory.
    std::filesystem::path project_dir;

    // Which configured build variant to produce. Empty means the project
    // configures none.
    std::string variant;

    std::filesystem::path ncpatcher_executable{"ncpatcher"};
    bool verbose{false};

    // Optional. Passed straight to the build; the apply itself is not
    // interruptible, and does not need to be -- it moves files that are
    // already written.
    patching::ProcessCancel* cancel{};
};

enum class PatchApplyOutcome {
    // NCPatcher reported a failure. Nothing was touched.
    BuildFailed,

    // The build succeeded but its output could not be validated. Nothing was
    // touched.
    OutputRejected,

    // The built ROM is in place and is what the editor now reads.
    Applied,
};

struct PatchApplyResult {
    PatchApplyOutcome outcome{PatchApplyOutcome::BuildFailed};
    patching::RunSummary summary;

    // The ROM the build produced, in its final place on success. This is both
    // what the editor reads and what there is to run in an emulator.
    std::filesystem::path built_rom;

    // Set when the outcome is OutputRejected, saying what about the output was
    // wrong. A build that succeeds and then produces something unreadable is a
    // different problem from a build that failed, and reads differently.
    std::string rejection;
};

// Runs the build and, if the ROM it produced checks out, moves it into place.
core::Result<PatchApplyResult> apply_patch_transactionally(
    const PatchApplyRequest& request,
    const patching::BuildObserver& observer = {});

// Where the staging directory and the backup area live, exposed so the frontend
// can mention them and so tests can assert nothing was left behind. Backups no
// longer hold anything a build put there -- what remains is level saves and
// patch imports.
std::filesystem::path patch_staging_dir(const std::filesystem::path& project_dir);
std::filesystem::path patch_backup_root(const std::filesystem::path& project_dir);

// The ROM a successful apply writes.
std::filesystem::path built_rom_path(const std::filesystem::path& project_dir);

// How many backups of each kind a sweep leaves behind.
inline constexpr std::size_t default_backups_kept = 10;

// Deletes all but the `keep` most recent backups of each kind, and answers how
// many it removed. A backup is named for what made it and then for when, and a
// timestamp that sorts lexically sorts chronologically, so the newest few of a
// kind are simply the largest names beginning with it.
//
// Per kind rather than overall: one afternoon of level saves must not sweep
// away the morning's patch import, which is the backup somebody would actually
// go looking for. Failing to remove one is not an error -- what is left behind
// is disk use, not a broken project -- so the count is the whole report.
std::size_t prune_patch_backups(const std::filesystem::path& project_dir,
                                std::size_t keep = default_backups_kept);

} // namespace nsmb::project
