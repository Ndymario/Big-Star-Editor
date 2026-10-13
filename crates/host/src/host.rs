use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bigstar_core::{Error, ErrorCode, Result, read_file_prefix, write_file_atomically};
use bigstar_level::{EditableLevel, LevelEntityInsertions, ModuleDataCatalog};
use bigstar_nds::{DEFAULT_HEADER_REGION_SIZE, parse_rom_header};
use bigstar_project::{
    DEFAULT_GENERATED_DIR, INPUT_ROM_FILE_NAME, MANIFEST_FILE_NAME, Project, RomReader, create_empty_project, default_rom_path, index_rom_files_from,
    load_manifest, load_module_data_catalog, nsmb_level_catalog, save_manifest,
};
use bigstar_protocol::{
    AreaSummary, DirectoryEntry, DirectoryEntryKind, DirectoryListing, EditOutcome, Event, LevelEdit, LevelId, LevelSnapshot, LevelSummary,
    LevelWorld, LogLevel, ProjectInfo, ProjectSummary, Request, Response, Service,
};

use crate::levels::{LevelFiles, RomIndex, edit_path, locate, read_source};
use crate::workspace::Workspace;

/// How deep `ListProjects` looks below the root, and how many projects it reports.
const PROJECT_SEARCH_DEPTH: usize = 4;
const PROJECT_LIMIT: usize = 500;

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidArgument, message)
}

fn not_found(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::NotFound, message)
}

struct LevelSession {
    area: String,
    title: String,
    files: LevelFiles,
    level: EditableLevel,
    revision: u64,
}

struct OpenProject {
    directory: PathBuf,
    rom: Option<RomReader>,
    index: RomIndex,
    catalog: ModuleDataCatalog,
    levels: BTreeMap<LevelId, LevelSession>,
    next_level: LevelId,
}

#[derive(Default)]
struct State {
    open: Option<OpenProject>,
}

pub(crate) struct Shared {
    workspace: Workspace,
    state: Mutex<State>,
    subscribers: Mutex<Vec<Sender<Event>>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The service behind every connection to one workspace root. Cheap to clone; every clone is the same host.
#[derive(Clone)]
pub struct Host {
    pub(crate) shared: Arc<Shared>,
}

impl Host {
    pub fn new(root: &Path) -> Result<Self> {
        Ok(Self {
            shared: Arc::new(Shared { workspace: Workspace::new(root)?, state: Mutex::new(State::default()), subscribers: Mutex::new(Vec::new()) }),
        })
    }

    pub fn workspace_name(&self) -> String {
        self.shared.workspace.name()
    }

    /// A new connection's view of the host. Events from all connections reach every session.
    pub fn session(&self) -> HostSession {
        let (sender, events) = channel();
        lock(&self.shared.subscribers).push(sender);
        HostSession { shared: Arc::clone(&self.shared), events }
    }
}

pub struct HostSession {
    shared: Arc<Shared>,
    events: Receiver<Event>,
}

#[cfg(test)]
impl HostSession {
    pub(crate) fn shared(&self) -> Arc<Shared> {
        Arc::clone(&self.shared)
    }
}

impl Service for HostSession {
    fn handle(&mut self, request: Request) -> Result<Response> {
        self.shared.handle(request)
    }

    fn drain_events(&mut self) -> Vec<Event> {
        self.events.try_iter().collect()
    }
}

impl Shared {
    fn emit(&self, event: Event) {
        lock(&self.subscribers).retain(|subscriber| subscriber.send(event.clone()).is_ok());
    }

    fn log(&self, level: LogLevel, message: impl Into<String>) {
        self.emit(Event::Log { level, message: message.into() });
    }

    fn handle(&self, request: Request) -> Result<Response> {
        match request {
            Request::ListProjects => self.list_projects(),
            Request::BrowseDirectory { path } => self.browse(&path),
            Request::CreateProject { path, name } => self.create_project(&path, &name),
            Request::ImportRom { rom_path, project_path, name } => self.import_rom(&rom_path, &project_path, name.as_deref()),
            Request::OpenProject { path } => self.open_project(&path),
            Request::CloseProject => self.close_project(),
            Request::ListLevels => self.list_levels(),
            Request::OpenLevel { area } => self.open_level(&area),
            Request::EditLevel { level, base_revision, edit } => self.edit_level(level, base_revision, edit),
            Request::UndoLevel { level } => self.step_history(level, true),
            Request::RedoLevel { level } => self.step_history(level, false),
            Request::SaveLevel { level } => self.save_level(level),
            Request::CloseLevel { level } => self.close_level(level),
        }
    }

    fn list_projects(&self) -> Result<Response> {
        let mut found = Vec::new();
        self.scan(self.workspace.root(), 0, &mut found);
        found.sort_by(|a: &ProjectSummary, b| a.path.cmp(&b.path));
        Ok(Response::Projects(found))
    }

    /// Symlinks are not followed, so a link cannot lead the search out of the root.
    fn scan(&self, directory: &Path, depth: usize, found: &mut Vec<ProjectSummary>) {
        if depth > PROJECT_SEARCH_DEPTH || found.len() >= PROJECT_LIMIT {
            return;
        }
        let manifest = directory.join(MANIFEST_FILE_NAME);
        if manifest.is_file() {
            if let Some(path) = self.workspace.relative(directory) {
                let name = load_manifest(&manifest)
                    .map(|project| project.name().to_owned())
                    .unwrap_or_else(|_| directory.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
                found.push(ProjectSummary { path, name });
            }
            return;
        }
        let Ok(entries) = std::fs::read_dir(directory) else { return };
        for entry in entries.flatten() {
            let hidden = entry.file_name().to_string_lossy().starts_with('.');
            if !hidden && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                self.scan(&entry.path(), depth + 1, found);
            }
        }
    }

    fn browse(&self, path: &str) -> Result<Response> {
        let directory = self.workspace.resolve(path)?;
        let read = std::fs::read_dir(&directory).map_err(|_| not_found("that directory cannot be read"))?;
        let mut entries = Vec::new();
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            // A link that points out of the root is not shown, rather than shown and then refused.
            let Some(child) = self.workspace.relative(&directory).map(|base| if base.is_empty() { name.clone() } else { format!("{base}/{name}") })
            else {
                continue;
            };
            if self.workspace.resolve(&child).is_err() {
                continue;
            }
            let Ok(metadata) = std::fs::metadata(entry.path()) else { continue };
            let kind = if metadata.is_dir() { DirectoryEntryKind::Directory } else { DirectoryEntryKind::File };
            entries.push(DirectoryEntry { name, kind, size: if metadata.is_dir() { 0 } else { metadata.len() } });
        }
        entries.sort_by(|a, b| (a.kind == DirectoryEntryKind::File, &a.name).cmp(&(b.kind == DirectoryEntryKind::File, &b.name)));
        let path = self.workspace.relative(&directory).unwrap_or_default();
        Ok(Response::Directory(DirectoryListing { path, entries }))
    }

    fn create_project(&self, path: &str, name: &str) -> Result<Response> {
        let name = name.trim();
        if name.is_empty() {
            return Err(invalid("a project needs a name"));
        }
        let directory = self.workspace.resolve(path)?;
        if directory == self.workspace.root() {
            return Err(invalid("a project needs a directory of its own"));
        }
        let manifest = directory.join(MANIFEST_FILE_NAME);
        if manifest.try_exists().map_err(|_| Error::new(ErrorCode::IoError, "could not inspect the project directory"))? {
            return Err(invalid("a project already exists there"));
        }
        std::fs::create_dir_all(&directory).map_err(|_| Error::new(ErrorCode::IoError, "could not create the project directory"))?;
        let project = create_empty_project(name, &directory)?;
        save_manifest(&project, &manifest)?;
        self.log(LogLevel::Info, format!("Created project \"{name}\""));
        Ok(Response::Project(project_info(&self.workspace, &directory, &project, None, 0)))
    }

    fn import_rom(&self, rom_path: &str, project_path: &str, name: Option<&str>) -> Result<Response> {
        let rom = self.workspace.resolve(rom_path)?;
        let directory = self.workspace.resolve(project_path)?;
        if directory == self.workspace.root() {
            return Err(invalid("a project needs a directory of its own"));
        }
        let manifest = directory.join(MANIFEST_FILE_NAME);
        if manifest.try_exists().map_err(|_| Error::new(ErrorCode::IoError, "could not inspect the project directory"))? {
            return Err(invalid("a project already exists there"));
        }
        let header = parse_rom_header(&read_file_prefix(&rom, DEFAULT_HEADER_REGION_SIZE)?)?;
        std::fs::create_dir_all(&directory).map_err(|_| Error::new(ErrorCode::IoError, "could not create the project directory"))?;

        let imported = directory.join(INPUT_ROM_FILE_NAME);
        let same_file = matches!((std::fs::canonicalize(&rom), std::fs::canonicalize(&imported)), (Ok(a), Ok(b)) if a == b);
        if !same_file {
            std::fs::copy(&rom, &imported).map_err(|_| Error::new(ErrorCode::IoError, "could not copy the ROM into the project"))?;
        }

        let fallback = directory.file_name().map(|n| n.to_string_lossy().into_owned()).filter(|n| !n.is_empty());
        let project_name = name
            .map(str::trim)
            .filter(|n| !n.is_empty())
            .map(str::to_owned)
            .or(fallback)
            .unwrap_or_else(|| if header.game_title.is_empty() { "Imported ROM".to_owned() } else { header.game_title.clone() });
        let mut project = create_empty_project(project_name, &directory)?;
        project.attach_rom(INPUT_ROM_FILE_NAME, Some(header));
        let reader = RomReader::open(&imported)?;
        index_rom_files_from(&mut project, &reader);
        save_manifest(&project, &manifest)?;
        self.log(LogLevel::Info, format!("Imported the ROM into \"{}\" ({} files)", project.name(), project.game_files().len()));
        Ok(Response::Project(project_info(&self.workspace, &directory, &project, Some(INPUT_ROM_FILE_NAME.to_owned()), reader.files().len())))
    }

    fn open_project(&self, path: &str) -> Result<Response> {
        let directory = self.workspace.resolve(path)?;
        let mut state = lock(&self.state);
        if state.open.as_ref().is_some_and(|open| open.levels.values().any(|session| session.level.dirty())) {
            return Err(invalid("save or close the open levels before opening another project"));
        }
        let project = load_manifest(&directory.join(MANIFEST_FILE_NAME))?;
        let relative_rom = default_rom_path(&directory);
        let rom_file = directory.join(&relative_rom);
        let (rom, rom_path) = if rom_file.is_file() {
            (Some(RomReader::open(&rom_file)?), Some(relative_rom.to_string_lossy().replace('\\', "/")))
        } else {
            (None, None)
        };
        let catalog = load_module_data_catalog(&directory, Path::new(DEFAULT_GENERATED_DIR))?;
        let index = rom.as_ref().map(RomIndex::new).unwrap_or_else(RomIndex::new_empty);
        let file_count = rom.as_ref().map_or(0, |rom| rom.files().len());
        let info = project_info(&self.workspace, &directory, &project, rom_path.clone(), file_count);
        let had_project = state.open.is_some();
        state.open = Some(OpenProject { directory, rom, index, catalog, levels: BTreeMap::new(), next_level: 1 });
        drop(state);
        if had_project {
            self.emit(Event::ProjectClosed);
        }
        self.log(LogLevel::Info, format!("Opened project \"{}\"", info.name));
        Ok(Response::Project(info))
    }

    fn close_project(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        if state.open.as_ref().is_some_and(|open| open.levels.values().any(|session| session.level.dirty())) {
            return Err(invalid("save or close the open levels before closing the project"));
        }
        let had_project = state.open.take().is_some();
        drop(state);
        if had_project {
            self.emit(Event::ProjectClosed);
        }
        Ok(Response::Done)
    }

    fn list_levels(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let mut worlds: Vec<LevelWorld> = Vec::new();
        for entry in nsmb_level_catalog() {
            let areas = entry
                .areas
                .iter()
                .map(|area| AreaSummary {
                    token: area.source_token.clone(),
                    display_name: area.display_name.clone(),
                    available: open.rom.as_mut().is_some_and(|rom| locate(rom, &open.index, area).is_some()),
                })
                .collect();
            let level = LevelSummary { display_name: entry.display_name.clone(), areas };
            match worlds.last_mut() {
                Some(world) if world.name == entry.world => world.levels.push(level),
                _ => worlds.push(LevelWorld { name: entry.world.clone(), levels: vec![level] }),
            }
        }
        Ok(Response::Levels(worlds))
    }

    fn open_level(&self, area_token: &str) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        if let Some((id, session)) = open.levels.iter().find(|(_, session)| session.area == area_token) {
            return Ok(Response::Level(Box::new(snapshot(*id, session))));
        }
        let (entry, area) = nsmb_level_catalog()
            .iter()
            .find_map(|entry| entry.areas.iter().find(|area| area.source_token == area_token).map(|area| (entry, area)))
            .ok_or_else(|| not_found(format!("there is no level area {area_token}")))?;
        let rom = open.rom.as_mut().ok_or_else(|| not_found("the project has no ROM"))?;
        let files = locate(rom, &open.index, area).ok_or_else(|| not_found(format!("the ROM does not hold {area_token}")))?;
        let course = read_source(rom, &open.directory, &files.course)?;
        let bgdat = read_source(rom, &open.directory, &files.bgdat)?;
        let level = EditableLevel::load(&course, &bgdat, Some(&open.catalog))?;
        let title = if entry.areas.len() == 1 { entry.display_name.clone() } else { format!("{} {}", entry.display_name, area.display_name) };
        let id = open.next_level;
        open.next_level += 1;
        let session = LevelSession { area: area_token.to_owned(), title, files, level, revision: 1 };
        let response = Response::Level(Box::new(snapshot(id, &session)));
        open.levels.insert(id, session);
        Ok(response)
    }

    fn edit_level(&self, id: LevelId, base_revision: u64, edit: LevelEdit) -> Result<Response> {
        let mut state = lock(&self.state);
        let session = session_mut(&mut state, id)?;
        if session.revision != base_revision {
            return Err(invalid(format!("the level changed since revision {base_revision}; it is at {}", session.revision)));
        }
        let level = &mut session.level;
        let outcome = match edit {
            LevelEdit::Update { updates, name } => level.update_entities(updates, &name).map(|()| EditOutcome::None),
            LevelEdit::Add { items, name } => add(level, items, &name),
            LevelEdit::Remove { selection, name } => level.remove_entities(selection, &name).map(|()| EditOutcome::None),
            LevelEdit::Reorder { kind, index, forward } => level.reorder_entity(kind, index, forward).map(EditOutcome::Moved),
            LevelEdit::SetObjectBank { slot, value } => level.set_object_bank(slot, value).map(|()| EditOutcome::None),
            LevelEdit::SetTilesetReferences { references } => level.set_tileset_references(references).map(|()| EditOutcome::None),
        }?;
        session.revision += 1;
        let (snapshot, event) = snapshot_and_event(id, session);
        drop(state);
        self.emit(event);
        Ok(Response::Edited { snapshot: Box::new(snapshot), outcome })
    }

    fn step_history(&self, id: LevelId, undo: bool) -> Result<Response> {
        let mut state = lock(&self.state);
        let session = session_mut(&mut state, id)?;
        if undo {
            session.level.undo()?
        } else {
            session.level.redo()?
        }
        session.revision += 1;
        let (snapshot, event) = snapshot_and_event(id, session);
        drop(state);
        self.emit(event);
        Ok(Response::Edited { snapshot: Box::new(snapshot), outcome: EditOutcome::None })
    }

    fn save_level(&self, id: LevelId) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let directory = open.directory.clone();
        let session = open.levels.get_mut(&id).ok_or_else(unknown_level)?;

        let course = session.level.serialize_course()?;
        let bgdat = session.level.serialize_bgdat()?;
        EditableLevel::load(&course, &bgdat, None)
            .map_err(|e| Error::new(ErrorCode::ParseError, format!("serialized level failed validation: {}", e.message)))?;

        let targets = [(directory.join(edit_path(&session.files.course)?), course), (directory.join(edit_path(&session.files.bgdat)?), bgdat)];
        write_all_or_none(&targets)?;
        session.level.mark_clean();
        session.revision += 1;
        let (snapshot, event) = snapshot_and_event(id, session);
        let message = format!("Saved {} to the project's edits; a build applies them to the ROM", session.title);
        drop(state);
        self.emit(event);
        self.log(LogLevel::Info, message);
        Ok(Response::Edited { snapshot: Box::new(snapshot), outcome: EditOutcome::None })
    }

    fn close_level(&self, id: LevelId) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        open.levels.remove(&id).ok_or_else(unknown_level)?;
        Ok(Response::Done)
    }
}

fn no_project() -> Error {
    invalid("no project is open")
}

fn unknown_level() -> Error {
    not_found("that level is not open")
}

fn session_mut(state: &mut State, id: LevelId) -> Result<&mut LevelSession> {
    state.open.as_mut().ok_or_else(no_project)?.levels.get_mut(&id).ok_or_else(unknown_level)
}

fn add(level: &mut EditableLevel, items: LevelEntityInsertions, name: &str) -> Result<EditOutcome> {
    level.add_entities(items, name).map(EditOutcome::Added)
}

fn snapshot(id: LevelId, session: &LevelSession) -> LevelSnapshot {
    LevelSnapshot {
        level: id,
        area: session.area.clone(),
        title: session.title.clone(),
        revision: session.revision,
        course: session.level.course().clone(),
        objects: session.level.objects().to_vec(),
        tileset_references: session.level.tileset_references(),
        glue_error: session.level.glue_error().map(str::to_owned),
        dirty: session.level.dirty(),
        can_undo: session.level.can_undo(),
        can_redo: session.level.can_redo(),
        undo_names: session.level.undo_names(),
        redo_names: session.level.redo_names(),
    }
}

fn snapshot_and_event(id: LevelId, session: &LevelSession) -> (LevelSnapshot, Event) {
    (snapshot(id, session), Event::LevelChanged { level: id, revision: session.revision, dirty: session.level.dirty() })
}

fn project_info(workspace: &Workspace, directory: &Path, project: &Project, rom_path: Option<String>, file_count: usize) -> ProjectInfo {
    let header = project.rom().and_then(|rom| rom.header.as_ref());
    ProjectInfo {
        path: workspace.relative(directory).unwrap_or_default(),
        name: project.name().to_owned(),
        rom_path,
        game_title: header.map(|h| h.game_title.clone()),
        game_code: header.map(|h| h.game_code.clone()),
        file_count,
    }
}

/// Writes every target, or none: a target already in place is put back when a later one fails.
fn write_all_or_none(targets: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    let mut previous: Vec<Option<Vec<u8>>> = Vec::new();
    for (index, (path, bytes)) in targets.iter().enumerate() {
        let attempt = (|| {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "could not create the edits directory"))?;
            }
            let before = if path.is_file() {
                Some(std::fs::read(path).map_err(|_| Error::new(ErrorCode::IoError, "could not back up an edited file"))?)
            } else {
                None
            };
            previous.push(before);
            write_file_atomically(path, bytes)
        })();
        if let Err(error) = attempt {
            for ((path, _), before) in targets.iter().zip(&previous).take(index) {
                match before {
                    Some(bytes) => {
                        let _ = write_file_atomically(path, bytes);
                    }
                    None => {
                        let _ = std::fs::remove_file(path);
                    }
                }
            }
            return Err(error);
        }
    }
    Ok(())
}
