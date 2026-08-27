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
// the pristine input ROM, and the modules. `extracted-rom/` is a *derived* view
// of the built ROM -- what the editor renders -- rather than the authoritative
// copy of anything.
//
// So a build is not applied in place. It runs with `--out` to a staging path,
// the result is validated and extracted there, and only once both have
// succeeded is the live tree backed up and replaced. A failed build, a ROM that
// will not parse, or an extraction that dies half way leaves the project
// exactly as it was.
struct PatchApplyRequest {
    // The project directory, which is also the NCPatcher project directory.
    std::filesystem::path project_dir;

    // The live extracted tree the editor renders, replaced on success.
    std::filesystem::path extracted_dir;

    // Which configured build variant to produce. Empty means the project
    // configures none.
    std::string variant;

    std::filesystem::path ncpatcher_executable{"ncpatcher"};
    bool verbose{false};
};

enum class PatchApplyOutcome {
    // NCPatcher reported a failure. Nothing was touched.
    BuildFailed,

    // The build succeeded but its output could not be validated or extracted.
    // Nothing was touched.
    OutputRejected,

    // The live tree was backed up and replaced.
    Applied,
};

struct PatchApplyResult {
    PatchApplyOutcome outcome{PatchApplyOutcome::BuildFailed};
    patching::RunSummary summary;

    // Where the previous extracted tree was moved. Empty unless applied.
    std::filesystem::path backup_dir;

    // The staged ROM the build produced. Kept on success so it can be run in an
    // emulator without rebuilding.
    std::filesystem::path built_rom;

    // Set when the outcome is OutputRejected, saying what about the output was
    // wrong. A build that succeeds and then produces something unreadable is a
    // different problem from a build that failed, and reads differently.
    std::string rejection;
};

// Runs the build and, if everything checks out, swaps the tree.
//
// `extraction` is the service used to unpack the staged ROM -- normally
// `nds::NitroRomService`. Passing it in keeps this function testable against a
// synthetic ROM without an NCPatcher on the machine.
core::Result<PatchApplyResult> apply_patch_transactionally(
    const PatchApplyRequest& request,
    nds::RomExtractionService& extraction,
    const patching::BuildObserver& observer = {});

// Where the staging and backup directories live, exposed so the frontend can
// mention them and so tests can assert nothing was left behind.
std::filesystem::path patch_staging_dir(const std::filesystem::path& project_dir);
std::filesystem::path patch_backup_root(const std::filesystem::path& project_dir);

} // namespace nsmb::project
