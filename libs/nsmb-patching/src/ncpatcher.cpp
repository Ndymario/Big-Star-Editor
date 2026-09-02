#include "nsmb/patching/ncpatcher.hpp"

#include <algorithm>
#include <system_error>
#include <utility>

#include "nsmb/core/file_io.hpp"

namespace nsmb::patching {

namespace {

using core::ErrorCode;
using core::JsonValue;
using core::make_error;
using core::Result;

// Every document NCPatcher and nsmbtool write names its own format and
// version. Reading one whose name we do not know -- because it is a different
// document, or a newer revision of this one -- is guessing, and the guess
// surfaces as a wrongly patched ROM long after the fact.
Result<void> require_schema(const JsonValue& root, std::string_view expected,
                            std::string_view what) {
    if (!root.is_object()) {
        return make_error(ErrorCode::ParseError,
                          std::string(what) + ": expected a JSON object at the top level");
    }
    const JsonValue& schema = root["schema"];
    if (!schema.is_string()) {
        return make_error(ErrorCode::ParseError,
                          std::string(what) + ": no \"schema\" field, so its format is unknown");
    }
    if (schema.as_string() != expected) {
        return make_error(ErrorCode::Unsupported,
                          std::string(what) + ": unsupported schema \"" + schema.as_string() +
                              "\", this build understands \"" + std::string(expected) +
                              "\". Update Big Star Editor, or use a matching NCPatcher.");
    }
    return {};
}

Result<JsonValue> parse_document(std::string_view json, std::string_view what) {
    auto parsed = core::parse_json(json);
    if (!parsed) {
        return make_error(ErrorCode::ParseError,
                          std::string(what) + ": " + parsed.error().message);
    }
    return parsed;
}

DiagnosticLevel diagnostic_level_from(const std::string& text) {
    if (text == "warning") {
        return DiagnosticLevel::Warning;
    }
    if (text == "info" || text == "note") {
        return DiagnosticLevel::Info;
    }
    return DiagnosticLevel::Error;
}

FileAction file_action_from(const std::string& text) {
    if (text == "modified") {
        return FileAction::Modified;
    }
    if (text == "created") {
        return FileAction::Created;
    }
    return FileAction::Unchanged;
}

std::optional<std::int64_t> optional_int(const JsonValue& value) {
    if (!value.is_number()) {
        return std::nullopt;
    }
    return value.as_int64();
}

Diagnostic read_diagnostic(const JsonValue& node) {
    Diagnostic diagnostic;
    diagnostic.level = diagnostic_level_from(node["level"].as_string());
    diagnostic.code = node["code"].as_string();
    diagnostic.message = node["message"].as_string();

    const JsonValue& location = node["location"];
    if (location.is_object()) {
        diagnostic.location.known = true;
        diagnostic.location.file = location["file"].as_string();
        diagnostic.location.line = static_cast<int>(location["line"].as_int64());
        diagnostic.location.col = static_cast<int>(location["col"].as_int64());
        diagnostic.location.node_path = location["path"].as_string();
    }
    return diagnostic;
}

Artifact read_artifact(const JsonValue& node) {
    Artifact artifact;
    artifact.kind = node["kind"].as_string();
    artifact.proc = node["proc"].as_string();
    artifact.action = node["action"].as_string();
    artifact.name = node["name"].as_string();
    artifact.ram_address = node["ram-address"].as_string();
    artifact.id = optional_int(node["id"]);
    artifact.size = optional_int(node["size"]);
    artifact.file_id = optional_int(node["file-id"]);
    return artifact;
}

BuildProgress read_progress(const JsonValue& node) {
    BuildProgress progress;
    progress.phase = node["phase"].as_string();
    progress.current = node["current"].as_int64();
    progress.total = node["total"].as_int64();
    progress.item = node["item"].as_string();
    return progress;
}

std::vector<std::string> read_string_list(const JsonValue& node) {
    std::vector<std::string> out;
    if (!node.is_array()) {
        return out;
    }
    out.reserve(node.size());
    for (const auto& element : node.elements()) {
        if (element.is_string()) {
            out.push_back(element.as_string());
        }
    }
    return out;
}

} // namespace

NCPatcherStage stage_from_exit_code(int exit_code) {
    switch (exit_code) {
    case 0: return NCPatcherStage::Ok;
    case 1: return NCPatcherStage::Internal;
    case 2: return NCPatcherStage::CommandLine;
    case 3: return NCPatcherStage::Configuration;
    case 4: return NCPatcherStage::ModuleResolution;
    case 5: return NCPatcherStage::Toolchain;
    case 6: return NCPatcherStage::Compilation;
    case 7: return NCPatcherStage::Linking;
    case 8: return NCPatcherStage::Patching;
    case 9: return NCPatcherStage::RomIo;
    case 10: return NCPatcherStage::Hook;
    case 11: return NCPatcherStage::Cancelled;
    default: return NCPatcherStage::Unknown;
    }
}

std::string_view describe_stage(NCPatcherStage stage) {
    switch (stage) {
    case NCPatcherStage::Ok: return "succeeded";
    case NCPatcherStage::Internal: return "NCPatcher hit an internal error";
    case NCPatcherStage::CommandLine: return "NCPatcher rejected the command line";
    case NCPatcherStage::Configuration: return "the project configuration is invalid";
    case NCPatcherStage::ModuleResolution: return "the modules could not be resolved";
    case NCPatcherStage::Toolchain: return "the toolchain or SDK header is missing or mismatched";
    case NCPatcherStage::Compilation: return "compilation failed";
    case NCPatcherStage::Linking: return "linking failed";
    case NCPatcherStage::Patching: return "patching the binaries failed";
    case NCPatcherStage::RomIo: return "the ROM could not be read or written";
    case NCPatcherStage::Hook: return "a build hook failed";
    case NCPatcherStage::Cancelled: return "the build was cancelled";
    case NCPatcherStage::Unknown: break;
    }
    return "NCPatcher failed for an unrecognised reason";
}

ProjectConfigLocation detect_project_config(const std::filesystem::path& project_dir) {
    std::error_code ec;
    for (const char* name : {"ncpatcher.yaml", "ncpatcher.yml"}) {
        const auto candidate = project_dir / name;
        if (std::filesystem::is_regular_file(candidate, ec)) {
            return {ProjectConfigKind::V2Yaml, candidate};
        }
    }
    const auto legacy = project_dir / "ncpatcher.json";
    if (std::filesystem::is_regular_file(legacy, ec)) {
        return {ProjectConfigKind::V1Json, legacy};
    }
    return {};
}

bool ProjectConfig::has_variant(std::string_view name) const {
    return std::any_of(variants.begin(), variants.end(),
                       [name](const BuildVariant& variant) { return variant.name == name; });
}

const ModuleInfo* ModuleGraph::find(std::string_view key) const {
    for (const auto& module : modules) {
        if (module.key == key) {
            return &module;
        }
    }
    return nullptr;
}

const RomFileEntry* FileManifest::find(std::string_view rom_path) const {
    for (const auto& entry : files) {
        if (entry.path == rom_path) {
            return &entry;
        }
    }
    return nullptr;
}

Result<RunSummary> parse_run_summary(std::string_view json) {
    auto document = parse_document(json, "NCPatcher run result");
    if (!document) {
        return document.error();
    }
    const JsonValue& root = document.value();
    if (auto ok = require_schema(root, "ncpatcher.result/1", "NCPatcher run result"); !ok) {
        return ok.error();
    }

    RunSummary summary;
    summary.status = root["status"].as_string();
    summary.exit_code = static_cast<int>(root["exit-code"].as_int64());
    summary.stage = stage_from_exit_code(summary.exit_code);
    summary.duration_ms = root["duration-ms"].as_int64();
    summary.errors = static_cast<int>(root["errors"].as_int64());
    summary.warnings = static_cast<int>(root["warnings"].as_int64());

    for (const auto& node : root["diagnostics"].elements()) {
        summary.diagnostics.push_back(read_diagnostic(node));
    }
    for (const auto& node : root["artifacts"].elements()) {
        summary.artifacts.push_back(read_artifact(node));
    }
    return summary;
}

Result<ProjectConfig> parse_project_config(std::string_view json) {
    auto document = parse_document(json, "NCPatcher project configuration");
    if (!document) {
        return document.error();
    }
    const JsonValue& root = document.value();
    if (auto ok = require_schema(root, "ncpatcher.config/1", "NCPatcher project configuration");
        !ok) {
        return ok.error();
    }

    ProjectConfig config;
    config.version = static_cast<int>(root["version"].as_int64());
    config.config_file = root["file"].as_string();
    config.project_root = root["project-root"].as_string();
    config.rom_dir = root["rom-dir"].as_string();
    config.rom_file = root["rom-file"].as_string();
    config.rom_output = root["rom-output"].as_string();
    config.rom_banner = root["rom-banner"].as_string();
    config.backup_dir = root["backup-dir"].as_string();
    config.toolchain = root["toolchain"].as_string();
    for (const auto& [path, source] : root["files"].members()) {
        config.files.emplace_back(path, source.as_string());
    }

    // Declaration order, straight from the document. The JSON reader preserves
    // member order precisely so this list is not silently alphabetised.
    for (const auto& [name, node] : root["variants"].members()) {
        BuildVariant variant;
        variant.name = name;
        variant.defines = read_string_list(node["defines"]);
        for (const auto& [path, source] : node["files"].members()) {
            variant.files.emplace_back(path, source.as_string());
        }
        variant.banner = node["banner"].as_string();
        for (const auto& [module, layer] : node["module-variants"].members()) {
            variant.module_variants.emplace_back(module, layer.as_string());
        }
        config.variants.push_back(std::move(variant));
    }
    return config;
}

Result<ModuleGraph> parse_module_graph(std::string_view json) {
    auto document = parse_document(json, "NCPatcher module graph");
    if (!document) {
        return document.error();
    }
    const JsonValue& root = document.value();
    if (auto ok = require_schema(root, "ncpatcher.modules/1", "NCPatcher module graph"); !ok) {
        return ok.error();
    }

    ModuleGraph graph;
    graph.dir = root["dir"].as_string();

    for (const auto& node : root["modules"].elements()) {
        ModuleInfo module;
        module.key = node["key"].as_string();
        module.id = node["id"].as_string();
        module.name = node["name"].as_string();
        module.enabled = node["enabled"].as_bool();
        module.description = node["description"].as_string();
        module.dir = node["dir"].as_string();
        module.file = node["file"].as_string();
        module.extra = node["extra"];

        // A disabled module ends at `enabled`: its module.yaml was never read,
        // so there is nothing further to report.
        const JsonValue& nitrofs = node["nitrofs"];
        if (nitrofs.is_object()) {
            ModuleNitroFs tree;
            tree.dir = nitrofs["dir"].as_string();
            tree.layered = nitrofs["layered"].as_bool();
            tree.base_variant = nitrofs["base-variant"].as_string();
            tree.into = nitrofs["into"].as_string();
            module.nitrofs = std::move(tree);
        }

        for (const auto& component_node : node["components"].elements()) {
            ModuleComponent component;
            component.name = component_node["name"].as_string();
            component.enabled = component_node["enabled"].as_bool();
            component.disabled_because = component_node["disabled-because"].as_string();
            component.files = read_string_list(component_node["files"]);
            component.extra = component_node["extra"];
            module.components.push_back(std::move(component));
        }

        graph.modules.push_back(std::move(module));
    }
    return graph;
}

Result<FileManifest> parse_file_manifest(std::string_view json) {
    auto document = parse_document(json, "NCPatcher file manifest");
    if (!document) {
        return document.error();
    }
    const JsonValue& root = document.value();
    if (auto ok = require_schema(root, "ncpatcher.files/1", "NCPatcher file manifest"); !ok) {
        return ok.error();
    }

    FileManifest manifest;
    manifest.variant = root["variant"].as_string();
    manifest.planned = root["planned"].as_bool();

    const JsonValue& files = root["files"];
    manifest.files.reserve(files.size());
    for (const auto& node : files.elements()) {
        RomFileEntry entry;
        entry.id = node["id"].as_int64();
        entry.path = node["path"].as_string();
        entry.size = node["size"].as_int64();
        entry.action = file_action_from(node["action"].as_string());
        entry.source = node["source"].as_string();
        entry.module = node["module"].as_string();
        entry.component = node["component"].as_string();
        entry.from_variant = node["from-variant"].as_string();
        entry.source_missing = node["source-missing"].as_bool();

        // Only an edited archive carries these, and only for the members the
        // run wrote. Everything else inside a container is read from the ROM.
        for (const auto& member_node : node["members"].elements()) {
            RomArchiveMember member;
            member.id = member_node["id"].as_int64();
            member.path = member_node["path"].as_string();
            member.size = member_node["size"].as_int64();
            member.action = file_action_from(member_node["action"].as_string());
            member.source = member_node["source"].as_string();
            member.module = member_node["module"].as_string();
            member.component = member_node["component"].as_string();
            member.from_variant = member_node["from-variant"].as_string();
            member.source_missing = member_node["source-missing"].as_bool();
            entry.members.push_back(std::move(member));
        }

        manifest.files.push_back(std::move(entry));
    }

    // `count` is the writer's own tally. A mismatch means the document was
    // truncated or concatenated, and reading it anyway would understate what
    // the ROM holds.
    const std::int64_t declared = root["count"].as_int64(-1);
    if (declared >= 0 && declared != static_cast<std::int64_t>(manifest.files.size())) {
        return make_error(ErrorCode::ParseError,
                          "NCPatcher file manifest declares " + std::to_string(declared) +
                              " files but carries " +
                              std::to_string(manifest.files.size()));
    }
    return manifest;
}

NCPatcherClient::NCPatcherClient(std::filesystem::path executable, core::Logger& logger)
    : executable_(std::move(executable)), logger_(&logger) {}

Result<NCPatcherClient::CapturedRun> NCPatcherClient::capture(
    const std::filesystem::path& project_dir, std::vector<std::string> arguments) {
    ProcessRequest request;
    request.executable = executable_;
    request.arguments = std::move(arguments);
    // The project is named, never entered. Commands whose product is stdout put
    // their log on stderr, so the two are kept apart here as well.
    request.working_directory = project_dir;

    CapturedRun run;
    const auto result = run_process(
        request,
        [&run](std::string_view line) {
            run.stdout_text.append(line);
            run.stdout_text.push_back('\n');
        },
        [&run](std::string_view line) {
            run.stderr_text.append(line);
            run.stderr_text.push_back('\n');
        });
    if (!result) {
        return result.error();
    }
    run.exit_code = result.value().exit_code;
    return run;
}

Result<std::string> NCPatcherClient::version() {
    auto run = capture({}, {"version"});
    if (!run) {
        return run.error();
    }
    if (run.value().exit_code != 0) {
        return make_error(ErrorCode::ExternalToolFailed,
                          "could not query the NCPatcher version: " + run.value().stderr_text);
    }
    std::string text = run.value().stdout_text;
    while (!text.empty() && (text.back() == '\n' || text.back() == '\r')) {
        text.pop_back();
    }
    return text;
}

Result<ProjectConfig> NCPatcherClient::dump_config(const std::filesystem::path& project_dir) {
    auto run = capture(project_dir, {"-C", project_dir.string(), "config", "dump", "--json"});
    if (!run) {
        return run.error();
    }
    if (run.value().exit_code != 0) {
        const auto stage = stage_from_exit_code(run.value().exit_code);
        return make_error(ErrorCode::ExternalToolFailed,
                          std::string(describe_stage(stage)) + ":\n" + run.value().stderr_text);
    }
    return parse_project_config(run.value().stdout_text);
}

Result<RunSummary> NCPatcherClient::validate_config(const std::filesystem::path& project_dir) {
    // The result file is where the structured diagnostics live; stdout in JSON
    // message mode carries the same events as they happen.
    std::error_code ec;
    const auto result_file =
        std::filesystem::temp_directory_path(ec) / "big-star-ncpatcher-validate.json";

    auto run = capture(project_dir, {"-C", project_dir.string(), "--message-format", "json",
                                     "--result", result_file.string(), "config", "validate"});
    if (!run) {
        return run.error();
    }

    auto text = core::read_file(result_file);
    std::filesystem::remove(result_file, ec);
    if (!text) {
        return make_error(ErrorCode::ExternalToolFailed,
                          "NCPatcher wrote no run result:\n" + run.value().stderr_text);
    }
    return parse_run_summary(
        std::string_view(reinterpret_cast<const char*>(text.value().data()), text.value().size()));
}

Result<ModuleGraph> NCPatcherClient::dump_modules(const std::filesystem::path& project_dir) {
    auto run = capture(project_dir, {"-C", project_dir.string(), "modules", "dump"});
    if (!run) {
        return run.error();
    }
    if (run.value().exit_code != 0) {
        const auto stage = stage_from_exit_code(run.value().exit_code);
        return make_error(ErrorCode::ExternalToolFailed,
                          std::string(describe_stage(stage)) + ":\n" + run.value().stderr_text);
    }
    return parse_module_graph(run.value().stdout_text);
}

Result<FileManifest> NCPatcherClient::rom_files(const std::filesystem::path& project_dir,
                                                const std::filesystem::path& rom_override) {
    std::vector<std::string> arguments{"-C", project_dir.string()};
    if (!rom_override.empty()) {
        arguments.push_back("--rom");
        arguments.push_back(rom_override.string());
    }
    arguments.insert(arguments.end(), {"rom", "files", "--json"});

    auto run = capture(project_dir, std::move(arguments));
    if (!run) {
        return run.error();
    }
    if (run.value().exit_code != 0) {
        const auto stage = stage_from_exit_code(run.value().exit_code);
        return make_error(ErrorCode::ExternalToolFailed,
                          std::string(describe_stage(stage)) + ":\n" + run.value().stderr_text);
    }
    return parse_file_manifest(run.value().stdout_text);
}

Result<RunSummary> NCPatcherClient::extract_rom(const std::filesystem::path& project_dir,
                                                const std::filesystem::path& output_dir,
                                                const std::filesystem::path& rom_override) {
    std::vector<std::string> arguments{"-C", project_dir.string()};
    if (!rom_override.empty()) {
        arguments.push_back("--rom");
        arguments.push_back(rom_override.string());
    }
    arguments.insert(arguments.end(), {"rom", "extract", output_dir.string()});

    auto run = capture(project_dir, std::move(arguments));
    if (!run) {
        return run.error();
    }
    RunSummary summary;
    summary.exit_code = run.value().exit_code;
    summary.stage = stage_from_exit_code(run.value().exit_code);
    summary.status = run.value().exit_code == 0 ? "ok" : "error";
    if (run.value().exit_code != 0) {
        summary.errors = 1;
        summary.diagnostics.push_back(Diagnostic{DiagnosticLevel::Error, {}, run.value().stderr_text, {}});
    }
    return summary;
}

Result<FileManifest> NCPatcherClient::plan_files(const std::filesystem::path& project_dir,
                                                 std::string_view variant) {
    std::vector<std::string> arguments{"-C", project_dir.string()};
    if (!variant.empty()) {
        arguments.push_back("--variant");
        arguments.emplace_back(variant);
    }
    arguments.insert(arguments.end(), {"files", "plan", "--json"});

    auto run = capture(project_dir, std::move(arguments));
    if (!run) {
        return run.error();
    }
    if (run.value().exit_code != 0) {
        const auto stage = stage_from_exit_code(run.value().exit_code);
        return make_error(ErrorCode::ExternalToolFailed,
                          std::string(describe_stage(stage)) + ":\n" + run.value().stderr_text);
    }

    auto manifest = parse_file_manifest(run.value().stdout_text);
    if (!manifest) {
        return manifest.error();
    }
    // A plan that does not say it is one would have its prospective z_new/ ids
    // read as ids the ROM actually holds, which is the one way this document
    // can mislead. An NCPatcher that answers `files plan` at all sets it.
    if (!manifest.value().planned) {
        return make_error(ErrorCode::ParseError,
                          "NCPatcher answered `files plan` with a document that is not marked "
                          "planned");
    }
    return manifest;
}

Result<FileManifest> NCPatcherClient::read_file_manifest(const std::filesystem::path& json_file) {
    auto text = core::read_file(json_file);
    if (!text) {
        return text.error();
    }
    return parse_file_manifest(
        std::string_view(reinterpret_cast<const char*>(text.value().data()), text.value().size()));
}

Result<RunSummary> NCPatcherClient::build(const BuildRequest& request,
                                          const BuildObserver& observer) {
    const auto config = detect_project_config(request.project_dir);
    if (config.kind == ProjectConfigKind::None) {
        return make_error(ErrorCode::NotFound,
                          "no ncpatcher.yaml in " + request.project_dir.string());
    }
    if (config.kind == ProjectConfigKind::V1Json) {
        // Building would work, and would leave a project half-migrated the first
        // time anything touched it. Converting is a decision, not a side effect.
        return make_error(ErrorCode::Unsupported,
                          "this project still uses the legacy ncpatcher.json. Run NCPatcher's "
                          "migrate step to convert it to ncpatcher.yaml first; the original "
                          "file is left in place until a build succeeds.");
    }

    std::error_code ec;
    const auto result_file =
        std::filesystem::temp_directory_path(ec) / "big-star-ncpatcher-build.json";

    std::vector<std::string> arguments{"-C", request.project_dir.string()};
    if (!request.rom_override.empty()) {
        arguments.push_back("--rom");
        arguments.push_back(request.rom_override.string());
    }
    if (!request.output_override.empty()) {
        arguments.push_back("--out");
        arguments.push_back(request.output_override.string());
    }
    if (request.verbose) {
        arguments.push_back("-v");
    }
    arguments.insert(arguments.end(),
                     {"--message-format", "json", "--result", result_file.string()});
    // A variant is a global option and belongs ahead of the subcommand.
    if (!request.variant.empty()) {
        arguments.push_back("--variant");
        arguments.push_back(request.variant);
    }
    arguments.push_back("build");
    // `--all-variants` is deliberately not offered: it patches one opened ROM,
    // and nine outputs have nowhere to go.

    ProcessRequest process;
    process.executable = executable_;
    process.arguments = std::move(arguments);
    process.working_directory = request.project_dir;
    process.cancel = request.cancel;

    logger_->info("running " + describe_command(process));

    std::string trailing_result;
    const auto run = run_process(
        process,
        [&](std::string_view line) {
            // One JSON object per line. A line that does not parse is a tool
            // bug rather than something to act on, so it goes to the log
            // instead of being silently swallowed.
            auto parsed = core::parse_json(line);
            if (!parsed || !parsed.value().is_object()) {
                if (observer.on_log) {
                    observer.on_log(line);
                }
                return;
            }
            const JsonValue& event = parsed.value();
            const std::string& type = event["type"].as_string();
            if (type == "progress") {
                if (observer.on_progress) {
                    observer.on_progress(read_progress(event));
                }
            } else if (type == "artifact") {
                if (observer.on_artifact) {
                    observer.on_artifact(read_artifact(event));
                }
            } else if (type == "diagnostic") {
                if (observer.on_diagnostic) {
                    observer.on_diagnostic(read_diagnostic(event));
                }
            } else if (type == "result") {
                trailing_result.assign(line);
            }
        },
        [&](std::string_view line) {
            if (observer.on_log) {
                observer.on_log(line);
            }
        });

    if (!run) {
        std::filesystem::remove(result_file, ec);
        return run.error();
    }

    auto text = core::read_file(result_file);
    std::filesystem::remove(result_file, ec);

    if (text) {
        auto summary = parse_run_summary(std::string_view(
            reinterpret_cast<const char*>(text.value().data()), text.value().size()));
        if (summary) {
            return summary;
        }
        logger_->warning("could not read NCPatcher's run result: " + summary.error().message);
    }

    // No usable result file. The streamed `result` event carries the same
    // numbers, so a run is still reportable rather than merely "it failed".
    if (!trailing_result.empty()) {
        RunSummary summary;
        auto parsed = core::parse_json(trailing_result);
        if (parsed) {
            const JsonValue& event = parsed.value();
            summary.status = event["status"].as_string();
            summary.exit_code = static_cast<int>(event["exit-code"].as_int64());
            summary.duration_ms = event["duration-ms"].as_int64();
            summary.errors = static_cast<int>(event["errors"].as_int64());
            summary.warnings = static_cast<int>(event["warnings"].as_int64());
            summary.stage = stage_from_exit_code(summary.exit_code);
            return summary;
        }
    }

    RunSummary summary;
    summary.exit_code = run.value().exit_code;
    summary.stage = stage_from_exit_code(summary.exit_code);
    summary.status = summary.exit_code == 0 ? "ok" : "error";
    summary.errors = summary.exit_code == 0 ? 0 : 1;
    return summary;
}

Result<RunSummary> NCPatcherClient::migrate(const std::filesystem::path& project_dir, bool write) {
    std::error_code ec;
    const auto result_file =
        std::filesystem::temp_directory_path(ec) / "big-star-ncpatcher-migrate.json";

    std::vector<std::string> arguments{"-C",       project_dir.string(), "--message-format",
                                       "json",     "--result",           result_file.string(),
                                       "migrate"};
    if (write) {
        arguments.push_back("--write");
    }

    auto run = capture(project_dir, std::move(arguments));
    if (!run) {
        return run.error();
    }

    auto text = core::read_file(result_file);
    std::filesystem::remove(result_file, ec);
    if (!text) {
        return make_error(ErrorCode::ExternalToolFailed,
                          "NCPatcher wrote no run result:\n" + run.value().stderr_text);
    }
    return parse_run_summary(
        std::string_view(reinterpret_cast<const char*>(text.value().data()), text.value().size()));
}

} // namespace nsmb::patching
