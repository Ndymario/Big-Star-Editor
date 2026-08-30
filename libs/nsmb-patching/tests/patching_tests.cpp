#include "nsmb/patching/ncpatcher.hpp"
#include "nsmb/patching/process.hpp"

#include <cassert>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
#include <string>
#include <vector>

namespace {

using nsmb::patching::LineCallback;
using nsmb::patching::ProcessRequest;
using nsmb::patching::describe_command;
using nsmb::patching::run_process;
using nsmb::patching::windows_command_line;

// Where this test binary lives, so it can launch itself as the child fixture.
// Re-running our own executable keeps the test free of any assumption about
// which shells or utilities the host has, and proves the runner can start a
// real program rather than a shell built-in.
std::filesystem::path self_path;

void test_windows_quoting() {
    // These are the strings the Windows C runtime has to be able to parse back
    // into the vector that was sent. Backslashes are literal unless they run
    // into a quote, which is exactly where a naive quoter loses an argument.
    assert(windows_command_line("ncpatcher", {"build"}) == "ncpatcher build");
    assert(windows_command_line("ncpatcher", {"-C", "C:\\My Projects\\coop"}) ==
           "ncpatcher -C \"C:\\My Projects\\coop\"");
    assert(windows_command_line("nc", {"a\\\\b"}) == "nc a\\\\b");
    assert(windows_command_line("nc", {"trailing\\"}) == "nc trailing\\");
    assert(windows_command_line("nc", {"with space\\"}) == "nc \"with space\\\\\"");
    assert(windows_command_line("nc", {"quote\"inside"}) == "nc \"quote\\\"inside\"");
    assert(windows_command_line("nc", {""}) == "nc \"\"");
    assert(windows_command_line("C:\\Program Files\\nc.exe", {}) ==
           "\"C:\\Program Files\\nc.exe\"");
}

void test_describe_command() {
    ProcessRequest request;
    request.executable = "ncpatcher";
    request.arguments = {"-C", "/home/a b/coop", "--variant", "fr", "build"};
    // For logs only. The runner never parses this back.
    assert(describe_command(request) ==
           "ncpatcher -C \"/home/a b/coop\" --variant fr build");
}

void test_runs_and_captures_both_streams() {
    ProcessRequest request;
    request.executable = self_path;
    request.arguments = {"--child-fixture"};

    std::vector<std::string> out_lines;
    std::vector<std::string> err_lines;
    const auto result = run_process(
        request,
        [&out_lines](std::string_view line) { out_lines.emplace_back(line); },
        [&err_lines](std::string_view line) { err_lines.emplace_back(line); });

    assert(result);
    // Exit codes survive: NCPatcher distinguishes configuration (3) from
    // compilation (6) from a failed hook (10), and the editor reports which.
    assert(result.value().exit_code == 7);
    assert(!result.value().terminated_by_signal);

    // Both streams are captured, and separately -- the old std::system path
    // returned neither.
    assert(out_lines.size() == 3);
    assert(out_lines[0] == "stdout one");
    assert(out_lines[1] == "stdout two");
    // A final line with no terminator is still delivered rather than dropped.
    assert(out_lines[2] == "no trailing newline");
    assert(err_lines.size() == 1);
    assert(err_lines[0] == "stderr one");
}

void test_arguments_are_not_shell_interpreted() {
    // The whole point of dropping std::system: these are ordinary argument
    // bytes now, not something a shell gets to expand, split or execute.
    const std::vector<std::string> hostile = {
        "$(touch /tmp/big-star-should-not-exist)",
        "; rm -rf /",
        "`id`",
        "a b\tc",
        "quote\"and'quote",
        "*",
    };

    ProcessRequest request;
    request.executable = self_path;
    request.arguments = {"--echo-args"};
    for (const auto& argument : hostile) {
        request.arguments.push_back(argument);
    }

    std::vector<std::string> out_lines;
    const auto result = run_process(
        request, [&out_lines](std::string_view line) { out_lines.emplace_back(line); }, nullptr);

    assert(result && result.value().exit_code == 0);
    assert(out_lines.size() == hostile.size());
    for (std::size_t i = 0; i < hostile.size(); ++i) {
        // Byte for byte, including the tab, which a shell would have split on.
        assert(out_lines[i] == hostile[i]);
    }
    assert(!std::filesystem::exists("/tmp/big-star-should-not-exist"));
}

void test_working_directory_is_child_only() {
    const auto before = std::filesystem::current_path();
    const auto elsewhere = std::filesystem::temp_directory_path();

    ProcessRequest request;
    request.executable = self_path;
    request.arguments = {"--print-cwd"};
    request.working_directory = elsewhere;

    std::vector<std::string> out_lines;
    const auto result = run_process(
        request, [&out_lines](std::string_view line) { out_lines.emplace_back(line); }, nullptr);

    assert(result && result.value().exit_code == 0);
    assert(out_lines.size() == 1);
    assert(std::filesystem::weakly_canonical(out_lines[0]) ==
           std::filesystem::weakly_canonical(elsewhere));

    // The caller's own directory is untouched. The old implementation changed
    // it process-wide and changed it back, which no threaded build can share.
    assert(std::filesystem::current_path() == before);
}

void test_environment_overrides_reach_the_child() {
    ProcessRequest request;
    request.executable = self_path;
    request.arguments = {"--print-env", "BIG_STAR_TEST_VAR"};
    request.environment = {"BIG_STAR_TEST_VAR=from-the-request"};

    std::vector<std::string> out_lines;
    const auto result = run_process(
        request, [&out_lines](std::string_view line) { out_lines.emplace_back(line); }, nullptr);

    assert(result && result.value().exit_code == 0);
    assert(out_lines.size() == 1);
    assert(out_lines[0] == "from-the-request");
}

void test_missing_executable_is_reported() {
    ProcessRequest request;
    request.executable = "big-star-no-such-tool-anywhere";
    const auto result = run_process(request, nullptr, nullptr);
    // A missing NCPatcher has to say so, rather than surfacing as some generic
    // nonzero status the editor would report as "the build failed".
    assert(!result);
    assert(result.error().code == nsmb::core::ErrorCode::NotFound ||
           result.error().code == nsmb::core::ErrorCode::ExternalToolFailed);
}

void test_missing_working_directory_is_reported() {
    ProcessRequest request;
    request.executable = self_path;
    request.arguments = {"--print-cwd"};
    request.working_directory = "/big-star-no-such-directory-anywhere";
    const auto result = run_process(request, nullptr, nullptr);
    assert(!result);
}

// ---------------------------------------------------------------------------
// Document readers. The fixtures below are trimmed copies of what NCPatcher
// 2.x actually writes, so a change in its output shows up here.
// ---------------------------------------------------------------------------

void test_parse_project_config() {
    const auto config = nsmb::patching::parse_project_config(R"({
      "schema": "ncpatcher.config/1",
      "version": 2,
      "file": "/p/ncpatcher.yaml",
      "project-root": "/p",
      "rom-dir": "/p",
      "rom-file": "/p/rom.nds",
      "rom-output": "/p/build/nds/rom.nds",
      "rom-banner": "/p/assets/banner.bin",
      "backup-dir": "/p/backup",
      "toolchain": "arm-none-eabi-",
      "threads": 0,
      "files": { "data/base.bin": "assets/base.bin" },
      "variants": {
        "en": { "defines": ["GAME_LANGUAGE_EN"], "files": {} },
        "fr": { "defines": ["GAME_LANGUAGE_FR"], "banner": "/p/assets/fr-banner.bin", "files": {
          "data/base.bin": "assets/fr.bin", "data/fr.bin": "assets/only-fr.bin"
        } },
        "de": { "defines": ["GAME_LANGUAGE_GE"], "files": {} },
        "pt": { "defines": ["GAME_LANGUAGE_PT"], "files": {} }
      }
    })");
    assert(config);
    assert(config.value().version == 2);
    assert(config.value().rom_file == std::filesystem::path("/p/rom.nds"));
    assert(config.value().rom_banner == std::filesystem::path("/p/assets/banner.bin"));
    assert(config.value().toolchain == "arm-none-eabi-");
    assert(config.value().files.size() == 1);
    assert(config.value().files[0].first == "data/base.bin");
    assert(config.value().files[0].second == std::filesystem::path("assets/base.bin"));

    // Declaration order, not alphabetical. A variant picker showing "de" first
    // would be reordering the project's own answer.
    assert(config.value().variants.size() == 4);
    assert(config.value().variants[0].name == "en");
    assert(config.value().variants[1].name == "fr");
    assert(config.value().variants[2].name == "de");
    assert(config.value().variants[3].name == "pt");
    assert(config.value().variants[1].defines.size() == 1);
    assert(config.value().variants[1].defines[0] == "GAME_LANGUAGE_FR");
    assert(config.value().variants[1].files.size() == 2);
    assert(config.value().variants[1].files[0].second == std::filesystem::path("assets/fr.bin"));
    assert(config.value().variants[1].banner == std::filesystem::path("/p/assets/fr-banner.bin"));
    assert(config.value().has_variant("pt"));
    assert(!config.value().has_variant("ru"));

    // A project that configures no variants is a different thing from a failed
    // query, and must parse cleanly rather than looking like an error.
    const auto none = nsmb::patching::parse_project_config(
        R"({"schema":"ncpatcher.config/1","version":2,"variants":{}})");
    assert(none && none.value().variants.empty());
}

void test_parse_module_graph() {
    const auto graph = nsmb::patching::parse_module_graph(R"({
      "schema": "ncpatcher.modules/1",
      "dir": "/p/modules",
      "modules": [
        { "key": "coop", "id": "Coop", "name": "NSMB Co-op", "enabled": true,
          "dir": "/p/modules/coop", "file": "/p/modules/coop/module.yaml",
          "nitrofs": { "dir": "/p/modules/coop/nitrofs", "layered": true, "base-variant": "en" },
          "components": [
            { "name": "Vanilla", "enabled": true,
              "files": ["z_new/coop/SE_VOC_MA_SHOT.nwav"],
              "extra": { "objects": [ { "name": "CoopFlagActor", "type": "actor",
                                        "stage": ["Default", "Big"] } ] } },
            { "name": "CustomWorldUnlock", "enabled": false,
              "disabled-because": "the project switched it off",
              "files": ["enemy/w3_sign.nsbmd", "enemy/w6_sign.nsbmd"] }
          ] },
        { "key": "glue", "id": "Glue", "name": "Interoperability Module", "enabled": true,
          "dir": "/p/modules/glue",
          "extra": { "level-data": { "stageObjects": "u32[?]" } },
          "components": [] },
        { "key": "unused", "id": "Unused", "name": "Unused", "enabled": false }
      ],
      "targets": { "arm7": {"includes":[],"defines":[],"regions":[]},
                   "arm9": {"includes":[],"defines":[],"regions":[]} }
    })");
    assert(graph);
    assert(graph.value().dir == std::filesystem::path("/p/modules"));
    assert(graph.value().modules.size() == 3);

    const auto* coop = graph.value().find("coop");
    assert(coop != nullptr);
    assert(coop->id == "Coop" && coop->enabled);
    assert(coop->nitrofs.has_value());
    assert(coop->nitrofs->layered);
    assert(coop->nitrofs->base_variant == "en");
    assert(coop->nitrofs->into.empty());
    assert(coop->components.size() == 2);

    // A disabled component still reports the destinations it owns: those are
    // subtracted from the module's tree rather than inserted, so the entry says
    // what switching the feature off removed.
    assert(!coop->components[1].enabled);
    assert(coop->components[1].files.size() == 2);
    assert(!coop->components[1].disabled_because.empty());

    // `extra` is the extension point NCPatcher preserves and never interprets.
    // Glue's level-data declaration and coop's object list both ride there.
    assert(coop->components[0].extra["objects"].at(0)["name"].as_string() == "CoopFlagActor");
    const auto* glue = graph.value().find("glue");
    assert(glue != nullptr);
    assert(glue->extra["level-data"]["stageObjects"].as_string() == "u32[?]");
    assert(!glue->nitrofs.has_value());

    // A disabled module carries only the four fields that need no module.yaml.
    const auto* unused = graph.value().find("unused");
    assert(unused != nullptr && !unused->enabled && !unused->nitrofs.has_value());

    assert(graph.value().find("nope") == nullptr);
}

void test_parse_file_manifest() {
    using nsmb::patching::FileAction;
    const auto manifest = nsmb::patching::parse_file_manifest(R"({
      "schema": "ncpatcher.files/1",
      "variant": "fr",
      "count": 3,
      "files": [
        { "id": 1500, "path": "uiStudio/title.bin", "size": 2031, "action": "unchanged" },
        { "id": 1957, "path": "z_new/reserved", "size": 0, "action": "created" },
        { "id": 2101, "path": "z_new/message/msg_data.bin", "size": 6144, "action": "created",
          "source": "modules/message/nitrofs/fr/z_new/message/msg_data.bin",
          "module": "message", "component": "Vanilla", "from-variant": "fr" }
      ] })");
    assert(manifest);
    assert(manifest.value().variant == "fr");
    assert(manifest.value().files.size() == 3);

    const auto* vanilla = manifest.value().find("uiStudio/title.bin");
    assert(vanilla != nullptr);
    assert(vanilla->id == 1500 && vanilla->action == FileAction::Unchanged);
    // Every entry is vanilla, replaced by a module, or added by one. That
    // distinction is what the filesystem view renders.
    assert(!vanilla->has_provenance());

    const auto* added = manifest.value().find("z_new/message/msg_data.bin");
    assert(added != nullptr);
    assert(added->action == FileAction::Created);
    assert(added->module == "message");
    assert(added->component == "Vanilla");
    assert(added->from_variant == "fr");
    assert(added->has_provenance());

    // The reservation placeholder takes the first new id and stays empty, so
    // the project's own additions start after it.
    const auto* reserved = manifest.value().find("z_new/reserved");
    assert(reserved != nullptr && reserved->size == 0);

    assert(manifest.value().find("nope") == nullptr);

    // A truncated or concatenated document would otherwise understate what the
    // ROM holds, and an id read from it would name the wrong file.
    assert(!nsmb::patching::parse_file_manifest(
        R"({"schema":"ncpatcher.files/1","count":9,"files":[]})"));
}

void test_parse_run_summary() {
    using nsmb::patching::DiagnosticLevel;
    using nsmb::patching::NCPatcherStage;

    const auto summary = nsmb::patching::parse_run_summary(R"({
      "schema": "ncpatcher.result/1",
      "status": "error",
      "exit-code": 3,
      "duration-ms": 8123,
      "errors": 1,
      "warnings": 2,
      "diagnostics": [
        { "level": "error", "code": "NCP0001", "message": "\"rom.backup\" was not found.",
          "location": { "file": "/p/ncpatcher.yaml", "line": 3, "col": 5, "path": "rom" } },
        { "level": "warning", "code": "NCP0100", "message": "no location here" }
      ],
      "artifacts": [
        { "kind": "overlay", "proc": "arm9", "id": 58, "action": "modified", "size": 4788,
          "ram-address": "0x021726C0", "file-id": 117, "name": "overlay9/overlay9_58.bin" }
      ] })");
    assert(summary);
    assert(!summary.value().succeeded());
    assert(summary.value().exit_code == 3);
    // Exit code 3 is a configuration fault, and the editor says so rather than
    // reporting every failure as "the build failed".
    assert(summary.value().stage == NCPatcherStage::Configuration);
    assert(summary.value().errors == 1 && summary.value().warnings == 2);

    assert(summary.value().diagnostics.size() == 2);
    assert(summary.value().diagnostics[0].level == DiagnosticLevel::Error);
    assert(summary.value().diagnostics[0].code == "NCP0001");
    assert(summary.value().diagnostics[0].location.known);
    assert(summary.value().diagnostics[0].location.line == 3);
    assert(summary.value().diagnostics[0].location.col == 5);
    assert(summary.value().diagnostics[0].location.node_path == "rom");
    // A diagnostic with no location must not pretend to have one, or the editor
    // would jump to line 0 of nothing.
    assert(summary.value().diagnostics[1].level == DiagnosticLevel::Warning);
    assert(!summary.value().diagnostics[1].location.known);

    assert(summary.value().artifacts.size() == 1);
    assert(summary.value().artifacts[0].kind == "overlay");
    assert(summary.value().artifacts[0].id.value() == 58);
    assert(summary.value().artifacts[0].file_id.value() == 117);
    assert(summary.value().artifacts[0].ram_address == "0x021726C0");

    const auto ok = nsmb::patching::parse_run_summary(
        R"({"schema":"ncpatcher.result/1","status":"ok","exit-code":0,"duration-ms":8,
            "errors":0,"warnings":0,"diagnostics":[],"artifacts":[]})");
    assert(ok && ok.value().succeeded() && ok.value().stage == NCPatcherStage::Ok);
}

void test_unknown_schemas_are_refused() {
    using nsmb::core::ErrorCode;

    // Reading a document written by a newer build as far as it happens to parse
    // is how a ROM ends up patched wrongly and nobody finds out for weeks.
    const auto newer = nsmb::patching::parse_file_manifest(
        R"({"schema":"ncpatcher.files/2","count":0,"files":[]})");
    assert(!newer && newer.error().code == ErrorCode::Unsupported);

    // A document of the right shape but the wrong kind must not be accepted
    // just because its fields happen to line up.
    const auto wrong_kind = nsmb::patching::parse_project_config(
        R"({"schema":"ncpatcher.modules/1","dir":"/p","modules":[]})");
    assert(!wrong_kind && wrong_kind.error().code == ErrorCode::Unsupported);

    // No schema at all means the format is unknown, not that it is the one we
    // were hoping for.
    assert(!nsmb::patching::parse_run_summary(R"({"status":"ok","exit-code":0})"));
    assert(!nsmb::patching::parse_module_graph(R"([])"));
    assert(!nsmb::patching::parse_file_manifest("not json at all"));
}

void test_exit_code_stages() {
    using nsmb::patching::NCPatcherStage;
    using nsmb::patching::stage_from_exit_code;
    assert(stage_from_exit_code(0) == NCPatcherStage::Ok);
    assert(stage_from_exit_code(3) == NCPatcherStage::Configuration);
    assert(stage_from_exit_code(4) == NCPatcherStage::ModuleResolution);
    assert(stage_from_exit_code(5) == NCPatcherStage::Toolchain);
    assert(stage_from_exit_code(6) == NCPatcherStage::Compilation);
    assert(stage_from_exit_code(7) == NCPatcherStage::Linking);
    assert(stage_from_exit_code(8) == NCPatcherStage::Patching);
    assert(stage_from_exit_code(9) == NCPatcherStage::RomIo);
    assert(stage_from_exit_code(10) == NCPatcherStage::Hook);
    assert(stage_from_exit_code(42) == NCPatcherStage::Unknown);
    assert(!describe_stage(NCPatcherStage::Hook).empty());
}

void test_project_config_detection() {
    using nsmb::patching::ProjectConfigKind;
    using nsmb::patching::detect_project_config;

    const auto root = std::filesystem::temp_directory_path() / "big-star-ncpatcher-detect";
    std::filesystem::remove_all(root);

    const auto empty = root / "empty";
    std::filesystem::create_directories(empty);
    assert(detect_project_config(empty).kind == ProjectConfigKind::None);

    const auto legacy = root / "legacy";
    std::filesystem::create_directories(legacy);
    { std::ofstream(legacy / "ncpatcher.json") << "{}\n"; }
    assert(detect_project_config(legacy).kind == ProjectConfigKind::V1Json);

    const auto modern = root / "modern";
    std::filesystem::create_directories(modern);
    { std::ofstream(modern / "ncpatcher.yaml") << "version: 2\n"; }
    assert(detect_project_config(modern).kind == ProjectConfigKind::V2Yaml);
    assert(detect_project_config(modern).file == modern / "ncpatcher.yaml");

    // A project mid-migration holds both. The v2 file is the one that counts,
    // so a converted project does not keep being treated as legacy.
    { std::ofstream(modern / "ncpatcher.json") << "{}\n"; }
    assert(detect_project_config(modern).kind == ProjectConfigKind::V2Yaml);

    // The .yml spelling is equally valid and must not read as "no project".
    const auto yml = root / "yml";
    std::filesystem::create_directories(yml);
    { std::ofstream(yml / "ncpatcher.yml") << "version: 2\n"; }
    assert(detect_project_config(yml).kind == ProjectConfigKind::V2Yaml);

    std::filesystem::remove_all(root);
}

void test_build_refuses_a_legacy_project() {
    using nsmb::core::ErrorCode;

    const auto root = std::filesystem::temp_directory_path() / "big-star-ncpatcher-legacy";
    std::filesystem::remove_all(root);
    std::filesystem::create_directories(root);
    { std::ofstream(root / "ncpatcher.json") << "{}\n"; }

    // Refused before anything is launched: converting a project is a decision,
    // not something that happens the first time someone presses Build.
    nsmb::patching::NCPatcherClient client("big-star-no-such-tool-anywhere");
    nsmb::patching::BuildRequest request;
    request.project_dir = root;
    const auto result = client.build(request);
    assert(!result && result.error().code == ErrorCode::Unsupported);

    std::filesystem::remove_all(root);

    // And a directory with no configuration at all is a missing project rather
    // than a build failure.
    nsmb::patching::BuildRequest missing;
    missing.project_dir = std::filesystem::temp_directory_path() / "big-star-no-project-here";
    const auto absent = client.build(missing);
    assert(!absent && absent.error().code == ErrorCode::NotFound);
}

int run_child_fixture(int argc, char** argv) {
    const std::string mode = argv[1];

    if (mode == "--child-fixture") {
        std::fputs("stdout one\nstdout two\n", stdout);
        std::fputs("stderr one\n", stderr);
        std::fflush(stderr);
        std::fputs("no trailing newline", stdout);
        std::fflush(stdout);
        return 7;
    }

    if (mode == "--echo-args") {
        for (int i = 2; i < argc; ++i) {
            std::fputs(argv[i], stdout);
            std::fputc('\n', stdout);
        }
        return 0;
    }

    if (mode == "--print-cwd") {
        std::fputs(std::filesystem::current_path().string().c_str(), stdout);
        std::fputc('\n', stdout);
        return 0;
    }

    if (mode == "--print-env") {
        const char* value = argc > 2 ? std::getenv(argv[2]) : nullptr;
        std::fputs(value ? value : "<unset>", stdout);
        std::fputc('\n', stdout);
        return 0;
    }

    return 99;
}

} // namespace

int main(int argc, char** argv) {
    if (argc > 1) {
        return run_child_fixture(argc, argv);
    }

    self_path = std::filesystem::absolute(argv[0]);

    test_windows_quoting();
    test_describe_command();
    test_runs_and_captures_both_streams();
    test_arguments_are_not_shell_interpreted();
    test_working_directory_is_child_only();
    test_environment_overrides_reach_the_child();
    test_missing_executable_is_reported();
    test_missing_working_directory_is_reported();

    test_parse_project_config();
    test_parse_module_graph();
    test_parse_file_manifest();
    test_parse_run_summary();
    test_unknown_schemas_are_refused();
    test_exit_code_stages();
    test_project_config_detection();
    test_build_refuses_a_legacy_project();
    return 0;
}
