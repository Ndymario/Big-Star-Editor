//! One connection to a host and everything the frontend has learned from it. Pure state: no drawing, so the
//! request/response bookkeeping can be tested without a window.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::Arc;

use bigstar_client::{Connection, ConnectionState, Incoming};
use bigstar_core::Error;
use bigstar_protocol::{
    ActorCatalog, DirectoryListing, Event, FilesystemListing, FoundEntity, LevelBytes, LevelEdit, LevelId, LevelSnapshot, LevelWorld, LogLevel,
    ProjectData, ProjectInfo, ProjectSettings, ProjectSummary, RecoverableLevel, Request, RequestId, Response, ResponseResult, TextureData,
    TextureId,
};

use crate::level_view::{ActorIndex, ActorKey, LevelView};

/// What a request in flight was for, so its response lands in the right place.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Pending {
    ListProjects,
    Browse,
    OpenProject,
    ImportRom,
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

/// The host-side file browser behind "Import ROM".
#[derive(Debug, Clone, Default)]
pub struct Browser {
    pub path: String,
    pub listing: Option<DirectoryListing>,
    pub rom: Option<String>,
    pub project_name: String,
}

const LOG_LIMIT: usize = 500;

pub struct Session {
    conn: Box<dyn Connection>,
    pub state: ConnectionState,
    pending: HashMap<RequestId, Pending>,
    pub projects: Vec<ProjectSummary>,
    pub project: Option<ProjectInfo>,
    pub worlds: Vec<LevelWorld>,
    /// Every level open on the host for this frontend, documents and preview alike.
    pub levels: BTreeMap<LevelId, LevelView>,
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
    pub log: Vec<LogLine>,
    pub notice: Option<Notice>,
    pub browser: Option<Browser>,
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
            worlds: Vec::new(),
            levels: BTreeMap::new(),
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
            log: Vec::new(),
            notice: None,
            browser: None,
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

    pub fn import_rom(&mut self, rom_path: &str, project_path: &str, name: &str) {
        let name = (!name.trim().is_empty()).then(|| name.trim().to_owned());
        self.request(Request::ImportRom { rom_path: rom_path.to_owned(), project_path: project_path.to_owned(), name }, Pending::ImportRom);
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
        self.levels.values().any(|view| view.snapshot.dirty)
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
            ConnectionState::Connected(_) if !was_connected => self.list_projects(),
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
            (Pending::OpenProject, Response::Project(info)) => {
                // Replacing an open project makes the host announce that the old one closed, to us as well.
                if self.project.is_some() {
                    self.own_closes += 1;
                }
                self.forget_project();
                self.project = Some(info);
                self.request(Request::ListLevels, Pending::ListLevels);
                self.request(Request::ProjectData, Pending::ProjectData);
                self.request(Request::Recovery, Pending::Recovery);
                self.refresh_filesystem();
            }
            (Pending::ImportRom, Response::Project(info)) => {
                self.browser = None;
                self.open_project(&info.path);
                self.list_projects();
            }
            (Pending::ListLevels, Response::Levels(worlds)) => self.worlds = worlds,
            (Pending::OpenLevel(opening), Response::Level(snapshot)) => self.take_snapshot(*snapshot, opening),
            (pending @ (Pending::Edit(_) | Pending::History(_) | Pending::Save(_)), Response::Edited { snapshot, outcome }) => {
                let level = snapshot.level;
                let owed = self.levels.get_mut(&level).and_then(|view| view.edited(*snapshot, &outcome));
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
                self.list_projects();
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
            (Pending::FileChange, Response::Done) => self.refresh_filesystem(),
            (Pending::FileTargets(path), Response::FileTargets(holding)) => self.file_targets = Some((path, holding)),
            (Pending::Banner, Response::Banner { bytes, target }) => self.banner = Some((bytes, target)),
            (Pending::BannerWrite, Response::Done) => {}
            (Pending::Settings, Response::Settings(settings)) => {
                self.settings = Some(settings);
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
            Some(view) => view.replace(snapshot),
            None => {
                let mut view = LevelView::new(snapshot);
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
    fn forget_project(&mut self) {
        self.worlds.clear();
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
            Event::FilesChanged { .. } => self.refresh_filesystem(),
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
            Event::ProjectClosed if self.own_closes > 0 => self.own_closes -= 1,
            Event::ProjectClosed => {
                self.project = None;
                self.forget_project();
                self.list_projects();
            }
        }
    }
}
