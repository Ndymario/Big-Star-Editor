//! Builds: an NCPatcher build of the active variant run off the request thread and reported as `Event::Build`, the ROM
//! it applies read in its place, the legacy-config migration and ROM extraction. A port of `PatcherRunner` and of
//! `MainWindow`'s `runPatcher`, `offerNcpatcherMigration`, `onPatcherFinished` and `extractRom`.

use bigstar_patching::{
    BuildObserver, DiagnosticLevel as PatcherLevel, NCPatcherStage, ProcessCancel, ProjectConfigKind, describe_stage, detect_project_config,
};
use bigstar_project::{PatchApplyOutcome, PatchApplyRequest, apply_patch_transactionally};
use bigstar_protocol::{BuildConfigKind, BuildDiagnostic, BuildEvent, BuildOutcome, DiagnosticLevel};

use super::*;

/// What a build reads and where it lands.
struct BuildPaths {
    clean_rom: PathBuf,
    /// The active variant's output, as NCPatcher's configuration has it.
    output: PathBuf,
}

fn level(level: PatcherLevel) -> DiagnosticLevel {
    match level {
        PatcherLevel::Info => DiagnosticLevel::Info,
        PatcherLevel::Warning => DiagnosticLevel::Warning,
        PatcherLevel::Error => DiagnosticLevel::Error,
    }
}

impl Shared {
    /// Saves every edited level, as a build or a patch export first does.
    fn save_dirty_levels(&self) -> Result<()> {
        let dirty: Vec<LevelId> = {
            let state = lock(&self.state);
            let open = state.open.as_ref().ok_or_else(no_project)?;
            open.levels.iter().filter(|(_, session)| session.level.dirty()).map(|(id, _)| *id).collect()
        };
        for id in dirty {
            self.save_level(id)?;
        }
        Ok(())
    }

    fn project_directory(&self) -> Result<PathBuf> {
        let state = lock(&self.state);
        Ok(state.open.as_ref().ok_or_else(no_project)?.directory.clone())
    }

    pub(super) fn build_config(&self) -> Result<Response> {
        let directory = self.project_directory()?;
        let config = detect_project_config(&directory);
        let kind = match config.kind {
            ProjectConfigKind::None => BuildConfigKind::None,
            ProjectConfigKind::V1Json => {
                let file = config.file.strip_prefix(&directory).unwrap_or(&config.file).to_string_lossy().replace('\\', "/");
                BuildConfigKind::Legacy(file)
            }
            _ => BuildConfigKind::Yaml,
        };
        Ok(Response::BuildConfig(kind))
    }

    /// Migrating rewrites how the project builds, so it is only done when asked; the old file stays put.
    pub(super) fn migrate_config(&self) -> Result<Response> {
        let directory = self.project_directory()?;
        let client = NCPatcherClient::new(&self.options.ncpatcher);
        let migrated = client.migrate(&directory, true)?;
        if !migrated.succeeded() {
            let reason = describe_stage(migrated.stage);
            self.log(LogLevel::Error, format!("NCPatcher migration failed: {reason}"));
            return Err(Error::new(ErrorCode::ExternalToolFailed, format!("NCPatcher could not convert the project:\n{reason}")));
        }
        self.log(LogLevel::Info, "Converted ncpatcher.json to ncpatcher.yaml. The original file was left in place.");
        Ok(Response::Done)
    }

    pub(super) fn build(self: &Arc<Self>, verbose: bool) -> Result<Response> {
        match detect_project_config(&self.project_directory()?).kind {
            ProjectConfigKind::None => return Err(invalid("This project has no ncpatcher.yaml, so there is nothing for NCPatcher to build.")),
            ProjectConfigKind::V1Json => {
                return Err(invalid("This project still uses the legacy ncpatcher.json; convert it to ncpatcher.yaml before building."));
            }
            _ => {}
        }
        let (directory, variant, clean_rom, output) = {
            let mut state = lock(&self.state);
            let open = state.open.as_mut().ok_or_else(no_project)?;
            if let Some(problem) = &open.settings.clean_rom_problem {
                return Err(invalid(format!("{problem} Choose one before building.")));
            }
            self.ensure_patcher(open);
            let Some(output) = open.built_rom.clone() else {
                return Err(invalid(open.settings.patcher_error.clone().unwrap_or_else(|| {
                    "ncpatcher.yaml sets no rom.output, so a build would have nowhere to go but over the clean ROM.".to_owned()
                })));
            };
            let clean_rom = clean_rom_file(&open.directory, &open.settings).unwrap_or_default();
            (open.directory.clone(), open.settings.active_variant.clone(), clean_rom, output)
        };
        if lock(&self.build).is_some() {
            return Err(invalid("NCPatcher is already running."));
        }
        self.save_dirty_levels()?;
        let cancel = Arc::new(ProcessCancel::new());
        *lock(&self.build) = Some(Arc::clone(&cancel));
        self.emit(Event::Build(BuildEvent::Started { variant: variant.clone() }));
        self.log(LogLevel::Info, format!("Running NCPatcher: {}", self.options.ncpatcher.display()));
        let shared = Arc::clone(self);
        std::thread::spawn(move || shared.run_build(&directory, variant, BuildPaths { clean_rom, output }, verbose, cancel));
        Ok(Response::Done)
    }

    /// A build becomes the ROM the project reads only after the one it produced in staging has been validated.
    fn run_build(self: &Arc<Self>, directory: &Path, variant: String, paths: BuildPaths, verbose: bool, cancel: Arc<ProcessCancel>) {
        let mut observer = BuildObserver::default();
        let shared = Arc::clone(self);
        observer.on_progress = Some(Box::new(move |p| {
            shared.emit(Event::Build(BuildEvent::Progress { phase: p.phase.clone(), current: p.current, total: p.total, item: p.item.clone() }));
        }));
        let shared = Arc::clone(self);
        observer.on_diagnostic = Some(Box::new(move |d| {
            let file = if d.location.known { d.location.file.to_string_lossy().into_owned() } else { String::new() };
            let diagnostic = BuildDiagnostic {
                level: level(d.level),
                code: d.code.clone(),
                message: d.message.clone(),
                file,
                line: d.location.line,
                column: d.location.col,
            };
            shared.emit(Event::Build(BuildEvent::Diagnostic(diagnostic)));
        }));
        let shared = Arc::clone(self);
        observer.on_artifact = Some(Box::new(move |a| {
            let mut description = if a.name.is_empty() { a.kind.clone() } else { a.name.clone() };
            if !a.action.is_empty() {
                description.push_str(&format!(" ({})", a.action));
            }
            shared.emit(Event::Build(BuildEvent::Artifact(description)));
        }));
        let shared = Arc::clone(self);
        observer.on_log = Some(Box::new(move |line: &str| shared.emit(Event::Build(BuildEvent::Log(line.to_owned())))));

        let request = PatchApplyRequest {
            project_dir: directory.to_path_buf(),
            variant,
            ncpatcher_executable: self.options.ncpatcher.clone(),
            clean_rom: paths.clean_rom,
            output: paths.output,
            verbose,
            cancel: Some(cancel),
        };
        let applied = apply_patch_transactionally(&request, &mut observer);
        drop(observer);
        *lock(&self.build) = None;
        let outcome = match applied {
            // The run never started, or the tool could not be launched: the message is the whole answer.
            Err(error) => {
                self.emit(Event::Build(BuildEvent::Log(error.message.clone())));
                BuildOutcome { ok: false, exit_code: -1, stage: error.message, errors: 1, ..BuildOutcome::default() }
            }
            Ok(result) => {
                let summary = &result.summary;
                let mut outcome = BuildOutcome {
                    ok: result.outcome == PatchApplyOutcome::Applied,
                    exit_code: summary.exit_code,
                    stage: describe_stage(summary.stage).to_owned(),
                    errors: summary.errors,
                    warnings: summary.warnings,
                    cancelled: summary.stage == NCPatcherStage::Cancelled,
                    applied: result.outcome == PatchApplyOutcome::Applied,
                };
                if result.outcome == PatchApplyOutcome::OutputRejected {
                    // The build worked and then produced something unusable; the project was left untouched.
                    self.emit(Event::Build(BuildEvent::Log(result.rejection.clone())));
                    outcome.stage = result.rejection.clone();
                    outcome.errors += 1;
                }
                if outcome.applied {
                    self.rom_rebuilt(directory, &result.built_rom);
                }
                outcome
            }
        };
        if outcome.cancelled {
            self.log(LogLevel::Info, "NCPatcher stopped: the build was cancelled.");
        } else if outcome.ok {
            let summary = if outcome.warnings > 0 {
                format!("NCPatcher completed with {} warning(s).", outcome.warnings)
            } else {
                "NCPatcher completed successfully.".to_owned()
            };
            self.log(LogLevel::Info, summary);
        } else {
            self.log(
                LogLevel::Error,
                format!(
                    "NCPatcher failed: {} (exit code {}, {} error(s), {} warning(s))",
                    outcome.stage, outcome.exit_code, outcome.errors, outcome.warnings
                ),
            );
        }
        self.emit(Event::Build(BuildEvent::Finished(outcome)));
    }

    /// The build replaced the ROM, so every level session holds bytes from a ROM that no longer exists: they close,
    /// the project reads the built ROM, and the frontends hear `SettingsChanged` and open their levels again.
    fn rom_rebuilt(&self, directory: &Path, built_rom: &Path) {
        let mut state = lock(&self.state);
        let Some(open) = state.open.as_mut().filter(|open| open.directory == directory) else { return };
        let reader = match RomReader::open(built_rom) {
            Ok(reader) => reader,
            Err(error) => {
                drop(state);
                self.log(LogLevel::Error, format!("NCPatcher completed, but the built ROM could not be read: {}", error.message));
                return;
            }
        };
        // Before the first build of this variant the project was reading the clean ROM.
        open.index = RomIndex::new(&reader);
        open.rom = Some(reader);
        open.rom_file = Some(built_rom.to_path_buf());
        open.levels.clear();
        open.patcher = None;
        open.layers = Default::default();
        open.filesystem = None;
        open.graphics = None;
        open.banner = BannerTarget::Unreported;
        let announce = self.project_changed(open, true);
        let settings = open.settings.clone();
        drop(state);
        if let Some(event) = announce {
            self.emit(event);
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::GraphicsChanged { generation });
        self.emit(Event::SettingsChanged(settings));
    }

    pub(super) fn cancel_build(&self) -> Result<Response> {
        if let Some(cancel) = lock(&self.build).as_ref() {
            cancel.request();
            self.emit(Event::Build(BuildEvent::Log("Asked NCPatcher to stop.".to_owned())));
        }
        Ok(Response::Done)
    }

    /// Delegated on purpose: NCPatcher's `rom extract` writes a directory NCPatcher itself can read back.
    pub(super) fn extract_rom(&self, target: &str) -> Result<Response> {
        let (directory, rom) = {
            let state = lock(&self.state);
            let open = state.open.as_ref().ok_or_else(no_project)?;
            let rom = open.rom_file.clone().ok_or_else(|| invalid("This project has no ROM to extract yet: choose its clean ROM, or build it."))?;
            (open.directory.clone(), rom)
        };
        let output = self.workspace()?.resolve(target)?;
        let client = NCPatcherClient::new(&self.options.ncpatcher);
        let extracted = client.extract_rom(&directory, &output, &rom)?;
        if !extracted.succeeded() {
            let message =
                extracted.diagnostics.first().map_or_else(|| "NCPatcher refused the extraction.".to_owned(), |diagnostic| diagnostic.message.clone());
            self.log(LogLevel::Error, format!("ROM extraction failed: {message}"));
            return Err(Error::new(ErrorCode::ExternalToolFailed, message));
        }
        self.log(LogLevel::Info, format!("Extracted ROM into {target}"));
        Ok(Response::Done)
    }
}
