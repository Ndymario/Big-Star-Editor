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
use bigstar_patching::{FileManifest, ModuleGraph, ModuleInfo, NCPatcherClient};
use bigstar_project::{
    DEFAULT_GENERATED_DIR, FilesystemView, FilesystemViewRequest, LevelCatalogArea, LevelCatalogEntry, RomReader, build_filesystem_view,
    find_object_bank_overlays, is_ncpatcher_project, load_module_data_catalog, module_file_target, nsmb_level_catalog, project_name, project_rom,
    prune_patch_backups,
};
use bigstar_protocol::{
    ActorInput, AreaSummary, DirectoryEntry, DirectoryEntryKind, DirectoryListing, EditOutcome, Event, FileEntry, FilesystemListing, FoundEntity,
    LevelBytes, LevelEdit, LevelId, LevelSnapshot, LevelSummary, LevelWorld, LogLevel, ModuleChoice, ProjectData, ProjectInfo, ProjectSettings,
    ProjectSummary, RecoverableLevel, Request, Response, Service, TextureId,
};

use crate::graphics::ProjectGraphics;
use crate::levels::{LevelFiles, RomIndex, Source, address, locate};
use crate::settings::{clean_rom_file, clean_rom_problem, load_settings, relative_to_project, save_settings};
use crate::watch::ProjectWatcher;
use crate::workspace::Workspace;

mod build;
mod rom;
mod uploads;

use rom::Backing;

/// What a host needs besides its root: where the shipped actor, tile and background scripts are, and which NCPatcher to ask.
#[derive(Debug, Clone)]
pub struct HostOptions {
    pub data_dir: PathBuf,
    pub ncpatcher: PathBuf,
    /// The xdelta3 executable the patch tools run.
    pub xdelta: PathBuf,
    /// The largest ROM a connection may upload.
    pub max_rom_bytes: usize,
    /// How many uploaded ROMs the host holds at once, across every connection; each one is a whole ROM in memory.
    pub max_uploaded_roms: usize,
}

/// A retail NSMB ROM is 8 MiB and an expanded one rarely passes 32; this leaves room without letting one connection
/// take the host's memory.
pub const DEFAULT_MAX_ROM_BYTES: usize = 64 * 1024 * 1024;
pub const DEFAULT_MAX_UPLOADED_ROMS: usize = 8;

impl Default for HostOptions {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            ncpatcher: PathBuf::from("ncpatcher"),
            xdelta: PathBuf::from("xdelta3"),
            max_rom_bytes: DEFAULT_MAX_ROM_BYTES,
            max_uploaded_roms: DEFAULT_MAX_UPLOADED_ROMS,
        }
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
    /// Where NCPatcher writes the active variant's build, from its configuration; asked with the rest of it.
    built_rom: Option<PathBuf>,
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
    /// Level files the host itself wrote, which the watcher is not to treat as someone else's change.
    own_writes: std::collections::HashSet<PathBuf>,
    /// Where an edited ROM banner goes, from NCPatcher's configuration; asked with the rest of it.
    banner: BannerTarget,
    _watcher: Option<ProjectWatcher>,
    backing: Backing,
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
    /// The folder of NCPatcher projects this host serves. A host that only edits ROMs has none.
    workspace: Option<Workspace>,
    /// Whether ROMs may be opened and saved by their path on this machine: the desktop app's own host, never a served one.
    local_files: bool,
    options: HostOptions,
    state: Mutex<State>,
    subscribers: Mutex<Vec<Sender<Event>>>,
    /// Grows whenever a watched file changes or the variant switches; graphics answers carry it.
    generation: AtomicU64,
    /// The running build's cancel token, while one runs.
    build: Mutex<Option<Arc<bigstar_patching::ProcessCancel>>>,
    /// How many uploaded ROMs the connections to this host hold, finished or arriving.
    uploads: std::sync::atomic::AtomicUsize,
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
        Ok(Self::start(Shared::new(Some(Workspace::new(root)?), false, options)))
    }

    /// The desktop app's own host for the workspace at `root`, which also takes files on this machine by their path, such
    /// as a clean ROM kept outside the workspace.
    pub fn for_desktop(root: &Path) -> Result<Self> {
        Ok(Self::start(Shared::new(Some(Workspace::new(root)?), true, HostOptions::default())))
    }

    /// The desktop app's host for a ROM picked on this machine: no workspace, and ROMs opened and saved by their path.
    pub fn for_local_roms(options: HostOptions) -> Self {
        Self::start(Shared::new(None, true, options))
    }

    /// A served host with no workspace: every connection edits only the ROM it uploads.
    pub fn for_uploads(options: HostOptions) -> Self {
        Self::start(Shared::new(None, false, options))
    }

    fn start(shared: Arc<Shared>) -> Self {
        // Every few minutes, as the project's preference says, the levels with unsaved edits are written where the next
        // session can offer them back. The thread holds the host weakly and ends with it.
        let weak = Arc::downgrade(&shared);
        let _ = std::thread::Builder::new().name("bigstar-autosave".to_owned()).spawn(move || {
            let mut last = std::time::Instant::now();
            loop {
                std::thread::sleep(AUTOSAVE_TICK);
                let Some(shared) = weak.upgrade() else { break };
                if last.elapsed() >= shared.autosave_interval() {
                    shared.write_recovery_snapshots();
                    last = std::time::Instant::now();
                }
            }
        });
        Self { shared }
    }

    /// The workspace's folder name; empty for a host without one.
    pub fn workspace_name(&self) -> String {
        self.shared.workspace.as_ref().map(Workspace::name).unwrap_or_default()
    }

    pub fn has_workspace(&self) -> bool {
        self.shared.workspace.is_some()
    }

    /// A new connection's view of the host. Events from all connections reach every session.
    pub fn session(&self) -> HostSession {
        HostSession { shared: Arc::clone(&self.shared), events: Some(self.shared.subscribe()), guest: false, uploads: uploads::Uploads::default() }
    }

    /// A connection that did not give the workspace token: it may upload a ROM and edit that, and reaches nothing of the
    /// workspace, its events included.
    pub fn guest_session(&self) -> HostSession {
        HostSession { shared: Arc::clone(&self.shared), events: None, guest: true, uploads: uploads::Uploads::default() }
    }
}

pub struct HostSession {
    shared: Arc<Shared>,
    /// The workspace's events; a guest hears none.
    events: Option<Receiver<Event>>,
    guest: bool,
    /// The ROM this connection uploaded, and one arriving.
    uploads: uploads::Uploads,
}

#[cfg(test)]
impl HostSession {
    pub(crate) fn shared(&self) -> Arc<Shared> {
        Arc::clone(&self.shared)
    }
}

impl Service for HostSession {
    fn handle(&mut self, request: Request) -> Result<Response> {
        self.route(request)
    }

    fn drain_events(&mut self) -> Vec<Event> {
        let mut events: Vec<Event> = self.events.as_ref().map(|events| events.try_iter().collect()).unwrap_or_default();
        events.extend(self.uploads.drain_events());
        events
    }
}

impl Shared {
    fn new(workspace: Option<Workspace>, local_files: bool, options: HostOptions) -> Arc<Self> {
        Arc::new(Shared {
            workspace,
            local_files,
            options,
            state: Mutex::new(State::default()),
            subscribers: Mutex::new(Vec::new()),
            generation: AtomicU64::new(1),
            build: Mutex::new(None),
            uploads: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    fn subscribe(&self) -> Receiver<Event> {
        let (sender, events) = channel();
        lock(&self.subscribers).push(sender);
        events
    }

    /// The workspace, or why there is none to reach.
    fn workspace(&self) -> Result<&Workspace> {
        self.workspace.as_ref().ok_or_else(|| invalid("This host has no workspace. It only edits ROMs."))
    }

    fn has_unsaved_levels(&self) -> bool {
        lock(&self.state).open.as_ref().is_some_and(|open| open.levels.values().any(|session| session.level.dirty()))
    }

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
            Request::OpenProject { path } => self.open_project(&path),
            Request::OpenRom { path } => self.open_rom(&path),
            Request::SaveRomAs { path } => self.save_rom_as(&path),
            Request::UploadRom { .. } => Err(invalid("a ROM is uploaded through a connection")),
            Request::DownloadRom { offset } => self.download_rom(offset),
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
            Request::SetActiveVariant { variant } => self.ncpatcher_only().and_then(|()| self.set_active_variant(variant)),
            Request::SetActiveModule { module } => self.ncpatcher_only().and_then(|()| self.set_active_module(module)),
            Request::SetCleanRom { path } => self.ncpatcher_only().and_then(|()| self.set_clean_rom(&path)),
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
            Request::RemoveFile { path } => self.ncpatcher_only().and_then(|()| self.remove_file(&path)),
            Request::Banner => self.banner(),
            Request::WriteBanner { bytes } => self.write_banner(&bytes),
            Request::TilesetResources { level } => self.tileset_resources(level),
            Request::WriteFiles { files } => self.write_files(&files),
            Request::BuildConfig => self.ncpatcher_only().and_then(|()| self.build_config()),
            Request::SetPreferences { autosave_minutes, backups_kept } => {
                self.ncpatcher_only().and_then(|()| self.set_preferences(autosave_minutes, backups_kept))
            }
            Request::MigrateConfig => self.ncpatcher_only().and_then(|()| self.migrate_config()),
            Request::Build { verbose } => self.ncpatcher_only().and_then(|()| self.build(verbose)),
            Request::CancelBuild => self.cancel_build(),
            Request::ExtractRom { directory } => self.ncpatcher_only().and_then(|()| self.extract_rom(&directory)),
            Request::ExportPatch => self.ncpatcher_only().and_then(|()| self.export_patch()),
            Request::ImportPatch { bytes, accept_id_mismatch } => self.ncpatcher_only().and_then(|()| self.import_patch(&bytes, accept_id_mismatch)),
            Request::Xdelta { create, source, target, output } => self.xdelta(create, &source, &target, &output),
        }
    }

    fn list_projects(&self) -> Result<Response> {
        let mut found = Vec::new();
        self.scan(self.workspace()?.root(), 0, &mut found);
        found.sort_by(|a: &ProjectSummary, b| a.path.cmp(&b.path));
        Ok(Response::Projects(found))
    }

    /// Symlinks are not followed, so a link cannot lead the search out of the root.
    fn scan(&self, directory: &Path, depth: usize, found: &mut Vec<ProjectSummary>) {
        if depth > PROJECT_SEARCH_DEPTH || found.len() >= PROJECT_LIMIT {
            return;
        }
        if is_ncpatcher_project(directory) {
            if let Some(path) = self.workspace.as_ref().and_then(|workspace| workspace.relative(directory)) {
                found.push(ProjectSummary { path, name: project_name(directory) });
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
        let workspace = self.workspace()?;
        let directory = workspace.resolve(path)?;
        let read = std::fs::read_dir(&directory).map_err(|_| not_found("that directory cannot be read"))?;
        let mut entries = Vec::new();
        for entry in read.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            // A link that points out of the root is not shown, rather than shown and then refused.
            let Some(child) = workspace.relative(&directory).map(|base| if base.is_empty() { name.clone() } else { format!("{base}/{name}") }) else {
                continue;
            };
            if workspace.resolve(&child).is_err() {
                continue;
            }
            let Ok(metadata) = std::fs::metadata(entry.path()) else { continue };
            let kind = if metadata.is_dir() { DirectoryEntryKind::Directory } else { DirectoryEntryKind::File };
            entries.push(DirectoryEntry { name, kind, size: if metadata.is_dir() { 0 } else { metadata.len() } });
        }
        entries.sort_by(|a, b| (a.kind == DirectoryEntryKind::File, &a.name).cmp(&(b.kind == DirectoryEntryKind::File, &b.name)));
        let path = workspace.relative(&directory).unwrap_or_default();
        Ok(Response::Directory(DirectoryListing { path, entries }))
    }

    fn open_project(self: &Arc<Self>, path: &str) -> Result<Response> {
        let workspace = self.workspace()?;
        let directory = workspace.resolve(path)?;
        if !is_ncpatcher_project(&directory) {
            return Err(not_found("that directory is not an NCPatcher project: it has no ncpatcher.yaml"));
        }
        let mut state = lock(&self.state);
        // Another frontend opening the project that is open joins it, as a level opened twice is one session: reopening
        // would close everyone's levels.
        if let Some(open) = state.open.as_ref().filter(|open| !open.backing.is_rom() && open.directory == directory) {
            return Ok(Response::Project(project_info(workspace, &directory, open.rom.as_ref(), open.rom_file.as_deref())));
        }
        if state.open.as_ref().is_some_and(|open| open.levels.values().any(|session| session.level.dirty())) {
            return Err(invalid("save or close the open levels before opening another project"));
        }
        let mut settings = load_settings(&directory);
        if settings.generated_path.is_empty() {
            settings.generated_path = DEFAULT_GENERATED_DIR.to_owned();
        }
        // A clean ROM that is missing or wrong does not keep the project shut: the frontend asks for another, and
        // meanwhile the last build, if there is one, is still there to edit.
        settings.clean_rom_problem = clean_rom_problem(&directory, &settings);
        // Backups accumulate for as long as somebody keeps working; a project that opens sheds the oldest.
        prune_patch_backups(&directory, open_backups_kept(&settings));
        let catalog = load_module_data_catalog(&directory, Path::new(&settings.generated_path))?;
        // Which ROM is read depends on where NCPatcher builds the active variant, so NCPatcher is asked first.
        let mut opening = OpenProject {
            directory: directory.clone(),
            rom: None,
            rom_file: None,
            built_rom: None,
            index: RomIndex::new_empty(),
            catalog,
            levels: BTreeMap::new(),
            next_level: 1,
            settings,
            patcher: None,
            banner: BannerTarget::Unreported,
            filesystem: None,
            graphics: None,
            own_writes: std::collections::HashSet::new(),
            _watcher: None,
            backing: Backing::NcPatcher,
        };
        self.ensure_patcher(&mut opening);
        select_rom(&mut opening)?;
        let info = project_info(workspace, &directory, opening.rom.as_ref(), opening.rom_file.as_deref());
        let had_project = state.open.is_some();
        // A watcher that cannot start costs live refreshes, not the project.
        opening._watcher =
            ProjectWatcher::start(&directory, Arc::downgrade(self)).map_err(|e| log::warn!("not watching {}: {e}", directory.display())).ok();
        state.open = Some(opening);
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
                    course_path: files.as_ref().and_then(|f| address(&f.course).ok()).unwrap_or_default(),
                    bgdat_path: files.as_ref().and_then(|f| address(&f.bgdat).ok()).unwrap_or_default(),
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
        // Read as a build would use them: the module's copy when there is one, else the ROM's.
        let (course_address, bgdat_address) = (address(&files.course)?, address(&files.bgdat)?);
        let graphics = self.ensure_graphics(open)?;
        let course = graphics.read_file(&course_address)?;
        let bgdat = graphics.read_file(&bgdat_address)?;
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
            LevelEdit::SetCameras { cameras, name } => level.set_cameras(cameras, &name).map(|()| EditOutcome::None),
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
        let keep = open_backups_kept(&open.settings);
        let (course, bgdat, course_address, bgdat_address) = {
            let session = open.levels.get(&id).ok_or_else(unknown_level)?;
            let course = session.level.serialize_course()?;
            let bgdat = session.level.serialize_bgdat()?;
            EditableLevel::load(&course, &bgdat, None)
                .map_err(|e| Error::new(ErrorCode::ParseError, format!("serialized level failed validation: {}", e.message)))?;
            (course, bgdat, address(&session.files.course)?, address(&session.files.bgdat)?)
        };
        if open.backing.is_rom() {
            self.commit_rom(open, vec![(course_address, course), (bgdat_address, bgdat)], None)?;
            let session = open.levels.get_mut(&id).ok_or_else(unknown_level)?;
            session.level.mark_clean();
            session.revision += 1;
            let (snapshot, event) = snapshot_and_event(id, session);
            let message = format!("Saved {} into the ROM", session.title);
            drop(state);
            self.emit(event);
            self.log(LogLevel::Info, message);
            return Ok(Response::Edited { snapshot: Box::new(snapshot), outcome: EditOutcome::None });
        }
        // Into the module that supplies each file, or the active one, as every other file edit; never the ROM.
        self.ensure_filesystem(open)?;
        let variant = open.settings.active_variant.clone();
        let target = |path: &str| -> Result<PathBuf> {
            let module = Self::target_module(open, path).map_err(|_| invalid("choose an active module in the Filesystem dock before saving"))?;
            module_file_target(&module, &variant, path, &[])
        };
        let targets = [(target(&course_address)?, course), (target(&bgdat_address)?, bgdat)];
        back_up(&directory, "levels", &targets)?;
        write_all_or_none(&targets)?;
        prune_patch_backups(&directory, keep);
        open.own_writes.extend(targets.iter().map(|(path, _)| path.clone()));
        // The files are pending now: the dock and the plan reads go through are asked again.
        open.filesystem = None;
        open.patcher = None;
        self.ensure_patcher(open);
        let plan = open.patcher.as_ref().and_then(|(_, plan)| plan.clone());
        if let Some(graphics) = open.graphics.as_mut() {
            graphics.set_plan(plan);
        }
        let session = open.levels.get_mut(&id).ok_or_else(unknown_level)?;
        session.level.mark_clean();
        session.revision += 1;
        let (snapshot, event) = snapshot_and_event(id, session);
        let message = format!("Saved {} to its module; run NCPatcher to build it into the ROM", session.title);
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
        // A ROM edited directly has no configuration, modules or plan to ask about.
        if open.backing.is_rom() {
            open.patcher = Some(Default::default());
            return;
        }
        let client = self.ncpatcher(open);
        let mut errors = Vec::new();
        let config = client.dump_config(&open.directory).map_err(|e| errors.push(e.message)).ok();
        // A project whose settings name no clean ROM takes the one ncpatcher.yaml names (`rom.file`), when it is there.
        // NCPatcher was asked without `--rom`, so that is what it reports, and every command so far read the same ROM.
        if open.settings.clean_rom.is_empty()
            && let Some(rom_file) = config.as_ref().map(|config| open.directory.join(&config.rom_file)).filter(|file| file.is_file())
        {
            open.settings.clean_rom = relative_to_project(&open.directory, &rom_file);
            open.settings.clean_rom_problem = clean_rom_problem(&open.directory, &open.settings);
            if let Err(error) = save_settings(&open.directory, &open.settings) {
                log::warn!("could not remember the clean ROM: {}", error.message);
            }
            self.log(LogLevel::Info, format!("The clean ROM is {}, as ncpatcher.yaml names it", open.settings.clean_rom));
        }
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
        // A build that would land on the clean ROM itself is no build to read; NCPatcher would be patching it in place.
        let clean_rom = clean_rom_file(&open.directory, &open.settings);
        open.built_rom = config
            .as_ref()
            .map(|config| config.build_output(&open.settings.active_variant))
            .filter(|output| !output.as_os_str().is_empty() && Some(output) != clean_rom.as_ref());
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
        if open.graphics.is_none()
            && let Backing::Rom(doc) = &open.backing
        {
            let rom = RomReader::from_bytes(&doc.name, Arc::clone(&doc.original))?;
            open.graphics = Some(ProjectGraphics::for_rom(&self.options.data_dir, rom, &doc.edits));
        }
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
        // Levels not open are read as a build would use them; the graphics own that view of the files.
        self.ensure_graphics(open)?;
        let mut graphics = open.graphics.take();
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
                        let (Ok(course_address), Ok(bgdat_address)) = (address(&files.course), address(&files.bgdat)) else { continue };
                        let Some(graphics) = graphics.as_mut() else { continue };
                        let (Ok(course), Ok(bgdat)) = (graphics.read_file(&course_address), graphics.read_file(&bgdat_address)) else {
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
        open.graphics = graphics;
        Ok(Response::Found(found))
    }

    /// Writes an autosave of every level with unsaved edits, and removes the autosaves of levels that have none. The
    /// host does this every minute; a session that ends without saving leaves its autosaves for the next to offer.
    pub(crate) fn write_recovery_snapshots(&self) {
        let mut state = lock(&self.state);
        let Some(open) = state.open.as_mut() else { return };
        let Some(root) = recovery_root(open) else { return };
        let mut keep = std::collections::BTreeSet::new();
        for session in open.levels.values().filter(|session| session.level.dirty()) {
            let written = (|| -> Result<String> {
                let snapshot = LevelRecoverySnapshot {
                    course_path: address(&session.files.course)?,
                    bgdat_path: address(&session.files.bgdat)?,
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
        let Some(root) = recovery_root(open) else { return Ok(Response::Recovery(Vec::new())) };
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
            recovery_root(state.open.as_ref().ok_or_else(no_project)?)
        };
        let Some(root) = root else { return Ok(Response::Recovered(Vec::new())) };
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
            let manifest = (!open.backing.is_rom() && manifest_file.is_file())
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
        self.write_files(&[bigstar_protocol::FileWrite { path: path.to_owned(), bytes: bytes.to_vec() }])
    }

    /// Writes each file into the module that supplies it, or the active one, then drops what was built from the old
    /// files once.
    fn write_files(&self, files: &[bigstar_protocol::FileWrite]) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        if open.backing.is_rom() {
            self.commit_rom(open, files.iter().map(|file| (file.path.clone(), file.bytes.clone())).collect(), None)?;
            // What the dock shows and what graphics read from have both changed.
            open.filesystem = None;
            open.graphics = None;
            drop(state);
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.emit(Event::GraphicsChanged { generation });
            for file in files {
                self.log(LogLevel::Info, format!("Wrote {} into the ROM", file.path));
            }
            return Ok(Response::Done);
        }
        self.ensure_filesystem(open)?;
        let mut written = Vec::new();
        let mut failure = None;
        for file in files {
            let known = open.filesystem.as_ref().is_some_and(|view| view.find(&file.path).is_some());
            match Self::write_one(known, open, &file.path, &file.bytes) {
                Ok(module) => written.push((file.path.clone(), module)),
                Err(error) => {
                    failure = Some(error);
                    break;
                }
            }
        }
        if !written.is_empty() {
            // What the dock shows and what graphics read from have both changed.
            open.filesystem = None;
            open.patcher = None;
            open.graphics = None;
        }
        drop(state);
        if !written.is_empty() {
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.emit(Event::GraphicsChanged { generation });
            for (path, module) in &written {
                self.log(LogLevel::Info, format!("Wrote {path} to module {module}"));
            }
        }
        match failure {
            Some(error) => Err(error),
            None => Ok(Response::Done),
        }
    }

    fn write_one(known: bool, open: &OpenProject, path: &str, bytes: &[u8]) -> Result<String> {
        if !known && !path.starts_with("z_new/") {
            return Err(invalid("New NitroFS files must be under z_new/."));
        }
        let module = Self::target_module(open, path)?;
        let target = module_file_target(&module, &open.settings.active_variant, path, &[])?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "could not create module target directory"))?;
        }
        write_file_atomically(&target, bytes)?;
        Ok(module.key)
    }

    fn tileset_resources(&self, level: LevelId) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let references = open.levels.get(&level).ok_or_else(unknown_level)?.level.tileset_references();
        let resources = self.ensure_graphics(open)?.tileset_resources(level, references)?;
        Ok(Response::TilesetResources(Box::new(resources)))
    }

    fn file_targets(&self, path: &str) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        if open.backing.is_rom() {
            return Ok(Response::FileTargets(Vec::new()));
        }
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
        if let Backing::Rom(doc) = &open.backing {
            let target = doc.file.as_ref().map(|file| file.display().to_string()).unwrap_or_else(|| doc.name.clone());
            let bytes = match doc.banner.clone() {
                Some(bytes) => bytes,
                None => open.rom.as_mut().ok_or_else(|| not_found("no ROM is open"))?.read_banner()?,
            };
            return Ok(Response::Banner { bytes, target });
        }
        let target = self.banner_target(open)?;
        let bytes =
            if target.is_file() { read_file(&target)? } else { open.rom.as_mut().ok_or_else(|| not_found("no built ROM is open"))?.read_banner()? };
        let shown = target.strip_prefix(&open.directory).unwrap_or(&target).to_string_lossy().replace('\\', "/");
        Ok(Response::Banner { bytes, target: shown })
    }

    fn write_banner(&self, bytes: &[u8]) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        if open.backing.is_rom() {
            self.commit_rom(open, Vec::new(), Some(bytes.to_vec()))?;
            drop(state);
            self.log(LogLevel::Info, "Saved the ROM banner into the ROM");
            return Ok(Response::Done);
        }
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
        // Each variant builds to a ROM of its own.
        select_rom(open)?;
        let settings = open.settings.clone();
        drop(state);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::SettingsChanged(settings.clone()));
        self.emit(Event::GraphicsChanged { generation });
        Ok(Response::Settings(settings))
    }

    fn set_preferences(&self, autosave_minutes: u32, backups_kept: u32) -> Result<Response> {
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        open.settings.autosave_minutes = autosave_minutes.clamp(1, 60);
        open.settings.backups_kept = backups_kept.clamp(1, 100);
        save_settings(&open.directory, &open.settings)?;
        prune_patch_backups(&open.directory, open_backups_kept(&open.settings));
        let settings = open.settings.clone();
        drop(state);
        self.emit(Event::SettingsChanged(settings.clone()));
        Ok(Response::Settings(settings))
    }

    /// The open project's autosave interval; a minute when none is open.
    fn autosave_interval(&self) -> std::time::Duration {
        let minutes = lock(&self.state).open.as_ref().map_or(crate::settings::DEFAULT_AUTOSAVE_MINUTES, |open| open.settings.autosave_minutes);
        std::time::Duration::from_secs(u64::from(minutes.clamp(1, 60)) * 60)
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

    /// NCPatcher, handed the open project's clean ROM when it has a usable one.
    fn ncpatcher(&self, open: &OpenProject) -> NCPatcherClient {
        let client = NCPatcherClient::new(&self.options.ncpatcher);
        match clean_rom_file(&open.directory, &open.settings).filter(|_| open.settings.clean_rom_problem.is_none()) {
            Some(rom) => client.with_rom(rom),
            None => client,
        }
    }

    /// Remembers the clean ROM the project builds from. Before a first build it is also the ROM the editor reads, so
    /// that is reopened, and NCPatcher is asked again now that it has a ROM to answer about.
    fn set_clean_rom(&self, path: &str) -> Result<Response> {
        // The desktop's own host takes a ROM anywhere on the machine by its full path; a served one, only under the root.
        let file = if self.local_files && Path::new(path).is_absolute() { PathBuf::from(path) } else { self.workspace()?.resolve(path)? };
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let mut settings = open.settings.clone();
        settings.clean_rom = relative_to_project(&open.directory, &file);
        if let Some(problem) = clean_rom_problem(&open.directory, &settings) {
            return Err(invalid(problem));
        }
        // Before a first build the clean ROM is the one being edited, so the levels open on it go with it.
        if open.levels.values().any(|session| session.level.dirty()) {
            return Err(invalid("save or close the open levels before choosing another clean ROM"));
        }
        settings.clean_rom_problem = None;
        save_settings(&open.directory, &settings)?;
        open.settings = settings;
        open.patcher = None;
        open.filesystem = None;
        open.graphics = None;
        open.banner = BannerTarget::Unreported;
        self.ensure_patcher(open);
        select_rom(open)?;
        let settings = open.settings.clone();
        drop(state);
        self.log(LogLevel::Info, format!("The clean ROM is now {}", settings.clean_rom));
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::SettingsChanged(settings.clone()));
        self.emit(Event::GraphicsChanged { generation });
        Ok(Response::Settings(settings))
    }

    /// The watcher's debounced report. Every change is announced; one that is not the host's own saving makes the
    /// graphics stale, so they are dropped and rebuilt on the next request.
    pub(crate) fn files_changed(&self, paths: Vec<PathBuf>) {
        let mut state = lock(&self.state);
        let Some(open) = state.open.as_mut() else { return };
        let written = std::mem::take(&mut open.own_writes);
        let own = |path: &Path| {
            written.contains(path)
                || path.strip_prefix(&open.directory).is_ok_and(|relative| {
                    relative.components().next().is_some_and(|first| first.as_os_str().to_string_lossy().starts_with(".big-star"))
                })
        };
        let stale = paths.iter().any(|path| !own(path));
        if stale {
            open.graphics = None;
            open.filesystem = None;
            open.patcher = None;
        }
        drop(state);
        let mut relative: Vec<String> =
            paths.iter().filter_map(|path| self.workspace.as_ref().and_then(|workspace| workspace.relative(path))).collect();
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

/// How many backups of each kind the project keeps.
fn open_backups_kept(settings: &bigstar_protocol::ProjectSettings) -> usize {
    settings.backups_kept.clamp(1, 100) as usize
}

/// How often the autosave thread looks at the clock; the interval itself is the project's preference.
const AUTOSAVE_TICK: std::time::Duration = std::time::Duration::from_secs(5);

/// A new backup directory under `.big-star-backups`: what made it, then when (UTC, to the millisecond, so two saves in
/// one second are two backups), then which process, since two hosts on one project are not unheard of.
fn new_backup_directory(project: &Path, kind: &str) -> PathBuf {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let (days, seconds) = ((now.as_secs() / 86_400) as i64, now.as_secs() % 86_400);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    let stamp = format!("{year:04}{month:02}{day:02}-{:02}{:02}{:02}-{:03}", seconds / 3600, seconds / 60 % 60, seconds % 60, now.subsec_millis());
    bigstar_project::patch_backup_root(project).join(format!("{kind}-{stamp}-{}", std::process::id()))
}

/// Copies every target that exists into a new backup directory of `kind`, keeping its path under the project, before
/// it is overwritten.
fn back_up(project: &Path, kind: &str, targets: &[(PathBuf, Vec<u8>)]) -> Result<()> {
    let root = new_backup_directory(project, kind);
    for (target, _) in targets.iter().filter(|(target, _)| target.is_file()) {
        let relative = target.strip_prefix(project).unwrap_or(target);
        let backup = root.join(relative);
        if let Some(parent) = backup.parent() {
            std::fs::create_dir_all(parent).map_err(|_| Error::new(ErrorCode::IoError, "could not create the backup directory"))?;
        }
        std::fs::copy(target, &backup).map_err(|_| Error::new(ErrorCode::IoError, format!("could not back up {}", target.display())))?;
    }
    Ok(())
}

/// Where removed module files go, relative to the project.
pub(crate) const TRASH_DIR: &str = ".big-star-trash";

/// Where autosaves live, relative to the project.
pub(crate) const AUTOSAVE_DIR: &str = ".big-star-autosave";

/// Where an open project's autosaves go: under the project, or for a ROM edited directly, a folder of that ROM's own
/// beside it, so two ROMs in one directory do not offer each other's levels back. An uploaded ROM has nowhere: the host
/// writes nothing of it to disk.
fn recovery_root(open: &OpenProject) -> Option<PathBuf> {
    match &open.backing {
        Backing::NcPatcher => Some(open.directory.join(AUTOSAVE_DIR)),
        Backing::Rom(doc) => doc.file.as_ref().map(|file| open.directory.join(AUTOSAVE_DIR).join(file.file_name().unwrap_or_default())),
    }
}

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

/// Reads the project's ROM: the active variant's build, or before there is one the clean ROM. A different ROM closes the
/// levels open on the old one, so callers refuse unsaved edits first.
fn select_rom(open: &mut OpenProject) -> Result<()> {
    let clean_rom = clean_rom_file(&open.directory, &open.settings).filter(|_| open.settings.clean_rom_problem.is_none());
    let rom_file = project_rom(open.built_rom.as_deref(), clean_rom.as_deref());
    if rom_file == open.rom_file && open.rom.is_some() == rom_file.is_some() {
        return Ok(());
    }
    let rom = rom_file.as_deref().map(RomReader::open).transpose()?;
    open.index = rom.as_ref().map(RomIndex::new).unwrap_or_else(RomIndex::new_empty);
    open.rom = rom;
    open.rom_file = rom_file;
    open.levels.clear();
    open.filesystem = None;
    open.graphics = None;
    Ok(())
}

fn project_info(workspace: &Workspace, directory: &Path, rom: Option<&RomReader>, rom_file: Option<&Path>) -> ProjectInfo {
    let header = rom.map(RomReader::header);
    ProjectInfo {
        path: workspace.relative(directory).unwrap_or_default(),
        name: project_name(directory),
        rom_path: rom_file.map(|file| relative_to_project(directory, file)),
        game_title: header.map(|h| h.game_title.clone()),
        game_code: header.map(|h| h.game_code.clone()),
        file_count: rom.map_or(0, |rom| rom.files().len()),
        mode: bigstar_protocol::ProjectMode::NcPatcher,
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
