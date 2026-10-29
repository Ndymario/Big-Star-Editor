//! Editing a ROM directly, without a project: the `.nds` held in memory, every edit kept over it by address, and the
//! ROM written whole -- the edits laid into the original the way NCPatcher would lay them -- whenever something is
//! saved. Writing from the original every time rather than from the last save keeps a ROM saved a hundred times the
//! size of one saved once: no extent abandoned by one save survives into the next.
//!
//! A ROM comes from one of two places, and neither is a workspace. The desktop app's host opens a file picked on this
//! machine by its path, and every save writes into that file. A served host takes a ROM a connection uploads and
//! keeps it only in memory, where it leaves as a download. There are no modules, variants or builds here, and nothing
//! for NCPatcher to do; requests that only mean something for an NCPatcher project are refused by name.

use std::sync::Arc;

use bigstar_project::{DEFAULT_BACKUPS_KEPT, nsmb_graphics_profile, write_edited_rom};
use bigstar_protocol::{ProjectMode, ROM_CHUNK_BYTES};

use super::*;

pub(super) const ROM_MODE_REFUSAL: &str = "This needs an NCPatcher project. It is not available when editing a ROM directly.";

/// A ROM being edited directly.
pub(super) struct RomDocument {
    /// The `.nds` saves go to; `None` for an uploaded ROM, which is only ever in memory.
    pub file: Option<PathBuf>,
    /// The file's name, or the name the ROM was uploaded under.
    pub name: String,
    pub original: Arc<[u8]>,
    pub edits: BTreeMap<String, Vec<u8>>,
    pub banner: Option<Vec<u8>>,
    /// Whether this session has kept a copy of the file it is about to overwrite for the first time.
    pub backed_up: bool,
    /// The ROM with every edit, written for the download under way; dropped by the next edit.
    pub written: Option<Arc<[u8]>>,
}

/// What an open project reads from and writes to.
pub(super) enum Backing {
    /// An NCPatcher project: edits go into modules, and NCPatcher builds the ROM.
    NcPatcher,
    Rom(RomDocument),
}

impl Backing {
    pub fn is_rom(&self) -> bool {
        matches!(self, Backing::Rom(_))
    }
}

fn rom_info(doc: &RomDocument, rom: &RomReader) -> ProjectInfo {
    let header = rom.header();
    let path = doc.file.as_ref().map(|file| file.display().to_string());
    ProjectInfo {
        path: path.clone().unwrap_or_else(|| doc.name.clone()),
        name: doc.name.clone(),
        rom_path: path,
        game_title: Some(header.game_title.clone()),
        game_code: Some(header.game_code.clone()),
        file_count: rom.files().len(),
        mode: if doc.file.is_some() { ProjectMode::Rom } else { ProjectMode::Uploaded },
    }
}

/// `bytes` as a ROM this editor can edit: a DS ROM, and New Super Mario Bros.
fn read_rom(name: &str, bytes: Arc<[u8]>) -> Result<RomReader> {
    let rom = RomReader::from_bytes(name, bytes)?;
    let code = &rom.header().game_code;
    if nsmb_graphics_profile(code).is_none() {
        return Err(Error::new(ErrorCode::Unsupported, format!("{name} is not New Super Mario Bros. (its game code is {code})")));
    }
    Ok(rom)
}

/// What `path` names when it is a ROM on this machine, which only a host for local ROMs opens.
fn local_path(shared: &Shared, path: &str) -> Result<PathBuf> {
    if !shared.local_files {
        return Err(invalid("This host does not open ROMs by path: upload the ROM instead."));
    }
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(invalid("Give the full path of the ROM."));
    }
    Ok(path)
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(|| path.display().to_string(), |name| name.to_string_lossy().into_owned())
}

/// A name an upload gives itself, kept to something safe to show and to suggest for the download.
fn upload_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let clean: String = base.chars().filter(|c| !c.is_control()).take(128).collect();
    if clean.trim().is_empty() { "rom.nds".to_owned() } else { clean }
}

impl Shared {
    /// Refuses a request that only means something for an NCPatcher project.
    pub(super) fn ncpatcher_only(&self) -> Result<()> {
        let state = lock(&self.state);
        if state.open.as_ref().is_some_and(|open| open.backing.is_rom()) { Err(invalid(ROM_MODE_REFUSAL)) } else { Ok(()) }
    }

    /// Opens a `.nds` on this machine to edit directly.
    pub(super) fn open_rom(&self, path: &str) -> Result<Response> {
        let file = local_path(self, path)?;
        {
            let state = lock(&self.state);
            // As with a project, a second frontend opening the ROM that is open joins it.
            if let Some(open) = state.open.as_ref()
                && let Backing::Rom(doc) = &open.backing
                && doc.file.as_ref() == Some(&file)
                && let Some(rom) = open.rom.as_ref()
            {
                return Ok(Response::Project(rom_info(doc, rom)));
            }
        }
        let original: Arc<[u8]> = read_file(&file)?.into();
        if let Some(parent) = file.parent() {
            adopt_retired_state(parent);
        }
        let doc = RomDocument {
            name: file_name(&file),
            file: Some(file.clone()),
            original,
            edits: BTreeMap::new(),
            banner: None,
            backed_up: false,
            written: None,
        };
        self.open_document(doc, file.parent().map(Path::to_path_buf).unwrap_or_default())
    }

    /// Opens a ROM a connection uploaded; this host is that connection's alone.
    pub(super) fn open_uploaded(&self, name: &str, bytes: Vec<u8>) -> Result<Response> {
        let doc = RomDocument {
            name: upload_name(name),
            file: None,
            original: bytes.into(),
            edits: BTreeMap::new(),
            banner: None,
            backed_up: true,
            written: None,
        };
        self.open_document(doc, PathBuf::new())
    }

    fn open_document(&self, doc: RomDocument, directory: PathBuf) -> Result<Response> {
        let rom = read_rom(&doc.name, Arc::clone(&doc.original))?;
        let info = rom_info(&doc, &rom);
        let mut state = lock(&self.state);
        if state.open.as_ref().is_some_and(|open| open.levels.values().any(|session| session.level.dirty())) {
            return Err(invalid("save or close the open levels before opening another ROM"));
        }
        let settings = ProjectSettings {
            autosave_minutes: crate::settings::DEFAULT_AUTOSAVE_MINUTES,
            backups_kept: DEFAULT_BACKUPS_KEPT as u32,
            ..ProjectSettings::default()
        };
        let had_project = state.open.is_some();
        state.open = Some(OpenProject {
            directory,
            index: RomIndex::new(&rom),
            rom: Some(rom),
            rom_file: doc.file.clone(),
            built_rom: None,
            catalog: ModuleDataCatalog::default(),
            levels: BTreeMap::new(),
            next_level: 1,
            settings,
            patcher: None,
            banner: BannerTarget::Unconfigured,
            filesystem: None,
            level_list: None,
            graphics: None,
            layers: Default::default(),
            virtual_rom: None,
            own_writes: std::collections::HashSet::new(),
            _watcher: None,
            backing: Backing::Rom(doc),
        });
        drop(state);
        if had_project {
            self.emit(Event::ProjectClosed);
        }
        self.log(LogLevel::Info, format!("Opened the ROM {} to edit directly", info.name));
        Ok(Response::Project(info))
    }

    /// Writes the ROM with every edit to `path` on this machine, which saves go to from then on.
    pub(super) fn save_rom_as(&self, path: &str) -> Result<Response> {
        let target = local_path(self, path)?;
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let Backing::Rom(doc) = &mut open.backing else { return Err(invalid("only a ROM edited directly can be saved as another ROM")) };
        if doc.file.is_none() {
            return Err(invalid("An uploaded ROM is saved by downloading it."));
        }
        let written = write_edited_rom(&doc.original, &doc.edits, doc.banner.as_deref())?;
        write_file_atomically(&target, &written)?;
        // The file now holds what this session wrote, so there is nothing of anyone else's in it to keep a copy of.
        doc.file = Some(target.clone());
        doc.name = file_name(&target);
        doc.backed_up = true;
        open.directory = target.parent().map(Path::to_path_buf).unwrap_or_default();
        open.rom_file = Some(target);
        let rom = open.rom.as_ref().ok_or_else(|| not_found("no ROM is open"))?;
        let info = rom_info(doc, rom);
        drop(state);
        self.log(LogLevel::Info, format!("Saved the ROM as {}", info.path));
        Ok(Response::Project(info))
    }

    /// The piece of the ROM, every edit written into it, that starts at `offset`. The whole is written when the first
    /// piece is asked for and kept until the download ends or something is edited.
    pub(super) fn download_rom(&self, offset: u64) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let Backing::Rom(doc) = &mut open.backing else { return Err(invalid("only a ROM edited directly can be downloaded")) };
        let written = match doc.written.clone() {
            Some(written) if offset != 0 => written,
            _ => {
                let written: Arc<[u8]> = write_edited_rom(&doc.original, &doc.edits, doc.banner.as_deref())?.into();
                doc.written = Some(Arc::clone(&written));
                written
            }
        };
        let start = usize::try_from(offset).ok().filter(|start| *start <= written.len()).ok_or_else(|| invalid("that is past the end of the ROM"))?;
        let end = written.len().min(start + ROM_CHUNK_BYTES);
        let bytes = written.get(start..end).unwrap_or_default().to_vec();
        if end == written.len() {
            doc.written = None;
        }
        Ok(Response::RomChunk { name: doc.name.clone(), total: written.len() as u64, offset, bytes })
    }

    /// Adds `files` (by address) and, when given, the banner to the ROM's edits and, for a ROM on this machine, writes
    /// the file. The first write of a session keeps a copy of what it overwrites. The ROM is written whole either way,
    /// so nothing is kept when it cannot be: the edits a refused write carried are not left behind to fail every save
    /// after it.
    pub(super) fn commit_rom(&self, open: &mut OpenProject, files: Vec<(String, Vec<u8>)>, banner: Option<Vec<u8>>) -> Result<()> {
        let Backing::Rom(doc) = &mut open.backing else { return Err(invalid("no ROM is being edited directly")) };
        let mut edits = doc.edits.clone();
        edits.extend(files.iter().cloned());
        let banner = banner.or_else(|| doc.banner.clone());
        let written = write_edited_rom(&doc.original, &edits, banner.as_deref()).map_err(|error| {
            if error.code == ErrorCode::NotFound {
                Error::new(ErrorCode::InvalidArgument, format!("{} When editing a ROM directly, files can be replaced but not added.", error.message))
            } else {
                error
            }
        })?;
        if let Some(file) = doc.file.clone() {
            if !doc.backed_up {
                let directory = open.directory.clone();
                back_up(&directory, "rom", &[(file.clone(), Vec::new())])?;
                prune_patch_backups(&directory, open_backups_kept(&open.settings));
                doc.backed_up = true;
            }
            write_file_atomically(&file, &written)?;
        }
        doc.edits = edits;
        doc.banner = banner;
        doc.written = None;
        if let Some(graphics) = open.graphics.as_mut() {
            for (address, bytes) in files {
                graphics.set_edit(address, bytes);
            }
        }
        Ok(())
    }
}
