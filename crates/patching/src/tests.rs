use std::path::{Path, PathBuf};

use bigstar_core::ErrorCode;

use super::*;

/// A scratch directory under the system temp dir, removed again on drop.
struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("bigstar-patching-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn dir(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).to_owned()).collect()
}

#[test]
fn windows_quoting_round_trips_through_the_c_runtime_rule() {
    // These are the strings the Windows C runtime has to be able to parse back into the vector that was sent.
    // Backslashes are literal unless they run into a quote, which is exactly where a naive quoter loses an argument.
    let nc = Path::new("nc");
    assert_eq!(windows_command_line(Path::new("ncpatcher"), &strings(&["build"])), "ncpatcher build");
    assert_eq!(windows_command_line(Path::new("ncpatcher"), &strings(&["-C", "C:\\My Projects\\coop"])), "ncpatcher -C \"C:\\My Projects\\coop\"");
    assert_eq!(windows_command_line(nc, &strings(&["a\\\\b"])), "nc a\\\\b");
    assert_eq!(windows_command_line(nc, &strings(&["trailing\\"])), "nc trailing\\");
    assert_eq!(windows_command_line(nc, &strings(&["with space\\"])), "nc \"with space\\\\\"");
    assert_eq!(windows_command_line(nc, &strings(&["quote\"inside"])), "nc \"quote\\\"inside\"");
    assert_eq!(windows_command_line(nc, &strings(&[""])), "nc \"\"");
    assert_eq!(windows_command_line(Path::new("C:\\Program Files\\nc.exe"), &[]), "\"C:\\Program Files\\nc.exe\"");
}

#[test]
fn describe_command_quotes_for_display_only() {
    let request = ProcessRequest {
        executable: PathBuf::from("ncpatcher"),
        arguments: strings(&["-C", "/home/a b/coop", "--variant", "fr", "build"]),
        ..ProcessRequest::default()
    };
    assert_eq!(describe_command(&request), "ncpatcher -C \"/home/a b/coop\" --variant fr build");
}

#[test]
fn project_config_parses_in_declaration_order() {
    let config = parse_project_config(
        r#"{
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
        }, "module-variants": { "message": "french", "glue": "base" } },
        "de": { "defines": ["GAME_LANGUAGE_GE"], "files": {} },
        "pt": { "defines": ["GAME_LANGUAGE_PT"], "files": {} }
      }
    }"#,
    )
    .unwrap();
    assert_eq!(config.version, 2);
    assert_eq!(config.rom_file, PathBuf::from("/p/rom.nds"));
    assert_eq!(config.rom_banner, PathBuf::from("/p/assets/banner.bin"));
    assert_eq!(config.toolchain, "arm-none-eabi-");
    assert_eq!(config.files, [("data/base.bin".to_owned(), PathBuf::from("assets/base.bin"))]);

    // Declaration order, not alphabetical. A variant picker showing "de" first would be reordering the project's own
    // answer.
    let names: Vec<&str> = config.variants.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["en", "fr", "de", "pt"]);
    assert_eq!(config.variants[1].defines, ["GAME_LANGUAGE_FR"]);
    assert_eq!(config.variants[1].files.len(), 2);
    assert_eq!(config.variants[1].files[0].1, PathBuf::from("assets/fr.bin"));
    assert_eq!(config.variants[1].banner, PathBuf::from("/p/assets/fr-banner.bin"));

    // Which layer of a layered module tree a variant selects.
    assert_eq!(config.variants[1].module_variants, [("message".to_owned(), "french".to_owned()), ("glue".to_owned(), "base".to_owned())]);
    assert!(config.variants[0].module_variants.is_empty());
    assert!(config.has_variant("pt"));
    assert!(!config.has_variant("ru"));

    // A project that configures no variants is a different thing from a failed query.
    let none = parse_project_config(r#"{"schema":"ncpatcher.config/1","version":2,"variants":{}}"#).unwrap();
    assert!(none.variants.is_empty());
}

#[test]
fn module_graph_keeps_extras_and_disabled_components() {
    let graph = parse_module_graph(
        r#"{
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
    }"#,
    )
    .unwrap();
    assert_eq!(graph.dir, PathBuf::from("/p/modules"));
    assert_eq!(graph.modules.len(), 3);

    let coop = graph.find("coop").unwrap();
    assert!(coop.id == "Coop" && coop.enabled);
    let nitrofs = coop.nitrofs.as_ref().unwrap();
    assert!(nitrofs.layered);
    assert_eq!(nitrofs.base_variant, "en");
    assert!(nitrofs.into.is_empty());
    assert_eq!(coop.components.len(), 2);

    // A disabled component still reports the destinations it owns.
    assert!(!coop.components[1].enabled);
    assert_eq!(coop.components[1].files.len(), 2);
    assert!(!coop.components[1].disabled_because.is_empty());

    // `extra` is the extension point NCPatcher preserves and never interprets.
    assert_eq!(coop.components[0].extra["objects"][0]["name"].as_str(), Some("CoopFlagActor"));
    let glue = graph.find("glue").unwrap();
    assert_eq!(glue.extra["level-data"]["stageObjects"].as_str(), Some("u32[?]"));
    assert!(glue.nitrofs.is_none());

    // A disabled module carries only the four fields that need no module.yaml.
    let unused = graph.find("unused").unwrap();
    assert!(!unused.enabled && unused.nitrofs.is_none());
    assert!(graph.find("nope").is_none());
}

#[test]
fn file_manifest_carries_provenance_and_checks_its_count() {
    let manifest = parse_file_manifest(
        r#"{
      "schema": "ncpatcher.files/1",
      "variant": "fr",
      "count": 3,
      "files": [
        { "id": 1500, "path": "uiStudio/title.bin", "size": 2031, "action": "unchanged" },
        { "id": 1957, "path": "z_new/reserved", "size": 0, "action": "created" },
        { "id": 2101, "path": "z_new/message/msg_data.bin", "size": 6144, "action": "created",
          "source": "modules/message/nitrofs/fr/z_new/message/msg_data.bin",
          "module": "message", "component": "Vanilla", "from-variant": "fr" }
      ] }"#,
    )
    .unwrap();
    assert_eq!(manifest.variant, "fr");
    assert_eq!(manifest.files.len(), 3);

    let vanilla = manifest.find("uiStudio/title.bin").unwrap();
    assert!(vanilla.id == 1500 && vanilla.action == FileAction::Unchanged);
    assert!(!vanilla.has_provenance());

    let added = manifest.find("z_new/message/msg_data.bin").unwrap();
    assert_eq!(added.action, FileAction::Created);
    assert_eq!(added.module, "message");
    assert_eq!(added.component, "Vanilla");
    assert_eq!(added.from_variant, "fr");
    assert!(added.has_provenance());

    assert_eq!(manifest.find("z_new/reserved").unwrap().size, 0);
    assert!(manifest.find("nope").is_none());

    // A truncated or concatenated document would otherwise understate what the ROM holds.
    assert!(parse_file_manifest(r#"{"schema":"ncpatcher.files/1","count":9,"files":[]}"#).is_err());
    // A build's manifest says nothing about planning, so every id in it is one the ROM holds.
    assert!(!manifest.planned);
}

#[test]
fn planned_manifest_reads_archive_members() {
    let plan = parse_file_manifest(
        r#"{
      "schema": "ncpatcher.files/1",
      "variant": "fr",
      "planned": true,
      "count": 3,
      "files": [
        { "id": 812, "path": "ARCHIVE/menu_title.narc", "size": 40960, "action": "modified",
          "members": [
            { "id": 14, "path": "menu/title/USA/vs.bmg", "size": 2031, "action": "modified",
              "source": "modules/message/nitrofs/fr/ARCHIVE/menu_title_narc/menu/title/USA/vs.bmg",
              "module": "message", "component": "Vanilla", "from-variant": "fr" },
            { "id": 31, "path": "menu/title/USA/opt.bmg", "size": 512, "action": "modified",
              "source": "modules/hud/nitrofs/ARCHIVE/menu_title_narc/menu/title/USA/opt.bmg",
              "module": "hud" }
          ] },
        { "id": 1957, "path": "z_new/reserved", "size": 0, "action": "created" },
        { "id": 1958, "path": "z_new/generated.bin", "size": 0, "action": "created",
          "source": "build/generated/generated.bin", "module": "glue",
          "source-missing": true }
      ] }"#,
    )
    .unwrap();
    assert!(plan.planned);

    let archive = plan.find("ARCHIVE/menu_title.narc").unwrap();
    assert!(archive.module.is_empty());
    assert_eq!(archive.members.len(), 2);
    assert_eq!(archive.members[0].id, 14);
    assert_eq!(archive.members[0].path, "menu/title/USA/vs.bmg");
    assert_eq!(archive.members[0].module, "message");
    assert_eq!(archive.members[0].from_variant, "fr");
    assert_eq!(archive.members[0].action, FileAction::Modified);
    assert_eq!(archive.members[1].id, 31);
    assert_eq!(archive.members[1].module, "hud");
    assert!(archive.members[1].from_variant.is_empty());

    let generated = plan.find("z_new/generated.bin").unwrap();
    assert!(generated.source_missing);
    assert_eq!(generated.module, "glue");

    let reserved = plan.find("z_new/reserved").unwrap();
    assert!(!reserved.source_missing && reserved.members.is_empty());
}

#[test]
fn run_summary_maps_stages_and_locations() {
    let summary = parse_run_summary(
        r#"{
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
      ] }"#,
    )
    .unwrap();
    assert!(!summary.succeeded());
    assert_eq!(summary.exit_code, 3);
    assert_eq!(summary.stage, NCPatcherStage::Configuration);
    assert_eq!((summary.errors, summary.warnings), (1, 2));

    assert_eq!(summary.diagnostics.len(), 2);
    let first = &summary.diagnostics[0];
    assert_eq!(first.level, DiagnosticLevel::Error);
    assert_eq!(first.code, "NCP0001");
    assert!(first.location.known);
    assert_eq!((first.location.line, first.location.col), (3, 5));
    assert_eq!(first.location.node_path, "rom");
    // A diagnostic with no location must not pretend to have one.
    assert_eq!(summary.diagnostics[1].level, DiagnosticLevel::Warning);
    assert!(!summary.diagnostics[1].location.known);

    assert_eq!(summary.artifacts.len(), 1);
    assert_eq!(summary.artifacts[0].kind, "overlay");
    assert_eq!(summary.artifacts[0].id, Some(58));
    assert_eq!(summary.artifacts[0].file_id, Some(117));
    assert_eq!(summary.artifacts[0].ram_address, "0x021726C0");

    let ok = parse_run_summary(
        r#"{"schema":"ncpatcher.result/1","status":"ok","exit-code":0,"duration-ms":8,"errors":0,"warnings":0,"diagnostics":[],"artifacts":[]}"#,
    )
    .unwrap();
    assert!(ok.succeeded() && ok.stage == NCPatcherStage::Ok);
}

#[test]
fn unknown_schemas_are_refused() {
    let newer = parse_file_manifest(r#"{"schema":"ncpatcher.files/2","count":0,"files":[]}"#).unwrap_err();
    assert_eq!(newer.code, ErrorCode::Unsupported);
    let wrong_kind = parse_project_config(r#"{"schema":"ncpatcher.modules/1","dir":"/p","modules":[]}"#).unwrap_err();
    assert_eq!(wrong_kind.code, ErrorCode::Unsupported);
    assert!(parse_run_summary(r#"{"status":"ok","exit-code":0}"#).is_err());
    assert!(parse_module_graph("[]").is_err());
    assert!(parse_file_manifest("not json at all").is_err());
}

#[test]
fn exit_codes_name_their_stage() {
    let expected = [
        (0, NCPatcherStage::Ok),
        (3, NCPatcherStage::Configuration),
        (4, NCPatcherStage::ModuleResolution),
        (5, NCPatcherStage::Toolchain),
        (6, NCPatcherStage::Compilation),
        (7, NCPatcherStage::Linking),
        (8, NCPatcherStage::Patching),
        (9, NCPatcherStage::RomIo),
        (10, NCPatcherStage::Hook),
        (11, NCPatcherStage::Cancelled),
        (42, NCPatcherStage::Unknown),
    ];
    for (code, stage) in expected {
        assert_eq!(stage_from_exit_code(code), stage);
    }
    assert!(!describe_stage(NCPatcherStage::Hook).is_empty());
}

#[test]
fn project_config_detection_prefers_v2() {
    let root = Scratch::new("detect");
    let empty = root.dir("empty");
    assert_eq!(detect_project_config(&empty).kind, ProjectConfigKind::None);

    let legacy = root.dir("legacy");
    std::fs::write(legacy.join("ncpatcher.json"), "{}\n").unwrap();
    assert_eq!(detect_project_config(&legacy).kind, ProjectConfigKind::V1Json);

    let modern = root.dir("modern");
    std::fs::write(modern.join("ncpatcher.yaml"), "version: 2\n").unwrap();
    assert_eq!(detect_project_config(&modern).kind, ProjectConfigKind::V2Yaml);
    assert_eq!(detect_project_config(&modern).file, modern.join("ncpatcher.yaml"));

    // A project mid-migration holds both. The v2 file is the one that counts.
    std::fs::write(modern.join("ncpatcher.json"), "{}\n").unwrap();
    assert_eq!(detect_project_config(&modern).kind, ProjectConfigKind::V2Yaml);

    let yml = root.dir("yml");
    std::fs::write(yml.join("ncpatcher.yml"), "version: 2\n").unwrap();
    assert_eq!(detect_project_config(&yml).kind, ProjectConfigKind::V2Yaml);
}

#[test]
fn build_refuses_a_legacy_or_missing_project_before_launching() {
    let root = Scratch::new("legacy");
    std::fs::write(root.0.join("ncpatcher.json"), "{}\n").unwrap();
    let client = NCPatcherClient::new("big-star-no-such-tool-anywhere");
    let request = BuildRequest { project_dir: root.0.clone(), ..BuildRequest::default() };
    assert_eq!(client.build(&request, &mut BuildObserver::default()).unwrap_err().code, ErrorCode::Unsupported);

    let missing = BuildRequest { project_dir: std::env::temp_dir().join("big-star-no-project-here"), ..BuildRequest::default() };
    assert_eq!(client.build(&missing, &mut BuildObserver::default()).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn a_variant_builds_beside_the_configured_output() {
    let config = ProjectConfig { rom_file: "/p/rom.nds".into(), rom_output: "/p/build/nds/rom.nds".into(), ..ProjectConfig::default() };
    assert_eq!(config.build_output(""), Path::new("/p/build/nds/rom.nds"));
    assert_eq!(config.build_output("en"), Path::new("/p/build/nds/rom_en.nds"));
    // With no output the input ROM's name is the base, as NCPatcher has it.
    let in_place = ProjectConfig { rom_file: "/roms/clean.nds".into(), ..ProjectConfig::default() };
    assert_eq!(in_place.build_output("fr"), Path::new("/roms/clean_fr.nds"));
    assert_eq!(ProjectConfig::default().build_output("fr"), Path::new(""));
}
