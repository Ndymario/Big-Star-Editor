use bigstar_core::Error;
use bigstar_formats::{GlueBlock, NsmbLevelCamera, NsmbLevelCourse, NsmbLevelHeader, NsmbLevelObject, NsmbLevelScreen, NsmbLevelView, RgbaImage};
use bigstar_level::{
    LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates, LevelTilesetReferences, ModuleDataCatalog, ReorderKind, ScreenBlock,
};
use serde::{Deserialize, Serialize};

use crate::graphics::{ActorCatalog, ActorDraw, ActorInput, LevelGraphics, ProjectSettings, TextureData, TextureId};

/// Bumped whenever a message changes shape. The host refuses a frontend that disagrees.
pub const PROTOCOL_VERSION: u32 = 22;

pub type RequestId = u64;

/// The most ROM bytes one `UploadRom` or `RomChunk` carries: a ROM travels in pieces well under any socket's message limit.
pub const ROM_CHUNK_BYTES: usize = 4 * 1024 * 1024;

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
    /// `has_workspace` is false when there is no workspace for this connection: the host has none, or it let the
    /// frontend in without the workspace token. Such a connection edits only ROMs it uploads.
    Welcome {
        protocol_version: u32,
        host_version: String,
        workspace_name: String,
        has_workspace: bool,
    },
    Refused {
        reason: String,
    },
    Response {
        id: RequestId,
        result: ResponseResult,
    },
    Event(Event),
}

/// Every path in a request is relative to the workspace root and `/`-separated. The empty string is the root itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Request {
    /// The NCPatcher projects under the workspace root: the directories with an `ncpatcher.yaml` (or a legacy
    /// `ncpatcher.json`).
    ListProjects,
    /// The entries of one directory under the root.
    BrowseDirectory {
        path: String,
    },
    /// Opens an NCPatcher project directory.
    OpenProject {
        path: String,
    },
    /// Opens a `.nds` under the root to edit directly, without a project: every save writes into that file.
    OpenRom {
        path: String,
    },
    /// Writes the ROM being edited directly to another `.nds` under the root, which later saves then go to.
    SaveRomAs {
        path: String,
    },
    /// One piece of a `.nds` the frontend sends to edit directly, held only in the host's memory and only for this
    /// connection. Pieces come in order from `offset` 0, each at most [`ROM_CHUNK_BYTES`]; the last one opens the ROM.
    UploadRom {
        name: String,
        offset: u64,
        total: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    /// The piece of the uploaded ROM, every edit written into it, that starts at `offset`.
    DownloadRom {
        offset: u64,
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
    /// Mirrors the ROM, as the active module and those after it see it, into `virtual-rom/` in the project, archives
    /// as folders of their members; a file changed there is inserted into the active module. Answers with the
    /// refreshed `Filesystem` listing. Only a host on the same machine as its frontend offers it.
    ExposeVirtualRom,
    /// Stops mirroring. The folder is left as it is.
    HideVirtualRom,
    /// The clean ROM the open project is built from, a `.nds` under the root, or anywhere by its full path when the host
    /// is the desktop app's own. It is kept in the project's settings,
    /// not in `ncpatcher.yaml`, and refused unless it is New Super Mario Bros.
    SetCleanRom {
        path: String,
    },
    /// The open project's autosave interval and how many backups it keeps; clamped to 1–60 and 1–100.
    SetPreferences {
        autosave_minutes: u32,
        backups_kept: u32,
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
    /// The tilesets and backgrounds a level loads, as the files that hold them, for the tileset workspace.
    TilesetResources {
        level: LevelId,
    },
    /// Writes several files into their modules at once, the way `WriteFile` writes one; the listing and graphics
    /// change once.
    WriteFiles {
        files: Vec<FileWrite>,
    },
    /// What NCPatcher configuration the project has, so a legacy one can be offered for migration before a build.
    BuildConfig,
    /// Converts a legacy `ncpatcher.json` to `ncpatcher.yaml`, leaving the original where it is.
    MigrateConfig,
    /// Saves the edited levels and starts a build of the active variant. It runs on the host; `Event::Build` reports
    /// it, and a build that succeeds becomes the ROM the project reads.
    Build {
        verbose: bool,
    },
    /// Asks the running build to stop; nothing happens when none is running.
    CancelBuild,
    /// Has NCPatcher extract the project's built ROM into a directory under the workspace.
    ExtractRom {
        directory: String,
    },
}

/// The project's NCPatcher configuration file, as far as building is concerned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildConfigKind {
    /// No `ncpatcher.yaml`: nothing for NCPatcher to build.
    None,
    /// The legacy `ncpatcher.json`, at this path relative to the project.
    Legacy(String),
    Yaml,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    #[default]
    Error,
}

/// One diagnostic NCPatcher reported, with where, when it knows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildDiagnostic {
    pub level: DiagnosticLevel,
    pub code: String,
    pub message: String,
    /// Empty when the location is unknown.
    pub file: String,
    pub line: i32,
    pub column: i32,
}

/// How a build ended. `ok` is the whole answer: NCPatcher can exit zero with errors recorded.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildOutcome {
    pub ok: bool,
    /// -1 when the run never started.
    pub exit_code: i32,
    /// The stage that failed, or why the run never started, or that the output was rejected.
    pub stage: String,
    pub errors: i32,
    pub warnings: i32,
    pub cancelled: bool,
    /// The build became the project's ROM.
    pub applied: bool,
}

/// What a running build reports, in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuildEvent {
    Started { variant: String },
    Progress { phase: String, current: i64, total: i64, item: String },
    Diagnostic(BuildDiagnostic),
    Log(String),
    Artifact(String),
    Finished(BuildOutcome),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileWrite {
    pub path: String,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

/// One file a tileset or background is built from: the ROM path an edit is saved to (empty when the level names none)
/// and the bytes as the active variant has them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResourceBytes {
    pub rom_path: String,
    #[serde(with = "serde_bytes")]
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilesetSlot {
    pub graphics: ResourceBytes,
    pub palette: ResourceBytes,
    pub map16: ResourceBytes,
    pub objects: ResourceBytes,
    pub object_index: ResourceBytes,
    pub behaviors: ResourceBytes,
    pub tile_offset: u16,
    pub palette_offset: u8,
    pub tile_page_offset: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilemapSlot {
    pub graphics: ResourceBytes,
    pub palette: ResourceBytes,
    pub screen: ResourceBytes,
}

/// What the game draws in place of one absolute Map16 block of the level's tileset: a texture, nothing, or another
/// block. See `bigstar_formats::TileOverlay`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Map16Overlay {
    pub block: u16,
    pub image: Option<RgbaImage>,
    pub remap: Option<u16>,
    pub replaces: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TilesetResources {
    pub level: LevelId,
    /// Slots 0 (shared), 1 (the level's) and 2 (sub-unit).
    pub tilesets: Vec<TilesetSlot>,
    pub background: TilemapSlot,
    pub foreground: TilemapSlot,
    pub references: LevelTilesetReferences,
    pub overlays: Vec<Map16Overlay>,
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
    /// Replaces the level's header.
    SetHeader {
        header: NsmbLevelHeader,
        name: String,
    },
    /// Replaces one block of the level's screen records, which its views name by id.
    SetScreens {
        block: ScreenBlock,
        screens: Vec<NsmbLevelScreen>,
        name: String,
    },
    /// Replaces the level's header and one block of its screen records as one undo step: a layer's graphics with its
    /// first record's palette and screen.
    SetLayer {
        header: NsmbLevelHeader,
        block: ScreenBlock,
        screens: Vec<NsmbLevelScreen>,
        name: String,
    },
    /// Replaces one view and one block of the level's screen records as one undo step: a view made to turn, with the
    /// rotating tileset entry it names added or removed.
    SetViewScreens {
        index: usize,
        view: NsmbLevelView,
        block: ScreenBlock,
        screens: Vec<NsmbLevelScreen>,
        name: String,
    },
    /// Replaces the level's camera settings, which its views name by id.
    SetCameras {
        cameras: Vec<NsmbLevelCamera>,
        name: String,
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
    Levels(LevelListing),
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
    TilesetResources(Box<TilesetResources>),
    BuildConfig(BuildConfigKind),
    /// How much of an upload the host holds so far.
    Uploaded {
        received: u64,
    },
    /// A piece of the uploaded ROM with its edits, and the name to suggest for the whole.
    RomChunk {
        name: String,
        total: u64,
        offset: u64,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
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
    /// The folder the ROM is mirrored into, while it is.
    pub virtual_rom: Option<String>,
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
    /// The open project now reads another ROM: a clean ROM was chosen, the variant switched, or a build applied. Its
    /// levels and files are read again.
    ProjectChanged(ProjectInfo),
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
    /// A build's progress; a build that applied is followed by `SettingsChanged`, since it closed every level
    /// session to read the new ROM.
    Build(BuildEvent),
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
    /// The project directory, or the ROM file a ROM edited directly came from.
    pub path: String,
    pub name: String,
    pub rom_path: Option<String>,
    pub game_title: Option<String>,
    pub game_code: Option<String>,
    pub file_count: usize,
    pub mode: ProjectMode,
}

/// What an open project is: an NCPatcher project, which NCPatcher builds, or a ROM edited directly.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectMode {
    #[default]
    NcPatcher,
    /// A `.nds` in the workspace, which every save writes into.
    Rom,
    /// A `.nds` the frontend uploaded, held only in the host's memory; it leaves as a download.
    Uploaded,
}

impl ProjectMode {
    /// Whether this is a ROM edited directly, wherever it came from.
    pub fn is_rom(self) -> bool {
        matches!(self, ProjectMode::Rom | ProjectMode::Uploaded)
    }
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

/// A group of the level list: a world, or anything else the list gathers, holding groups of its own before its levels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelGroup {
    /// May hold `${world}`, which the frontend reads as the word for "World" in its language.
    pub name: String,
    pub groups: Vec<LevelGroup>,
    pub levels: Vec<LevelSummary>,
}

impl LevelGroup {
    /// Every area of the group and the groups in it, in the order the list shows them.
    pub fn areas(&self) -> Vec<&AreaSummary> {
        let mut areas: Vec<&AreaSummary> = self.groups.iter().flat_map(LevelGroup::areas).collect();
        areas.extend(self.levels.iter().flat_map(|level| &level.areas));
        areas
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelListing {
    /// The unnamed group the list's top level makes.
    pub root: LevelGroup,
    /// Why the project's own `levels.yaml` was not used, when it was not; the editor's own list stands in for it.
    pub problem: Option<String>,
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
