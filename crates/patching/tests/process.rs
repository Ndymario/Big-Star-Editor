//! The process runner against a real child: this test binary re-runs itself with a fixture argument, as
//! `patching_tests.cpp` does, so nothing depends on which shells or utilities the host has. The client tests at the
//! end drive `bigstar-fake-ncpatcher`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bigstar_core::ErrorCode;
use bigstar_patching::{BuildObserver, BuildRequest, NCPatcherClient, PlanScope, ProcessCancel, ProcessRequest, run_process};

fn self_path() -> PathBuf {
    std::env::current_exe().unwrap()
}

fn request(arguments: &[&str]) -> ProcessRequest {
    ProcessRequest { executable: self_path(), arguments: arguments.iter().map(|a| (*a).to_owned()).collect(), ..ProcessRequest::default() }
}

fn stdout_lines(request: &ProcessRequest) -> (bigstar_core::Result<bigstar_patching::ProcessResult>, Vec<String>) {
    let mut lines = Vec::new();
    let mut on_out = |line: &str| lines.push(line.to_owned());
    let result = run_process(request, Some(&mut on_out), None);
    (result, lines)
}

fn runs_and_captures_both_streams() {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut on_out = |line: &str| out.push(line.to_owned());
    let mut on_err = |line: &str| err.push(line.to_owned());
    let result = run_process(&request(&["--child-fixture"]), Some(&mut on_out), Some(&mut on_err)).unwrap();
    // Exit codes survive: NCPatcher distinguishes configuration (3) from compilation (6) from a failed hook (10).
    assert_eq!(result.exit_code, 7);
    assert!(!result.terminated_by_signal);
    // Both streams are captured, and separately. A final line with no terminator is still delivered.
    assert_eq!(out, ["stdout one", "stdout two", "no trailing newline"]);
    assert_eq!(err, ["stderr one"]);
}

fn arguments_are_not_shell_interpreted() {
    let hostile = ["$(touch /tmp/big-star-should-not-exist)", "; rm -rf /", "`id`", "a b\tc", "quote\"and'quote", "*"];
    let mut arguments = vec!["--echo-args"];
    arguments.extend(hostile);
    let (result, lines) = stdout_lines(&request(&arguments));
    assert_eq!(result.unwrap().exit_code, 0);
    // Byte for byte, including the tab, which a shell would have split on.
    assert_eq!(lines, hostile);
    assert!(!Path::new("/tmp/big-star-should-not-exist").exists());
}

fn working_directory_is_child_only() {
    let before = std::env::current_dir().unwrap();
    let elsewhere = std::env::temp_dir();
    let mut child = request(&["--print-cwd"]);
    child.working_directory = elsewhere.clone();
    let (result, lines) = stdout_lines(&child);
    assert_eq!(result.unwrap().exit_code, 0);
    assert_eq!(lines.len(), 1);
    assert_eq!(std::fs::canonicalize(&lines[0]).unwrap(), std::fs::canonicalize(&elsewhere).unwrap());
    // The caller's own directory is untouched.
    assert_eq!(std::env::current_dir().unwrap(), before);
}

fn environment_overrides_reach_the_child() {
    let mut child = request(&["--print-env", "BIG_STAR_TEST_VAR"]);
    child.environment = vec!["BIG_STAR_TEST_VAR=from-the-request".to_owned()];
    let (result, lines) = stdout_lines(&child);
    assert_eq!(result.unwrap().exit_code, 0);
    assert_eq!(lines, ["from-the-request"]);
}

fn a_run_can_be_cancelled() {
    // NCPatcher answers a signal to the process group by unwinding and exiting 11, which is what makes an
    // interrupted build safe to run again -- so the editor has to be able to send one.
    let cancel = Arc::new(ProcessCancel::new());
    let mut child = request(&["--wait-for-cancel"]);
    child.cancel = Some(cancel.clone());
    let ready = AtomicBool::new(false);
    let result = std::thread::scope(|scope| {
        scope.spawn(|| {
            while !ready.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            cancel.request();
        });
        let mut on_out = |line: &str| {
            if line == "ready" {
                ready.store(true, Ordering::SeqCst);
            }
        };
        run_process(&child, Some(&mut on_out), None)
    })
    .unwrap();
    assert!(cancel.requested());
    // 11 is the tool's own answer. A run killed outright would report a signal instead.
    assert!(!result.terminated_by_signal);
    assert_eq!(result.exit_code, 11);

    // The same token drives the next run, and a request meant for the run that just ended must not reach it.
    cancel.reset();
    assert!(!cancel.requested());

    // A cancel with nothing running is not an error.
    let idle = ProcessCancel::new();
    idle.request();
    assert!(idle.requested());
}

fn missing_executable_is_reported() {
    let missing = ProcessRequest { executable: PathBuf::from("big-star-no-such-tool-anywhere"), ..ProcessRequest::default() };
    let error = run_process(&missing, None, None).unwrap_err();
    assert!(matches!(error.code, ErrorCode::NotFound | ErrorCode::ExternalToolFailed));
}

fn missing_working_directory_is_reported() {
    let mut child = request(&["--print-cwd"]);
    child.working_directory = PathBuf::from("/big-star-no-such-directory-anywhere");
    assert!(run_process(&child, None, None).is_err());
}

/// A project directory with answers staged for the fake NCPatcher.
struct FakeProject(PathBuf);

impl FakeProject {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!("bigstar-fake-ncpatcher-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(path.join("fake-ncpatcher")).unwrap();
        Self(path)
    }

    fn stage(&self, command: &str, json: &str) {
        std::fs::write(self.0.join("fake-ncpatcher").join(format!("{command}.json")), json).unwrap();
    }
}

impl Drop for FakeProject {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn client_drives_the_fake_ncpatcher() {
    let client = NCPatcherClient::new(env!("CARGO_BIN_EXE_bigstar-fake-ncpatcher"));
    assert_eq!(client.version().unwrap(), "ncpatcher 2.0.0 (fake)");

    let project = FakeProject::new("client");
    // Nothing staged: NCPatcher's own "cannot make sense of the project" exit, reported with its stage.
    let error = client.dump_config(&project.0).unwrap_err();
    assert_eq!(error.code, ErrorCode::ExternalToolFailed);
    assert!(error.message.starts_with("the project configuration is invalid:\n"), "{}", error.message);

    project.stage("config-dump", r#"{"schema":"ncpatcher.config/1","version":2,"toolchain":"arm-none-eabi-","variants":{"en":{},"fr":{}}}"#);
    let config = client.dump_config(&project.0).unwrap();
    assert_eq!(config.variants.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["en", "fr"]);

    project.stage("modules-dump", r#"{"schema":"ncpatcher.modules/1","dir":"/p/modules","modules":[{"key":"coop","enabled":true}]}"#);
    assert!(client.dump_modules(&project.0).unwrap().find("coop").unwrap().enabled);
    let passed_rom = || std::fs::read_to_string(project.0.join("fake-ncpatcher/rom.txt")).ok();
    assert_eq!(passed_rom(), None, "a client with no clean ROM leaves it to the configuration");

    // The clean ROM rides along on every command that reads the project, unless one names another ROM.
    let clean = client.clone().with_rom("/roms/clean.nds");
    assert!(clean.dump_modules(&project.0).is_ok());
    assert_eq!(passed_rom().as_deref(), Some("/roms/clean.nds"));

    project.stage("rom-files", r#"{"schema":"ncpatcher.files/1","count":1,"files":[{"id":3,"path":"a.bin","size":4}]}"#);
    assert_eq!(client.rom_files(&project.0, Path::new("")).unwrap().files[0].id, 3);
    assert!(clean.rom_files(&project.0, Path::new("/roms/built.nds")).is_ok());
    assert_eq!(passed_rom().as_deref(), Some("/roms/built.nds"));

    // A plan that does not say it is one is refused; one that does is read.
    project.stage("files-plan", r#"{"schema":"ncpatcher.files/1","count":0,"files":[]}"#);
    assert_eq!(client.plan_files(&project.0, "fr", None).unwrap_err().code, ErrorCode::ParseError);
    project.stage("files-plan", r#"{"schema":"ncpatcher.files/1","planned":true,"count":0,"files":[]}"#);
    assert!(client.plan_files(&project.0, "fr", None).unwrap().planned);

    // A scoped plan is answered from its own document; an NCPatcher that does not know the option says to update.
    let below = PlanScope::BelowModule("coop".into());
    let error = client.plan_files(&project.0, "fr", Some(&below)).unwrap_err();
    assert_eq!(error.code, ErrorCode::Unsupported);
    assert!(error.message.contains("update NCPatcher"), "{}", error.message);
    project.stage("files-plan-below", r#"{"schema":"ncpatcher.files/1","planned":true,"count":1,"files":[{"id":1,"path":"a.bin","size":4}]}"#);
    assert_eq!(client.plan_files(&project.0, "fr", Some(&below)).unwrap().files.len(), 1);

    // `build` is not something the fake answers: it exits 2 and writes no result, which reads as a failed run.
    std::fs::write(project.0.join("ncpatcher.yaml"), "version: 2\n").unwrap();
    let mut logged = Vec::new();
    let summary = {
        let (sender, receiver) = std::sync::mpsc::channel();
        let mut observer = BuildObserver { on_log: Some(Box::new(move |line: &str| drop(sender.send(line.to_owned())))), ..BuildObserver::default() };
        let summary =
            client.build(&BuildRequest { project_dir: project.0.clone(), variant: "fr".into(), ..BuildRequest::default() }, &mut observer).unwrap();
        drop(observer);
        logged.extend(receiver);
        summary
    };
    assert_eq!(summary.exit_code, 2);
    assert!(!summary.succeeded());
    assert_eq!(logged, ["fake ncpatcher: unrecognised command"]);
}

fn run_child_fixture(arguments: &[String]) -> i32 {
    use std::io::Write;
    let mode = arguments[0].as_str();
    let mut out = std::io::stdout();
    match mode {
        "--child-fixture" => {
            out.write_all(b"stdout one\nstdout two\n").unwrap();
            out.flush().unwrap();
            let mut err = std::io::stderr();
            err.write_all(b"stderr one\n").unwrap();
            err.flush().unwrap();
            out.write_all(b"no trailing newline").unwrap();
            out.flush().unwrap();
            7
        }
        "--echo-args" => {
            for argument in &arguments[1..] {
                writeln!(out, "{argument}").unwrap();
            }
            0
        }
        "--print-cwd" => {
            writeln!(out, "{}", std::env::current_dir().unwrap().display()).unwrap();
            0
        }
        // Stands in for NCPatcher's own cancellation handling: acknowledge the signal, unwind, and report code 11
        // rather than dying where it stands.
        "--wait-for-cancel" => {
            ctrlc::set_handler(|| std::process::exit(11)).unwrap();
            writeln!(out, "ready").unwrap();
            out.flush().unwrap();
            std::thread::sleep(Duration::from_secs(10));
            0
        }
        "--print-env" => {
            let value = arguments.get(1).and_then(|name| std::env::var(name).ok()).unwrap_or_else(|| "<unset>".to_owned());
            writeln!(out, "{value}").unwrap();
            0
        }
        _ => 99,
    }
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    const FIXTURES: [&str; 5] = ["--child-fixture", "--echo-args", "--print-cwd", "--wait-for-cancel", "--print-env"];
    if arguments.first().is_some_and(|a| FIXTURES.contains(&a.as_str())) {
        std::process::exit(run_child_fixture(&arguments));
    }
    let tests: [(&str, fn()); 8] = [
        ("runs_and_captures_both_streams", runs_and_captures_both_streams),
        ("arguments_are_not_shell_interpreted", arguments_are_not_shell_interpreted),
        ("working_directory_is_child_only", working_directory_is_child_only),
        ("environment_overrides_reach_the_child", environment_overrides_reach_the_child),
        ("a_run_can_be_cancelled", a_run_can_be_cancelled),
        ("missing_executable_is_reported", missing_executable_is_reported),
        ("missing_working_directory_is_reported", missing_working_directory_is_reported),
        ("client_drives_the_fake_ncpatcher", client_drives_the_fake_ncpatcher),
    ];
    for (name, test) in tests {
        test();
        println!("test {name} ... ok");
    }
    println!("test result: ok. {} passed", tests.len());
}
