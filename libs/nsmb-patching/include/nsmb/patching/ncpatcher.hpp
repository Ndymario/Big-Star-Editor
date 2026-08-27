#pragma once

#include <cstdint>
#include <filesystem>
#include <functional>
#include <optional>
#include <string>
#include <utility>
#include <vector>

#include "nsmb/core/json.hpp"
#include "nsmb/core/logging.hpp"
#include "nsmb/core/result.hpp"
#include "nsmb/patching/process.hpp"

namespace nsmb::patching {

// The editor's side of the NCPatcher 2.x contract.
//
// NCPatcher stays an external program: it is not vendored, not linked and not
// reimplemented. Everything here drives its documented command line and reads
// its documented machine-readable documents, so a newer NCPatcher that keeps
// those promises keeps working.
//
// Three rules run through this file:
//
//   * `-C <project>` names the project, so nothing ever depends on the
//     process-wide working directory.
//   * Every document carries a `schema` string, and an unrecognised one is
//     refused outright rather than read as far as it happens to parse. A
//     document written by a newer build that is half-understood ends in a ROM
//     patched wrongly, which is discovered much later than a refusal is.
//   * Nothing here shells out. Arguments reach the child as a vector.

// NCPatcher's documented exit codes. The editor reports which stage failed, so
// a configuration mistake does not read like a compiler crash.
enum class NCPatcherStage {
    Ok = 0,
    Internal = 1,
    CommandLine = 2,
    Configuration = 3,
    ModuleResolution = 4,
    Toolchain = 5,
    Compilation = 6,
    Linking = 7,
    Patching = 8,
    RomIo = 9,
    Hook = 10,
    Unknown = -1,
};

NCPatcherStage stage_from_exit_code(int exit_code);
std::string_view describe_stage(NCPatcherStage stage);

// Which configuration file a directory holds. A v1 `ncpatcher.json` is
// readable by NCPatcher but the editor refuses to build from it directly and
// offers `migrate --write` instead, so a project is converted deliberately and
// with a verified result rather than as a side effect of pressing Build.
enum class ProjectConfigKind { None, V2Yaml, V1Json };

struct ProjectConfigLocation {
    ProjectConfigKind kind{ProjectConfigKind::None};
    std::filesystem::path file;
};

ProjectConfigLocation detect_project_config(const std::filesystem::path& project_dir);

// ---------------------------------------------------------------------------
// ncpatcher.result/1 -- diagnostics and artifacts from one run
// ---------------------------------------------------------------------------

enum class DiagnosticLevel { Info, Warning, Error };

struct DiagnosticLocation {
    bool known{false};
    std::filesystem::path file;
    int line{};
    int col{};
    std::string node_path; // e.g. "targets.arm9.regions[12].maxsize"
};

struct Diagnostic {
    DiagnosticLevel level{DiagnosticLevel::Error};
    std::string code; // e.g. "NCP0001"
    std::string message;
    DiagnosticLocation location;
};

struct Artifact {
    std::string kind;   // "overlay", "arm9", ...
    std::string proc;   // "arm9" / "arm7"
    std::string action; // "modified" / "created" / "unchanged"
    std::string name;
    std::string ram_address;
    std::optional<std::int64_t> id;
    std::optional<std::int64_t> size;
    std::optional<std::int64_t> file_id;
};

struct RunSummary {
    std::string status; // "ok" / "error"
    int exit_code{};
    NCPatcherStage stage{NCPatcherStage::Ok};
    std::int64_t duration_ms{};
    int errors{};
    int warnings{};
    std::vector<Diagnostic> diagnostics;
    std::vector<Artifact> artifacts;

    [[nodiscard]] bool succeeded() const { return exit_code == 0 && errors == 0; }
};

struct BuildProgress {
    std::string phase; // "compile", "link", ...
    std::int64_t current{};
    std::int64_t total{};
    std::string item;
};

// Callbacks fire while the build runs, from the process reader thread and never
// concurrently with each other. A frontend that touches its UI from them must
// marshal to its own thread; the Qt shell queues them.
struct BuildObserver {
    std::function<void(const BuildProgress&)> on_progress;
    std::function<void(const Artifact&)> on_artifact;
    std::function<void(const Diagnostic&)> on_diagnostic;

    // NCPatcher's human-readable log, which in JSON message mode is on stderr.
    std::function<void(std::string_view)> on_log;
};

// ---------------------------------------------------------------------------
// ncpatcher.config/1 -- what the project is configured to do
// ---------------------------------------------------------------------------

struct BuildVariant {
    std::string name;
    std::vector<std::string> defines;
    std::vector<std::pair<std::string, std::filesystem::path>> files;
    std::filesystem::path banner;
};

struct ProjectConfig {
    int version{};
    std::filesystem::path config_file;
    std::filesystem::path project_root;
    std::filesystem::path rom_dir;
    std::filesystem::path rom_file;
    std::filesystem::path rom_output;
    std::filesystem::path rom_banner;
    std::filesystem::path backup_dir;
    std::string toolchain;
    std::vector<std::pair<std::string, std::filesystem::path>> files;

    // In declaration order, which is the order the project wrote them and the
    // order a variant picker must offer. Empty means the project configures no
    // variants -- a different thing from the query having failed.
    std::vector<BuildVariant> variants;

    [[nodiscard]] bool has_variant(std::string_view name) const;
};

// ---------------------------------------------------------------------------
// ncpatcher.modules/1 -- the resolved module graph
// ---------------------------------------------------------------------------

struct ModuleNitroFs {
    std::filesystem::path dir;

    // When layered, the first path segment under `dir` is a variant name rather
    // than part of the ROM path.
    bool layered{false};

    // Applied underneath the built variant, so the other layers only carry what
    // they actually override.
    std::string base_variant;

    // A ROM path prefix for everything this tree produces.
    std::string into;
};

struct ModuleComponent {
    std::string name;
    bool enabled{true};
    std::string disabled_because;

    // Glob patterns naming the ROM destinations this component owns. While the
    // component is enabled they record ownership; while it is disabled they are
    // subtracted from the module's own tree.
    std::vector<std::string> files;

    // Component keys NCPatcher does not define, verbatim. nsmb-coop-module's
    // `objects:` rides here.
    core::JsonValue extra;
};

struct ModuleInfo {
    std::string key;  // the directory name, and the name `modules.enabled` uses
    std::string id;   // the module's own id, which generated code names it by
    std::string name; // human-readable title
    bool enabled{false};
    std::string description;
    std::filesystem::path dir;
    std::filesystem::path file;
    std::optional<ModuleNitroFs> nitrofs;
    std::vector<ModuleComponent> components;

    // Module root keys NCPatcher does not define. The glue module's
    // `level-data:` rides here.
    core::JsonValue extra;
};

struct ModuleGraph {
    std::filesystem::path dir;

    // In `modules.enabled` order, which is also the order that settles two
    // modules claiming one NitroFS destination.
    std::vector<ModuleInfo> modules;

    [[nodiscard]] const ModuleInfo* find(std::string_view key) const;
};

// ---------------------------------------------------------------------------
// ncpatcher.files/1 -- the ROM's NitroFS table with provenance
// ---------------------------------------------------------------------------

enum class FileAction { Unchanged, Modified, Created };

struct RomFileEntry {
    // The FAT index exactly as the ROM stores it. NSMB subtracts its overlay
    // count at run time; that is a property of the game, and neither NCPatcher
    // nor this layer applies it.
    std::int64_t id{};
    std::string path; // slash-separated, no leading slash
    std::int64_t size{};
    FileAction action{FileAction::Unchanged};

    // Present only for files the run wrote.
    std::string source;
    std::string module;
    std::string component;
    std::string from_variant;

    [[nodiscard]] bool has_provenance() const { return !module.empty() || !source.empty(); }
};

struct FileManifest {
    // The variant this build produced. Empty when the run had no variant, which
    // includes `rom files --json` reading a ROM off disk.
    std::string variant;
    std::vector<RomFileEntry> files;

    [[nodiscard]] const RomFileEntry* find(std::string_view rom_path) const;
};

// ---------------------------------------------------------------------------
// Document readers, exposed so they can be tested without running anything
// ---------------------------------------------------------------------------

core::Result<RunSummary> parse_run_summary(std::string_view json);
core::Result<ProjectConfig> parse_project_config(std::string_view json);
core::Result<ModuleGraph> parse_module_graph(std::string_view json);
core::Result<FileManifest> parse_file_manifest(std::string_view json);

// ---------------------------------------------------------------------------
// The client
// ---------------------------------------------------------------------------

struct BuildRequest {
    std::filesystem::path project_dir;

    // Empty means the project configures no variants. When it does configure
    // some, NCPatcher requires an explicit selection so a build cannot silently
    // produce the wrong language.
    std::string variant;

    // Patch a ROM other than the project's own, and write the result somewhere
    // other than in place. Both are what makes a transactional apply possible:
    // stage a copy, patch that, validate it, and only then replace the live
    // tree.
    std::filesystem::path rom_override;
    std::filesystem::path output_override;

    bool verbose{false};
};

class NCPatcherClient {
public:
    explicit NCPatcherClient(std::filesystem::path executable = "ncpatcher",
                             core::Logger& logger = core::NullLogger::instance());

    [[nodiscard]] const std::filesystem::path& executable() const { return executable_; }

    // Preflight. Answers whether the configured executable exists and runs.
    core::Result<std::string> version();

    core::Result<ProjectConfig> dump_config(const std::filesystem::path& project_dir);
    core::Result<RunSummary> validate_config(const std::filesystem::path& project_dir);
    core::Result<ModuleGraph> dump_modules(const std::filesystem::path& project_dir);

    // Reads a ROM's file table. Without provenance -- it reads a result rather
    // than performing a build, so every entry is `unchanged`. `rom_override`
    // points at a ROM or an extracted directory other than the project's.
    core::Result<FileManifest> rom_files(const std::filesystem::path& project_dir,
                                         const std::filesystem::path& rom_override = {});

    // Reads a manifest a build already wrote (`files-dump`). This is the one
    // with provenance, and the one the filesystem view needs.
    core::Result<FileManifest> read_file_manifest(const std::filesystem::path& json_file);

    core::Result<RunSummary> build(const BuildRequest& request, const BuildObserver& observer = {});

    // Converts a legacy `ncpatcher.json`. Without `write` this is a dry run that
    // verifies the resolved build is unchanged.
    core::Result<RunSummary> migrate(const std::filesystem::path& project_dir, bool write);

private:
    struct CapturedRun {
        int exit_code{};
        std::string stdout_text;
        std::string stderr_text;
    };

    core::Result<CapturedRun> capture(const std::filesystem::path& project_dir,
                                      std::vector<std::string> arguments);

    std::filesystem::path executable_;
    core::Logger* logger_;
};

} // namespace nsmb::patching
