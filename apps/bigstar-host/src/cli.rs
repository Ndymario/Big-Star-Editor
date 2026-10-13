use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use bigstar_core::read_file_prefix;
use bigstar_formats::{GraphicsFormatKind, identify_graphics_format};
use bigstar_host::{Host, HostOptions};
use bigstar_nds::{DEFAULT_HEADER_REGION_SIZE, RomHeader, parse_rom_header};
use bigstar_patching::{BuildObserver, BuildRequest, NCPatcherClient, describe_stage};
use bigstar_project::{INPUT_ROM_FILE_NAME, MANIFEST_FILE_NAME, Project, create_empty_project, index_rom_files, save_manifest};

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DEFAULT_PORT: u16 = 7878;

use crate::server;

fn usage(out: &mut dyn Write) {
    let _ = write!(
        out,
        "bigstar-host {VERSION}\nCommands:\n  --version\n  inspect-rom <path>\n  import-rom <rom> <project-dir>\n  run-patcher <project-dir> [--variant <name>]\n  patcher-info <project-dir>\n  oracle <project-dir> <data-dir>\n  serve <root> [--bind <address>] [--port <port>] [--data <dir>] [--ncpatcher <path>] [--xdelta <path>]\n"
    );
}

fn read_header(path: &Path) -> Result<RomHeader, String> {
    let data = read_file_prefix(path, DEFAULT_HEADER_REGION_SIZE).map_err(|e| e.message)?;
    parse_rom_header(&data).map_err(|e| e.message)
}

fn count_graphics_candidates(project: &Project) -> usize {
    project
        .game_files()
        .iter()
        .filter(|file| identify_graphics_format(&[], &file.path.to_string_lossy().replace('\\', "/")) != GraphicsFormatKind::Unknown)
        .count()
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

fn import_rom(rom_path: &Path, project_dir: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    if let Err(e) = std::fs::create_dir_all(project_dir) {
        let _ = writeln!(err, "Could not create project directory: {e}");
        return 4;
    }
    let manifest = project_dir.join(MANIFEST_FILE_NAME);
    match manifest.try_exists() {
        Err(e) => {
            let _ = writeln!(err, "Could not inspect project directory: {e}");
            return 4;
        }
        Ok(true) => {
            let _ = writeln!(err, "Project already exists: \"{}\"", manifest.display());
            return 4;
        }
        Ok(false) => {}
    }

    let header = match read_header(rom_path) {
        Ok(header) => header,
        Err(message) => {
            let _ = writeln!(err, "{message}");
            // Unreadable and unparseable are different failures, as they are in `inspect-rom`.
            return if read_file_prefix(rom_path, 1).is_err() { 5 } else { 6 };
        }
    };

    // The ROM is read where it lies rather than unpacked, so importing one is copying it in and reading its table --
    // and a manifest path has to stay inside the project, so a ROM from elsewhere is copied rather than pointed at.
    let imported = project_dir.join(INPUT_ROM_FILE_NAME);
    let same_file = match (std::fs::canonicalize(rom_path), std::fs::canonicalize(&imported)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same_file && let Err(e) = std::fs::copy(rom_path, &imported) {
        let _ = writeln!(err, "Could not copy the ROM into the project: {e}");
        return 7;
    }

    let mut project_name = project_dir.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    if project_name.is_empty() || project_name == "." {
        project_name = if header.game_title.is_empty() { "Imported ROM".to_owned() } else { header.game_title.clone() };
    }
    let mut project = match create_empty_project(project_name, project_dir) {
        Ok(project) => project,
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            return 8;
        }
    };
    project.attach_rom(PathBuf::from(INPUT_ROM_FILE_NAME), Some(header));
    if let Err(e) = index_rom_files(&mut project).and_then(|()| save_manifest(&project, &manifest)) {
        let _ = writeln!(err, "{}", e.message);
        return 8;
    }

    let _ = write!(
        out,
        "Imported ROM into \"{}\"\nFiles indexed: {}\nGraphics candidates: {}\n",
        project_dir.display(),
        project.game_files().len(),
        count_graphics_candidates(&project)
    );
    0
}

const NCPATCHER: &str = "ncpatcher";

/// `run-patcher <project-dir> [--variant <name>]`: one NCPatcher build, its progress and diagnostics on stderr. A
/// project that configures variants requires an explicit one, so NCPatcher cannot silently build the wrong language.
fn run_patcher(executable: &str, project_dir: &Path, variant: &str, err: &mut dyn Write) -> i32 {
    let client = NCPatcherClient::new(executable).with_logger(std::sync::Arc::new(|level, message| {
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

/// `patcher-info <project-dir>`: what the project says about itself, without building it.
fn patcher_info(executable: &str, project_dir: &Path, out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let client = NCPatcherClient::new(executable);
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

/// `serve <root> [--bind <address>] [--port <port>] [--data <dir>] [--ncpatcher <path>] [--xdelta <path>]`: hosts a workspace until
/// interrupted. `--data` is where the shipped actor and tile scripts are, `--ncpatcher` which NCPatcher builds and is asked about
/// variants, modules and plans, `--xdelta` which xdelta3 the patch tools run.
fn serve(args: &[&str], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let Some((root, flags)) = args.split_first() else {
        usage(err);
        return 1;
    };
    let (mut bind, mut port) = ("127.0.0.1".to_owned(), DEFAULT_PORT);
    let mut options = HostOptions::default();
    let mut flags = flags.iter();
    while let Some(flag) = flags.next() {
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
    let host = match Host::with_options(Path::new(root), options) {
        Ok(host) => host,
        Err(e) => {
            let _ = writeln!(err, "{}", e.message);
            return 2;
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
        let _ = writeln!(out, "Serving \"{}\" on ws://{bound}/ws", host.workspace_name());
        let _ = writeln!(out, "Token: {token}");
        if server::has_web_bundle() {
            let _ = writeln!(out, "Open http://{bound}/?token={token} in a browser");
        }
        if !bound.ip().is_loopback() {
            let _ = writeln!(err, "warning: {bound} is not a loopback address; the connection is not encrypted");
        }
        let _ = out.flush();
        tokio::select! {
            result = server::serve(listener, host, token) => match result {
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

/// Runs one invocation (without the program name) and returns the process exit code.
pub fn run(args: &[String], out: &mut dyn Write, err: &mut dyn Write) -> i32 {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["--version"] => {
            let _ = writeln!(out, "bigstar-host {VERSION}");
            0
        }
        ["inspect-rom", path] => inspect_rom(Path::new(path), out, err),
        ["import-rom", rom, dir] => import_rom(Path::new(rom), Path::new(dir), out, err),
        ["run-patcher", dir] => run_patcher(NCPATCHER, Path::new(dir), "", err),
        ["run-patcher", dir, "--variant", variant] => run_patcher(NCPATCHER, Path::new(dir), variant, err),
        ["run-patcher", _, _, _] => {
            usage(out);
            2
        }
        ["patcher-info", dir] => patcher_info(NCPATCHER, Path::new(dir), out, err),
        ["oracle", dir, data] => crate::oracle::oracle(Path::new(dir), Path::new(data), out, err),
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
    use bigstar_project::{MANIFEST_FILE_NAME, load_manifest};

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
        assert!(out.contains("import-rom <rom> <project-dir>"));
        assert_eq!(invoke(&["inspect-rom"]).0, 1);
    }

    #[test]
    fn patcher_verbs_check_their_arguments_and_report_a_missing_tool() {
        let (code, out, _) = invoke(&["run-patcher", "p", "--colour", "fr"]);
        assert_eq!(code, 2);
        assert!(out.contains("run-patcher <project-dir> [--variant <name>]"));
        assert_eq!(invoke(&["run-patcher"]).0, 1);
        assert_eq!(invoke(&["patcher-info"]).0, 1);

        let dir = TempDir::new("cli-patcher");
        std::fs::write(dir.path().join("ncpatcher.yaml"), "version: 2\n").unwrap();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(super::patcher_info("big-star-no-such-tool-anywhere", dir.path(), &mut out, &mut err), 10);
        assert!(String::from_utf8_lossy(&err).contains("could not run"));
        assert_eq!(super::run_patcher("big-star-no-such-tool-anywhere", dir.path(), "fr", &mut err), 10);
        // No configuration at all is a missing project, reported before anything is launched.
        let empty = TempDir::new("cli-patcher-empty");
        let mut err = Vec::new();
        assert_eq!(super::run_patcher("big-star-no-such-tool-anywhere", empty.path(), "", &mut err), 10);
        assert!(String::from_utf8_lossy(&err).contains("no ncpatcher.yaml"));
    }

    #[test]
    fn oracle_reports_every_file_and_actor() {
        let dir = TempDir::new("cli-oracle");
        let rom = dir.path().join("source.nds");
        write_bytes(&rom, &sample_rom());
        let project = dir.path().join("Oracle");
        assert_eq!(invoke(&["import-rom", &rom.to_string_lossy(), &project.to_string_lossy()]).0, 0);
        let data = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
        let (code, out, err) = invoke(&["oracle", &project.to_string_lossy(), &data.to_string_lossy()]);
        assert_eq!(code, 0, "{err}");
        assert!(out.starts_with("rom A2DE files=3\n"), "{out}");
        assert!(out.contains(" b/map.nscr graphics size=1\n"), "{out}");
        assert_eq!(out.lines().filter(|line| line.starts_with("actor ")).count(), 305);
        assert_eq!(invoke(&["oracle", &dir.path().join("none").to_string_lossy(), &data.to_string_lossy()]).0, 2);
    }

    #[test]
    fn serve_rejects_bad_arguments_before_listening() {
        let dir = TempDir::new("cli-serve");
        let root = dir.path().to_string_lossy().into_owned();
        assert_eq!(invoke(&["serve"]).0, 1);
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

    #[test]
    fn import_rom_creates_a_project_once() {
        let dir = TempDir::new("cli-import");
        let rom = dir.path().join("source.nds");
        write_bytes(&rom, &sample_rom());
        let project = dir.path().join("Imported");
        let (code, out, err) = invoke(&["import-rom", &rom.to_string_lossy(), &project.to_string_lossy()]);
        assert_eq!(code, 0, "{err}");
        assert!(out.contains("Files indexed: 3\n"));
        assert!(out.contains("Graphics candidates: 2\n"));
        assert!(project.join("rom.nds").is_file());

        let loaded = load_manifest(&project.join(MANIFEST_FILE_NAME)).unwrap();
        assert_eq!(loaded.name(), "Imported");
        assert_eq!(loaded.game_files().len(), 3);
        assert_eq!(loaded.rom().unwrap().header.as_ref().unwrap().game_code, "A2DE");

        let (code, _, err) = invoke(&["import-rom", &rom.to_string_lossy(), &project.to_string_lossy()]);
        assert_eq!(code, 4);
        assert!(err.contains("Project already exists"));

        let (code, ..) = invoke(&["import-rom", &dir.path().join("none.nds").to_string_lossy(), &dir.path().join("Other").to_string_lossy()]);
        assert_eq!(code, 5);
    }
}
