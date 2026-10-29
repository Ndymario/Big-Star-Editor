//! One connection to a host and everything the frontend has learned from it. Pure state: no drawing, so the
//! request/response bookkeeping can be tested without a window.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use bigstar_client::{Connection, ConnectionState, Incoming};
use bigstar_core::Error;
use bigstar_protocol::{
    ActorCatalog, DirectoryListing, Event, FileWrite, FilesystemListing, FoundEntity, LevelBytes, LevelEdit, LevelId, LevelListing, LevelSnapshot,
    LogLevel, ProjectData, ProjectInfo, ProjectMode, ProjectSettings, ProjectSummary, ROM_CHUNK_BYTES, RecoverableLevel, Request, RequestId,
    Response, ResponseResult, TextureData, TextureId, TilesetResources,
};
use bigstar_protocol::{BuildConfigKind, BuildDiagnostic, BuildEvent, BuildOutcome, DiagnosticLevel};

use crate::level_view::{ActorIndex, ActorKey, CanvasOptions, LevelView};

/// What a request in flight was for, so its response lands in the right place.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    ListProjects,
    Browse,
    OpenProject,
    SaveRomAs,
    CleanRom,
    /// A piece of a ROM being uploaded; the last one's answer is the ROM opened.
    Upload,
    Download,
    ListLevels,
    OpenLevel(Opening),
    Edit(LevelId),
    History(LevelId),
    Save(LevelId),
    CloseLevel,
    CloseProject,
    Textures,
    LevelGraphics(LevelId),
    ActorCatalog,
    ActorDraws(LevelId, Vec<ActorKey>),
    ProjectData,
    LevelBytes(LevelId, BytesFor),
    Find {
        sprites: bool,
        id: u16,
    },
    Recovery,
    Recover,
    Filesystem,
    ListArchive(String),
    ReadFile(FileRead),
    /// A file write or removal: when it is answered the listing is fetched again.
    FileChange,
    FileTargets(String),
    Settings,
    Banner,
    BannerWrite,
    TilesetResources,
    BuildConfig,
    Migrate,
    Build,
    /// A request whose answer is only `Done`; what it did, the host logs.
    Quiet,
}

/// What the Output panel shows about the last build.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BuildState {
    pub running: bool,
    pub cancelling: bool,
    /// The phase and item in progress, or how the build ended.
    pub status: String,
    /// Done and total, when the phase says; `None` for work of unknown length.
    pub progress: Option<(i64, i64)>,
    pub diagnostics: Vec<BuildDiagnostic>,
    pub outcome: Option<BuildOutcome>,
}

impl BuildState {
    /// The status line for how a build ended, as the Output dock words it.
    pub fn finished_status(outcome: &BuildOutcome) -> String {
        if outcome.cancelled {
            // Not a failure: it did what it was asked to do.
            "Cancelled".to_owned()
        } else if outcome.ok && outcome.warnings > 0 {
            format!("Succeeded with {} warning(s)", outcome.warnings)
        } else if outcome.ok {
            "Succeeded".to_owned()
        } else {
            format!("Failed: {}", outcome.stage)
        }
    }
}

/// What a file's bytes were asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRead {
    /// To say which editor opens it.
    Classify,
    /// To open it in that editor.
    Open,
    /// To open it as hex whatever it is.
    Hex,
    /// To save a copy outside the project.
    Extract,
    /// A palette or graphics file an editor pairs the open one with.
    Companion,
}

/// Why a level is being opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opening {
    /// As a document of its own.
    Document,
    /// In the reusable Preview tab, read-only.
    Preview,
    /// Fetched again because the host's copy is the truth.
    Refresh,
    /// Fetched again after the host dropped the session (a variant switch); the answer takes over this view.
    Reopen(LevelId),
}

/// What a level's serialized bytes were asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BytesFor {
    HexEditor,
    Export,
}

/// What a finder found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    pub sprites: bool,
    pub id: u16,
    pub entities: Vec<FoundEntity>,
}

impl Pending {
    /// Graphics are not the person's request: when one fails it is logged, not put in front of them.
    fn graphics(&self) -> bool {
        matches!(self, Pending::Textures | Pending::LevelGraphics(_) | Pending::ActorCatalog | Pending::ActorDraws(..) | Pending::ProjectData)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogLine {
    pub level: LogLevel,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub error: bool,
    pub text: String,
}

/// The host-side file browser the clean ROM is chosen with.
#[derive(Debug, Clone, Default)]
pub struct Browser {
    pub path: String,
    pub listing: Option<DirectoryListing>,
    pub rom: Option<String>,
}

const LOG_LIMIT: usize = 500;

/// The level list a project keeps in its root, which the host reads in place of its own.
const PROJECT_LEVEL_LIST: &str = "levels.yaml";

pub struct Session {
    conn: Box<dyn Connection>,
    pub state: ConnectionState,
    pending: HashMap<RequestId, Pending>,
    pub projects: Vec<ProjectSummary>,
    pub project: Option<ProjectInfo>,
    pub level_list: LevelListing,
    /// Every level open on the host for this frontend, documents and preview alike.
    pub levels: BTreeMap<LevelId, LevelView>,
    /// The canvas switches a level arriving starts with: the ones every other canvas has.
    pub canvas_options: CanvasOptions,
    /// The document the panels, the menus and the shortcuts act on.
    pub active: Option<LevelId>,
    opened: Vec<(LevelId, Opening)>,
    renamed: Vec<(LevelId, LevelId)>,
    recovered: Vec<String>,
    /// Bytes asked for by the hex editor or an export, until they are taken.
    pub level_bytes: Option<(LevelId, BytesFor, LevelBytes)>,
    pub found: Option<Found>,
    /// Levels an interrupted session left autosaved, waiting for the person to say whether to recover them.
    pub recovery_offer: Option<Vec<RecoverableLevel>>,
    /// The Filesystem dock's rows, with the members of the archives opened so far added in.
    pub filesystem: Option<FilesystemListing>,
    filesystem_requested: bool,
    /// Archives whose members have been listed (or asked for).
    pub listed_archives: HashSet<String>,
    /// File bytes that arrived, for whoever asked: (path, why, bytes).
    pub file_reads: Vec<(String, FileRead, Vec<u8>)>,
    /// Which variants already hold a module copy of a path, once asked.
    pub file_targets: Option<(String, Vec<String>)>,
    /// The ROM banner, once asked for: its bytes and the configured file an edit goes to.
    pub banner: Option<(Vec<u8>, String)>,
    /// A level's tilesets and backgrounds as files, once asked for.
    pub tileset_resources: Option<TilesetResources>,
    pub build: BuildState,
    /// The project's NCPatcher configuration, once asked for before a build.
    pub build_config: Option<BuildConfigKind>,
    /// A legacy configuration was converted; the build it was converted for can start.
    pub migrated: bool,
    pub log: Vec<LogLine>,
    pub notice: Option<Notice>,
    pub browser: Option<Browser>,
    /// Whether the host is the desktop app's own, which takes a clean ROM anywhere on this machine, chosen with the
    /// system's file dialog rather than the host's browser.
    pub local_files: bool,
    /// Whether the open project has asked for its clean ROM already, so a dismissed question is not asked again.
    clean_rom_asked: bool,
    /// How much of a ROM being uploaded the host holds, and of how much.
    pub upload: Option<(u64, u64)>,
    /// A download of the uploaded ROM under way: what has arrived of it.
    downloading: Option<Vec<u8>>,
    /// A finished download, the name to suggest for it and its bytes, until it is taken.
    pub downloaded: Option<(String, Vec<u8>)>,
    /// The uploaded ROM holds saved edits that no download has taken yet; they live only in the host's memory.
    pub undownloaded: bool,
    /// Set once the host is gone: why.
    pub ended: Option<String>,
    /// `ProjectClosed` events still to come that this frontend caused itself and has already acted on.
    own_closes: u32,
    /// The newest graphics generation the host has announced; anything older is stale.
    pub graphics_generation: u64,
    pub settings: Option<ProjectSettings>,
    /// Every texture the host has sent, by content hash. Kept across levels: the same pixels have the same id.
    pub textures: HashMap<TextureId, Arc<TextureData>>,
    textures_requested: HashSet<TextureId>,
    /// The project's actors; every level view shares it.
    catalog: Option<Arc<ActorCatalog>>,
    catalog_requested: bool,
    /// What the open project's modules declare, and which overlay each object bank loads.
    pub project_data: Option<ProjectData>,
    /// Undo (`true`) or redo steps still to take after the one in flight: a pick from the history menu.
    history_steps: Option<(bool, usize)>,
}

impl Session {
    pub fn new(conn: Box<dyn Connection>) -> Self {
        let mut session = Self {
            state: ConnectionState::Connecting,
            conn,
            pending: HashMap::new(),
            projects: Vec::new(),
            project: None,
            level_list: LevelListing::default(),
            levels: BTreeMap::new(),
            canvas_options: CanvasOptions::default(),
            active: None,
            opened: Vec::new(),
            renamed: Vec::new(),
            recovered: Vec::new(),
            level_bytes: None,
            found: None,
            recovery_offer: None,
            filesystem: None,
            filesystem_requested: false,
            listed_archives: HashSet::new(),
            file_reads: Vec::new(),
            file_targets: None,
            banner: None,
            tileset_resources: None,
            build: BuildState::default(),
            build_config: None,
            migrated: false,
            log: Vec::new(),
            notice: None,
            browser: None,
            local_files: false,
            clean_rom_asked: false,
            upload: None,
            downloading: None,
            downloaded: None,
            undownloaded: false,
            ended: None,
            own_closes: 0,
            graphics_generation: 0,
            settings: None,
            textures: HashMap::new(),
            textures_requested: HashSet::new(),
            catalog: None,
            catalog_requested: false,
            project_data: None,
            history_steps: None,
        };
        session.note_state();
        session
    }

    pub fn connected(&self) -> bool {
        matches!(self.state, ConnectionState::Connected(_))
    }

    /// An edit, history step or save is waiting for its answer; the canvas takes no new input meanwhile.
    pub fn busy(&self) -> bool {
        self.pending.values().any(|kind| matches!(kind, Pending::Edit(_) | Pending::History(_) | Pending::Save(_)))
    }

    /// Whether something a level is drawn from is still on its way: its graphics, its actors' draw lists, or textures.
    pub fn level_arriving(&self, level: LevelId) -> bool {
        self.pending.values().any(|kind| match kind {
            Pending::LevelGraphics(of) | Pending::ActorDraws(of, _) => *of == level,
            Pending::Textures | Pending::ActorCatalog => true,
            _ => false,
        })
    }

    /// The active document.
    pub fn level(&self) -> Option<&LevelView> {
        self.levels.get(&self.active?)
    }

    pub fn level_mut(&mut self) -> Option<&mut LevelView> {
        self.levels.get_mut(&self.active?)
    }

    /// Levels that arrived since last asked, and why they were opened.
    pub fn take_opened(&mut self) -> Vec<(LevelId, Opening)> {
        std::mem::take(&mut self.opened)
    }

    /// Views the host gave a new id (old, new), since last asked.
    pub fn take_renamed(&mut self) -> Vec<(LevelId, LevelId)> {
        std::mem::take(&mut self.renamed)
    }

    /// Areas whose autosaves were just recovered, to open as documents.
    pub fn take_recovered(&mut self) -> Vec<String> {
        std::mem::take(&mut self.recovered)
    }

    fn request(&mut self, request: Request, kind: Pending) {
        let id = self.conn.send(request);
        self.pending.insert(id, kind);
    }

    pub fn list_projects(&mut self) {
        self.request(Request::ListProjects, Pending::ListProjects);
    }

    pub fn browse(&mut self, path: &str) {
        let browser = self.browser.get_or_insert_with(Browser::default);
        browser.path = path.to_owned();
        self.request(Request::BrowseDirectory { path: path.to_owned() }, Pending::Browse);
    }

    pub fn open_project(&mut self, path: &str) {
        self.request(Request::OpenProject { path: path.to_owned() }, Pending::OpenProject);
    }

    /// Opens a `.nds` on this machine, by the full path a file dialog gave, to edit directly.
    pub fn open_rom(&mut self, path: &str) {
        self.request(Request::OpenRom { path: path.to_owned() }, Pending::OpenProject);
    }

    /// Writes the ROM being edited to another file on this machine, which later saves then go to.
    pub fn save_rom_as(&mut self, path: &str) {
        self.request(Request::SaveRomAs { path: path.to_owned() }, Pending::SaveRomAs);
    }

    /// Sends a ROM to the host to edit, in pieces; the last piece's answer opens it.
    pub fn upload_rom(&mut self, name: &str, bytes: &[u8]) {
        let total = bytes.len() as u64;
        self.upload = Some((0, total));
        let mut offset = 0usize;
        loop {
            let piece = bytes.get(offset..bytes.len().min(offset + ROM_CHUNK_BYTES)).unwrap_or_default();
            let request = Request::UploadRom { name: name.to_owned(), offset: offset as u64, total, bytes: piece.to_vec() };
            self.request(request, Pending::Upload);
            offset += piece.len();
            if offset >= bytes.len() || piece.is_empty() {
                break;
            }
        }
    }

    /// Fetches the uploaded ROM with its edits; it lands in `downloaded`.
    pub fn download_rom(&mut self) {
        if self.downloading.is_none() {
            self.downloading = Some(Vec::new());
            self.request(Request::DownloadRom { offset: 0 }, Pending::Download);
        }
    }

    pub fn downloading(&self) -> bool {
        self.downloading.is_some()
    }

    /// Whether the open project is a ROM edited directly, which has no modules, variants or builds.
    pub fn rom_mode(&self) -> bool {
        self.project.as_ref().is_some_and(|project| project.mode.is_rom())
    }

    /// Whether the open project is a ROM this frontend uploaded, which leaves as a download.
    pub fn uploaded(&self) -> bool {
        self.project.as_ref().is_some_and(|project| project.mode == ProjectMode::Uploaded)
    }

    /// Whether this connection reaches a workspace of NCPatcher projects.
    pub fn has_workspace(&self) -> bool {
        matches!(&self.state, ConnectionState::Connected(info) if info.has_workspace)
    }

    fn list_projects_if_any(&mut self) {
        if self.has_workspace() {
            self.list_projects();
        }
    }

    /// Opens the browser to choose the clean ROM the project builds from, starting in the project. On the desktop's own
    /// host it opens empty: the system's file dialog does the browsing.
    pub fn choose_clean_rom(&mut self) {
        if self.local_files {
            self.browser = Some(Browser::default());
            return;
        }
        let start = self.project.as_ref().map(|project| project.path.clone()).unwrap_or_default();
        self.browser = Some(Browser::default());
        self.browse(&start);
    }

    pub fn set_clean_rom(&mut self, path: &str) {
        self.request(Request::SetCleanRom { path: path.to_owned() }, Pending::CleanRom);
    }

    pub fn close_project(&mut self) {
        self.request(Request::CloseProject, Pending::CloseProject);
    }

    /// Opens an area as a document, or brings it forward if it is open.
    pub fn open_level(&mut self, area: &str) {
        self.request(Request::OpenLevel { area: area.to_owned() }, Pending::OpenLevel(Opening::Document));
    }

    /// Shows an area in the Preview tab.
    pub fn preview_level(&mut self, area: &str) {
        self.request(Request::OpenLevel { area: area.to_owned() }, Pending::OpenLevel(Opening::Preview));
    }

    /// An edit of the active document.
    pub fn edit(&mut self, edit: LevelEdit) {
        if let Some(level) = self.active {
            self.edit_level(level, edit);
        }
    }

    pub fn edit_level(&mut self, level: LevelId, edit: LevelEdit) {
        if let Some(view) = self.levels.get(&level) {
            let request = Request::EditLevel { level, base_revision: view.snapshot.revision, edit };
            self.request(request, Pending::Edit(level));
        }
    }

    /// Undoes (or redoes) `steps` steps, one after the other, as a pick from the history menu does.
    pub fn step_history(&mut self, undo: bool, steps: usize) {
        if steps == 0 || self.busy() {
            return;
        }
        self.history_steps = Some((undo, steps - 1));
        if undo { self.undo() } else { self.redo() }
    }

    pub fn undo(&mut self) {
        if let Some(level) = self.level().filter(|v| v.snapshot.can_undo).map(|v| v.snapshot.level) {
            self.request(Request::UndoLevel { level }, Pending::History(level));
        }
    }

    pub fn redo(&mut self) {
        if let Some(level) = self.level().filter(|v| v.snapshot.can_redo).map(|v| v.snapshot.level) {
            self.request(Request::RedoLevel { level }, Pending::History(level));
        }
    }

    pub fn save(&mut self) {
        if let Some(level) = self.active {
            self.save_level(level);
        }
    }

    pub fn save_level(&mut self, level: LevelId) {
        if self.levels.contains_key(&level) {
            self.request(Request::SaveLevel { level }, Pending::Save(level));
        }
    }

    /// Leaves a level. The host keeps the session (and its unsaved edits) only until this arrives, so a dirty level
    /// is closed only on the person's say-so; the caller asks first.
    pub fn close_level(&mut self, level: LevelId) {
        if self.levels.remove(&level).is_some() {
            self.request(Request::CloseLevel { level }, Pending::CloseLevel);
        }
        if self.active == Some(level) {
            self.active = None;
        }
    }

    /// The active document serialized, for the hex editor or an export.
    pub fn request_level_bytes(&mut self, purpose: BytesFor) {
        if let Some(level) = self.active {
            self.request(Request::LevelBytes { level }, Pending::LevelBytes(level, purpose));
        }
    }

    /// Looks for every placement of an object (or sprite type) in the project.
    pub fn find(&mut self, sprites: bool, id: u16) {
        self.found = None;
        self.request(Request::FindEntities { sprites, id }, Pending::Find { sprites, id });
    }

    /// The person's answer to the recovery offer.
    pub fn answer_recovery(&mut self, accept: bool) {
        self.recovery_offer = None;
        self.request(Request::Recover { accept }, Pending::Recover);
    }

    /// Fetches the Filesystem dock's rows again.
    pub fn refresh_filesystem(&mut self) {
        if self.project.is_some() && !self.filesystem_requested {
            self.filesystem_requested = true;
            self.request(Request::Filesystem, Pending::Filesystem);
        }
    }

    /// Fetches the listing if it has never been fetched.
    pub fn refresh_filesystem_once(&mut self) {
        if self.filesystem.is_none() {
            self.refresh_filesystem();
        }
    }

    /// Chooses the module that add and replace write into.
    pub fn set_active_module(&mut self, module: &str) {
        self.request(Request::SetActiveModule { module: module.to_owned() }, Pending::Settings);
    }

    /// Mirrors the ROM into the project's `virtual-rom/` folder, from which changed files go into the active module.
    pub fn expose_virtual_rom(&mut self) {
        self.request(Request::ExposeVirtualRom, Pending::Filesystem);
    }

    /// Stops mirroring the ROM; the folder stays.
    pub fn hide_virtual_rom(&mut self) {
        self.request(Request::HideVirtualRom, Pending::Filesystem);
    }

    /// Lists an archive's members the first time it is opened.
    pub fn list_archive(&mut self, path: &str) {
        if self.listed_archives.insert(path.to_owned()) {
            self.request(Request::ListArchive { path: path.to_owned() }, Pending::ListArchive(path.to_owned()));
        }
    }

    pub fn read_file(&mut self, path: &str, purpose: FileRead) {
        self.request(Request::ReadFile { path: path.to_owned() }, Pending::ReadFile(purpose));
    }

    pub fn write_file(&mut self, path: &str, bytes: Vec<u8>) {
        self.request(Request::WriteFile { path: path.to_owned(), bytes }, Pending::FileChange);
    }

    pub fn remove_file(&mut self, path: &str) {
        self.request(Request::RemoveFile { path: path.to_owned() }, Pending::FileChange);
    }

    /// Asks for the ROM banner; it arrives in `banner`.
    pub fn request_banner(&mut self) {
        self.request(Request::Banner, Pending::Banner);
    }

    /// Asks for the files behind a level's tilesets and backgrounds; they arrive in `tileset_resources`.
    pub fn request_tileset_resources(&mut self, level: LevelId) {
        self.request(Request::TilesetResources { level }, Pending::TilesetResources);
    }

    /// Chooses the build variant whose module layers the editor reads and writes; the host closes every level session
    /// and the open levels are opened again.
    pub fn set_active_variant(&mut self, variant: &str) {
        self.request(Request::SetActiveVariant { variant: variant.to_owned() }, Pending::Settings);
    }

    /// The open project's autosave interval and how many backups it keeps; the host clamps and keeps them.
    pub fn set_preferences(&mut self, autosave_minutes: u32, backups_kept: u32) {
        self.request(Request::SetPreferences { autosave_minutes, backups_kept }, Pending::Settings);
    }

    /// Asks what NCPatcher configuration the project has; the answer lands in `build_config`.
    pub fn request_build_config(&mut self) {
        self.build_config = None;
        self.request(Request::BuildConfig, Pending::BuildConfig);
    }

    pub fn migrate_config(&mut self) {
        self.migrated = false;
        self.request(Request::MigrateConfig, Pending::Migrate);
    }

    /// Starts a build; `Event::Build` reports it into `build`.
    pub fn start_build(&mut self, verbose: bool) {
        self.build = BuildState { running: true, status: "Starting NCPatcher...".to_owned(), ..BuildState::default() };
        self.request(Request::Build { verbose }, Pending::Build);
    }

    pub fn cancel_build(&mut self) {
        if self.build.running {
            self.build.cancelling = true;
            self.build.status = "Cancelling...".to_owned();
            self.request(Request::CancelBuild, Pending::Quiet);
        }
    }

    pub fn extract_rom(&mut self, directory: &str) {
        self.request(Request::ExtractRom { directory: directory.to_owned() }, Pending::Quiet);
    }

    /// Writes several files into their modules in one request.
    pub fn write_files(&mut self, files: Vec<FileWrite>) {
        self.request(Request::WriteFiles { files }, Pending::FileChange);
    }

    pub fn write_banner(&mut self, bytes: Vec<u8>) {
        self.request(Request::WriteBanner { bytes }, Pending::BannerWrite);
    }

    pub fn ask_file_targets(&mut self, path: &str) {
        self.file_targets = None;
        self.request(Request::FileTargets { path: path.to_owned() }, Pending::FileTargets(path.to_owned()));
    }

    /// Takes the file reads made for `purpose`.
    pub fn take_file_reads(&mut self, purpose: FileRead) -> Vec<(String, Vec<u8>)> {
        let (taken, kept) = std::mem::take(&mut self.file_reads).into_iter().partition(|(_, why, _)| *why == purpose);
        self.file_reads = kept;
        taken.into_iter().map(|(path, _, bytes)| (path, bytes)).collect()
    }

    /// Whether any open level has edits that were not saved.
    pub fn dirty(&self) -> bool {
        self.undownloaded || self.levels.values().any(|view| view.snapshot.dirty)
    }

    /// Tells the person something went wrong.
    pub fn report(&mut self, message: String) {
        self.notice = Some(Notice { error: true, text: message });
    }

    pub fn dismiss_notice(&mut self) {
        self.notice = None;
    }

    fn note_state(&mut self) {
        let state = self.conn.state().clone();
        if state == self.state && !matches!(state, ConnectionState::Connected(_)) {
            return;
        }
        let was_connected = self.connected();
        self.state = state;
        match &self.state {
            ConnectionState::Connected(_) if !was_connected => self.list_projects_if_any(),
            ConnectionState::Refused(reason) | ConnectionState::Closed(reason) => {
                self.ended.get_or_insert_with(|| reason.clone());
            }
            _ => {}
        }
    }

    /// Takes in everything that has arrived. Returns whether anything changed, so a caller can repaint.
    pub fn poll(&mut self) -> bool {
        let incoming = self.conn.poll();
        let changed = !incoming.is_empty();
        for item in incoming {
            match item {
                Incoming::State(_) => {}
                Incoming::Response { id, result } => {
                    if let Some(kind) = self.pending.remove(&id) {
                        self.respond(kind, result);
                    }
                }
                Incoming::Event(event) => self.event(event),
            }
        }
        self.note_state();
        changed
    }

    fn fail(&mut self, error: &Error) {
        self.notice = Some(Notice { error: true, text: error.message.clone() });
    }

    fn respond(&mut self, kind: Pending, result: ResponseResult) {
        let response = match result {
            Ok(response) => response,
            Err(error) if kind.graphics() => {
                self.log_line(LogLevel::Warning, format!("Graphics: {}", error.message));
                // Sprites whose draw lists will not come are drawn as markers rather than left blank.
                if let Pending::ActorDraws(level, keys) = kind
                    && let Some(view) = self.levels.get_mut(&level)
                {
                    for key in keys {
                        view.art.requested.remove(&key);
                        view.art.draws.insert(key, Default::default());
                    }
                }
                return;
            }
            Err(error) => {
                self.history_steps = None;
                if kind == Pending::Filesystem {
                    self.filesystem_requested = false;
                }
                // A build the host refused never started.
                if kind == Pending::Build {
                    self.build.running = false;
                    self.build.status = format!("Failed: {}", error.message);
                }
                if kind == Pending::Download {
                    self.downloading = None;
                }
                // The pieces after one the host refused are refused too; one notice is enough.
                if kind == Pending::Upload && self.upload.take().is_none() {
                    return;
                }
                self.fail(&error);
                // The host's version of the level is the truth; fetch it again.
                if let Pending::Edit(level) | Pending::History(level) | Pending::Save(level) = kind {
                    self.refresh_level(level);
                    if let Some(view) = self.levels.get_mut(&level) {
                        view.settled();
                    }
                }
                return;
            }
        };
        match (kind, response) {
            (Pending::ListProjects, Response::Projects(projects)) => self.projects = projects,
            (Pending::Browse, Response::Directory(listing)) => {
                if let Some(browser) = &mut self.browser {
                    browser.path.clone_from(&listing.path);
                    browser.listing = Some(listing);
                }
            }
            (Pending::OpenProject, Response::Project(info)) => self.opened_project(info),
            (Pending::Upload, Response::Uploaded { received }) => {
                if let Some((done, _)) = &mut self.upload {
                    *done = received;
                }
            }
            (Pending::Upload, Response::Project(info)) => {
                self.upload = None;
                self.opened_project(info);
            }
            (Pending::Download, Response::RomChunk { name, total, offset, bytes }) => {
                let Some(mut arrived) = self.downloading.take() else { return };
                if offset != arrived.len() as u64 {
                    self.report("The download arrived out of order; try again.".to_owned());
                    return;
                }
                arrived.extend_from_slice(&bytes);
                if arrived.len() as u64 >= total || bytes.is_empty() {
                    self.undownloaded = false;
                    self.downloaded = Some((name, arrived));
                } else {
                    let next = arrived.len() as u64;
                    self.downloading = Some(arrived);
                    self.request(Request::DownloadRom { offset: next }, Pending::Download);
                }
            }
            (Pending::SaveRomAs, Response::Project(info)) => {
                self.browser = None;
                self.project = Some(info);
            }
            // When it is the ROM being edited, `ProjectChanged` follows and the project is read again.
            (Pending::CleanRom, Response::Settings(settings)) => {
                self.browser = None;
                self.settings = Some(settings);
            }
            (Pending::ListLevels, Response::Levels(list)) => self.level_list = list,
            (Pending::OpenLevel(opening), Response::Level(snapshot)) => self.take_snapshot(*snapshot, opening),
            (pending @ (Pending::Edit(_) | Pending::History(_) | Pending::Save(_)), Response::Edited { snapshot, outcome }) => {
                let level = snapshot.level;
                if matches!(pending, Pending::Save(_)) {
                    self.note_saved();
                }
                // An edit, an undo or a redo that changes which tilesets, layers or palettes the level names leaves its
                // graphics out of date.
                let renamed = self.levels.get(&level).is_some_and(|view| view.snapshot.tileset_references != snapshot.tileset_references);
                let owed = self.levels.get_mut(&level).and_then(|view| view.edited(*snapshot, &outcome));
                if renamed {
                    self.request(Request::LevelGraphics { level }, Pending::LevelGraphics(level));
                }
                if let Some(edit) = owed {
                    self.edit_level(level, edit);
                }
                if matches!(pending, Pending::History(_))
                    && let Some((undo, left)) = self.history_steps.take()
                    && left > 0
                {
                    self.history_steps = Some((undo, left - 1));
                    if undo { self.undo() } else { self.redo() }
                }
                self.request_actor_draws();
            }
            (Pending::ProjectData, Response::ProjectData(data)) => self.project_data = Some(*data),
            (Pending::CloseProject, Response::Done) => {
                self.own_closes += 1;
                self.project = None;
                self.forget_project();
                self.list_projects_if_any();
            }
            (Pending::LevelBytes(level, purpose), Response::LevelBytes(bytes)) => self.level_bytes = Some((level, purpose, *bytes)),
            (Pending::Find { sprites, id }, Response::Found(entities)) => self.found = Some(Found { sprites, id, entities }),
            (Pending::Recovery, Response::Recovery(levels)) => self.recovery_offer = (!levels.is_empty()).then_some(levels),
            (Pending::Recover, Response::Recovered(areas)) => self.recovered.extend(areas),
            (Pending::Filesystem, Response::Filesystem(listing)) => {
                self.filesystem_requested = false;
                // Archives opened before keep their members: they are listed again on demand.
                self.listed_archives.clear();
                self.filesystem = Some(*listing);
            }
            (Pending::ListArchive(_), Response::Files(members)) => {
                if let Some(listing) = &mut self.filesystem {
                    let known: HashSet<String> = listing.entries.iter().map(|e| e.path.clone()).collect();
                    listing.entries.extend(members.into_iter().filter(|m| !known.contains(&m.path)));
                }
            }
            (Pending::ReadFile(purpose), Response::FileBytes { path, bytes }) => self.file_reads.push((path, purpose, bytes)),
            (Pending::FileChange, Response::Done) => {
                self.note_saved();
                self.refresh_filesystem();
            }
            (Pending::FileTargets(path), Response::FileTargets(holding)) => self.file_targets = Some((path, holding)),
            (Pending::Banner, Response::Banner { bytes, target }) => self.banner = Some((bytes, target)),
            (Pending::BannerWrite, Response::Done) => self.note_saved(),
            (Pending::TilesetResources, Response::TilesetResources(resources)) => self.tileset_resources = Some(*resources),
            (Pending::BuildConfig, Response::BuildConfig(kind)) => self.build_config = Some(kind),
            (Pending::Migrate, Response::Done) => self.migrated = true,
            (Pending::Quiet | Pending::Build, Response::Done) => {}
            (Pending::Settings, Response::Settings(settings)) => {
                // A project that cannot be built until it has a clean ROM asks for one when it opens.
                if settings.clean_rom_problem.is_some() && !self.clean_rom_asked {
                    self.clean_rom_asked = true;
                    self.settings = Some(settings);
                    self.choose_clean_rom();
                } else {
                    self.settings = Some(settings);
                }
                self.refresh_filesystem();
            }
            (Pending::CloseLevel, Response::Done) => {}
            (Pending::Textures, Response::Textures(textures)) => {
                for texture in textures {
                    self.textures_requested.remove(&texture.id);
                    self.textures.insert(texture.id.clone(), Arc::new(texture));
                }
            }
            (Pending::LevelGraphics(level), Response::LevelGraphics(graphics)) => {
                if graphics.generation < self.graphics_generation {
                    return;
                }
                if let Some(error) = &graphics.error {
                    self.log_line(LogLevel::Warning, format!("Level graphics: {error}"));
                }
                let slots =
                    graphics.tilesets.iter().enumerate().filter_map(|(slot, t)| t.error.as_ref().map(|e| format!("Tileset slot {slot}: {e}")));
                let images = [("Background", &graphics.background), ("Foreground", &graphics.foreground)]
                    .into_iter()
                    .filter_map(|(name, image)| image.error.as_ref().map(|e| format!("{name}: {e}")));
                let warnings: Vec<String> = slots.chain(images).collect();
                for warning in warnings {
                    self.log_line(LogLevel::Warning, warning);
                }
                let Some(view) = self.levels.get_mut(&level) else { return };
                view.art.set_graphics(*graphics);
                let ids = view.art.texture_ids();
                self.request_textures(ids);
            }
            (Pending::ActorCatalog, Response::ActorCatalog(catalog)) => {
                self.catalog_requested = false;
                if catalog.generation < self.graphics_generation {
                    return;
                }
                let catalog = Arc::new(*catalog);
                self.catalog = Some(catalog.clone());
                for view in self.levels.values_mut() {
                    view.art.actors = ActorIndex::new(catalog.clone());
                }
            }
            (Pending::ActorDraws(level, keys), Response::ActorDraws { generation, draws }) => {
                let Some(view) = self.levels.get_mut(&level) else { return };
                for key in &keys {
                    view.art.requested.remove(key);
                }
                if generation < self.graphics_generation {
                    self.request_actor_draws();
                    return;
                }
                let ids: Vec<TextureId> = draws.iter().flat_map(|d| d.commands.iter().filter_map(|c| c.texture.clone())).collect();
                view.art.draws.extend(keys.into_iter().zip(draws));
                self.request_textures(ids);
            }
            (_, other) => log::warn!("unexpected response {other:?}"),
        }
    }

    fn take_snapshot(&mut self, snapshot: LevelSnapshot, opening: Opening) {
        let level = snapshot.level;
        // A view the host gave a new id keeps its camera, its selection and what it has drawn.
        if let Opening::Reopen(old) = opening
            && old != level
            && let Some(view) = self.levels.remove(&old)
        {
            self.levels.insert(level, view);
            self.renamed.push((old, level));
            if self.active == Some(old) {
                self.active = Some(level);
            }
            self.request(Request::LevelGraphics { level }, Pending::LevelGraphics(level));
        }
        match self.levels.get_mut(&level) {
            Some(view) => {
                // A level reopened with other tilesets, layers or palettes than it had needs its graphics made again.
                let renamed = view.snapshot.tileset_references != snapshot.tileset_references;
                view.replace(snapshot);
                if renamed {
                    self.request(Request::LevelGraphics { level }, Pending::LevelGraphics(level));
                }
            }
            None => {
                let mut view = LevelView::new(snapshot);
                view.options = self.canvas_options;
                if let Some(catalog) = &self.catalog {
                    view.art.actors = ActorIndex::new(catalog.clone());
                }
                self.levels.insert(level, view);
                self.request(Request::LevelGraphics { level }, Pending::LevelGraphics(level));
                self.request_catalog();
            }
        }
        if opening == Opening::Document {
            self.active = Some(level);
        }
        if matches!(opening, Opening::Document | Opening::Preview) {
            self.opened.push((level, opening));
        }
        self.request_actor_draws();
    }

    /// Everything that belonged to the project that just closed.
    /// Something was saved: into the ROM's file, or for an uploaded ROM into the host's memory only, until downloaded.
    fn note_saved(&mut self) {
        if self.uploaded() {
            self.undownloaded = true;
        }
    }

    fn opened_project(&mut self, info: ProjectInfo) {
        // Replacing an open project makes the host announce that the old one closed, to us as well.
        if self.project.is_some() {
            self.own_closes += 1;
        }
        self.forget_project();
        self.browser = None;
        self.clean_rom_asked = false;
        self.project = Some(info);
        self.request(Request::ListLevels, Pending::ListLevels);
        self.request(Request::ProjectSettings, Pending::Settings);
        self.request(Request::ProjectData, Pending::ProjectData);
        self.request(Request::Recovery, Pending::Recovery);
        self.refresh_filesystem();
    }

    fn forget_project(&mut self) {
        self.undownloaded = false;
        self.downloading = None;
        self.level_list = LevelListing::default();
        self.levels.clear();
        self.active = None;
        self.level_bytes = None;
        self.found = None;
        self.recovery_offer = None;
        self.filesystem = None;
        self.filesystem_requested = false;
        self.listed_archives.clear();
        self.file_reads.clear();
        self.file_targets = None;
        self.forget_project_graphics();
    }

    /// Another project's actors are not this one's.
    fn forget_project_graphics(&mut self) {
        self.catalog = None;
        self.catalog_requested = false;
        self.project_data = None;
    }

    fn request_catalog(&mut self) {
        if !self.catalog_requested {
            self.catalog_requested = true;
            self.request(Request::ActorCatalog, Pending::ActorCatalog);
        }
    }

    /// Asks for the draw lists of the sprites, and of anything else the view wants drawn, that have none yet.
    pub fn request_actor_draws(&mut self) {
        let wanted: Vec<(LevelId, Vec<ActorKey>)> =
            self.levels.iter_mut().map(|(&level, view)| (level, view.take_missing_actor_keys())).filter(|(_, keys)| !keys.is_empty()).collect();
        for (level, keys) in wanted {
            let inputs = keys.iter().map(ActorKey::input).collect();
            self.request(Request::RenderActors { inputs }, Pending::ActorDraws(level, keys));
        }
    }

    fn request_textures(&mut self, ids: Vec<TextureId>) {
        let mut wanted: Vec<TextureId> = Vec::new();
        for id in ids {
            if !self.textures.contains_key(&id) && self.textures_requested.insert(id.clone()) {
                wanted.push(id);
            }
        }
        if !wanted.is_empty() {
            self.request(Request::GetTextures { ids: wanted }, Pending::Textures);
        }
    }

    fn follow_build(&mut self, event: BuildEvent) {
        match event {
            BuildEvent::Started { variant } => {
                self.build = BuildState { running: true, ..BuildState::default() };
                self.build.status = if variant.is_empty() { "Building...".to_owned() } else { format!("Building {variant}...") };
            }
            BuildEvent::Progress { phase, current, total, item } => {
                // An indeterminate phase should look like work, not like a bar stuck at zero.
                self.build.progress = (total > 0).then_some((current, total));
                self.build.status = if item.is_empty() { phase } else { format!("{phase}  {item}") };
            }
            BuildEvent::Diagnostic(diagnostic) => {
                let level = match diagnostic.level {
                    DiagnosticLevel::Error => LogLevel::Error,
                    DiagnosticLevel::Warning => LogLevel::Warning,
                    DiagnosticLevel::Info => LogLevel::Info,
                };
                self.log_line(level, format!("{}: {}", diagnostic.code, diagnostic.message));
                self.build.diagnostics.push(diagnostic);
            }
            BuildEvent::Log(line) => self.log_line(LogLevel::Info, line),
            BuildEvent::Artifact(description) => self.log_line(LogLevel::Info, format!("  -> {description}")),
            BuildEvent::Finished(outcome) => {
                self.build.running = false;
                self.build.cancelling = false;
                self.build.status = BuildState::finished_status(&outcome);
                self.build.progress = outcome.ok.then_some((1, 1));
                if !outcome.cancelled {
                    self.notice = Some(if outcome.ok {
                        let text = if outcome.warnings > 0 {
                            format!("NCPatcher completed with {} warning(s).", outcome.warnings)
                        } else {
                            "NCPatcher completed successfully.".to_owned()
                        };
                        Notice { error: false, text }
                    } else {
                        let text = format!(
                            "NCPatcher did not finish: {} (exit code {}, {} error(s), {} warning(s)). See the Output panel for the diagnostics.",
                            outcome.stage, outcome.exit_code, outcome.errors, outcome.warnings
                        );
                        Notice { error: true, text }
                    });
                }
                // The build may have rewritten what the modules supply: which areas the project holds, and its files, may have changed.
                if outcome.applied {
                    self.request(Request::ListLevels, Pending::ListLevels);
                    self.refresh_filesystem();
                }
                self.build.outcome = Some(outcome);
            }
        }
    }

    fn log_line(&mut self, level: LogLevel, text: String) {
        self.log.push(LogLine { level, text });
        if self.log.len() > LOG_LIMIT {
            self.log.remove(0);
        }
    }

    fn refresh_level(&mut self, level: LevelId) {
        if let Some(area) = self.levels.get(&level).map(|v| v.snapshot.area.clone()) {
            self.request(Request::OpenLevel { area }, Pending::OpenLevel(Opening::Refresh));
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Log { level, message } => self.log_line(level, message),
            Event::LevelChanged { level, revision, dirty } => {
                let stale = self.levels.get(&level).is_some_and(|v| v.snapshot.revision < revision || v.snapshot.dirty != dirty);
                if stale && !self.busy() {
                    self.refresh_level(level);
                }
            }
            // A module tree or the ROM changed on disk: the dock's statuses may have.
            Event::FilesChanged { paths } => {
                // The project's own level list is read again when it changes.
                let list = self.project.as_ref().map(|project| match project.path.as_str() {
                    "" => PROJECT_LEVEL_LIST.to_owned(),
                    path => format!("{path}/{PROJECT_LEVEL_LIST}"),
                });
                if list.is_some_and(|list| paths.contains(&list)) {
                    self.request(Request::ListLevels, Pending::ListLevels);
                }
                self.refresh_filesystem();
            }
            // Everything served before is stale. What is on screen stays there until its replacement arrives.
            Event::GraphicsChanged { generation } if generation > self.graphics_generation => {
                self.graphics_generation = generation;
                self.catalog_requested = false;
                self.request_catalog();
                let levels: Vec<LevelId> = self.levels.keys().copied().collect();
                for level in levels {
                    if let Some(view) = self.levels.get_mut(&level) {
                        view.art.forget_draws();
                    }
                    self.request(Request::LevelGraphics { level }, Pending::LevelGraphics(level));
                }
                self.request_actor_draws();
            }
            Event::GraphicsChanged { .. } => {}
            // A variant switch closes every level session on the host, so each open level is opened again.
            Event::SettingsChanged(settings) => {
                self.settings = Some(settings);
                let open: Vec<(LevelId, String)> = self.levels.iter().map(|(&level, view)| (level, view.snapshot.area.clone())).collect();
                for (level, area) in open {
                    self.request(Request::OpenLevel { area }, Pending::OpenLevel(Opening::Reopen(level)));
                }
            }
            Event::Build(event) => self.follow_build(event),
            // Another ROM under the same project: what it holds is asked again. The levels open on the old one
            // reopen with the `SettingsChanged` that comes with it.
            Event::ProjectChanged(info) => {
                if self.project.as_ref().is_some_and(|project| project.path == info.path) {
                    self.project = Some(info);
                    self.request(Request::ListLevels, Pending::ListLevels);
                    self.refresh_filesystem();
                }
            }
            Event::ProjectClosed if self.own_closes > 0 => self.own_closes -= 1,
            Event::ProjectClosed => {
                self.project = None;
                self.forget_project();
                self.list_projects_if_any();
            }
        }
    }
}
