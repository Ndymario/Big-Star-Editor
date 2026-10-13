use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bigstar_core::{Error, ErrorCode, Result, read_file, read_file_prefix, write_file_atomically};
use bigstar_formats::decompress_blz_overlay;
use bigstar_level::{
    EditableLevel, LevelEntityInsertions, LevelPackage, LevelRecoverySnapshot, ModuleDataCatalog, decode_level_recovery, encode_level_recovery,
};
use bigstar_nds::{DEFAULT_HEADER_REGION_SIZE, parse_rom_header};
use bigstar_patching::{FileManifest, ModuleGraph, ModuleInfo, NCPatcherClient};
use bigstar_project::{
    DEFAULT_GENERATED_DIR, FilesystemView, FilesystemViewRequest, INPUT_ROM_FILE_NAME, LevelCatalogArea, LevelCatalogEntry, MANIFEST_FILE_NAME,
    Project, RomReader, build_filesystem_view, create_empty_project, default_rom_path, find_object_bank_overlays, index_rom_files_from,
    load_manifest, load_module_data_catalog, module_file_target, nsmb_level_catalog, save_manifest,
};
use bigstar_protocol::{
    ActorInput, AreaSummary, DirectoryEntry, DirectoryEntryKind, DirectoryListing, EditOutcome, Event, FileEntry, FilesystemListing, FoundEntity,
    LevelBytes, LevelEdit, LevelId, LevelSnapshot, LevelSummary, LevelWorld, LogLevel, ModuleChoice, ProjectData, ProjectInfo, ProjectSettings,
    ProjectSummary, RecoverableLevel, Request, Response, Service, TextureId,
};

use crate::graphics::ProjectGraphics;
use crate::levels::{LevelFiles, RomIndex, Source, edit_path, locate, read_source};
use crate::settings::{load_settings, save_settings};
use crate::watch::ProjectWatcher;
use crate::workspace::Workspace;

/// What a host needs besides its root: where the shipped actor and tile scripts are, and which NCPatcher to ask.
#[derive(Debug, Clone)]
pub struct HostOptions {
    pub data_dir: PathBuf,
    pub ncpatcher: PathBuf,
}

impl Default for HostOptions {
    fn default() -> Self {
        Self { data_dir: default_data_dir(), ncpatcher: PathBuf::from("ncpatcher") }
    }
}

/// `BIGSTAR_DATA` when set; otherwise a `data` directory beside the executable or one level up (an installed layout);
/// otherwise the repository's own, for builds run from the source tree.
pub fn default_data_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("BIGSTAR_DATA") {
        return PathBuf::from(dir);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        for candidate in [dir.join("data"), dir.join("../share/bigstar/data")] {
            if candidate.join("actors").is_dir() {
                return candidate;
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data")
}

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
    rom_file: Option<PathBuf>,
    index: RomIndex,
    catalog: ModuleDataCatalog,
    levels: BTreeMap<LevelId, LevelSession>,
    next_level: LevelId,
    settings: ProjectSettings,
    /// What NCPatcher said, asked once per project open and variant: the module graph and the file plan.
    patcher: Option<(ModuleGraph, Option<FileManifest>)>,
    /// The Filesystem dock's rows, built from the patcher's answer; dropped when files change.
    filesystem: Option<FilesystemView>,
    graphics: Option<ProjectGraphics>,
    /// Where an edited ROM banner goes, from NCPatcher's configuration; asked with the rest of it.
    banner: BannerTarget,
    _watcher: Option<ProjectWatcher>,
}

/// What NCPatcher's configuration says about the ROM banner.
#[derive(Debug, Clone, Default)]
enum BannerTarget {
    /// NCPatcher has not reported the configuration.
    #[default]
    Unreported,
    /// It has, and it names no banner.
    Unconfigured,
    /// The variant's banner, or the project's; relative paths are against the project.
    File(PathBuf),
}

#[derive(Default)]
struct State {
    open: Option<OpenProject>,
}

pub(crate) struct Shared {
    workspace: Workspace,
    options: HostOptions,
    state: Mutex<State>,
    subscribers: Mutex<Vec<Sender<Event>>>,
    /// Grows whenever a watched file changes or the variant switches; graphics answers carry it.
    generation: AtomicU64,
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
        Self::with_options(root, HostOptions::default())
    }

    pub fn with_options(root: &Path, options: HostOptions) -> Result<Self> {
        let shared = Arc::new(Shared {
            workspace: Workspace::new(root)?,
            options,
            state: Mutex::new(State::default()),
            subscribers: Mutex::new(Vec::new()),
            generation: AtomicU64::new(1),
        });
        // Every minute, the levels with unsaved edits are written where the next session can offer them back. The
        // thread holds the host weakly and ends with it.
        let weak = Arc::downgrade(&shared);
        let _ = std::thread::Builder::new().name("bigstar-autosave".to_owned()).spawn(move || {
            loop {
                std::thread::sleep(AUTOSAVE_INTERVAL);
                let Some(shared) = weak.upgrade() else { break };
                shared.write_recovery_snapshots();
            }
        });
        Ok(Self { shared })
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
        Shared::handle(&self.shared, request)
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

    fn handle(self: &Arc<Self>, request: Request) -> Result<Response> {
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
            Request::GetTextures { ids } => self.textures(&ids),
            Request::LevelGraphics { level } => self.level_graphics(level),
            Request::ActorCatalog => self.actor_catalog(),
            Request::RenderActors { inputs } => self.render_actors(&inputs),
            Request::ProjectSettings => self.project_settings(),
            Request::SetActiveVariant { variant } => self.set_active_variant(variant),
            Request::SetActiveModule { module } => self.set_active_module(module),
            Request::ProjectData => self.project_data(),
            Request::LevelBytes { level } => self.level_bytes(level),
            Request::FindEntities { sprites, id } => self.find_entities(sprites, id),
            Request::Recovery => self.recovery(),
            Request::Recover { accept } => self.recover(accept),
            Request::Filesystem => self.filesystem(),
            Request::ListArchive { path } => self.list_archive(&path),
            Request::ReadFile { path } => self.read_file(&path),
            Request::WriteFile { path, bytes } => self.write_file(&path, &bytes),
            Request::FileTargets { path } => self.file_targets(&path),
            Request::RemoveFile { path } => self.remove_file(&path),
            Request::Banner => self.banner(),
            Request::WriteBanner { bytes } => self.write_banner(&bytes),
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

    fn open_project(self: &Arc<Self>, path: &str) -> Result<Response> {
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
        let mut settings = load_settings(&directory);
        if settings.generated_path.is_empty() {
            settings.generated_path = DEFAULT_GENERATED_DIR.to_owned();
        }
        let catalog = load_module_data_catalog(&directory, Path::new(&settings.generated_path))?;
        let index = rom.as_ref().map(RomIndex::new).unwrap_or_else(RomIndex::new_empty);
        let file_count = rom.as_ref().map_or(0, |rom| rom.files().len());
        let info = project_info(&self.workspace, &directory, &project, rom_path.clone(), file_count);
        let had_project = state.open.is_some();
        // A watcher that cannot start costs live refreshes, not the project.
        let watcher =
            ProjectWatcher::start(&directory, Arc::downgrade(self)).map_err(|e| log::warn!("not watching {}: {e}", directory.display())).ok();
        state.open = Some(OpenProject {
            directory,
            rom,
            rom_file: rom_file.is_file().then_some(rom_file),
            index,
            catalog,
            levels: BTreeMap::new(),
            next_level: 1,
            settings,
            patcher: None,
            banner: BannerTarget::Unreported,
            filesystem: None,
            graphics: None,
            _watcher: watcher,
        });
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
                .map(|area| (area, open.rom.as_mut().and_then(|rom| locate(rom, &open.index, area))))
                .map(|(area, files)| AreaSummary {
                    token: area.source_token.clone(),
                    display_name: area.display_name.clone(),
                    available: files.is_some(),
                    course_path: files.as_ref().map(|f| address(&f.course)).unwrap_or_default(),
                    bgdat_path: files.as_ref().map(|f| address(&f.bgdat)).unwrap_or_default(),
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
        let (entry, area) = find_area(area_token).ok_or_else(|| not_found(format!("there is no level area {area_token}")))?;
        let rom = open.rom.as_mut().ok_or_else(|| not_found("the project has no ROM"))?;
        let files = locate(rom, &open.index, area).ok_or_else(|| not_found(format!("the ROM does not hold {area_token}")))?;
        let course = read_source(rom, &open.directory, &files.course)?;
        let bgdat = read_source(rom, &open.directory, &files.bgdat)?;
        let level = EditableLevel::load(&course, &bgdat, Some(&open.catalog))?;
        let title = area_title(entry, area);
        let id = open.next_level;
        open.next_level += 1;
        let session = LevelSession { area: area_token.to_owned(), title, files, level, revision: 1 };
        let response = Response::Level(Box::new(snapshot(id, &session)));
        open.levels.insert(id, session);
        Ok(response)
    }

    fn edit_level(&self, id: LevelId, base_revision: u64, edit: LevelEdit) -> Result<Response> {
        let mut state = lock(&self.state);
        let catalog = state.open.as_ref().map(|open| open.catalog.clone());
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
            LevelEdit::SetGlue { glue, name } => level.set_glue(glue, &name).map(|()| EditOutcome::None),
            LevelEdit::ReplaceSerialized { course, bgdat, name } => {
                level.replace_serialized(&course, &bgdat, catalog.as_ref(), &name).map(|()| EditOutcome::None)
            }
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

    fn generation(&self) -> u64 {
        self.generation.load(Ordering::SeqCst)
    }

    /// Asks NCPatcher about the open project once: the configured variants, the module graph, and the plan for the
    /// active variant. Without NCPatcher the editor reads the ROM alone, and says why.
    fn ensure_patcher(&self, open: &mut OpenProject) {
        if open.patcher.is_some() {
            return;
        }
        let client = NCPatcherClient::new(&self.options.ncpatcher);
        let mut errors = Vec::new();
        let config = client.dump_config(&open.directory).map_err(|e| errors.push(e.message)).ok();
        let modules = client.dump_modules(&open.directory).map_err(|e| errors.push(e.message)).ok();
        let plan = if config.is_some() {
            client.plan_files(&open.directory, &open.settings.active_variant).map_err(|e| errors.push(e.message)).ok()
        } else {
            None
        };
        open.banner = match &config {
            None => BannerTarget::Unreported,
            Some(config) => {
                let variant = config.variants.iter().find(|variant| variant.name == open.settings.active_variant);
                let file = variant.map(|variant| variant.banner.clone()).filter(|banner| !banner.as_os_str().is_empty());
                match file.unwrap_or_else(|| config.rom_banner.clone()) {
                    banner if banner.as_os_str().is_empty() => BannerTarget::Unconfigured,
                    banner => BannerTarget::File(banner),
                }
            }
        };
        open.settings.variants = config.map(|config| config.variants.into_iter().map(|variant| variant.name).collect()).unwrap_or_default();
        // Without NCPatcher, the last build's own record of the modules still says where their trees are.
        let modules = modules.or_else(|| {
            let generated = if open.settings.generated_path.is_empty() { DEFAULT_GENERATED_DIR } else { &open.settings.generated_path };
            let file = open.directory.join(generated).join("modules.json");
            let text = String::from_utf8(read_file(&file).ok()?).ok()?;
            bigstar_patching::parse_module_graph(&text).ok()
        });
        let modules = modules.unwrap_or_default();
        open.settings.modules = modules.modules.iter().filter(|module| module.enabled).map(|module| module.key.clone()).collect();
        open.settings.patcher_error = errors.into_iter().next();
        open.patcher = Some((modules, plan));
    }

    fn ensure_graphics<'a>(&self, open: &'a mut OpenProject) -> Result<&'a mut ProjectGraphics> {
        if open.graphics.is_none() {
            self.ensure_patcher(open);
            let (modules, plan) = open.patcher.clone().unwrap_or_default();
            let graphics =
                ProjectGraphics::new(&open.directory, &self.options.data_dir, open.rom_file.as_deref(), plan, modules, open.catalog.clone())?;
            open.graphics = Some(graphics);
        }
        open.graphics.as_mut().ok_or_else(no_project)
    }

    fn textures(&self, ids: &[TextureId]) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let graphics = self.ensure_graphics(open)?;
        Ok(Response::Textures(ids.iter().filter_map(|id| graphics.textures.get(id)).collect()))
    }

    fn level_graphics(&self, id: LevelId) -> Result<Response> {
        let generation = self.generation();
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let references = open.levels.get(&id).ok_or_else(unknown_level)?.level.tileset_references();
        let graphics = self.ensure_graphics(open)?;
        Ok(Response::LevelGraphics(Box::new(graphics.level_graphics(id, generation, references))))
    }

    fn actor_catalog(&self) -> Result<Response> {
        let generation = self.generation();
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let graphics = self.ensure_graphics(open)?;
        Ok(Response::ActorCatalog(Box::new(graphics.actor_catalog(generation))))
    }

    fn render_actors(&self, inputs: &[ActorInput]) -> Result<Response> {
        let generation = self.generation();
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let graphics = self.ensure_graphics(open)?;
        Ok(Response::ActorDraws { generation, draws: graphics.render_actors(inputs) })
    }

    fn project_settings(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        self.ensure_patcher(open);
        Ok(Response::Settings(open.settings.clone()))
    }

    fn project_data(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        // The table sits in the decompressed ARM9; a ROM whose ARM9 does not hold it simply has none.
        let object_bank_overlays = open
            .rom
            .as_mut()
            .and_then(|rom| rom.read_arm9().ok())
            .and_then(|arm9| decompress_blz_overlay(&arm9).ok())
            .and_then(|arm9| find_object_bank_overlays(&arm9).ok());
        Ok(Response::ProjectData(Box::new(ProjectData { modules: open.catalog.clone(), object_bank_overlays })))
    }

    fn level_bytes(&self, id: LevelId) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let session = open.levels.get(&id).ok_or_else(unknown_level)?;
        let file_id = |source: &Source| match source {
            Source::File(path) => open.rom.as_ref().and_then(|rom| rom.file_id(path)),
            Source::Member { .. } => None,
        };
        Ok(Response::LevelBytes(Box::new(LevelBytes {
            course: session.level.serialize_course()?,
            bgdat: session.level.serialize_bgdat()?,
            course_file_id: file_id(&session.files.course),
            bgdat_file_id: file_id(&session.files.bgdat),
        })))
    }

    /// Reads every level the ROM holds -- an open session as it stands, the rest as saved -- and lists where object
    /// (or sprite type) `id` is placed.
    fn find_entities(&self, sprites: bool, id: u16) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let mut found = Vec::new();
        for entry in nsmb_level_catalog() {
            for area in &entry.areas {
                let title = area_title(entry, area);
                let loaded;
                let level = match open.levels.values().find(|session| session.area == area.source_token) {
                    Some(session) => &session.level,
                    None => {
                        let Some(rom) = open.rom.as_mut() else { continue };
                        let Some(files) = locate(rom, &open.index, area) else { continue };
                        let (Ok(course), Ok(bgdat)) =
                            (read_source(rom, &open.directory, &files.course), read_source(rom, &open.directory, &files.bgdat))
                        else {
                            continue;
                        };
                        let Ok(level) = EditableLevel::load(&course, &bgdat, Some(&open.catalog)) else { continue };
                        loaded = level;
                        &loaded
                    }
                };
                let place =
                    |index, x: i32, y: i32, settings| FoundEntity { area: area.source_token.clone(), title: title.clone(), index, x, y, settings };
                if sprites {
                    let matches = level.course().sprites.iter().enumerate().filter(|(_, s)| s.id == id);
                    found.extend(matches.map(|(i, s)| place(i, i32::from(s.x), i32::from(s.y), Some(s.settings))));
                } else {
                    let matches = level.objects().iter().enumerate().filter(|(_, o)| o.object_id == id);
                    found.extend(matches.map(|(i, o)| place(i, i32::from(o.x), i32::from(o.y), None)));
                }
            }
        }
        Ok(Response::Found(found))
    }

    /// Writes an autosave of every level with unsaved edits, and removes the autosaves of levels that have none. The
    /// host does this every minute; a session that ends without saving leaves its autosaves for the next to offer.
    pub(crate) fn write_recovery_snapshots(&self) {
        let mut state = lock(&self.state);
        let Some(open) = state.open.as_mut() else { return };
        let root = open.directory.join(AUTOSAVE_DIR);
        let mut keep = std::collections::BTreeSet::new();
        for session in open.levels.values().filter(|session| session.level.dirty()) {
            let written = (|| -> Result<String> {
                let snapshot = LevelRecoverySnapshot {
                    course_path: edit_path(&session.files.course)?.to_string_lossy().replace('\\', "/"),
                    bgdat_path: edit_path(&session.files.bgdat)?.to_string_lossy().replace('\\', "/"),
                    package: LevelPackage {
                        course: session.level.serialize_course()?,
                        bgdat: session.level.serialize_bgdat()?,
                        ..Default::default()
                    },
                };
                let name = recovery_name(&session.area);
                std::fs::create_dir_all(&root).map_err(|_| Error::new(ErrorCode::IoError, "could not create the autosave directory"))?;
                write_file_atomically(&root.join(&name), &encode_level_recovery(&snapshot)?)?;
                Ok(name)
            })();
            match written {
                Ok(name) => {
                    keep.insert(name);
                }
                Err(error) => self.log(LogLevel::Warning, format!("Autosave of {}: {}", session.title, error.message)),
            }
        }
        for (name, path) in recovery_files(&root) {
            if !keep.contains(&name) {
                let _ = std::fs::remove_file(path);
            }
        }
    }

    fn recovery(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let root = open.directory.join(AUTOSAVE_DIR);
        let mut levels = Vec::new();
        for (name, _) in recovery_files(&root) {
            let Some(area) = recovery_area(&name) else { continue };
            // A level already open here is this session's own autosave, not one left behind.
            if open.levels.values().any(|session| session.area == area) {
                continue;
            }
            let title = find_area(&area).map_or_else(|| area.clone(), |(entry, found)| area_title(entry, found));
            levels.push(RecoverableLevel { area, title });
        }
        Ok(Response::Recovery(levels))
    }

    fn recover(&self, accept: bool) -> Result<Response> {
        let root = {
            let state = lock(&self.state);
            state.open.as_ref().ok_or_else(no_project)?.directory.join(AUTOSAVE_DIR)
        };
        let files = recovery_files(&root);
        if !accept {
            for (_, path) in files {
                let _ = std::fs::remove_file(path);
            }
            return Ok(Response::Recovered(Vec::new()));
        }
        let mut recovered = Vec::new();
        for (name, path) in files {
            let Some(area) = recovery_area(&name) else { continue };
            let applied = (|| -> Result<()> {
                let snapshot = decode_level_recovery(&read_file(&path)?)?;
                let Response::Level(opened) = self.open_level(&area)? else { return Err(invalid("the level did not open")) };
                let mut state = lock(&self.state);
                let catalog = state.open.as_ref().map(|open| open.catalog.clone());
                let session = session_mut(&mut state, opened.level)?;
                session.level.replace_serialized(&snapshot.package.course, &snapshot.package.bgdat, catalog.as_ref(), "Recover autosave")?;
                session.revision += 1;
                let (_, event) = snapshot_and_event(opened.level, session);
                drop(state);
                self.emit(event);
                Ok(())
            })();
            match applied {
                Ok(()) => recovered.push(area),
                Err(error) => self.log(LogLevel::Warning, format!("Recovery of {area}: {}", error.message)),
            }
        }
        if !recovered.is_empty() {
            self.log(LogLevel::Info, format!("Recovered {} autosaved level(s). Save them to keep them.", recovered.len()));
        }
        Ok(Response::Recovered(recovered))
    }

    /// The filesystem view of the open project, built once per change of files or variant.
    fn ensure_filesystem<'a>(&self, open: &'a mut OpenProject) -> Result<&'a FilesystemView> {
        if open.filesystem.is_none() {
            self.ensure_patcher(open);
            let (mut modules, plan) = open.patcher.clone().unwrap_or_default();
            crate::filesystem::localize_modules(&mut modules, &open.directory);
            let generated =
                if open.settings.generated_path.is_empty() { DEFAULT_GENERATED_DIR.to_owned() } else { open.settings.generated_path.clone() };
            let manifest_file = open.directory.join(generated).join("files.json");
            let manifest = manifest_file
                .is_file()
                .then(|| read_file(&manifest_file).ok())
                .flatten()
                .and_then(|bytes| String::from_utf8(bytes).ok().and_then(|text| bigstar_patching::parse_file_manifest(&text).ok()));
            let view = build_filesystem_view(FilesystemViewRequest {
                rom: open.rom.as_mut(),
                project_dir: open.directory.clone(),
                modules,
                plan,
                manifest,
                variant: open.settings.active_variant.clone(),
            })?;
            open.filesystem = Some(view);
        }
        open.filesystem.as_ref().ok_or_else(no_project)
    }

    /// The module a write lands in: the one already supplying the file, else the active one, else the first that can.
    fn target_module(open: &OpenProject, path: &str) -> Result<ModuleInfo> {
        let mut modules = open.patcher.as_ref().map(|(modules, _)| modules.clone()).unwrap_or_default();
        crate::filesystem::localize_modules(&mut modules, &open.directory);
        let supplier = open.filesystem.as_ref().and_then(|view| view.find(path)).map(|entry| entry.module.clone()).filter(|m| !m.is_empty());
        let key = supplier.unwrap_or_else(|| open.settings.active_module.clone());
        let found = modules.find(&key).cloned().or_else(|| modules.modules.iter().find(|m| m.enabled && m.nitrofs.is_some()).cloned());
        found.ok_or_else(|| invalid("choose an active module target first"))
    }

    fn filesystem(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let view = self.ensure_filesystem(open)?.clone();
        let mut entries = Vec::with_capacity(view.entries.len());
        for entry in &view.entries {
            // The head is enough to recognise a format; members are named only, since reading one means reading its
            // archive.
            let head = if !entry.source.as_os_str().is_empty() {
                let source = if entry.source.is_absolute() { entry.source.clone() } else { open.directory.join(&entry.source) };
                read_file_prefix(&source, crate::filesystem::HEAD).unwrap_or_default()
            } else if entry.inside_archive() {
                Vec::new()
            } else {
                open.rom.as_mut().and_then(|rom| rom.read_prefix(&entry.path, crate::filesystem::HEAD).ok()).unwrap_or_default()
            };
            entries.push(crate::filesystem::entry(entry, crate::filesystem::icon(&head, &entry.path)));
        }
        let modules: Vec<ModuleChoice> = open
            .patcher
            .as_ref()
            .map(|(graph, _)| graph.modules.iter().filter(|m| m.enabled && m.nitrofs.is_some()).collect::<Vec<_>>())
            .unwrap_or_default()
            .into_iter()
            .map(|m| ModuleChoice { key: m.key.clone(), name: if m.name.is_empty() { m.key.clone() } else { m.name.clone() } })
            .collect();
        let active_module = Some(open.settings.active_module.clone())
            .filter(|key| modules.iter().any(|m| &m.key == key))
            .or_else(|| modules.first().map(|m| m.key.clone()))
            .unwrap_or_default();
        Ok(Response::Filesystem(Box::new(FilesystemListing {
            pending: view.has_pending(),
            entries,
            modules,
            active_module,
            active_variant: open.settings.active_variant.clone(),
            error: open.settings.patcher_error.clone(),
        })))
    }

    /// The members of an archive the plan had no reason to name, read from the container itself.
    fn list_archive(&self, container: &str) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let known: std::collections::BTreeSet<String> = self.ensure_filesystem(open)?.entries.iter().map(|e| e.path.clone()).collect();
        let rom = open.rom.as_mut().ok_or_else(|| not_found("the project has no ROM"))?;
        let mut rows = Vec::new();
        for member in rom.list_archive(container)? {
            // A member the container does not name has no address to open it by.
            if member.path.is_empty() {
                continue;
            }
            let path = format!("{container}!{}", member.path);
            if known.contains(&path) {
                continue;
            }
            let head = rom
                .read_member_id(container, usize::from(member.id))
                .map(|bytes| bytes.into_iter().take(crate::filesystem::HEAD).collect::<Vec<_>>());
            let icon = crate::filesystem::icon(&head.unwrap_or_default(), &path);
            rows.push(FileEntry { path, member_id: Some(i64::from(member.id)), size: u64::from(member.size), icon, ..FileEntry::default() });
        }
        Ok(Response::Files(rows))
    }

    fn read_file(&self, path: &str) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let bytes = self.ensure_graphics(open)?.read_file(path)?;
        Ok(Response::FileBytes { path: path.to_owned(), bytes })
    }

    fn write_file(&self, path: &str, bytes: &[u8]) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let known = self.ensure_filesystem(open)?.find(path).is_some();
        if !known && !path.starts_with("z_new/") {
            return Err(invalid("New NitroFS files must be under z_new/."));
        }
        let module = Self::target_module(open, path)?;
        let target = module_file_target(&module, &open.settings.active_variant, path, &[])?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "could not create module target directory"))?;
        }
        write_file_atomically(&target, bytes)?;
        // What the dock shows and what graphics read from have both changed.
        open.filesystem = None;
        open.patcher = None;
        open.graphics = None;
        drop(state);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::GraphicsChanged { generation });
        self.log(LogLevel::Info, format!("Wrote {path} to module {}", module.key));
        Ok(Response::Done)
    }

    fn file_targets(&self, path: &str) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        self.ensure_filesystem(open)?;
        let module = Self::target_module(open, path)?;
        let variants = std::iter::once(String::new()).chain(open.settings.variants.iter().cloned());
        let holding = variants.filter(|variant| module_file_target(&module, variant, path, &[]).is_ok_and(|target| target.is_file())).collect();
        Ok(Response::FileTargets(holding))
    }

    /// Moves a module's copy of a file into `.big-star-trash`, kept for the person to restore by hand.
    fn remove_file(&self, path: &str) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let entry = self.ensure_filesystem(open)?.find(path).cloned().ok_or_else(|| not_found(format!("there is no file {path}")))?;
        if entry.inside_archive() {
            return Err(invalid("a member of an archive cannot be removed; replace it instead"));
        }
        if entry.source.as_os_str().is_empty() || entry.module.is_empty() {
            return Err(invalid("only a file a module supplies can be removed"));
        }
        let source = if entry.source.is_absolute() { entry.source.clone() } else { open.directory.join(&entry.source) };
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let trash = open.directory.join(TRASH_DIR).join(stamp.to_string()).join(path.replace('!', "/"));
        if let Some(parent) = trash.parent() {
            std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "could not create the trash directory"))?;
        }
        std::fs::rename(&source, &trash).map_err(|_| Error::new(ErrorCode::IoError, "could not move the module file to the trash"))?;
        open.filesystem = None;
        open.patcher = None;
        open.graphics = None;
        drop(state);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::GraphicsChanged { generation });
        self.log(LogLevel::Info, format!("Moved {path} from module {} to {TRASH_DIR}", entry.module));
        Ok(Response::Done)
    }

    /// The configured banner file, or why there is nowhere for an edited banner to go.
    fn banner_target(&self, open: &mut OpenProject) -> Result<PathBuf> {
        self.ensure_patcher(open);
        match &open.banner {
            BannerTarget::File(file) if file.is_relative() => Ok(open.directory.join(file)),
            BannerTarget::File(file) => Ok(file.clone()),
            // `config dump` reports both the project banner and the variant's override, so an empty answer means the
            // project configures no banner at all. Asking for a file would invent a source no build reads; the fix
            // belongs in ncpatcher.yaml.
            BannerTarget::Unconfigured => Err(invalid(
                "This project configures no rom.banner, so there is nowhere for an edited banner to go. Add one to ncpatcher.yaml and the editor will write to it.",
            )),
            BannerTarget::Unreported => {
                Err(invalid("NCPatcher has not reported this project's configuration yet, so there is nowhere for an edited banner to go."))
            }
        }
    }

    /// The configured banner once it exists, and otherwise the one in the ROM, which is what the game shows today.
    fn banner(&self) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let target = self.banner_target(open)?;
        let bytes =
            if target.is_file() { read_file(&target)? } else { open.rom.as_mut().ok_or_else(|| not_found("no built ROM is open"))?.read_banner()? };
        let shown = target.strip_prefix(&open.directory).unwrap_or(&target).to_string_lossy().replace('\\', "/");
        Ok(Response::Banner { bytes, target: shown })
    }

    fn write_banner(&self, bytes: &[u8]) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let target = self.banner_target(open)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "Could not create the configured banner directory."))?;
        }
        write_file_atomically(&target, bytes)?;
        drop(state);
        self.log(LogLevel::Info, format!("Saved ROM banner: {}", target.display()));
        Ok(Response::Done)
    }

    fn set_active_variant(&self, variant: String) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        if open.levels.values().any(|session| session.level.dirty()) {
            return Err(invalid("save or close the open levels before switching the variant"));
        }
        // Everything read through the old variant's layers is now wrong: the level sessions and the graphics.
        open.settings.active_variant = variant;
        open.levels.clear();
        open.patcher = None;
        open.filesystem = None;
        open.graphics = None;
        save_settings(&open.directory, &open.settings)?;
        self.ensure_patcher(open);
        let settings = open.settings.clone();
        drop(state);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::SettingsChanged(settings.clone()));
        self.emit(Event::GraphicsChanged { generation });
        Ok(Response::Settings(settings))
    }

    fn set_active_module(&self, module: String) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        open.settings.active_module = module;
        save_settings(&open.directory, &open.settings)?;
        let settings = open.settings.clone();
        drop(state);
        self.emit(Event::SettingsChanged(settings.clone()));
        Ok(Response::Settings(settings))
    }

    /// The watcher's debounced report. Every change is announced; one that is not the host's own saving makes the
    /// graphics stale, so they are dropped and rebuilt on the next request.
    pub(crate) fn files_changed(&self, paths: Vec<PathBuf>) {
        let mut state = lock(&self.state);
        let Some(open) = state.open.as_mut() else { return };
        let own = |path: &Path| {
            path.strip_prefix(&open.directory).is_ok_and(|relative| {
                relative.starts_with(crate::levels::EDITS_DIR)
                    || relative.components().next().is_some_and(|first| first.as_os_str().to_string_lossy().starts_with(".big-star"))
            })
        };
        let stale = paths.iter().any(|path| !own(path));
        if stale {
            open.graphics = None;
            open.filesystem = None;
            open.patcher = None;
        }
        drop(state);
        let mut relative: Vec<String> = paths.iter().filter_map(|path| self.workspace.relative(path)).collect();
        relative.sort();
        relative.dedup();
        if !relative.is_empty() {
            self.emit(Event::FilesChanged { paths: relative });
        }
        if stale {
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.emit(Event::GraphicsChanged { generation });
        }
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

const AUTOSAVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

/// Where removed module files go, relative to the project.
pub(crate) const TRASH_DIR: &str = ".big-star-trash";

/// A level file as the Filesystem dock names it.
fn address(source: &Source) -> String {
    match source {
        Source::File(path) => path.clone(),
        Source::Member { archive, member } => format!("{archive}!{member}"),
    }
}

/// Where autosaves live, relative to the project.
pub(crate) const AUTOSAVE_DIR: &str = ".big-star-autosave";

/// An autosave is named for its area, hex-encoded so any token makes a file name.
fn recovery_name(area: &str) -> String {
    let hex: String = area.bytes().map(|b| format!("{b:02x}")).collect();
    format!("{hex}.bsrecover")
}

#[cfg(test)]
pub(crate) fn recovery_name_for_tests(area: &str) -> String {
    recovery_name(area)
}

fn recovery_area(name: &str) -> Option<String> {
    let hex = name.strip_suffix(".bsrecover")?;
    let bytes: Option<Vec<u8>> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok()).collect();
    String::from_utf8(bytes?).ok()
}

/// The autosaves in `root`, by file name, in name order.
fn recovery_files(root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut files: Vec<(String, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file())
        .filter_map(|entry| Some((entry.file_name().into_string().ok()?, entry.path())))
        .filter(|(name, _)| name.ends_with(".bsrecover"))
        .collect();
    files.sort();
    files
}

fn find_area(token: &str) -> Option<(&'static LevelCatalogEntry, &'static LevelCatalogArea)> {
    nsmb_level_catalog().iter().find_map(|entry| entry.areas.iter().find(|area| area.source_token == token).map(|area| (entry, area)))
}

/// "1-1", or "1-1 Area 2" for a level of several areas.
fn area_title(entry: &LevelCatalogEntry, area: &LevelCatalogArea) -> String {
    if entry.areas.len() == 1 { entry.display_name.clone() } else { format!("{} {}", entry.display_name, area.display_name) }
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
        glue: session.level.glue().clone(),
        stage_object_id_base: session.level.stage_object_id_base(),
        registration_hashes: session.level.registration_hashes(),
        sprite_hashes: (0..session.level.course().sprites.len()).map(|index| session.level.sprite_hash(index)).collect(),
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
