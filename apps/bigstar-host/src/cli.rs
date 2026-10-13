use std::io::Write;
use std::path::{Path, PathBuf};

use bigstar_core::read_file_prefix;
use bigstar_formats::{GraphicsFormatKind, identify_graphics_format};
use bigstar_nds::{DEFAULT_HEADER_REGION_SIZE, RomHeader, parse_rom_header};
use bigstar_project::{INPUT_ROM_FILE_NAME, MANIFEST_FILE_NAME, Project, create_empty_project, index_rom_files, save_manifest};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn usage(out: &mut dyn Write) {
    let _ = write!(out, "bigstar-host {VERSION}\nCommands:\n  --version\n  inspect-rom <path>\n  import-rom <rom> <project-dir>\n");
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
