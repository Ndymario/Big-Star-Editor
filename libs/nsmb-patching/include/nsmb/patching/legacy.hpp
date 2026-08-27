#pragma once

#include <filesystem>

#include "nsmb/core/result.hpp"
#include "nsmb/patching/process.hpp"

namespace nsmb::patching {

enum class LegacyPatchBackend {
    Fireflower,
    NsmbeWrapper,
};

// Runs an older code project against an isolated copy of the extracted ROM.
// Fireflower retains its historical contract: no arguments, project cwd, and
// a temporary `__tmp` directory. The NSMBe option intentionally delegates to
// an external clean-room wrapper instead of embedding GPL editor code; it is
// called with `--project <dir> --filesystem <dir>/__tmp`.
struct LegacyPatchRequest {
    LegacyPatchBackend backend{LegacyPatchBackend::Fireflower};
    std::filesystem::path executable;
    std::filesystem::path project_dir;
    std::filesystem::path extracted_dir;
    std::filesystem::path output_dir;
};

core::Result<void> run_legacy_patch(const LegacyPatchRequest& request,
                                    const LineCallback& on_stdout_line = {},
                                    const LineCallback& on_stderr_line = {});

} // namespace nsmb::patching
