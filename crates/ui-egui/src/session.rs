//! One connection to a host and everything the frontend has learned from it. Pure state: no drawing, so the
//! request/response bookkeeping can be tested without a window.

use std::collections::HashMap;

use bigstar_client::{Connection, ConnectionState, Incoming};
use bigstar_core::Error;
use bigstar_protocol::{
    DirectoryListing, Event, LevelEdit, LevelSnapshot, LevelWorld, LogLevel, ProjectInfo, ProjectSettings, ProjectSummary, Request, RequestId,
    Response, ResponseResult,
};

use crate::level_view::LevelView;

/// What a request in flight was for, so its response lands in the right place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pending {
    ListProjects,
    Browse,
    OpenProject,
    ImportRom,
    ListLevels,
    OpenLevel,
    Edit,
    History,
    Save,
    CloseLevel,
    CloseProject,
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
    pub level: Option<LevelView>,
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
            level: None,
            log: Vec::new(),
            notice: None,
            browser: None,
            ended: None,
            own_closes: 0,
            graphics_generation: 0,
            settings: None,
        };
        session.note_state();
        session
    }

    pub fn connected(&self) -> bool {
        matches!(self.state, ConnectionState::Connected(_))
    }

    /// An edit, history step or save is waiting for its answer; the canvas takes no new input meanwhile.
    pub fn busy(&self) -> bool {
        self.pending.values().any(|kind| matches!(kind, Pending::Edit | Pending::History | Pending::Save | Pending::OpenLevel))
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

    pub fn open_level(&mut self, area: &str) {
        self.request(Request::OpenLevel { area: area.to_owned() }, Pending::OpenLevel);
    }

    pub fn edit(&mut self, edit: LevelEdit) {
        if let Some(view) = &self.level {
            let request = Request::EditLevel { level: view.snapshot.level, base_revision: view.snapshot.revision, edit };
            self.request(request, Pending::Edit);
        }
    }

    pub fn undo(&mut self) {
        if let Some(level) = self.level.as_ref().filter(|v| v.snapshot.can_undo).map(|v| v.snapshot.level) {
            self.request(Request::UndoLevel { level }, Pending::History);
        }
    }

    pub fn redo(&mut self) {
        if let Some(level) = self.level.as_ref().filter(|v| v.snapshot.can_redo).map(|v| v.snapshot.level) {
            self.request(Request::RedoLevel { level }, Pending::History);
        }
    }

    pub fn save(&mut self) {
        if let Some(level) = self.level.as_ref().map(|v| v.snapshot.level) {
            self.request(Request::SaveLevel { level }, Pending::Save);
        }
    }

    /// Leaves the level. The host keeps the session (and its unsaved edits) only until this arrives, so a dirty level
    /// is closed only on the person's say-so; the caller asks first.
    pub fn close_level(&mut self) {
        if let Some(view) = self.level.take() {
            self.request(Request::CloseLevel { level: view.snapshot.level }, Pending::CloseLevel);
        }
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
            Err(error) => {
                self.fail(&error);
                // The host's version of the level is the truth; fetch it again.
                if matches!(kind, Pending::Edit | Pending::History | Pending::Save) {
                    self.refresh_level();
                }
                if let Some(view) = &mut self.level {
                    view.settled();
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
                self.worlds.clear();
                self.level = None;
                self.project = Some(info);
                self.request(Request::ListLevels, Pending::ListLevels);
            }
            (Pending::ImportRom, Response::Project(info)) => {
                self.browser = None;
                self.open_project(&info.path);
                self.list_projects();
            }
            (Pending::ListLevels, Response::Levels(worlds)) => self.worlds = worlds,
            (Pending::OpenLevel, Response::Level(snapshot)) => self.take_snapshot(*snapshot),
            (Pending::Edit | Pending::History | Pending::Save, Response::Edited { snapshot, outcome }) => {
                if let Some(view) = &mut self.level {
                    view.edited(*snapshot, &outcome);
                }
            }
            (Pending::CloseProject, Response::Done) => {
                self.own_closes += 1;
                self.project = None;
                self.worlds.clear();
                self.level = None;
                self.list_projects();
            }
            (Pending::CloseLevel, Response::Done) => {}
            (_, other) => log::warn!("unexpected response {other:?}"),
        }
    }

    fn take_snapshot(&mut self, snapshot: LevelSnapshot) {
        match &mut self.level {
            Some(view) if view.snapshot.level == snapshot.level => view.replace(snapshot),
            slot => *slot = Some(LevelView::new(snapshot)),
        }
    }

    fn refresh_level(&mut self) {
        if let Some(area) = self.level.as_ref().map(|v| v.snapshot.area.clone()) {
            self.open_level(&area);
        }
    }

    fn event(&mut self, event: Event) {
        match event {
            Event::Log { level, message } => {
                self.log.push(LogLine { level, text: message });
                if self.log.len() > LOG_LIMIT {
                    self.log.remove(0);
                }
            }
            Event::LevelChanged { level, revision, dirty } => {
                let stale =
                    self.level.as_ref().is_some_and(|v| v.snapshot.level == level && (v.snapshot.revision < revision || v.snapshot.dirty != dirty));
                if stale && !self.busy() {
                    self.refresh_level();
                }
            }
            Event::FilesChanged { .. } => {}
            Event::GraphicsChanged { generation } => self.graphics_generation = self.graphics_generation.max(generation),
            // A variant switch closes every level session on the host, so the open level is fetched again.
            Event::SettingsChanged(settings) => {
                self.settings = Some(settings);
                self.refresh_level();
            }
            Event::ProjectClosed if self.own_closes > 0 => self.own_closes -= 1,
            Event::ProjectClosed => {
                self.project = None;
                self.worlds.clear();
                self.level = None;
                self.list_projects();
            }
        }
    }
}
