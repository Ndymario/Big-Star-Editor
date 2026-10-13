use std::path::{Path, PathBuf};

use bigstar_core::{Error, ErrorCode, Result, write_text_file_atomically};
use bigstar_nds::RomHeader;

use crate::paths::{contains_parent_reference, lexically_normal, to_generic_string};
use crate::project::{GameFile, Project};
use crate::rom_reader::RomReader;

pub const MANIFEST_FILE_NAME: &str = "big-star-project.yaml";

/// 1 carried a `files:` mirror of the extracted ROM; 2 dropped it and indexed the extracted tree on open; 3 names the
/// built ROM instead, because there is no extracted tree any more. All three load -- an older manifest is rewritten as
/// version 3 on the next save, and a version 2 `rom.extracted_path` is read and discarded rather than honoured, since
/// the directory it named is derived.
pub const MANIFEST_VERSION: u64 = 3;

/// What `rom.rom_path` defaults to. A build writes `big-star-build.nds`; before the first one the project's own input
/// ROM is the only thing to browse, which is better than an editor that shows nothing until something is compiled.
pub const BUILT_ROM_FILE_NAME: &str = "big-star-build.nds";
pub const INPUT_ROM_FILE_NAME: &str = "rom.nds";

fn parse_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

fn trim(value: &str) -> &str {
    value.trim_matches([' ', '\t', '\r', '\n'])
}

fn parse_scalar(value: &str) -> Result<String> {
    let value = trim(value);
    if !value.starts_with('"') {
        return Ok(value.to_owned());
    }
    let inner = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|_| value.len() >= 2)
        .ok_or_else(|| parse_error("unterminated quoted manifest value"))?;

    let mut decoded = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Err(parse_error("unescaped quote in manifest value")),
            '\\' => match chars.next() {
                None => return Err(parse_error("incomplete escape in manifest value")),
                Some('\\') => decoded.push('\\'),
                Some('"') => decoded.push('"'),
                Some('n') => decoded.push('\n'),
                Some('r') => decoded.push('\r'),
                Some('t') => decoded.push('\t'),
                Some(_) => return Err(parse_error("unsupported escape in manifest value")),
            },
            other => decoded.push(other),
        }
    }
    Ok(decoded)
}

fn yaml_scalar(key: &str, value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    format!("{key}: \"{escaped}\"\n")
}

fn parse_manifest_path(value: &str, field: &str) -> Result<PathBuf> {
    let text = parse_scalar(value)?;
    let bytes = text.as_bytes();
    let has_drive_prefix = bytes.first().is_some_and(u8::is_ascii_alphabetic) && bytes.get(1) == Some(&b':');
    let path = lexically_normal(Path::new(&text));
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || text.starts_with('/')
        || has_drive_prefix
        || text.contains('\\')
        || contains_parent_reference(&path)
    {
        return Err(parse_error(format!("{field} must be a project-relative path")));
    }
    Ok(path)
}

fn parse_size(value: &str) -> Result<u64> {
    trim(value).parse::<u64>().map_err(|_| parse_error("manifest file size is not a valid integer"))
}

pub fn default_rom_path(project_dir: &Path) -> PathBuf {
    if project_dir.join(BUILT_ROM_FILE_NAME).is_file() {
        PathBuf::from(BUILT_ROM_FILE_NAME)
    } else if project_dir.join(INPUT_ROM_FILE_NAME).is_file() {
        PathBuf::from(INPUT_ROM_FILE_NAME)
    } else {
        PathBuf::from(BUILT_ROM_FILE_NAME)
    }
}

pub fn make_project_relative(project_dir: &Path, path: &Path) -> Result<PathBuf> {
    let normalized = lexically_normal(path);
    if normalized.as_os_str().is_empty() {
        return Err(Error::new(ErrorCode::InvalidArgument, "project manifest paths cannot be empty"));
    }
    if !normalized.is_absolute() {
        if contains_parent_reference(&normalized) {
            return Err(Error::new(ErrorCode::InvalidArgument, "project manifest paths must stay inside the project directory"));
        }
        return Ok(normalized);
    }
    let outside = || Error::new(ErrorCode::InvalidArgument, "absolute path is outside the project directory");
    let relative = normalized.strip_prefix(lexically_normal(project_dir)).map_err(|_| outside())?;
    if relative.as_os_str().is_empty() {
        return Err(outside());
    }
    Ok(relative.to_path_buf())
}

pub fn save_manifest(project: &Project, path: &Path) -> Result<()> {
    let project_dir = path.parent().unwrap_or_else(|| Path::new(""));
    let mut out = String::new();
    out.push_str("format: big-star-editor-project\n");
    out.push_str(&format!("version: {MANIFEST_VERSION}\n"));
    out.push_str(&yaml_scalar("name", project.name()));

    if let Some(rom) = project.rom() {
        let rom_path = make_project_relative(project_dir, &rom.rom_path)?;
        out.push_str("rom:\n");
        out.push_str("  ");
        out.push_str(&yaml_scalar("rom_path", &to_generic_string(&rom_path)));
        if let Some(header) = &rom.header {
            out.push_str("  ");
            out.push_str(&yaml_scalar("game_title", &header.game_title));
            out.push_str("  ");
            out.push_str(&yaml_scalar("game_code", &header.game_code));
        }
    }
    write_text_file_atomically(path, &out)
}

pub fn load_manifest(path: &Path) -> Result<Project> {
    let bytes = std::fs::read(path).map_err(|_| Error::new(ErrorCode::IoError, "could not open project manifest for reading"))?;
    let text = String::from_utf8_lossy(&bytes);

    let mut project = Project::default();
    project.set_directory(path.parent().unwrap_or_else(|| Path::new("")));

    let (mut saw_format, mut saw_version, mut saw_name, mut saw_rom, mut in_rom) = (false, false, false, false, false);
    let mut header: Option<RomHeader> = None;
    let mut rom_path = PathBuf::new();

    for line in text.lines() {
        let stripped = trim(line);
        if stripped.is_empty() || stripped.starts_with('#') {
            continue;
        }
        if stripped == "format: big-star-editor-project" {
            saw_format = true;
            in_rom = false;
        } else if let Some(rest) = stripped.strip_prefix("version:") {
            match parse_size(rest) {
                Ok(version) if version != 0 && version <= MANIFEST_VERSION => {}
                _ => return Err(parse_error("unsupported project manifest version")),
            }
            saw_version = true;
            in_rom = false;
        } else if let Some(rest) = stripped.strip_prefix("name:") {
            project.set_name(parse_scalar(rest)?);
            saw_name = true;
            in_rom = false;
        } else if stripped == "rom:" {
            in_rom = true;
            saw_rom = true;
        } else if stripped == "files:" {
            // Version 1 mirrored the whole extracted tree here. That list is derived and carries no ids, so it is
            // neither written nor read now -- its entries fall through unmatched.
            in_rom = false;
        } else if in_rom && let Some(rest) = stripped.strip_prefix("rom_path:") {
            rom_path = parse_manifest_path(rest, "rom.rom_path")?;
        } else if in_rom && let Some(rest) = stripped.strip_prefix("extracted_path:") {
            // Still validated, so a corrupt manifest is still refused, and then discarded.
            parse_manifest_path(rest, "rom.extracted_path")?;
        } else if in_rom && let Some(rest) = stripped.strip_prefix("game_title:") {
            header.get_or_insert_with(RomHeader::default).game_title = parse_scalar(rest)?;
        } else if in_rom && let Some(rest) = stripped.strip_prefix("game_code:") {
            header.get_or_insert_with(RomHeader::default).game_code = parse_scalar(rest)?;
        }
    }

    if !saw_format {
        return Err(parse_error("manifest format marker is missing"));
    }
    if !saw_version {
        return Err(parse_error("manifest version is missing"));
    }
    if !saw_name || project.name().is_empty() {
        return Err(parse_error("project name is missing"));
    }
    if saw_rom {
        let rom_path = if rom_path.as_os_str().is_empty() { default_rom_path(project.directory()) } else { rom_path };
        project.attach_rom(rom_path, header);
        // The file index is the ROM's own table, so it is read on open rather than persisted. A project whose ROM has
        // not been built yet still loads -- it just has an empty index until the next build.
        let _ = index_rom_files(&mut project);
    }
    project.mark_clean();
    Ok(project)
}

/// Fills the project's file list from the ROM's own table, which is the list the game addresses. Sizes come from the
/// FAT, so nothing is read to measure it.
pub fn index_rom_files_from(project: &mut Project, rom: &RomReader) {
    let mut files: Vec<GameFile> =
        rom.files().iter().map(|file| GameFile { path: PathBuf::from(&file.path), size: u64::from(rom.file_size(file.id).unwrap_or(0)) }).collect();
    files.sort_by_key(|file| to_generic_string(&file.path));
    project.clear_game_files();
    for file in files {
        project.add_game_file(file);
    }
}

pub fn index_rom_files(project: &mut Project) -> Result<()> {
    let configured = match project.rom() {
        Some(rom) if !rom.rom_path.as_os_str().is_empty() => rom.rom_path.clone(),
        _ => return Err(Error::new(ErrorCode::InvalidArgument, "project needs a ROM before indexing files")),
    };
    let full = if configured.is_absolute() { configured } else { project.directory().join(configured) };
    let rom = RomReader::open(full)?;
    index_rom_files_from(project, &rom);
    Ok(())
}
