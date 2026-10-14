//! Builds and patch exchange: an NCPatcher build of the active variant run off the request thread and reported as
//! `Event::Build`, the ROM it applies read in its place, the legacy-config migration, ROM extraction, `.nsmbpatch`
//! export and import, and xdelta. A port of `PatcherRunner` and of `MainWindow`'s `runPatcher`,
//! `offerNcpatcherMigration`, `onPatcherFinished`, `extractRom`, `exportPatch`, `importPatch` and `runXdelta`.

use bigstar_level::{NsmbPatchEntry, decode_nsmbpatch, encode_nsmbpatch};
use bigstar_patching::{
    BuildObserver, DiagnosticLevel as PatcherLevel, NCPatcherStage, ProcessCancel, ProcessRequest, ProjectConfigKind, describe_stage,
    detect_project_config, run_process,
};
use bigstar_project::{BUILT_ROM_FILE_NAME, FilesystemStatus, PatchApplyOutcome, PatchApplyRequest, apply_patch_transactionally};
use bigstar_protocol::{BuildConfigKind, BuildDiagnostic, BuildEvent, BuildOutcome, DiagnosticLevel};

use super::*;

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
        let (directory, variant) = {
            let state = lock(&self.state);
            let open = state.open.as_ref().ok_or_else(no_project)?;
            if open.rom.is_none() {
                return Err(invalid("Open or import a ROM project first."));
            }
            (open.directory.clone(), open.settings.active_variant.clone())
        };
        if lock(&self.build).is_some() {
            return Err(invalid("NCPatcher is already running."));
        }
        self.save_dirty_levels()?;
        match detect_project_config(&directory).kind {
            ProjectConfigKind::None => return Err(invalid("This project has no ncpatcher.yaml, so there is nothing for NCPatcher to build.")),
            ProjectConfigKind::V1Json => {
                return Err(invalid("This project still uses the legacy ncpatcher.json; convert it to ncpatcher.yaml before building."));
            }
            _ => {}
        }
        let cancel = Arc::new(ProcessCancel::new());
        *lock(&self.build) = Some(Arc::clone(&cancel));
        self.emit(Event::Build(BuildEvent::Started { variant: variant.clone() }));
        self.log(LogLevel::Info, format!("Running NCPatcher: {}", self.options.ncpatcher.display()));
        let shared = Arc::clone(self);
        std::thread::spawn(move || shared.run_build(&directory, variant, verbose, cancel));
        Ok(Response::Done)
    }

    /// A build becomes the ROM the project reads only after the one it produced in staging has been validated.
    fn run_build(self: &Arc<Self>, directory: &Path, variant: String, verbose: bool, cancel: Arc<ProcessCancel>) {
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
        // A build writes `big-star-build.nds`, which is not necessarily what the project was reading before.
        let manifest = directory.join(MANIFEST_FILE_NAME);
        let refreshed = load_manifest(&manifest).and_then(|mut project| {
            project.attach_rom(BUILT_ROM_FILE_NAME, Some(reader.header().clone()));
            index_rom_files_from(&mut project, &reader);
            save_manifest(&project, &manifest)
        });
        open.index = RomIndex::new(&reader);
        open.rom = Some(reader);
        open.rom_file = Some(built_rom.to_path_buf());
        open.levels.clear();
        open.patcher = None;
        open.filesystem = None;
        open.graphics = None;
        open.banner = BannerTarget::Unreported;
        let settings = open.settings.clone();
        drop(state);
        if let Err(error) = refreshed {
            self.log(LogLevel::Warning, format!("NCPatcher completed, but the project manifest could not be refreshed: {}", error.message));
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
            if open.rom.is_none() {
                return Err(invalid("Open or import a ROM project first."));
            }
            (open.directory.clone(), open.directory.join(default_rom_path(&open.directory)))
        };
        if !rom.is_file() {
            return Err(invalid("This project has no built ROM yet. Run NCPatcher first."));
        }
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

    /// Every module file that differs from the ROM, with the file ID it replaces so an import can tell a different ROM.
    pub(super) fn export_patch(&self) -> Result<Response> {
        self.save_dirty_levels()?;
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        let directory = open.directory.clone();
        let entries: Vec<_> = self
            .ensure_filesystem(open)?
            .entries
            .iter()
            .filter(|file| {
                file.status != FilesystemStatus::Vanilla && file.status != FilesystemStatus::Orphaned && !file.source.as_os_str().is_empty()
            })
            .cloned()
            .collect();
        let name = load_manifest(&directory.join(MANIFEST_FILE_NAME)).map(|project| project.name().to_owned()).unwrap_or_default();
        drop(state);
        let mut patch = Vec::new();
        for file in entries {
            let source = if file.source.is_absolute() { file.source.clone() } else { directory.join(&file.source) };
            let contents = read_file(&source)?;
            let expected_file_id = file.id.and_then(|id| i32::try_from(id).ok()).unwrap_or(-1);
            patch.push(NsmbPatchEntry { path: file.path.clone(), expected_file_id, contents });
        }
        if patch.is_empty() {
            return Err(invalid("There are no module changes to distribute."));
        }
        let bytes = encode_nsmbpatch(&patch)?;
        let name = if name.is_empty() { "patch".to_owned() } else { name };
        Ok(Response::Patch { name: format!("{name}.nsmbpatch"), bytes })
    }

    /// Every file of the patch goes into the module that supplies it, or the active one, all or none.
    pub(super) fn import_patch(&self, bytes: &[u8], accept_id_mismatch: bool) -> Result<Response> {
        let patch = decode_nsmbpatch(bytes)?;
        let mut state = lock(&self.state);
        let open = state.open.as_mut().ok_or_else(no_project)?;
        self.ensure_filesystem(open)?;
        let mut targets = Vec::new();
        let mut ids_differ = false;
        for entry in &patch {
            let current = open.filesystem.as_ref().and_then(|view| view.find(&entry.path));
            if let Some(id) = current.and_then(|file| file.id)
                && entry.expected_file_id >= 0
                && id != i64::from(entry.expected_file_id)
            {
                ids_differ = true;
            }
            let module = Self::target_module(open, &entry.path).map_err(|_| invalid("Choose an active module in the Filesystem dock first."))?;
            let target = module_file_target(&module, &open.settings.active_variant, &entry.path, &[])?;
            targets.push((target, entry.contents.clone()));
        }
        if ids_differ && !accept_id_mismatch {
            return Ok(Response::PatchIdsDiffer);
        }
        back_up(&open.directory, "import", &targets)?;
        write_all_or_none(&targets)?;
        prune_patch_backups(&open.directory, open_backups_kept(&open.settings));
        open.filesystem = None;
        open.patcher = None;
        open.graphics = None;
        drop(state);
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.emit(Event::GraphicsChanged { generation });
        self.log(LogLevel::Info, format!("Imported {} patch file(s) transactionally.", patch.len()));
        Ok(Response::Done)
    }

    pub(super) fn xdelta(&self, create: bool, source: &str, target: &str, output: &str) -> Result<Response> {
        let workspace = self.workspace()?;
        let paths = [workspace.resolve(source)?, workspace.resolve(target)?, workspace.resolve(output)?];
        let mode = if create { "-e" } else { "-d" };
        let mut arguments = vec![mode.to_owned(), "-f".to_owned(), "-s".to_owned()];
        arguments.extend(paths.iter().map(|path| path.to_string_lossy().into_owned()));
        let request = ProcessRequest { executable: self.options.xdelta.clone(), arguments, ..ProcessRequest::default() };
        let mut errors = String::new();
        let mut collect = |line: &str| {
            errors.push_str(line);
            errors.push('\n');
        };
        let result = run_process(&request, None, Some(&mut collect))?;
        if result.exit_code != 0 {
            return Err(Error::new(ErrorCode::ExternalToolFailed, if errors.is_empty() { "xdelta3 failed".to_owned() } else { errors }));
        }
        let message = if create { "Patch created successfully." } else { "Patched ROM created successfully." };
        self.log(LogLevel::Info, message);
        Ok(Response::Done)
    }
}
