//! Transactional patch apply. A build that fails, or that produces something unusable, must leave the project exactly
//! as it was. These tests stand in for NCPatcher by re-running this test binary, so they need no toolchain and no ROM.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use bigstar_core::ErrorCode;
use bigstar_patching::BuildObserver;
use bigstar_project::test_support::{RomFile, make_rom};
use bigstar_project::{
    PatchApplyOutcome, PatchApplyRequest, RomReader, apply_patch_transactionally, built_rom_path, patch_backup_root, patch_staging_dir,
    prune_patch_backups,
};

const PREVIOUS_ROM: &str = "the ROM that was here before";

fn make_project(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("big-star-apply-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("ncpatcher.yaml"), "version: 2\n").unwrap();
    std::fs::write(root.join("big-star-build.nds"), PREVIOUS_ROM).unwrap();
    root
}

fn previous_rom_is_untouched(project: &Path) -> bool {
    std::fs::read(project.join("big-star-build.nds")).unwrap() == PREVIOUS_ROM.as_bytes()
}

fn request_for(project: &Path, mode: &str) -> PatchApplyRequest {
    // The stand-in reads its behaviour out of the argument vector NCPatcher would have received, so the real command
    // construction is exercised too.
    PatchApplyRequest {
        project_dir: project.to_path_buf(),
        ncpatcher_executable: std::env::current_exe().unwrap(),
        variant: mode.to_owned(),
        ..PatchApplyRequest::default()
    }
}

fn apply(request: &PatchApplyRequest) -> bigstar_core::Result<bigstar_project::PatchApplyResult> {
    apply_patch_transactionally(request, &mut BuildObserver::default())
}

fn build_failure_changes_nothing() {
    let project = make_project("build-failed");
    let result = apply(&request_for(&project, "fake-fail")).unwrap();
    assert_eq!(result.outcome, PatchApplyOutcome::BuildFailed);
    assert!(!result.summary.succeeded());
    assert_eq!(result.summary.exit_code, 6);
    assert!(result.built_rom.as_os_str().is_empty());
    // Nothing moved, and no staging directory left to confuse the next run.
    assert!(previous_rom_is_untouched(&project));
    assert!(!patch_staging_dir(&project).exists());
    let _ = std::fs::remove_dir_all(&project);
}

fn success_without_a_rom_is_rejected() {
    // NCPatcher exits zero but writes nothing. Trusting the exit code alone would leave the editor reading a ROM that
    // is not there.
    let project = make_project("no-rom");
    let result = apply(&request_for(&project, "fake-ok-no-rom")).unwrap();
    assert_eq!(result.outcome, PatchApplyOutcome::OutputRejected);
    assert!(!result.rejection.is_empty());
    assert!(previous_rom_is_untouched(&project));
    assert!(!patch_staging_dir(&project).exists());
    let _ = std::fs::remove_dir_all(&project);
}

fn unparseable_output_is_rejected() {
    let project = make_project("bad-rom");
    let result = apply(&request_for(&project, "fake-ok-bad-rom")).unwrap();
    assert_eq!(result.outcome, PatchApplyOutcome::OutputRejected);
    assert!(result.rejection.contains("NDS ROM") || result.rejection.contains("could not be"), "{}", result.rejection);
    assert!(previous_rom_is_untouched(&project));
    assert!(!patch_staging_dir(&project).exists());
    let _ = std::fs::remove_dir_all(&project);
}

fn successful_build_replaces_the_rom() {
    let project = make_project("applied");
    let result = apply(&request_for(&project, "fake-ok")).unwrap();
    assert_eq!(result.outcome, PatchApplyOutcome::Applied);
    assert!(result.summary.succeeded());
    // The whole of the apply: one ROM arriving where the editor reads it.
    assert_eq!(result.built_rom, built_rom_path(&project));
    assert!(!previous_rom_is_untouched(&project));
    let mut reopened = RomReader::open(&result.built_rom).unwrap();
    assert_eq!(reopened.read("course.bin").unwrap(), b"LEVEL");
    // Nothing was unpacked and nothing was moved aside.
    assert!(!project.join("extracted-rom").exists());
    assert!(!patch_backup_root(&project).exists());
    assert!(!patch_staging_dir(&project).exists());
    let _ = std::fs::remove_dir_all(&project);
}

fn backups_are_pruned_per_kind() {
    let project = std::env::temp_dir().join(format!("big-star-backup-pruning-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&project);
    let root = patch_backup_root(&project);
    // Names sort lexically, and a fixed-width timestamp makes that chronological.
    for minute in 0..12 {
        let directory = root.join(format!("levels-20260901-1200{minute:02}-000-4242"));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("course.bin"), "L").unwrap();
    }
    for minute in 0..3 {
        std::fs::create_dir_all(root.join(format!("import-20260901-11000{minute}-000-4242"))).unwrap();
    }
    assert_eq!(prune_patch_backups(&project, 10), 2);
    // The two oldest level saves went; every import stayed, because its own kind never reached the limit.
    assert!(!root.join("levels-20260901-120000-000-4242").exists());
    assert!(!root.join("levels-20260901-120001-000-4242").exists());
    assert!(root.join("levels-20260901-120002-000-4242/course.bin").exists());
    assert!(root.join("levels-20260901-120011-000-4242").exists());
    assert!(root.join("import-20260901-110000-000-4242").exists());
    assert_eq!(prune_patch_backups(&project, 10), 0);
    assert_eq!(prune_patch_backups(&std::env::temp_dir().join("big-star-no-such-project"), 10), 0);
    let _ = std::fs::remove_dir_all(&project);
}

fn legacy_and_missing_projects_are_refused() {
    let legacy = std::env::temp_dir().join(format!("big-star-apply-legacy-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&legacy);
    std::fs::create_dir_all(&legacy).unwrap();
    std::fs::write(legacy.join("ncpatcher.json"), "{}\n").unwrap();
    assert_eq!(apply(&request_for(&legacy, "fake-ok")).unwrap_err().code, ErrorCode::Unsupported);
    let _ = std::fs::remove_dir_all(&legacy);

    let missing = PatchApplyRequest { project_dir: std::env::temp_dir().join("big-star-apply-nowhere"), ..PatchApplyRequest::default() };
    assert_eq!(apply(&missing).unwrap_err().code, ErrorCode::NotFound);
}

/// Stands in for NCPatcher. Reads `--variant` for which behaviour to act out and `--out` for where a build would have
/// written, then writes the run result the client reads back.
fn fake_ncpatcher(arguments: &[String]) -> i32 {
    let value = |flag: &str| arguments.windows(2).find(|pair| pair[0] == flag).map(|pair| pair[1].clone()).unwrap_or_default();
    let (mode, out, result_path) = (value("--variant"), value("--out"), value("--result"));
    let ok = mode != "fake-fail";
    let exit_code = if ok { 0 } else { 6 };
    if !result_path.is_empty() {
        let status = if ok { "ok" } else { "error" };
        let errors = if ok { 0 } else { 1 };
        std::fs::write(
            &result_path,
            format!(
                r#"{{"schema":"ncpatcher.result/1","status":"{status}","exit-code":{exit_code},"duration-ms":1,"errors":{errors},"warnings":0,"diagnostics":[],"artifacts":[]}}"#
            ),
        )
        .unwrap();
    }
    if ok && !out.is_empty() && mode != "fake-ok-no-rom" {
        let out = Path::new(&out);
        std::fs::create_dir_all(out.parent().unwrap()).unwrap();
        if mode == "fake-ok-bad-rom" {
            std::fs::write(out, "this is not a ROM").unwrap();
        } else {
            std::fs::write(out, make_rom(&[RomFile::new("course.bin", b"LEVEL")], &[])).unwrap();
        }
    }
    exit_code
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    // Re-invoked as a stand-in for NCPatcher: the client always names the project first.
    if arguments.first().is_some_and(|a| a == "-C") {
        std::process::exit(fake_ncpatcher(&arguments));
    }
    let tests: [(&str, fn()); 6] = [
        ("build_failure_changes_nothing", build_failure_changes_nothing),
        ("success_without_a_rom_is_rejected", success_without_a_rom_is_rejected),
        ("unparseable_output_is_rejected", unparseable_output_is_rejected),
        ("successful_build_replaces_the_rom", successful_build_replaces_the_rom),
        ("legacy_and_missing_projects_are_refused", legacy_and_missing_projects_are_refused),
        ("backups_are_pruned_per_kind", backups_are_pruned_per_kind),
    ];
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    println!("test result: ok. {} passed", tests.len());
}
