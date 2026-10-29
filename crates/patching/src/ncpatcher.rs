//! The editor's side of the NCPatcher 2.x contract. Port of `libs/nsmb-patching/src/ncpatcher.cpp`; the contract is
//! `docs/ncpatcher-integration.md`.
//!
//! NCPatcher stays an external program: it is not vendored, not linked and not reimplemented. Three rules run
//! through this file:
//!
//! * `-C <project>` names the project, so nothing ever depends on the process-wide working directory.
//! * Every document carries a `schema` string, and an unrecognised one is refused outright rather than read as far
//!   as it happens to parse.
//! * Nothing here shells out. Arguments reach the child as a vector.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use bigstar_core::{Error, ErrorCode, Result, read_file};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::process::{ProcessCancel, ProcessRequest, describe_command, run_process};

/// NCPatcher's documented exit codes. The editor reports which stage failed, so a configuration mistake does not
/// read like a compiler crash.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum NCPatcherStage {
    #[default]
    Ok,
    Internal,
    CommandLine,
    Configuration,
    ModuleResolution,
    Toolchain,
    Compilation,
    Linking,
    Patching,
    RomIo,
    Hook,
    Cancelled,
    Unknown,
}

pub fn stage_from_exit_code(exit_code: i32) -> NCPatcherStage {
    match exit_code {
        0 => NCPatcherStage::Ok,
        1 => NCPatcherStage::Internal,
        2 => NCPatcherStage::CommandLine,
        3 => NCPatcherStage::Configuration,
        4 => NCPatcherStage::ModuleResolution,
        5 => NCPatcherStage::Toolchain,
        6 => NCPatcherStage::Compilation,
        7 => NCPatcherStage::Linking,
        8 => NCPatcherStage::Patching,
        9 => NCPatcherStage::RomIo,
        10 => NCPatcherStage::Hook,
        11 => NCPatcherStage::Cancelled,
        _ => NCPatcherStage::Unknown,
    }
}

pub fn describe_stage(stage: NCPatcherStage) -> &'static str {
    match stage {
        NCPatcherStage::Ok => "succeeded",
        NCPatcherStage::Internal => "NCPatcher hit an internal error",
        NCPatcherStage::CommandLine => "NCPatcher rejected the command line",
        NCPatcherStage::Configuration => "the project configuration is invalid",
        NCPatcherStage::ModuleResolution => "the modules could not be resolved",
        NCPatcherStage::Toolchain => "the toolchain or SDK header is missing or mismatched",
        NCPatcherStage::Compilation => "compilation failed",
        NCPatcherStage::Linking => "linking failed",
        NCPatcherStage::Patching => "patching the binaries failed",
        NCPatcherStage::RomIo => "the ROM could not be read or written",
        NCPatcherStage::Hook => "a build hook failed",
        NCPatcherStage::Cancelled => "the build was cancelled",
        NCPatcherStage::Unknown => "NCPatcher failed for an unrecognised reason",
    }
}

/// Which configuration file a directory holds. A v1 `ncpatcher.json` is readable by NCPatcher but the editor refuses
/// to build from it directly and offers `migrate --write` instead, so a project is converted deliberately.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProjectConfigKind {
    #[default]
    None,
    V2Yaml,
    V1Json,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfigLocation {
    pub kind: ProjectConfigKind,
    pub file: PathBuf,
}

pub fn detect_project_config(project_dir: &Path) -> ProjectConfigLocation {
    for name in ["ncpatcher.yaml", "ncpatcher.yml"] {
        let candidate = project_dir.join(name);
        if candidate.is_file() {
            return ProjectConfigLocation { kind: ProjectConfigKind::V2Yaml, file: candidate };
        }
    }
    let legacy = project_dir.join("ncpatcher.json");
    if legacy.is_file() {
        return ProjectConfigLocation { kind: ProjectConfigKind::V1Json, file: legacy };
    }
    ProjectConfigLocation::default()
}

// ---------------------------------------------------------------------------------------------------------------------
// ncpatcher.result/1 -- diagnostics and artifacts from one run
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    #[default]
    Error,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticLocation {
    pub known: bool,
    pub file: PathBuf,
    pub line: i32,
    pub col: i32,
    /// e.g. `targets.arm9.regions[12].maxsize`.
    pub node_path: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    /// e.g. `NCP0001`.
    pub code: String,
    pub message: String,
    pub location: DiagnosticLocation,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Artifact {
    /// `overlay`, `arm9`, ...
    pub kind: String,
    /// `arm9` / `arm7`.
    pub proc: String,
    /// `modified` / `created` / `unchanged`.
    pub action: String,
    pub name: String,
    pub ram_address: String,
    pub id: Option<i64>,
    pub size: Option<i64>,
    pub file_id: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunSummary {
    /// `ok` / `error`.
    pub status: String,
    pub exit_code: i32,
    pub stage: NCPatcherStage,
    pub duration_ms: i64,
    pub errors: i32,
    pub warnings: i32,
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Vec<Artifact>,
}

impl RunSummary {
    pub fn succeeded(&self) -> bool {
        self.exit_code == 0 && self.errors == 0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildProgress {
    /// `compile`, `link`, ...
    pub phase: String,
    pub current: i64,
    pub total: i64,
    pub item: String,
}

/// One of the [`BuildObserver`] callbacks.
pub type Observe<T> = Option<Box<dyn FnMut(&T) + Send>>;

/// Callbacks fire while the build runs, from the process reader thread and never concurrently with each other. A
/// frontend that touches its UI from them must marshal to its own thread.
#[derive(Default)]
pub struct BuildObserver {
    pub on_progress: Observe<BuildProgress>,
    pub on_artifact: Observe<Artifact>,
    pub on_diagnostic: Observe<Diagnostic>,
    /// NCPatcher's human-readable log, which in JSON message mode is on stderr.
    pub on_log: Observe<str>,
}

// ---------------------------------------------------------------------------------------------------------------------
// ncpatcher.config/1 -- what the project is configured to do
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildVariant {
    pub name: String,
    pub defines: Vec<String>,
    pub files: Vec<(String, PathBuf)>,
    pub banner: PathBuf,
    /// Which layer of a layered module tree this variant selects, per module. A module absent from the list uses the
    /// variant's own name as its layer. NCPatcher resolves with this; the editor only needs it to write an edit back
    /// into the layer the next build will read.
    pub module_variants: Vec<(String, String)>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub version: i32,
    pub config_file: PathBuf,
    pub project_root: PathBuf,
    pub rom_dir: PathBuf,
    pub rom_file: PathBuf,
    pub rom_output: PathBuf,
    pub rom_banner: PathBuf,
    pub backup_dir: PathBuf,
    pub toolchain: String,
    pub files: Vec<(String, PathBuf)>,
    /// In declaration order, which is the order the project wrote them and the order a variant picker must offer.
    /// Empty means the project configures no variants -- a different thing from the query having failed.
    pub variants: Vec<BuildVariant>,
}

impl ProjectConfig {
    pub fn has_variant(&self, name: &str) -> bool {
        self.variants.iter().any(|variant| variant.name == name)
    }

    /// Where NCPatcher writes a build of `variant` to a `.nds`: `rom.output`, or the input ROM without one, with
    /// `_<variant>` before the extension when there is a variant. Empty when the project names neither, or patches an
    /// extracted directory, which has no output of its own.
    pub fn build_output(&self, variant: &str) -> PathBuf {
        let base = if self.rom_output.as_os_str().is_empty() { &self.rom_file } else { &self.rom_output };
        if base.as_os_str().is_empty() || variant.is_empty() {
            return base.clone();
        }
        let stem = base.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
        let extension = base.extension().map(|extension| format!(".{}", extension.to_string_lossy())).unwrap_or_default();
        base.with_file_name(format!("{stem}_{variant}{extension}"))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ncpatcher.modules/1 -- the resolved module graph
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleNitroFs {
    pub dir: PathBuf,
    /// When layered, the first path segment under `dir` is a variant name rather than part of the ROM path.
    pub layered: bool,
    /// Applied underneath the built variant, so the other layers only carry what they actually override.
    pub base_variant: String,
    /// A ROM path prefix for everything this tree produces.
    pub into: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleComponent {
    pub name: String,
    pub enabled: bool,
    pub disabled_because: String,
    /// Glob patterns naming the ROM destinations this component owns. While the component is enabled they record
    /// ownership; while it is disabled they are subtracted from the module's own tree.
    pub files: Vec<String>,
    /// Component keys NCPatcher does not define, verbatim. nsmb-coop-module's `objects:` rides here.
    pub extra: Value,
}

impl Default for ModuleComponent {
    fn default() -> Self {
        Self { name: String::new(), enabled: true, disabled_because: String::new(), files: Vec::new(), extra: Value::Null }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleInfo {
    /// The directory name, and the name `modules.enabled` uses.
    pub key: String,
    /// The module's own id, which generated code names it by.
    pub id: String,
    /// Human-readable title.
    pub name: String,
    pub enabled: bool,
    pub description: String,
    pub dir: PathBuf,
    pub file: PathBuf,
    pub nitrofs: Option<ModuleNitroFs>,
    pub components: Vec<ModuleComponent>,
    /// Module root keys NCPatcher does not define. The glue module's `level-data:` rides here.
    pub extra: Value,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleGraph {
    pub dir: PathBuf,
    /// In `modules.enabled` order, which is also the order that settles two modules claiming one NitroFS
    /// destination.
    pub modules: Vec<ModuleInfo>,
}

impl ModuleGraph {
    pub fn find(&self, key: &str) -> Option<&ModuleInfo> {
        self.modules.iter().find(|module| module.key == key)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// ncpatcher.files/1 -- the ROM's NitroFS table with provenance
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum FileAction {
    #[default]
    Unchanged,
    Modified,
    Created,
}

/// One member of a Nitro archive the run edited. Present only for members it wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomArchiveMember {
    /// The member's file id, assigned by the container's own name table exactly as the ROM's FNT assigns one to a
    /// loose file. It addresses a member of this archive and means nothing in the ROM's table.
    pub id: i64,
    /// Inside the archive, as its own name table spells it.
    pub path: String,
    pub size: i64,
    /// Only ever `Unchanged` or `Modified`. The archive codec is replace-only, because inserting a member renumbers
    /// every member after it.
    pub action: FileAction,
    pub source: String,
    pub module: String,
    pub component: String,
    pub from_variant: String,
    pub source_missing: bool,
}

impl Default for RomArchiveMember {
    fn default() -> Self {
        Self {
            id: 0,
            path: String::new(),
            size: 0,
            action: FileAction::Modified,
            source: String::new(),
            module: String::new(),
            component: String::new(),
            from_variant: String::new(),
            source_missing: false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomFileEntry {
    /// The FAT index exactly as the ROM stores it. NSMB subtracts its overlay count at run time; that is a property
    /// of the game, and neither NCPatcher nor this layer applies it.
    pub id: i64,
    /// Slash-separated, no leading slash.
    pub path: String,
    pub size: i64,
    pub action: FileAction,
    /// Present only for files the run wrote.
    pub source: String,
    pub module: String,
    pub component: String,
    pub from_variant: String,
    /// Planning only: the source was not on disk to be measured, because a hook generates it. The destination, the
    /// id and the provenance still hold; the size does not.
    pub source_missing: bool,
    /// Only for an archive this run edited, and only the members it edited.
    pub members: Vec<RomArchiveMember>,
}

impl RomFileEntry {
    pub fn has_provenance(&self) -> bool {
        !self.module.is_empty() || !self.source.is_empty()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileManifest {
    /// The variant this build produced. Empty when the run had no variant, which includes `rom files --json` reading
    /// a ROM off disk.
    pub variant: String,
    /// True only for a document `files plan` wrote. Every id under `z_new/` in one is a prediction a build confirms.
    pub planned: bool,
    pub files: Vec<RomFileEntry>,
}

impl FileManifest {
    pub fn find(&self, rom_path: &str) -> Option<&RomFileEntry> {
        self.files.iter().find(|entry| entry.path == rom_path)
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Document readers. The accessors read like the C++ JsonValue ones: a missing or mistyped member reads as empty.
// ---------------------------------------------------------------------------------------------------------------------

static NULL: Value = Value::Null;

fn at<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.get(key).unwrap_or(&NULL)
}

fn text(value: &Value, key: &str) -> String {
    at(value, key).as_str().unwrap_or_default().to_owned()
}

fn flag(value: &Value, key: &str) -> bool {
    at(value, key).as_bool().unwrap_or(false)
}

/// Only an exact integer inside the range doubles represent exactly counts; a fractional or oversized value is not
/// the id or size a caller expects.
fn integer(value: &Value, fallback: i64) -> i64 {
    match value.as_f64() {
        Some(number) if number.fract() == 0.0 && number.abs() < 9_007_199_254_740_992.0 => number as i64,
        _ => fallback,
    }
}

fn int_of(value: &Value, key: &str) -> i64 {
    integer(at(value, key), 0)
}

fn optional_int(value: &Value) -> Option<i64> {
    value.is_number().then(|| integer(value, 0))
}

fn elements<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    at(value, key).as_array().map_or(&[], Vec::as_slice)
}

fn members<'a>(value: &'a Value, key: &str) -> impl Iterator<Item = (&'a String, &'a Value)> {
    at(value, key).as_object().into_iter().flatten()
}

fn string_list(value: &Value, key: &str) -> Vec<String> {
    elements(value, key).iter().filter_map(Value::as_str).map(str::to_owned).collect()
}

/// Every document NCPatcher writes names its own format and version. Reading one whose name we do not know is
/// guessing, and the guess surfaces as a wrongly patched ROM long after the fact.
fn require_schema(root: &Value, expected: &str, what: &str) -> Result<()> {
    if !root.is_object() {
        return Err(Error::new(ErrorCode::ParseError, format!("{what}: expected a JSON object at the top level")));
    }
    let Some(schema) = at(root, "schema").as_str() else {
        return Err(Error::new(ErrorCode::ParseError, format!("{what}: no \"schema\" field, so its format is unknown")));
    };
    if schema != expected {
        return Err(Error::new(
            ErrorCode::Unsupported,
            format!(
                "{what}: unsupported schema \"{schema}\", this build understands \"{expected}\". Update Big Star Editor, or use a matching NCPatcher."
            ),
        ));
    }
    Ok(())
}

fn parse_document(json: &str, what: &str, schema: &str) -> Result<Value> {
    let root: Value = serde_json::from_str(json).map_err(|error| Error::new(ErrorCode::ParseError, format!("{what}: {error}")))?;
    require_schema(&root, schema, what)?;
    Ok(root)
}

fn diagnostic_level(text: &str) -> DiagnosticLevel {
    match text {
        "warning" => DiagnosticLevel::Warning,
        "info" | "note" => DiagnosticLevel::Info,
        _ => DiagnosticLevel::Error,
    }
}

fn file_action(text: &str) -> FileAction {
    match text {
        "modified" => FileAction::Modified,
        "created" => FileAction::Created,
        _ => FileAction::Unchanged,
    }
}

fn read_diagnostic(node: &Value) -> Diagnostic {
    let location = at(node, "location");
    Diagnostic {
        level: diagnostic_level(&text(node, "level")),
        code: text(node, "code"),
        message: text(node, "message"),
        location: if location.is_object() {
            DiagnosticLocation {
                known: true,
                file: PathBuf::from(text(location, "file")),
                line: int_of(location, "line") as i32,
                col: int_of(location, "col") as i32,
                node_path: text(location, "path"),
            }
        } else {
            DiagnosticLocation::default()
        },
    }
}

fn read_artifact(node: &Value) -> Artifact {
    Artifact {
        kind: text(node, "kind"),
        proc: text(node, "proc"),
        action: text(node, "action"),
        name: text(node, "name"),
        ram_address: text(node, "ram-address"),
        id: optional_int(at(node, "id")),
        size: optional_int(at(node, "size")),
        file_id: optional_int(at(node, "file-id")),
    }
}

fn read_progress(node: &Value) -> BuildProgress {
    BuildProgress { phase: text(node, "phase"), current: int_of(node, "current"), total: int_of(node, "total"), item: text(node, "item") }
}

fn path_pairs(value: &Value, key: &str) -> Vec<(String, PathBuf)> {
    members(value, key).map(|(path, source)| (path.clone(), PathBuf::from(source.as_str().unwrap_or_default()))).collect()
}

pub fn parse_run_summary(json: &str) -> Result<RunSummary> {
    let root = parse_document(json, "NCPatcher run result", "ncpatcher.result/1")?;
    let exit_code = int_of(&root, "exit-code") as i32;
    Ok(RunSummary {
        status: text(&root, "status"),
        exit_code,
        stage: stage_from_exit_code(exit_code),
        duration_ms: int_of(&root, "duration-ms"),
        errors: int_of(&root, "errors") as i32,
        warnings: int_of(&root, "warnings") as i32,
        diagnostics: elements(&root, "diagnostics").iter().map(read_diagnostic).collect(),
        artifacts: elements(&root, "artifacts").iter().map(read_artifact).collect(),
    })
}

pub fn parse_project_config(json: &str) -> Result<ProjectConfig> {
    let root = parse_document(json, "NCPatcher project configuration", "ncpatcher.config/1")?;
    let path = |key: &str| PathBuf::from(text(&root, key));
    Ok(ProjectConfig {
        version: int_of(&root, "version") as i32,
        config_file: path("file"),
        project_root: path("project-root"),
        rom_dir: path("rom-dir"),
        rom_file: path("rom-file"),
        rom_output: path("rom-output"),
        rom_banner: path("rom-banner"),
        backup_dir: path("backup-dir"),
        toolchain: text(&root, "toolchain"),
        files: path_pairs(&root, "files"),
        // Declaration order, straight from the document: serde_json preserves member order (`preserve_order`) so
        // this list is not silently alphabetised.
        variants: members(&root, "variants")
            .map(|(name, node)| BuildVariant {
                name: name.clone(),
                defines: string_list(node, "defines"),
                files: path_pairs(node, "files"),
                banner: PathBuf::from(text(node, "banner")),
                module_variants: members(node, "module-variants")
                    .map(|(module, layer)| (module.clone(), layer.as_str().unwrap_or_default().to_owned()))
                    .collect(),
            })
            .collect(),
    })
}

pub fn parse_module_graph(json: &str) -> Result<ModuleGraph> {
    let root = parse_document(json, "NCPatcher module graph", "ncpatcher.modules/1")?;
    let modules = elements(&root, "modules")
        .iter()
        .map(|node| {
            // A disabled module ends at `enabled`: its module.yaml was never read, so there is nothing further.
            let nitrofs = at(node, "nitrofs");
            ModuleInfo {
                key: text(node, "key"),
                id: text(node, "id"),
                name: text(node, "name"),
                enabled: flag(node, "enabled"),
                description: text(node, "description"),
                dir: PathBuf::from(text(node, "dir")),
                file: PathBuf::from(text(node, "file")),
                extra: at(node, "extra").clone(),
                nitrofs: nitrofs.is_object().then(|| ModuleNitroFs {
                    dir: PathBuf::from(text(nitrofs, "dir")),
                    layered: flag(nitrofs, "layered"),
                    base_variant: text(nitrofs, "base-variant"),
                    into: text(nitrofs, "into"),
                }),
                components: elements(node, "components")
                    .iter()
                    .map(|component| ModuleComponent {
                        name: text(component, "name"),
                        enabled: flag(component, "enabled"),
                        disabled_because: text(component, "disabled-because"),
                        files: string_list(component, "files"),
                        extra: at(component, "extra").clone(),
                    })
                    .collect(),
            }
        })
        .collect();
    Ok(ModuleGraph { dir: PathBuf::from(text(&root, "dir")), modules })
}

pub fn parse_file_manifest(json: &str) -> Result<FileManifest> {
    let root = parse_document(json, "NCPatcher file manifest", "ncpatcher.files/1")?;
    let files: Vec<RomFileEntry> = elements(&root, "files")
        .iter()
        .map(|node| RomFileEntry {
            id: int_of(node, "id"),
            path: text(node, "path"),
            size: int_of(node, "size"),
            action: file_action(&text(node, "action")),
            source: text(node, "source"),
            module: text(node, "module"),
            component: text(node, "component"),
            from_variant: text(node, "from-variant"),
            source_missing: flag(node, "source-missing"),
            // Only an edited archive carries these, and only for the members the run wrote.
            members: elements(node, "members")
                .iter()
                .map(|member| RomArchiveMember {
                    id: int_of(member, "id"),
                    path: text(member, "path"),
                    size: int_of(member, "size"),
                    action: file_action(&text(member, "action")),
                    source: text(member, "source"),
                    module: text(member, "module"),
                    component: text(member, "component"),
                    from_variant: text(member, "from-variant"),
                    source_missing: flag(member, "source-missing"),
                })
                .collect(),
        })
        .collect();

    // `count` is the writer's own tally. A mismatch means the document was truncated or concatenated, and reading it
    // anyway would understate what the ROM holds.
    let declared = integer(at(&root, "count"), -1);
    if declared >= 0 && declared != files.len() as i64 {
        return Err(Error::new(ErrorCode::ParseError, format!("NCPatcher file manifest declares {declared} files but carries {}", files.len())));
    }
    Ok(FileManifest { variant: text(&root, "variant"), planned: flag(&root, "planned"), files })
}

// ---------------------------------------------------------------------------------------------------------------------
// The client
// ---------------------------------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct BuildRequest {
    pub project_dir: PathBuf,
    /// Empty means the project configures no variants. When it does configure some, NCPatcher requires an explicit
    /// selection so a build cannot silently produce the wrong language.
    pub variant: String,
    /// Patch a ROM other than the client's clean one, and write the result somewhere other than in place. The output
    /// is what makes a transactional apply possible.
    pub rom_override: PathBuf,
    pub output_override: PathBuf,
    pub verbose: bool,
    /// Set it and a caller can stop the build; NCPatcher answers with exit code 11 and a result document saying what
    /// it had finished.
    pub cancel: Option<Arc<ProcessCancel>>,
}

/// Where the client's own log lines go. The default forwards to the `log` crate.
pub type LogSink = Arc<dyn Fn(log::Level, &str) + Send + Sync>;

/// Which part of the module stack a plan answers for; see [`NCPatcherClient::plan_files`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanScope {
    /// The module with this key and every module after it in `modules.enabled`, without the project's own files.
    FromModule(String),
    /// As `FromModule`, and without the layer of that module a write would land in.
    BelowModule(String),
}

impl PlanScope {
    fn flag(&self) -> &'static str {
        match self {
            Self::FromModule(_) => "--from-module",
            Self::BelowModule(_) => "--below-module",
        }
    }

    pub fn module(&self) -> &str {
        match self {
            Self::FromModule(key) | Self::BelowModule(key) => key,
        }
    }
}

#[derive(Clone)]
pub struct NCPatcherClient {
    executable: PathBuf,
    logger: LogSink,
    /// The clean ROM, passed as `--rom` to every command that reads the project. The editor keeps it per project
    /// rather than in `ncpatcher.yaml`, which names none. Empty leaves the configuration to say.
    rom: PathBuf,
}

impl Default for NCPatcherClient {
    fn default() -> Self {
        Self::new("ncpatcher")
    }
}

struct CapturedRun {
    exit_code: i32,
    stdout_text: String,
    stderr_text: String,
}

fn temp_result_file(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn read_text(path: &Path) -> Result<String> {
    read_file(path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
}

impl NCPatcherClient {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self { executable: executable.into(), logger: Arc::new(|level, message| log::log!(level, "{message}")), rom: PathBuf::new() }
    }

    pub fn with_rom(mut self, rom: impl Into<PathBuf>) -> Self {
        self.rom = rom.into();
        self
    }

    pub fn with_logger(mut self, logger: LogSink) -> Self {
        self.logger = logger;
        self
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    fn capture(&self, project_dir: &Path, arguments: Vec<String>) -> Result<CapturedRun> {
        // The project is named, never entered by this process. Commands whose product is stdout put their log on
        // stderr, so the two are kept apart here as well.
        let request =
            ProcessRequest { executable: self.executable.clone(), arguments, working_directory: project_dir.to_path_buf(), ..Default::default() };
        let (mut stdout_text, mut stderr_text) = (String::new(), String::new());
        let mut on_out = |line: &str| {
            stdout_text.push_str(line);
            stdout_text.push('\n');
        };
        let mut on_err = |line: &str| {
            stderr_text.push_str(line);
            stderr_text.push('\n');
        };
        let result = run_process(&request, Some(&mut on_out), Some(&mut on_err))?;
        Ok(CapturedRun { exit_code: result.exit_code, stdout_text, stderr_text })
    }

    fn stage_error(run: &CapturedRun) -> Error {
        Error::new(ErrorCode::ExternalToolFailed, format!("{}:\n{}", describe_stage(stage_from_exit_code(run.exit_code)), run.stderr_text))
    }

    /// `-C`, and `--rom` with `rom` or else the client's clean ROM.
    fn project_arguments(&self, project_dir: &Path, rom: &Path) -> Vec<String> {
        let mut arguments = vec!["-C".to_owned(), project_dir.to_string_lossy().into_owned()];
        let rom = if rom.as_os_str().is_empty() { &self.rom } else { rom };
        if !rom.as_os_str().is_empty() {
            arguments.push("--rom".to_owned());
            arguments.push(rom.to_string_lossy().into_owned());
        }
        arguments
    }

    /// Preflight. Answers whether the configured executable exists and runs.
    pub fn version(&self) -> Result<String> {
        let run = self.capture(Path::new(""), vec!["version".to_owned()])?;
        if run.exit_code != 0 {
            return Err(Error::new(ErrorCode::ExternalToolFailed, format!("could not query the NCPatcher version: {}", run.stderr_text)));
        }
        Ok(run.stdout_text.trim_end_matches(['\n', '\r']).to_owned())
    }

    pub fn dump_config(&self, project_dir: &Path) -> Result<ProjectConfig> {
        let mut arguments = self.project_arguments(project_dir, Path::new(""));
        arguments.extend(["config", "dump", "--json"].map(str::to_owned));
        let run = self.capture(project_dir, arguments)?;
        if run.exit_code != 0 {
            return Err(Self::stage_error(&run));
        }
        parse_project_config(&run.stdout_text)
    }

    pub fn validate_config(&self, project_dir: &Path) -> Result<RunSummary> {
        // The result file is where the structured diagnostics live; stdout in JSON message mode carries the same
        // events as they happen.
        let result_file = temp_result_file("big-star-ncpatcher-validate.json");
        let mut arguments = self.project_arguments(project_dir, Path::new(""));
        arguments.extend(["--message-format", "json", "--result"].map(str::to_owned));
        arguments.push(result_file.to_string_lossy().into_owned());
        arguments.extend(["config", "validate"].map(str::to_owned));
        let run = self.capture(project_dir, arguments)?;
        let text = read_text(&result_file);
        let _ = std::fs::remove_file(&result_file);
        let Ok(text) = text else {
            return Err(Error::new(ErrorCode::ExternalToolFailed, format!("NCPatcher wrote no run result:\n{}", run.stderr_text)));
        };
        parse_run_summary(&text)
    }

    pub fn dump_modules(&self, project_dir: &Path) -> Result<ModuleGraph> {
        let mut arguments = self.project_arguments(project_dir, Path::new(""));
        arguments.extend(["modules", "dump"].map(str::to_owned));
        let run = self.capture(project_dir, arguments)?;
        if run.exit_code != 0 {
            return Err(Self::stage_error(&run));
        }
        parse_module_graph(&run.stdout_text)
    }

    /// Reads a ROM's file table, without provenance. `rom_override` points at a ROM or an extracted directory other
    /// than the clean one (empty for the clean one).
    pub fn rom_files(&self, project_dir: &Path, rom_override: &Path) -> Result<FileManifest> {
        let mut arguments = self.project_arguments(project_dir, rom_override);
        arguments.extend(["rom", "files", "--json"].map(str::to_owned));
        let run = self.capture(project_dir, arguments)?;
        if run.exit_code != 0 {
            return Err(Self::stage_error(&run));
        }
        parse_file_manifest(&run.stdout_text)
    }

    /// Unpacks a ROM into a directory NCPatcher itself can read back. There is one extractor and one layout on
    /// purpose.
    pub fn extract_rom(&self, project_dir: &Path, output_dir: &Path, rom_override: &Path) -> Result<RunSummary> {
        let mut arguments = self.project_arguments(project_dir, rom_override);
        arguments.extend(["rom", "extract"].map(str::to_owned));
        arguments.push(output_dir.to_string_lossy().into_owned());
        let run = self.capture(project_dir, arguments)?;
        let mut summary = RunSummary {
            exit_code: run.exit_code,
            stage: stage_from_exit_code(run.exit_code),
            status: if run.exit_code == 0 { "ok" } else { "error" }.to_owned(),
            ..RunSummary::default()
        };
        if run.exit_code != 0 {
            summary.errors = 1;
            summary.diagnostics.push(Diagnostic { level: DiagnosticLevel::Error, message: run.stderr_text, ..Diagnostic::default() });
        }
        Ok(summary)
    }

    /// Asks what a build would place, without building. The answer is the same document a build writes, with
    /// `planned` set and prospective ids for the files it would append. A `scope` asks for the ROM as one module and
    /// those after it see it, or as the layers beneath that module's own write layer supply it.
    pub fn plan_files(&self, project_dir: &Path, variant: &str, scope: Option<&PlanScope>) -> Result<FileManifest> {
        let mut arguments = self.project_arguments(project_dir, Path::new(""));
        if !variant.is_empty() {
            arguments.push("--variant".to_owned());
            arguments.push(variant.to_owned());
        }
        arguments.extend(["files", "plan", "--json"].map(str::to_owned));
        if let Some(scope) = scope {
            arguments.push(scope.flag().to_owned());
            arguments.push(scope.module().to_owned());
        }
        let run = self.capture(project_dir, arguments)?;
        // An NCPatcher from before scoped plans refuses the option as a usage error, and names it.
        if let Some(scope) = scope.filter(|scope| run.exit_code == 2 && run.stderr_text.contains(scope.flag())) {
            return Err(Error::new(
                ErrorCode::Unsupported,
                format!("this NCPatcher does not know `files plan {}`; update NCPatcher to 2.1 or later", scope.flag()),
            ));
        }
        if run.exit_code != 0 {
            return Err(Self::stage_error(&run));
        }
        let manifest = parse_file_manifest(&run.stdout_text)?;
        // A plan that does not say it is one would have its prospective z_new/ ids read as ids the ROM actually
        // holds, which is the one way this document can mislead.
        if !manifest.planned {
            return Err(Error::new(ErrorCode::ParseError, "NCPatcher answered `files plan` with a document that is not marked planned"));
        }
        Ok(manifest)
    }

    /// Reads a manifest a build already wrote (`files-dump`). This is the one with provenance.
    pub fn read_file_manifest(&self, json_file: &Path) -> Result<FileManifest> {
        parse_file_manifest(&read_text(json_file)?)
    }

    pub fn build(&self, request: &BuildRequest, observer: &mut BuildObserver) -> Result<RunSummary> {
        let config = detect_project_config(&request.project_dir);
        match config.kind {
            ProjectConfigKind::None => {
                return Err(Error::new(ErrorCode::NotFound, format!("no ncpatcher.yaml in {}", request.project_dir.display())));
            }
            ProjectConfigKind::V1Json => {
                // Building would work, and would leave a project half-migrated the first time anything touched it.
                // Converting is a decision, not a side effect.
                return Err(Error::new(
                    ErrorCode::Unsupported,
                    "this project still uses the legacy ncpatcher.json. Run NCPatcher's migrate step to convert it to ncpatcher.yaml first; the original file is left in place until a build succeeds.",
                ));
            }
            ProjectConfigKind::V2Yaml => {}
        }

        let result_file = temp_result_file("big-star-ncpatcher-build.json");
        let mut arguments = self.project_arguments(&request.project_dir, &request.rom_override);
        if !request.output_override.as_os_str().is_empty() {
            arguments.push("--out".to_owned());
            arguments.push(request.output_override.to_string_lossy().into_owned());
        }
        if request.verbose {
            arguments.push("-v".to_owned());
        }
        arguments.extend(["--message-format", "json", "--result"].map(str::to_owned));
        arguments.push(result_file.to_string_lossy().into_owned());
        // A variant is a global option and belongs ahead of the subcommand.
        if !request.variant.is_empty() {
            arguments.push("--variant".to_owned());
            arguments.push(request.variant.clone());
        }
        // `--all-variants` is deliberately not offered: it patches one opened ROM, and nine outputs have nowhere to go.
        arguments.push("build".to_owned());

        let process = ProcessRequest {
            executable: self.executable.clone(),
            arguments,
            working_directory: request.project_dir.clone(),
            environment: Vec::new(),
            cancel: request.cancel.clone(),
        };
        (self.logger)(log::Level::Info, &format!("running {}", describe_command(&process)));

        let observer = Mutex::new(observer);
        let mut trailing_result = String::new();
        let mut on_out = |line: &str| {
            let mut observer = observer.lock().unwrap_or_else(PoisonError::into_inner);
            // One JSON object per line. A line that does not parse is a tool bug rather than something to act on, so
            // it goes to the log instead of being silently swallowed.
            let event = match serde_json::from_str::<Value>(line) {
                Ok(event) if event.is_object() => event,
                _ => {
                    if let Some(on_log) = observer.on_log.as_mut() {
                        on_log(line);
                    }
                    return;
                }
            };
            match at(&event, "type").as_str().unwrap_or_default() {
                "progress" => {
                    if let Some(callback) = observer.on_progress.as_mut() {
                        callback(&read_progress(&event));
                    }
                }
                "artifact" => {
                    if let Some(callback) = observer.on_artifact.as_mut() {
                        callback(&read_artifact(&event));
                    }
                }
                "diagnostic" => {
                    if let Some(callback) = observer.on_diagnostic.as_mut() {
                        callback(&read_diagnostic(&event));
                    }
                }
                "result" => trailing_result = line.to_owned(),
                _ => {}
            }
        };
        let mut on_err = |line: &str| {
            if let Some(on_log) = observer.lock().unwrap_or_else(PoisonError::into_inner).on_log.as_mut() {
                on_log(line);
            }
        };
        let run = run_process(&process, Some(&mut on_out), Some(&mut on_err));
        let run = match run {
            Ok(run) => run,
            Err(error) => {
                let _ = std::fs::remove_file(&result_file);
                return Err(error);
            }
        };

        let result_text = read_text(&result_file);
        let _ = std::fs::remove_file(&result_file);
        if let Ok(result_text) = result_text {
            match parse_run_summary(&result_text) {
                Ok(summary) => return Ok(summary),
                Err(error) => (self.logger)(log::Level::Warn, &format!("could not read NCPatcher's run result: {}", error.message)),
            }
        }

        // No usable result file. The streamed `result` event carries the same numbers, so a run is still reportable
        // rather than merely "it failed".
        if !trailing_result.is_empty()
            && let Ok(event) = serde_json::from_str::<Value>(&trailing_result)
        {
            let exit_code = int_of(&event, "exit-code") as i32;
            return Ok(RunSummary {
                status: text(&event, "status"),
                exit_code,
                duration_ms: int_of(&event, "duration-ms"),
                errors: int_of(&event, "errors") as i32,
                warnings: int_of(&event, "warnings") as i32,
                stage: stage_from_exit_code(exit_code),
                ..RunSummary::default()
            });
        }

        Ok(RunSummary {
            exit_code: run.exit_code,
            stage: stage_from_exit_code(run.exit_code),
            status: if run.exit_code == 0 { "ok" } else { "error" }.to_owned(),
            errors: i32::from(run.exit_code != 0),
            ..RunSummary::default()
        })
    }

    /// Converts a legacy `ncpatcher.json`. Without `write` this is a dry run that verifies the resolved build is
    /// unchanged.
    pub fn migrate(&self, project_dir: &Path, write: bool) -> Result<RunSummary> {
        let result_file = temp_result_file("big-star-ncpatcher-migrate.json");
        // A legacy configuration names its own ROM, so the clean one is not passed.
        let mut arguments = vec!["-C".to_owned(), project_dir.to_string_lossy().into_owned()];
        arguments.extend(["--message-format", "json", "--result"].map(str::to_owned));
        arguments.push(result_file.to_string_lossy().into_owned());
        arguments.push("migrate".to_owned());
        if write {
            arguments.push("--write".to_owned());
        }
        let run = self.capture(project_dir, arguments)?;
        let text = read_text(&result_file);
        let _ = std::fs::remove_file(&result_file);
        let Ok(text) = text else {
            return Err(Error::new(ErrorCode::ExternalToolFailed, format!("NCPatcher wrote no run result:\n{}", run.stderr_text)));
        };
        parse_run_summary(&text)
    }
}
