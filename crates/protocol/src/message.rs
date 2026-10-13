use bigstar_core::Error;
use bigstar_formats::{GlueBlock, NsmbLevelCourse, NsmbLevelObject};
use bigstar_level::{LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, LevelTilesetReferences, ModuleDataCatalog, ReorderKind};
use serde::{Deserialize, Serialize};

use crate::graphics::{ActorCatalog, ActorDraw, ActorInput, LevelGraphics, ProjectSettings, TextureData, TextureId};

/// Bumped whenever a message changes shape. The host refuses a frontend that disagrees.
pub const PROTOCOL_VERSION: u32 = 6;

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
    /// The pixels behind texture ids the frontend does not hold yet. Ids the host no longer has are left out.
    GetTextures {
        ids: Vec<TextureId>,
    },
    /// The open level's tileset atlases, object layouts and background images.
    LevelGraphics {
        level: LevelId,
    },
    /// The actors the open project knows, for the picker and the properties panel.
    ActorCatalog,
    /// What each sprite draws and where it can be grabbed, in the order asked.
    RenderActors {
        inputs: Vec<ActorInput>,
    },
    /// The active variant and module, and what NCPatcher says the project configures.
    ProjectSettings,
    /// Switches the variant whose module layers the editor reads. Closes every level session, so it is refused while
    /// one has unsaved edits.
    SetActiveVariant {
        variant: String,
    },
    SetActiveModule {
        module: String,
    },
    /// What the open project's modules declare and which overlay each object bank loads.
    ProjectData,
    /// The open level serialized as it would be saved, for the hex editor and `.nml` export.
    LevelBytes {
        level: LevelId,
    },
    /// Every placement of object `id` (or, with `sprites`, sprite type `id`) in every level the project holds.
    FindEntities {
        sprites: bool,
        id: u16,
    },
    /// The levels an interrupted session left autosaved and unsaved.
    Recovery,
    /// Applies (`accept`) or throws away the autosaved levels. Applied ones become dirty level sessions.
    Recover {
        accept: bool,
    },
    /// The Filesystem dock's rows: the ROM, the plan and the last build joined.
    Filesystem,
    /// An archive's members that the plan had no reason to name, listed when the archive is first opened.
    ListArchive {
        path: String,
    },
    /// A file's bytes as the active variant has them: the module's copy when there is one, else the ROM's.
    ReadFile {
        path: String,
    },
    /// Writes a file into its module (or the active module) for the next build. A path the ROM does not hold must be
    /// under `z_new/`.
    WriteFile {
        path: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    /// Which variants (`""` for the base layer) already hold a module copy of `path` in the target module.
    FileTargets {
        path: String,
    },
    /// Moves a module's copy of a file into the project's trash; the ROM's copy shows again.
    RemoveFile {
        path: String,
    },
    /// The ROM banner as the next build will use it: the configured banner file once it exists, and otherwise the one
    /// in the ROM.
    Banner,
    /// Writes the configured banner file.
    WriteBanner {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
}

/// The compound mutations of `EditableLevel`, each one undo step.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum LevelEdit {
    Update {
        updates: LevelEntityUpdates,
        name: String,
    },
    Add {
        items: LevelEntityInsertions,
        name: String,
    },
    Remove {
        selection: LevelEntitySelection,
        name: String,
    },
    Reorder {
        kind: ReorderKind,
        index: usize,
        forward: bool,
    },
    SetObjectBank {
        slot: usize,
        value: u8,
    },
    SetTilesetReferences {
        references: LevelTilesetReferences,
    },
    /// Replaces the level's glue block: its level-data flags and values. The stage-object registration is derived from
    /// the sprites and is not the frontend's to write.
    SetGlue {
        glue: GlueBlock,
        name: String,
    },
    /// Replaces the whole level with these serialized files, as one undo step: the hex editor and `.nml` import.
    ReplaceSerialized {
        #[serde(with = "serde_bytes")]
        course: Vec<u8>,
        #[serde(with = "serde_bytes")]
        bgdat: Vec<u8>,
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Response {
    Projects(Vec<ProjectSummary>),
    Directory(DirectoryListing),
    Project(ProjectInfo),
    Levels(Vec<LevelWorld>),
    Level(Box<LevelSnapshot>),
    Edited {
        snapshot: Box<LevelSnapshot>,
        outcome: EditOutcome,
    },
    Textures(Vec<TextureData>),
    LevelGraphics(Box<LevelGraphics>),
    ActorCatalog(Box<ActorCatalog>),
    ActorDraws {
        generation: u64,
        draws: Vec<ActorDraw>,
    },
    Settings(ProjectSettings),
    ProjectData(Box<ProjectData>),
    LevelBytes(Box<LevelBytes>),
    Found(Vec<FoundEntity>),
    Recovery(Vec<RecoverableLevel>),
    /// The areas whose autosaves were applied; open them to see the recovered edits.
    Recovered(Vec<String>),
    Filesystem(Box<FilesystemListing>),
    Files(Vec<FileEntry>),
    FileBytes {
        path: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    FileTargets(Vec<String>),
    /// The banner's bytes, and the configured file an edit goes to, relative to the project where it is inside it.
    Banner {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        target: String,
    },
    Done,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileStatus {
    #[default]
    Vanilla,
    Replaced,
    Added,
    /// The modules differ from the last build: a rebuild (or a first insertion) is due.
    Pending,
    /// The last build placed it and no module supplies it any more; the next build reverts it.
    Orphaned,
}

/// What a file is, for its icon. Read from the head of the file, then its name.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileIcon {
    #[default]
    File,
    Archive,
    Image,
    Palette,
    ScreenMap,
    Model,
    Animation,
    Audio,
    Level,
    Text,
    Particle,
    TouchHitbox,
    OamPositions,
}

/// One row of the Filesystem dock. `path` is a ROM address; `a.narc!b/c` is member `b/c` of archive `a.narc`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub path: String,
    pub status: FileStatus,
    /// The module supplying it, empty when the ROM does.
    pub module: String,
    pub component: String,
    pub variant: String,
    pub id: Option<i64>,
    /// `id` is what the next build would assign, not an address the game can use yet.
    pub id_is_planned: bool,
    /// A member's id inside its archive's own name table.
    pub member_id: Option<i64>,
    pub size: u64,
    /// A module file supplies it, so it can be removed.
    pub from_module: bool,
    pub icon: FileIcon,
}

impl FileEntry {
    pub fn inside_archive(&self) -> bool {
        self.member_id.is_some()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleChoice {
    pub key: String,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilesystemListing {
    pub entries: Vec<FileEntry>,
    /// Modules differ from the last build.
    pub pending: bool,
    /// The enabled modules with a NitroFS tree: where add and replace can write.
    pub modules: Vec<ModuleChoice>,
    pub active_module: String,
    pub active_variant: String,
    /// Why there is no plan, when NCPatcher could not be asked.
    pub error: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelBytes {
    #[serde(with = "serde_bytes")]
    pub course: Vec<u8>,
    #[serde(with = "serde_bytes")]
    pub bgdat: Vec<u8>,
    /// The ROM file ids of the two files, which an `.nml` records; `None` for a file inside an archive.
    pub course_file_id: Option<u16>,
    pub bgdat_file_id: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FoundEntity {
    pub area: String,
    pub title: String,
    pub index: usize,
    pub x: i32,
    pub y: i32,
    /// A sprite's settings word.
    pub settings: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverableLevel {
    pub area: String,
    pub title: String,
}

/// Which code overlay object bank `bank` (1-10) of tileset slot `slot` loads: `table[slot][bank - 1]`.
pub type ObjectBankOverlays = [[u32; 10]; 16];

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectData {
    pub modules: ModuleDataCatalog,
    /// Read out of the ROM's ARM9; `None` when its table is not where the host knows to look.
    pub object_bank_overlays: Option<ObjectBankOverlays>,
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
    Log {
        level: LogLevel,
        message: String,
    },
    LevelChanged {
        level: LevelId,
        revision: u64,
        dirty: bool,
    },
    ProjectClosed,
    /// Files under the workspace changed on disk, debounced, relative to the root.
    FilesChanged {
        paths: Vec<String>,
    },
    /// Something the graphics were read from changed. Everything served for an older generation is stale.
    GraphicsChanged {
        generation: u64,
    },
    /// The project's settings changed, from this connection or another. A variant switch closes every level session.
    SettingsChanged(ProjectSettings),
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
    /// The area's two files as the Filesystem dock names them (`dat/X.narc!Y.bin` inside an archive); empty when
    /// missing.
    pub course_path: String,
    pub bgdat_path: String,
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
    /// The level-data flags and values; when `glue_error` is set this is what could be read, and the block is kept
    /// byte for byte.
    pub glue: GlueBlock,
    /// The id the first registered module sprite takes; the rest follow in `registration_hashes` order.
    pub stage_object_id_base: u16,
    /// The module sprites this level registers, derived from its sprites.
    pub registration_hashes: Vec<u32>,
    /// Per sprite, the hash a module sprite is registered under in the level's glue block; `None` for a vanilla one.
    pub sprite_hashes: Vec<Option<u32>>,
    pub dirty: bool,
    pub can_undo: bool,
    pub can_redo: bool,
    pub undo_names: Vec<String>,
    pub redo_names: Vec<String>,
}
