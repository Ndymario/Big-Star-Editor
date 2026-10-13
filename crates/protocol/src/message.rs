use bigstar_core::Error;
use bigstar_formats::{NsmbLevelCourse, NsmbLevelObject};
use bigstar_level::{LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, LevelTilesetReferences, ReorderKind};
use serde::{Deserialize, Serialize};

/// Bumped whenever a message changes shape. The host refuses a frontend that disagrees.
pub const PROTOCOL_VERSION: u32 = 1;

pub type RequestId = u64;

/// A level session on the host, for as long as the level stays open.
pub type LevelId = u64;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ClientFrame {
    Hello { protocol_version: u32, token: String },
    Request { id: RequestId, request: Request },
}

pub type ResponseResult = Result<Response, Error>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ServerFrame {
    Welcome { protocol_version: u32, host_version: String, workspace_name: String },
    Refused { reason: String },
    Response { id: RequestId, result: ResponseResult },
    Event(Event),
}

/// Every path in a request is relative to the workspace root and `/`-separated. The empty string is the root itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Request {
    /// The projects under the workspace root.
    ListProjects,
    /// The entries of one directory under the root, for choosing a ROM or a place for a project.
    BrowseDirectory {
        path: String,
    },
    CreateProject {
        path: String,
        name: String,
    },
    /// Copies a `.nds` that is already under the root into a new project directory.
    ImportRom {
        rom_path: String,
        project_path: String,
        name: Option<String>,
    },
    OpenProject {
        path: String,
    },
    CloseProject,
    /// The vanilla level list of the open project, with which areas its ROM holds.
    ListLevels,
    /// Opens (or rejoins) the session of one area, named by its catalog source token such as `A01_1`.
    OpenLevel {
        area: String,
    },
    /// `base_revision` is the revision the frontend last saw; an edit against any other is refused.
    EditLevel {
        level: LevelId,
        base_revision: u64,
        edit: LevelEdit,
    },
    UndoLevel {
        level: LevelId,
    },
    RedoLevel {
        level: LevelId,
    },
    SaveLevel {
        level: LevelId,
    },
    CloseLevel {
        level: LevelId,
    },
}

/// The compound mutations of `EditableLevel`, each one undo step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LevelEdit {
    Update { updates: LevelEntityUpdates, name: String },
    Add { items: LevelEntityInsertions, name: String },
    Remove { selection: LevelEntitySelection, name: String },
    Reorder { kind: ReorderKind, index: usize, forward: bool },
    SetObjectBank { slot: usize, value: u8 },
    SetTilesetReferences { references: LevelTilesetReferences },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Projects(Vec<ProjectSummary>),
    Directory(DirectoryListing),
    Project(ProjectInfo),
    Levels(Vec<LevelWorld>),
    Level(Box<LevelSnapshot>),
    Edited { snapshot: Box<LevelSnapshot>, outcome: EditOutcome },
    Done,
}

/// What an edit produced besides the new state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum EditOutcome {
    None,
    /// Where an `Add` put the new entities.
    Added(LevelEntitySelection),
    /// Where a `Reorder` left the entity.
    Moved(usize),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Event {
    Log { level: LogLevel, message: String },
    LevelChanged { level: LevelId, revision: u64, dirty: bool },
    ProjectClosed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub path: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DirectoryEntryKind {
    Directory,
    File,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryEntry {
    pub name: String,
    pub kind: DirectoryEntryKind,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirectoryListing {
    pub path: String,
    /// Directories first, then files, each group by name.
    pub entries: Vec<DirectoryEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub rom_path: Option<String>,
    pub game_title: Option<String>,
    pub game_code: Option<String>,
    pub file_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AreaSummary {
    pub token: String,
    pub display_name: String,
    /// Whether the ROM holds both of the area's files.
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelSummary {
    pub display_name: String,
    pub areas: Vec<AreaSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelWorld {
    pub name: String,
    pub levels: Vec<LevelSummary>,
}

/// A level as the frontend draws it, with everything it needs to offer undo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LevelSnapshot {
    pub level: LevelId,
    pub area: String,
    pub title: String,
    /// Starts at 1 and grows with every change to the session, including saves.
    pub revision: u64,
    pub course: NsmbLevelCourse,
    pub objects: Vec<NsmbLevelObject>,
    pub tileset_references: LevelTilesetReferences,
    pub glue_error: Option<String>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_names: Vec<String>,
    pub redo_names: Vec<String>,
}
