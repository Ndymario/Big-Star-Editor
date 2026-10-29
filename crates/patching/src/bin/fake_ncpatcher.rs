//! A stand-in for NCPatcher, so the editor's own behaviour can be tested without a toolchain, a ROM, or a real
//! project to build. Port of `apps/nsmb-qt/tests/fake_ncpatcher.cpp`.
//!
//! It answers a subcommand by printing a file the test wrote next to the project:
//! `<project>/fake-ncpatcher/<command>.json`, where `<command>` is `config-dump`, `modules-dump`, `files-plan` or
//! `rom-files`; a plan scoped with `--from-module` or `--below-module` reads `files-plan-from.json` or
//! `files-plan-below.json`, and with neither staged, refuses the option as an NCPatcher before 2.1 does. A command
//! with no file behind it exits 3, which is what NCPatcher does when it cannot make sense of
//! the project -- so "the editor was not given an answer" is a case the tests can set up as easily as any other.
//! The `--rom` a command was given is recorded in `<project>/fake-ncpatcher/rom.txt`, and removed when none was.

use std::io::Write;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let has = |word: &str| args.iter().any(|arg| arg == word);

    let project = args
        .windows(2)
        .filter(|pair| matches!(pair, [flag, _] if flag == "-C"))
        .filter_map(|pair| pair.get(1))
        .next_back()
        .cloned()
        .unwrap_or_default();

    if !project.is_empty() {
        let record = format!("{project}/fake-ncpatcher/rom.txt");
        match args.windows(2).find_map(|pair| matches!(pair, [flag, _] if flag == "--rom").then(|| pair.get(1)).flatten()) {
            Some(rom) => drop(std::fs::write(record, rom)),
            None => drop(std::fs::remove_file(record)),
        }
    }

    if has("version") {
        println!("ncpatcher 2.0.0 (fake)");
        return ExitCode::SUCCESS;
    }
    let command = if has("config") && has("dump") {
        "config-dump"
    } else if has("modules") && has("dump") {
        "modules-dump"
    } else if has("files") && has("plan") {
        let scope = [("--from-module", "files-plan-from"), ("--below-module", "files-plan-below")].into_iter().find(|(flag, _)| has(flag));
        match scope {
            Some((flag, command)) if !std::path::Path::new(&format!("{project}/fake-ncpatcher/{command}.json")).is_file() => {
                eprintln!("The following argument was not expected: {flag}");
                return ExitCode::from(2);
            }
            Some((_, command)) => command,
            None => "files-plan",
        }
    } else if has("rom") && has("files") {
        "rom-files"
    } else {
        eprintln!("fake ncpatcher: unrecognised command");
        return ExitCode::from(2);
    };

    match std::fs::read(format!("{project}/fake-ncpatcher/{command}.json")) {
        Ok(bytes) => {
            let _ = std::io::stdout().write_all(&bytes);
            ExitCode::SUCCESS
        }
        Err(_) => {
            eprintln!("fake ncpatcher: nothing staged for `{command}`");
            ExitCode::from(3)
        }
    }
}
