use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use bigstar_core::read_file_prefix;
use bigstar_host::{Host, HostOptions, stored_clean_rom};
use bigstar_nds::{DEFAULT_HEADER_REGION_SIZE, parse_rom_header};
use bigstar_patching::{BuildObserver, BuildRequest, NCPatcherClient, describe_stage};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PORT: u16 = 7878;

use crate::server;

fn usage(out: &mut dyn Write) {
    let _ = write!(
        out,
        "bigstar-host {VERSION}\nCommands:\n  --version\n  inspect-rom <path>\n  run-patcher <project-dir> [--variant <name>] [--rom <clean-rom>]\n  patcher-info <project-dir> [--rom <clean-rom>]\n  oracle <rom> <data-dir>\n  serve [<root>] [--rom-sessions] [--bind <address>] [--port <port>] [--data <dir>] [--ncpatcher <path>] [--xdelta <path>]\n"
    );
}

fn inspect_rom(path: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let data = match read_file_prefix(path, DEFAULT_HEADER_REGION_SIZE) {
        Ok(data) => data,
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            return 2;
        }
    };
    match parse_rom_header(&data) {
        Ok(header) => {
            let _ = write!(out, "Title: {}\nGame code: {}\n", header.game_title, header.game_code);
            0
        }
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            3
        }
    }
}

const NCPATCHER: &str = "ncpatcher";

/// NCPatcher for `project_dir`, handed `rom`, or else the clean ROM the editor remembers for the project:
/// `ncpatcher.yaml` names none.
fn client_for(executable: &str, project_dir: &Path, rom: Option<&Path>) -> NCPatcherClient {
    let client = NCPatcherClient::new(executable);
    match rom.map(Path::to_path_buf).or_else(|| stored_clean_rom(project_dir)) {
        Some(rom) => client.with_rom(rom),
        None => client,
    }
}

/// `run-patcher <project-dir> [--variant <name>] [--rom <clean-rom>]`: one NCPatcher build, its progress and
/// diagnostics on stderr. A project that configures variants requires an explicit one, so NCPatcher cannot silently
/// build the wrong language.
fn run_patcher(executable: &str, project_dir: &Path, variant: &str, rom: Option<&Path>, err: &mut dyn Write) -> i32 {
    let client = client_for(executable, project_dir, rom).with_logger(std::sync::Arc::new(|level, message| {
        let name = match level {
            log::Level::Error => "error",
            log::Level::Warn => "warning",
            _ => "info",
        };
        eprintln!("[{name}] {message}");
    }));
    let mut observer = BuildObserver {
        on_progress: Some(Box::new(|progress| eprintln!("{} {}/{}  {}", progress.phase, progress.current, progress.total, progress.item))),
        on_diagnostic: Some(Box::new(|diagnostic| eprintln!("{}: {}", diagnostic.code, diagnostic.message))),
        on_log: Some(Box::new(|line| eprintln!("{line}"))),
        ..BuildObserver::default()
    };
    let request = BuildRequest { project_dir: project_dir.to_path_buf(), variant: variant.to_owned(), ..BuildRequest::default() };
    let summary = match client.build(&request, &mut observer) {
        Ok(summary) => summary,
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            return 10;
        }
    };
    if !summary.succeeded() {
        let _ = writeln!(err, "NCPatcher: {} ({} error(s), {} warning(s))", describe_stage(summary.stage), summary.errors, summary.warnings);
    }
    summary.exit_code
}

/// `patcher-info <project-dir> [--rom <clean-rom>]`: what the project says about itself, without building it.
fn patcher_info(executable: &str, project_dir: &Path, rom: Option<&Path>, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let client = client_for(executable, project_dir, rom);
    let version = match client.version() {
        Ok(version) => version,
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            return 10;
        }
    };
    let _ = writeln!(out, "{version}");
    let config = match client.dump_config(project_dir) {
        Ok(config) => config,
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            return 10;
        }
    };
    let _ = write!(
        out,
        "Config: \"{}\"\nROM: \"{}\"\nToolchain: {}\nVariants: ",
        config.config_file.display(),
        config.rom_file.display(),
        config.toolchain
    );
    if config.variants.is_empty() {
        let _ = write!(out, "(this project configures none)");
    }
    for variant in &config.variants {
        let _ = write!(out, "{} ", variant.name);
    }
    let _ = writeln!(out);
    if let Ok(graph) = client.dump_modules(project_dir) {
        let _ = writeln!(out, "Modules ({}):", graph.modules.len());
        for module in &graph.modules {
            let _ = write!(out, "  {}{}", module.key, if module.enabled { "" } else { " (disabled)" });
            if let Some(nitrofs) = &module.nitrofs {
                let _ = write!(out, " nitrofs=\"{}\"{}", nitrofs.dir.display(), if nitrofs.layered { " [layered]" } else { "" });
            }
            let _ = writeln!(out);
        }
    }
    0
}

/// `serve [<root>] [--rom-sessions] [--bind <address>] [--port <port>] [--data <dir>] [--ncpatcher <path>] [--xdelta <path>]`: hosts a
/// workspace until interrupted. `--rom-sessions` also lets in a browser without the token, to upload a ROM and edit that and nothing
/// else; without a root it is all the host does. `--data` is where the shipped actor, tile and background scripts are, `--ncpatcher`
/// which NCPatcher builds and is asked about variants, modules and plans, `--xdelta` which xdelta3 the patch tools run.
fn serve(args: &[&str], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let (root, flags) = match args.split_first() {
        Some((first, rest)) if !first.starts_with("--") => (Some(*first), rest),
        _ => (None, args),
    };
    let (mut bind, mut port) = ("127.0.0.1".to_owned(), DEFAULT_PORT);
    let mut options = HostOptions::default();
    let mut rom_sessions = false;
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
        if *flag == "--rom-sessions" {
            rom_sessions = true;
            continue;
        }
        let value = flags.next();
        match (*flag, value) {
            ("--bind", Some(value)) => bind = (*value).to_owned(),
            ("--data", Some(value)) => options.data_dir = PathBuf::from(value),
            ("--ncpatcher", Some(value)) => options.ncpatcher = PathBuf::from(value),
            ("--xdelta", Some(value)) => options.xdelta = PathBuf::from(value),
            ("--port", Some(value)) => match value.parse::<u16>() {
                Ok(parsed) => port = parsed,
                Err(_) => {
                    let _ = writeln!(err, "--port needs a number from 0 to 65535");
                    return 1;
                }
            },
            _ => {
                usage(err);
                return 1;
            }
        }
    }
    let address = match format!("{bind}:{port}").parse::<SocketAddr>().or_else(|_| format!("[{bind}]:{port}").parse()) {
        Ok(address) => address,
        Err(_) => {
            let _ = writeln!(err, "\"{bind}\" is not an IP address");
            return 1;
        }
    };
    let host = match root {
        Some(root) => match Host::with_options(Path::new(root), options) {
            Ok(host) => host,
            Err(e) => {
                let _ = writeln!(err, "{}", e.message);
                return 2;
            }
        },
        None if rom_sessions => Host::for_uploads(options),
        None => {
            usage(err);
            return 1;
        }
    };
    let token = match server::generate_token() {
        Ok(token) => token,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            return 2;
        }
    };
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(e) => {
            let _ = writeln!(err, "could not start the runtime: {e}");
            return 2;
        }
    };
    runtime.block_on(async {
        let (listener, bound) = match server::bind(address).await {
            Ok(bound) => bound,
            Err(e) => {
                let _ = writeln!(err, "could not listen on {address}: {e}");
                return 2;
            }
        };
        if host.has_workspace() {
            let _ = writeln!(out, "Serving \"{}\" on ws://{bound}/ws", host.workspace_name());
            let _ = writeln!(out, "Token: {token}");
            if server::has_web_bundle() {
                let _ = writeln!(out, "Open http://{bound}/?token={token} in a browser");
            }
            if rom_sessions && server::has_web_bundle() {
                let _ = writeln!(out, "Without the token, http://{bound}/ edits an uploaded ROM and nothing of the workspace");
            }
        } else {
            let _ = writeln!(out, "Serving uploaded ROMs on ws://{bound}/ws");
            if server::has_web_bundle() {
                let _ = writeln!(out, "Open http://{bound}/ in a browser");
            }
        }
        if !bound.ip().is_loopback() {
            let _ = writeln!(err, "warning: {bound} is not a loopback address; the connection is not encrypted");
        }
        let _ = out.flush();
        tokio::select! {
            result = server::serve(listener, host, token, rom_sessions) => match result {
                Ok(()) => 0,
                Err(e) => {
                    let _ = writeln!(err, "the server stopped: {e}");
                    2
                }
            },
            _ = tokio::signal::ctrl_c() => 0,
        }
    })
}

/// `--variant <name>` (when `variant` is allowed) and `--rom <path>`, each at most once; `None` for anything else.
fn patcher_options<'a>(words: &[&'a str], variant: bool) -> Option<(&'a str, Option<&'a Path>)> {
    let (mut chosen, mut rom) = (None, None);
    for pair in words.chunks(2) {
        match pair {
            ["--variant", name] if variant && chosen.is_none() => chosen = Some(*name),
            ["--rom", path] if rom.is_none() => rom = Some(Path::new(*path)),
            _ => return None,
        }
    }
    Some((chosen.unwrap_or_default(), rom))
}

/// Runs one invocation (without the program name) and returns the process exit code.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["--version"] => {
            let _ = writeln!(out, "bigstar-host {VERSION}");
            0
        }
        ["inspect-rom", path] => inspect_rom(Path::new(path), out, err),
        ["run-patcher", dir, rest @ ..] => match patcher_options(rest, true) {
            Some((variant, rom)) => run_patcher(NCPATCHER, Path::new(dir), variant, rom, err),
            None => {
                usage(out);
                2
            }
        },
        ["patcher-info", dir, rest @ ..] => match patcher_options(rest, false) {
            Some((_, rom)) => patcher_info(NCPATCHER, Path::new(dir), rom, out, err),
            None => {
                usage(out);
                2
            }
        },
        ["oracle", rom, data] => crate::oracle::oracle(Path::new(rom), Path::new(data), out, err),
        ["serve", rest @ ..] => serve(rest, out, err),
        _ => {
            usage(out);
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use bigstar_project::test_support::{RomFile, TempDir, make_rom, write_bytes};

    use super::run;

    fn invoke(args: &[&str]) -> (i32, String, String) {
        let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(&args, &mut out, &mut err);
        (code, String::from_utf8_lossy(&out).into_owned(), String::from_utf8_lossy(&err).into_owned())
    }

    fn sample_rom() -> Vec<u8> {
        make_rom(&[RomFile::new("a/title_ncg.bin", b"x"), RomFile::new("a/other.bin", b"y"), RomFile::new("b/map.nscr", b"z")], &[])
    }

    #[test]
    fn version_and_usage() {
        let (code, out, _) = invoke(&["--version"]);
        assert_eq!((code, out), (0, format!("bigstar-host {}\n", env!("CARGO_PKG_VERSION"))));
        let (code, out, _) = invoke(&[]);
        assert_eq!(code, 1);
        assert!(out.contains("patcher-info <project-dir> [--rom <clean-rom>]"));
        assert!(!out.contains("import-rom"));
        assert_eq!(invoke(&["inspect-rom"]).0, 1);
    }

    #[test]
    fn patcher_verbs_check_their_arguments_and_report_a_missing_tool() {
        let (code, out, _) = invoke(&["run-patcher", "p", "--colour", "fr"]);
        assert_eq!(code, 2);
        assert!(out.contains("run-patcher <project-dir> [--variant <name>] [--rom <clean-rom>]"));
        assert_eq!(invoke(&["run-patcher"]).0, 1);
        assert_eq!(invoke(&["patcher-info"]).0, 1);
        assert_eq!(invoke(&["patcher-info", "p", "--variant", "fr"]).0, 2, "patcher-info builds nothing, so it takes no variant");
        assert_eq!(invoke(&["run-patcher", "p", "--rom", "a.nds", "--rom", "b.nds"]).0, 2);
        assert_eq!(super::patcher_options(&["--rom", "c.nds", "--variant", "fr"], true), Some(("fr", Some(std::path::Path::new("c.nds")))));
        assert_eq!(super::patcher_options(&[], true), Some(("", None)));

        let dir = TempDir::new("cli-patcher");
        std::fs::write(dir.path().join("ncpatcher.yaml"), "version: 2\n").unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(super::patcher_info("big-star-no-such-tool-anywhere", dir.path(), None, &mut out, &mut err), 10);
        assert!(String::from_utf8_lossy(&err).contains("could not run"));
        assert_eq!(super::run_patcher("big-star-no-such-tool-anywhere", dir.path(), "fr", None, &mut err), 10);
        // No configuration at all is a missing project, reported before anything is launched.
        let empty = TempDir::new("cli-patcher-empty");
        let mut err = Vec::new();
        assert_eq!(super::run_patcher("big-star-no-such-tool-anywhere", empty.path(), "", None, &mut err), 10);
        assert!(String::from_utf8_lossy(&err).contains("no ncpatcher.yaml"));
    }

    #[test]
    fn oracle_reports_every_file_and_actor() {
        let dir = TempDir::new("cli-oracle");
        let rom = dir.path().join("source.nds");
        write_bytes(&rom, &sample_rom());
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (code, out, err) = invoke(&["oracle", &rom.to_string_lossy(), &data.to_string_lossy()]);
        assert_eq!(code, 0, "{err}");
        assert!(out.starts_with("rom A2DE files=3\n"), "{out}");
        assert!(out.contains(" b/map.nscr graphics size=1\n"), "{out}");
        assert_eq!(out.lines().filter(|line| line.starts_with("actor ")).count(), 305);
        assert_eq!(invoke(&["oracle", &dir.path().join("none.nds").to_string_lossy(), &data.to_string_lossy()]).0, 2);
    }

    #[test]
    fn serve_rejects_bad_arguments_before_listening() {
        let dir = TempDir::new("cli-serve");
        let root = dir.path().to_string_lossy().into_owned();
        assert_eq!(invoke(&["serve"]).0, 1);
        // Without a root there is nothing to serve but uploaded ROMs, which has to be asked for.
        assert_eq!(invoke(&["serve", "--port", "0"]).0, 1);
        assert_eq!(invoke(&["serve", "--rom-sessions", "--port", "99999"]).0, 1);
        assert_eq!(invoke(&["serve", &root, "--rom-sessions", "--bind", "not-an-address"]).0, 1);
        assert_eq!(invoke(&["serve", &root, "--port", "99999"]).0, 1);
        assert_eq!(invoke(&["serve", &root, "--port"]).0, 1);
        assert_eq!(invoke(&["serve", &root, "--bind", "not-an-address"]).0, 1);
        assert_eq!(invoke(&["serve", &root, "--colour", "red"]).0, 1);
        let (code, _, err) = invoke(&["serve", &format!("{root}/missing")]);
        assert_eq!(code, 2);
        assert!(err.contains("does not exist"), "{err}");
    }

    #[test]
    fn inspect_rom_prints_the_header() {
        let dir = TempDir::new("cli-inspect");
        let rom = dir.path().join("x.nds");
        write_bytes(&rom, &sample_rom());
        let (code, out, _) = invoke(&["inspect-rom", &rom.to_string_lossy()]);
        assert_eq!((code, out.as_str()), (0, "Title: READER TEST\nGame code: A2DE\n"));
        assert_eq!(invoke(&["inspect-rom", &dir.path().join("none.nds").to_string_lossy()]).0, 2);
        let junk = dir.path().join("junk.nds");
        write_bytes(&junk, b"short");
        assert_eq!(invoke(&["inspect-rom", &junk.to_string_lossy()]).0, 3);
    }
}
