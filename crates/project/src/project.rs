use std::path::{Path, PathBuf};

use bigstar_core::Result;
use bigstar_nds::RomHeader;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameFile {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rom {
    pub header: Option<RomHeader>,
    /// The built `.nds` the editor reads, project-relative. Not an input to NCPatcher and not a copy of one: it is
    /// what the last build produced.
    pub rom_path: PathBuf,
    pub files: Vec<GameFile>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Project {
    name: String,
    directory: PathBuf,
    rom: Option<Rom>,
    dirty: bool,
}

impl Default for Project {
    fn default() -> Self {
        Self { name: "Untitled Project".to_owned(), directory: PathBuf::new(), rom: None, dirty: false }
    }
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), ..Self::default() }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
        self.dirty = true;
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub fn set_directory(&mut self, directory: impl Into<PathBuf>) {
        self.directory = directory.into();
        self.dirty = true;
    }

    pub fn dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn attach_rom(&mut self, relative_path: impl Into<PathBuf>, header: Option<RomHeader>) {
        self.rom = Some(Rom { header, rom_path: relative_path.into(), files: Vec::new() });
        self.dirty = true;
    }

    pub fn rom(&self) -> Option<&Rom> {
        self.rom.as_ref()
    }

    pub fn add_game_file(&mut self, file: GameFile) {
        self.rom.get_or_insert_with(Rom::default).files.push(file);
        self.dirty = true;
    }

    pub fn update_game_file_size(&mut self, path: &Path, size: u64) {
        if let Some(file) = self.rom.as_mut().and_then(|rom| rom.files.iter_mut().find(|file| file.path == path)) {
            file.size = size;
            self.dirty = true;
        }
    }

    pub fn clear_game_files(&mut self) {
        if let Some(rom) = self.rom.as_mut() {
            rom.files.clear();
            self.dirty = true;
        }
    }

    pub fn game_files(&self) -> &[GameFile] {
        self.rom.as_ref().map_or(&[], |rom| rom.files.as_slice())
    }
}

pub fn create_empty_project(name: impl Into<String>, directory: impl Into<PathBuf>) -> Result<Project> {
    let mut project = Project::new(name);
    project.set_directory(directory);
    project.mark_clean();
    Ok(project)
}
