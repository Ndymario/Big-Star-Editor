//! What the editor sees when it reads a ROM address. Port of `libs/nsmb-project/src/effective_filesystem.cpp`.
//!
//! The ROM holds what the last build produced, and a build has not necessarily run since the last edit. So a read
//! asks the plan first: when a module supplies this destination and that source is on disk, the source is what the
//! editor renders. Archive addresses resolve the same way and per member, so a module supplying one member of an
//! archive does not stop the others coming from the ROM.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use bigstar_core::{Error, ErrorCode, Result, read_file, read_file_prefix};
use bigstar_formats::{parse_narc, unwrap_narc};
use bigstar_patching::FileManifest;

use crate::RomReader;

/// `a/b.narc!c/d` split in two. `member` is empty for an ordinary file, which is the only way to tell a container's
/// own bytes from what is inside it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RomAddress {
    pub container: String,
    pub member: String,
}

impl RomAddress {
    pub fn inside_archive(&self) -> bool {
        !self.member.is_empty()
    }
}

pub fn split_rom_address(address: &str) -> RomAddress {
    match address.split_once('!') {
        Some((container, member)) => RomAddress { container: container.to_owned(), member: member.to_owned() },
        None => RomAddress { container: address.to_owned(), member: String::new() },
    }
}

/// A plan's `source`, which is relative to the project unless NCPatcher wrote it absolute.
pub(crate) fn source_path(project_dir: &Path, source: &str) -> PathBuf {
    let path = Path::new(source);
    if path.is_absolute() { path.to_path_buf() } else { project_dir.join(path) }
}

/// A source the plan names but the disk does not hold cannot be read, whatever the plan intends -- a `pre-build` hook
/// has not run yet, or somebody deleted it. The ROM's copy is then the only answer there is.
fn readable(path: PathBuf) -> Option<PathBuf> {
    path.is_file().then_some(path)
}

#[derive(Default)]
pub struct EffectiveFilesystem {
    rom: Option<RomReader>,
    plan: Option<FileManifest>,
    project_dir: PathBuf,
    /// Files changed in memory, by address, when a ROM is edited directly: they answer before anything else, and
    /// are what saving that ROM writes into it.
    edits: BTreeMap<String, Vec<u8>>,
}

impl EffectiveFilesystem {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_rom(&mut self, rom: Option<RomReader>) {
        self.rom = rom;
    }

    pub fn set_plan(&mut self, project_dir: impl Into<PathBuf>, plan: Option<FileManifest>) {
        self.project_dir = project_dir.into();
        self.plan = plan;
    }

    pub fn clear(&mut self) {
        self.rom = None;
        self.plan = None;
        self.project_dir = PathBuf::new();
        self.edits.clear();
    }

    /// Replaces what `address` reads as, in memory only.
    pub fn set_edit(&mut self, address: impl Into<String>, bytes: Vec<u8>) {
        self.edits.insert(address.into(), bytes);
    }

    pub fn edits(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.edits
    }

    pub fn clear_edits(&mut self) {
        self.edits.clear();
    }

    /// An edit that answers `address`: its own, or for an archive member, the member inside an edited container.
    fn edited(&self, address: &str) -> Option<Result<Vec<u8>>> {
        if let Some(bytes) = self.edits.get(address) {
            return Some(Ok(bytes.clone()));
        }
        let split = split_rom_address(address);
        let container = self.edits.get(&split.container).filter(|_| split.inside_archive())?;
        let member = || -> Result<Vec<u8>> {
            let (_, plain) =
                unwrap_narc(container).ok_or_else(|| Error::new(ErrorCode::Unsupported, format!("{} is not a Nitro archive", split.container)))?;
            let archive = parse_narc(&plain)?;
            let index = archive.find(&split.member)?;
            archive.files.get(index).cloned().ok_or_else(|| Error::new(ErrorCode::NotFound, format!("{address} is not in its archive")))
        };
        Some(member())
    }

    pub fn has_rom(&self) -> bool {
        self.rom.is_some()
    }

    pub fn rom(&self) -> Option<&RomReader> {
        self.rom.as_ref()
    }

    pub fn rom_mut(&mut self) -> Option<&mut RomReader> {
        self.rom.as_mut()
    }

    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    /// The file on disk this address reads from, or `None` when the ROM supplies it. This is what a caller watches:
    /// the ROM changes only when a build replaces it, and a module source changes whenever somebody saves.
    pub fn source(&self, address: &str) -> Option<PathBuf> {
        let plan = self.plan.as_ref()?;
        let split = split_rom_address(address);
        let file = plan.files.iter().find(|file| file.path == split.container)?;
        if !split.inside_archive() {
            if file.source.is_empty() {
                return None;
            }
            return readable(source_path(&self.project_dir, &file.source));
        }
        let member = file.members.iter().find(|member| member.path == split.member)?;
        if member.source.is_empty() {
            return None;
        }
        readable(source_path(&self.project_dir, &member.source))
    }

    pub fn read(&mut self, address: &str) -> Result<Vec<u8>> {
        if let Some(edited) = self.edited(address) {
            return edited;
        }
        if let Some(path) = self.source(address) {
            return read_file(&path);
        }
        let Some(rom) = self.rom.as_mut() else {
            return Err(Error::new(ErrorCode::NotFound, format!("no built ROM is open, and nothing on disk supplies {address}")));
        };
        let split = split_rom_address(address);
        if split.inside_archive() { rom.read_member(&split.container, &split.member) } else { rom.read(&split.container) }
    }

    /// The head of what [`EffectiveFilesystem::read`] would return, for callers that only need to recognise a format.
    /// A member still costs its container a parse, because an archive does not hand out a member without one.
    pub fn read_prefix(&mut self, address: &str, maximum: usize) -> Result<Vec<u8>> {
        if let Some(edited) = self.edited(address) {
            return edited.map(|mut data| {
                data.truncate(maximum);
                data
            });
        }
        if let Some(path) = self.source(address) {
            return read_file_prefix(&path, maximum);
        }
        let split = split_rom_address(address);
        if let Some(rom) = self.rom.as_mut()
            && !split.inside_archive()
        {
            return rom.read_prefix(&split.container, maximum);
        }
        let mut data = self.read(address)?;
        data.truncate(maximum);
        Ok(data)
    }

    /// The ROM path a NitroFS id names. Resource tables in the game's own code address files by id, so this is how a
    /// table entry becomes an address.
    pub fn path_for_id(&self, file_id: u16) -> Option<String> {
        let files = self.rom.as_ref()?.files();
        let index = files.binary_search_by_key(&file_id, |file| file.id).ok()?;
        files.get(index).map(|file| file.path.clone())
    }
}
