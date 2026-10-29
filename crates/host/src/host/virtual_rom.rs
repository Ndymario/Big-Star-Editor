//! The ROM as a folder of plain files, `virtual-rom/` in the project, kept in step both ways so that tools which only
//! know files can edit the game. It shows the ROM as the active module and the modules after it see it -- the clean
//! ROM under `files plan --from-module` -- with every archive unpacked into a folder of its members, spelled the way a
//! module holds them (`dat/Dat_Field_narc/J01_1.bin`).
//!
//! A file changed there is inserted into the active module, unless it reads as the layers below the module already
//! give it, in which case the module's copy goes instead. A file deleted there takes the module's copy away with it,
//! and so reads as the layers below give it, or is gone when they have nothing; a file the module has no copy of
//! cannot be deleted, and comes back. A change to the modules is written back into the folder, only into the files
//! whose bytes it changed, so a tool watching them sees nothing else move.
//!
//! On Linux the folder is mounted, through FUSE, as a filesystem of its own ([`mount`]): its files are never written
//! to the disk, and an edit reaches the host as the file is closed rather than when the watcher reports it. Elsewhere,
//! or where it cannot be mounted, it is a folder of plain files.
//!
//! What was last written is recorded, by ROM address, with a hash of the bytes. That is how the host tells its own
//! writes from somebody else's when the watcher reports them, how a path in the folder is traced back to its address,
//! and, kept in `.bigstar/virtual-rom.json`, how an edit made while the editor was closed is found when it reopens.

use std::collections::{BTreeMap, BTreeSet};

use bigstar_core::write_text_file_atomically;
use bigstar_patching::{FileManifest, PlanScope};
use bigstar_project::{EffectiveFilesystem, archive_folder_path};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::*;

#[cfg(target_os = "linux")]
mod mount;

/// Only Linux mounts the folder; elsewhere there never is a mount.
#[cfg(not(target_os = "linux"))]
mod mount {
    use super::*;

    pub(in crate::host) enum Mounted {}

    impl Mounted {
        pub(in crate::host) fn start(_root: &Path, _host: std::sync::Weak<Shared>) -> std::io::Result<Self> {
            Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "only Linux mounts the ROM's folder"))
        }

        pub(in crate::host) fn id(&self) -> u64 {
            match *self {}
        }

        pub(in crate::host) fn put(&self, _relative: &str, _address: &str, _bytes: &[u8]) {
            match *self {}
        }

        pub(in crate::host) fn remove(&self, _relative: &str) {
            match *self {}
        }

        pub(in crate::host) fn unsaved(&self) -> (Files, Files) {
            match *self {}
        }
    }

    pub fn unmount_all() {}
}

pub use mount::unmount_all;

/// The mirror's folder, relative to the project.
pub(crate) const VIRTUAL_ROM_DIR: &str = "virtual-rom";
/// What the mirror last wrote, relative to the project. Being under `.bigstar` keeps the watcher from treating it as an
/// edit.
const RECORD_FILE: &str = ".bigstar/virtual-rom.json";
/// Where files left in the folder that were never inserted go when it is removed, relative to the project.
pub(crate) const LOST_DIR: &str = ".bigstar/lost";
/// How long an edit made in the mounted folder waits for the host before it fails: the host may be busy with
/// something that reads the folder itself.
const MOUNT_PATIENCE: std::time::Duration = std::time::Duration::from_secs(10);

/// One file of the folder: where it is under the folder (`/`-separated), and the SHA-256 of what was last written
/// there or taken from there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Recorded {
    path: String,
    hash: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Record {
    module: String,
    variant: String,
    files: BTreeMap<String, Recorded>,
}

pub(super) struct VirtualRom {
    root: PathBuf,
    module: ModuleInfo,
    variant: String,
    /// The clean ROM under the plan from the active module down.
    view: EffectiveFilesystem,
    /// That plan; `None` when NCPatcher could not answer, and then the active module's own copy of a file is read
    /// before the clean ROM's.
    plan: Option<FileManifest>,
    /// By ROM address.
    written: BTreeMap<String, Recorded>,
    /// The folder mounted, while it is; its files are then in there, not on the disk.
    mount: Option<mount::Mounted>,
}

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Files with what they hold: by ROM address, or by where they are in the folder.
type Files = Vec<(String, Vec<u8>)>;

/// A path in the folder, relative to it, that holds a new file of the ROM's: one under `z_new/` that is not a dot file.
fn new_file_address(relative: &str) -> Option<String> {
    (relative.starts_with("z_new/") && !hidden(relative)).then(|| relative.to_owned())
}

/// What an edit made in the folder came to.
enum Imported {
    Inserted,
    Dropped,
    Unchanged,
}

impl VirtualRom {
    /// What the folder should hold for `address`.
    fn read(&mut self, address: &str) -> Result<Vec<u8>> {
        if self.plan.is_none()
            && let Ok(own) = module_file_target(&self.module, &self.variant, address, &[])
            && own.is_file()
        {
            return read_file(&own);
        }
        self.view.read(address)
    }

    /// Every address the folder holds: the ROM's files with each archive in its named members, and the new files
    /// under `z_new/`.
    fn addresses(&mut self) -> BTreeSet<String> {
        let mut out = BTreeSet::new();
        let planned_members = |plan: Option<&FileManifest>, container: &str| -> Vec<String> {
            let entry = plan.and_then(|plan| plan.find(container));
            entry
                .map(|entry| entry.members.iter().filter(|member| !member.path.is_empty()).map(|member| member.path.clone()).collect())
                .unwrap_or_default()
        };
        if let Some(rom) = self.view.rom_mut() {
            let files: Vec<String> = rom.files().iter().map(|file| file.path.clone()).collect();
            for path in files {
                // An archive whose name gives no folder spelling is kept whole.
                if archive_folder_path(&format!("{path}!member")).is_err() || !rom.is_archive(&path) {
                    out.insert(path);
                    continue;
                }
                let members = rom.list_archive(&path).unwrap_or_default();
                out.extend(members.into_iter().filter(|member| !member.path.is_empty()).map(|member| format!("{path}!{}", member.path)));
                out.extend(planned_members(self.plan.as_ref(), &path).into_iter().map(|member| format!("{path}!{member}")));
            }
        }
        match &self.plan {
            Some(plan) => out.extend(plan.files.iter().filter(|file| file.path.starts_with("z_new/")).map(|file| file.path.clone())),
            None => out.extend(self.written.keys().filter(|address| address.starts_with("z_new/")).cloned()),
        }
        out
    }

    /// Writes what the folder should hold for each address in `only`, or for every address, into any file whose bytes
    /// differ, and drops what the ROM no longer has. Unchanged files are left alone, so their times do not move.
    fn materialize(&mut self, only: Option<&BTreeSet<String>>) -> Result<()> {
        let addresses = match only {
            Some(only) => only.clone(),
            None => self.addresses(),
        };
        for address in &addresses {
            let Ok(bytes) = self.read(address) else {
                self.forget(address);
                continue;
            };
            let Ok(relative) = archive_folder_path(address) else { continue };
            if let Some(mount) = &self.mount {
                mount.put(&relative, address, &bytes);
            } else {
                let file = self.root.join(&relative);
                if read_file(&file).ok().as_deref() != Some(bytes.as_slice()) {
                    write_file_atomically(&file, &bytes)?;
                }
            }
            self.written.insert(address.clone(), Recorded { path: relative, hash: hash(&bytes) });
        }
        if only.is_none() {
            let gone: Vec<String> = self.written.keys().filter(|address| !addresses.contains(*address)).cloned().collect();
            for address in gone {
                self.forget(&address);
            }
        }
        Ok(())
    }

    /// Stops holding `address`. Its file goes only when it is still what was written there: an edit nobody has
    /// inserted yet is not the mirror's to delete.
    fn forget(&mut self, address: &str) {
        let Some(recorded) = self.written.remove(address) else { return };
        if let Some(mount) = &self.mount {
            mount.remove(&recorded.path);
            return;
        }
        let file = self.root.join(&recorded.path);
        match read_file(&file) {
            Ok(bytes) if hash(&bytes) != recorded.hash => return,
            Ok(_) => {
                let _ = std::fs::remove_file(&file);
            }
            // Deleted already, by whoever deleted it from the folder; the folders it leaves empty go all the same.
            Err(_) => {}
        }
        let mut directory = file.parent();
        while let Some(dir) = directory.filter(|dir| dir.starts_with(&self.root) && *dir != self.root) {
            if std::fs::remove_dir(dir).is_err() {
                break;
            }
            directory = dir.parent();
        }
    }

    /// Keeps what was written for the next session; a mount writes nothing that would outlast it.
    fn save_record(&self, project: &Path) {
        if self.mount.is_some() {
            return;
        }
        let record = Record { module: self.module.key.clone(), variant: self.variant.clone(), files: self.written.clone() };
        let saved = serde_json::to_string(&record)
            .map_err(|e| Error::new(ErrorCode::InvalidArgument, e.to_string()))
            .and_then(|text| write_text_file_atomically(&project.join(RECORD_FILE), &text));
        if let Err(error) = saved {
            log::warn!("could not record the virtual ROM folder: {}", error.message);
        }
    }

    /// The address a file in the folder holds, by where it is under the folder.
    fn address_of(&self, relative: &str) -> Option<String> {
        self.written.iter().find(|(_, recorded)| recorded.path == relative).map(|(address, _)| address.clone())
    }
}

/// `path` relative to the project, `/`-separated. Either may be spelled through a link (`/var` for `/private/var`), so
/// failing the plain spelling, both are resolved: the path through its nearest ancestor that still exists, since a
/// file reported as deleted no longer does.
fn project_relative(directory: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(directory).ok().map(Path::to_path_buf).or_else(|| {
        let real_directory = directory.canonicalize().ok()?;
        let (existing, rest) = path.ancestors().find_map(|ancestor| Some((ancestor.canonicalize().ok()?, path.strip_prefix(ancestor).ok()?)))?;
        existing.join(rest).strip_prefix(real_directory).ok().map(Path::to_path_buf)
    })?;
    Some(relative.components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/"))
}

fn read_record(project: &Path) -> Option<Record> {
    std::fs::read_to_string(project.join(RECORD_FILE)).ok().and_then(|text| serde_json::from_str(&text).ok())
}

/// Every file under `root`, at any depth.
fn files_under(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(&directory).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() { pending.push(path) } else { out.push(path) }
        }
    }
    out
}

/// Whether a path in the folder, relative to it, is or is under a dot file: something another tool keeps there
/// (`.DS_Store`, an editor's swap file), never a file of the ROM.
fn hidden(relative: &str) -> bool {
    relative.split('/').any(|part| part.starts_with('.'))
}

/// Whether the watcher's `path` is in the mirror's folder.
pub(super) fn in_virtual_rom(directory: &Path, path: &Path) -> bool {
    project_relative(directory, path).is_some_and(|relative| relative == VIRTUAL_ROM_DIR || relative.starts_with(&format!("{VIRTUAL_ROM_DIR}/")))
}

/// Every address a plan names a source for, members included, with that source and its module.
fn sources(plan: Option<&FileManifest>) -> BTreeMap<String, (String, String)> {
    let mut out = BTreeMap::new();
    for file in plan.map(|plan| plan.files.as_slice()).unwrap_or_default() {
        out.insert(file.path.clone(), (file.source.clone(), file.module.clone()));
        for member in &file.members {
            out.insert(format!("{}!{}", file.path, member.path), (member.source.clone(), member.module.clone()));
        }
    }
    out
}

/// What the dock shows, what graphics read from and what NCPatcher said have all changed with a module file.
fn modules_changed(open: &mut OpenProject) {
    open.filesystem = None;
    open.patcher = None;
    open.layers = Default::default();
    open.graphics = None;
}

/// Locks `mutex`, waiting no longer than `patience`.
fn lock_within<T>(mutex: &Mutex<T>, patience: std::time::Duration) -> Option<MutexGuard<'_, T>> {
    let start = std::time::Instant::now();
    loop {
        match mutex.try_lock() {
            Ok(guard) => return Some(guard),
            Err(std::sync::TryLockError::Poisoned(poisoned)) => return Some(poisoned.into_inner()),
            Err(std::sync::TryLockError::WouldBlock) if start.elapsed() < patience => std::thread::sleep(std::time::Duration::from_millis(5)),
            Err(std::sync::TryLockError::WouldBlock) => return None,
        }
    }
}

/// The module edits in the folder go to: the active one, or the first that can take files, as the dock shows it.
fn active_module(open: &OpenProject) -> Result<ModuleInfo> {
    let mut modules = open.patcher.as_ref().map(|(modules, _)| modules.clone()).unwrap_or_default();
    crate::filesystem::localize_modules(&mut modules, &open.directory);
    let candidates: Vec<&ModuleInfo> = modules.modules.iter().filter(|module| module.enabled && module.nitrofs.is_some()).collect();
    let found = candidates.iter().find(|module| module.key == open.settings.active_module).or_else(|| candidates.first());
    found
        .map(|module| (*module).clone())
        .ok_or_else(|| invalid("choose an active module in the Filesystem dock first; edits in the folder go into it"))
}

impl Shared {
    pub(super) fn expose_virtual_rom(&self) -> Result<Response> {
        if !self.local_files {
            return Err(invalid("Only the desktop app's own host can expose the ROM as a folder, on the machine that holds the project."));
        }
        {
            let mut state = lock(&self.state);
            let open = state.open.as_mut().ok_or_else(no_project)?;
            if open.virtual_rom.is_none() {
                self.start_virtual_rom(open)?;
                open.settings.virtual_rom = true;
                save_settings(&open.directory, &open.settings)?;
                let root = open.directory.join(VIRTUAL_ROM_DIR);
                self.log(
                    LogLevel::Info,
                    format!("The ROM is exposed as the folder {}; files changed there go into the active module", root.display()),
                );
                self.emit(Event::SettingsChanged(open.settings.clone()));
            }
        }
        self.filesystem()
    }

    /// Stops the mirror and removes its folder. Edits not inserted yet are inserted first; a file that still is not
    /// what the record says -- one that could not be inserted, or one nobody should have put there -- goes to
    /// `.bigstar/lost` rather than with the folder.
    pub(super) fn hide_virtual_rom(&self) -> Result<Response> {
        let mut inserted = false;
        {
            let mut state = lock(&self.state);
            let open = state.open.as_mut().ok_or_else(no_project)?;
            if open.virtual_rom.is_some() || open.settings.virtual_rom {
                match open.virtual_rom.take() {
                    Some(mirror) if mirror.mount.is_some() => inserted = self.unmount_virtual_rom(open, mirror),
                    mirror => {
                        let root = open.directory.join(VIRTUAL_ROM_DIR);
                        open.virtual_rom = mirror;
                        inserted = self.virtual_rom_changed(open, &files_under(&root));
                        let written = match open.virtual_rom.take() {
                            Some(mirror) => mirror.written,
                            None => read_record(&open.directory).map(|record| record.files).unwrap_or_default(),
                        };
                        self.remove_virtual_rom(&open.directory, &written);
                    }
                }
                open.settings.virtual_rom = false;
                save_settings(&open.directory, &open.settings)?;
                self.log(LogLevel::Info, "The ROM is no longer exposed as a folder");
                self.emit(Event::SettingsChanged(open.settings.clone()));
            }
        }
        if inserted {
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.emit(Event::GraphicsChanged { generation });
        }
        self.filesystem()
    }

    /// Inserts what the mount holds that was never inserted, unmounts it, and keeps the files made there that are no
    /// file of the ROM's in `.bigstar/lost`. Returns whether any module file changed.
    fn unmount_virtual_rom(&self, open: &mut OpenProject, mut mirror: VirtualRom) -> bool {
        let (edits, strays) = mirror.mount.as_ref().map(mount::Mounted::unsaved).unwrap_or_default();
        let inserted = !edits.is_empty() && self.import(open, &mut mirror, edits);
        drop(mirror);
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let lost = open.directory.join(LOST_DIR).join(stamp.to_string());
        let kept = strays.iter().filter(|(relative, bytes)| write_file_atomically(&lost.join(relative), bytes).is_ok()).count();
        if kept > 0 {
            self.log(LogLevel::Warning, format!("{kept} file(s) in the ROM's folder were never inserted; moved them to {}", lost.display()));
        }
        // Empty once unmounted; anything else in it is not the mirror's to remove.
        let _ = std::fs::remove_dir(open.directory.join(VIRTUAL_ROM_DIR));
        let _ = std::fs::remove_file(open.directory.join(RECORD_FILE));
        inserted
    }

    /// Deletes the folder and its record, moving each file that is not what `written` says was last written or taken
    /// from there into `.bigstar/lost`, under the time and where it was in the folder. Dot files go with the folder.
    fn remove_virtual_rom(&self, project: &Path, written: &BTreeMap<String, Recorded>) {
        let root = project.join(VIRTUAL_ROM_DIR);
        let known: BTreeMap<&str, &str> = written.values().map(|recorded| (recorded.path.as_str(), recorded.hash.as_str())).collect();
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let lost = project.join(LOST_DIR).join(stamp.to_string());
        let mut moved = 0;
        for file in files_under(&root) {
            let Some(relative) = project_relative(&root, &file) else { continue };
            if hidden(&relative) {
                continue;
            }
            if read_file(&file).is_ok_and(|bytes| known.get(relative.as_str()) == Some(&hash(&bytes).as_str())) {
                continue;
            }
            let target = lost.join(&relative);
            let rescued = target.parent().is_some_and(|parent| std::fs::create_dir_all(parent).is_ok()) && std::fs::rename(&file, &target).is_ok();
            if rescued {
                moved += 1;
            } else {
                self.log(LogLevel::Warning, format!("Could not move {VIRTUAL_ROM_DIR}/{relative} to {LOST_DIR}; the folder is left in place"));
                return;
            }
        }
        if moved > 0 {
            self.log(LogLevel::Warning, format!("{moved} file(s) in the ROM's folder were never inserted; moved them to {}", lost.display()));
        }
        if root.exists()
            && let Err(error) = std::fs::remove_dir_all(&root)
        {
            self.log(LogLevel::Warning, format!("Could not remove {}: {error}", root.display()));
        }
        let _ = std::fs::remove_file(project.join(RECORD_FILE));
    }

    /// Picks the mirror up again when the project opens, as it was left; a failure costs the folder, not the project.
    pub(super) fn resume_virtual_rom(&self, open: &mut OpenProject) {
        if !open.settings.virtual_rom || !self.local_files {
            return;
        }
        if let Err(error) = self.start_virtual_rom(open) {
            self.log(LogLevel::Warning, format!("Could not expose the ROM as a folder again: {}", error.message));
        }
    }

    /// Starts the mirror over for the active module and variant, after either changed or the clean ROM did.
    pub(super) fn restart_virtual_rom(&self, open: &mut OpenProject) {
        let Some(mut mirror) = open.virtual_rom.take() else { return };
        // What the mount holds that was never inserted goes into the module it was meant for, before it unmounts.
        if let Some((edits, _)) = mirror.mount.as_ref().map(mount::Mounted::unsaved)
            && !edits.is_empty()
        {
            self.import(open, &mut mirror, edits);
        }
        drop(mirror);
        if let Err(error) = self.start_virtual_rom(open) {
            self.log(LogLevel::Warning, format!("Could not refresh the ROM's folder: {}", error.message));
        }
    }

    fn start_virtual_rom(&self, open: &mut OpenProject) -> Result<()> {
        if open.backing.is_rom() {
            return Err(invalid(rom::ROM_MODE_REFUSAL));
        }
        self.ensure_patcher(open);
        let module = active_module(open)?;
        let variant = open.settings.active_variant.clone();
        let record = read_record(&open.directory);
        // A record of another module or variant describes another view; its files are simply brought up to date.
        let written = record.filter(|record| record.module == module.key && record.variant == variant).map(|record| record.files).unwrap_or_default();
        let mut mirror = VirtualRom {
            root: open.directory.join(VIRTUAL_ROM_DIR),
            module,
            variant,
            view: EffectiveFilesystem::new(),
            plan: None,
            written,
            mount: None,
        };
        self.plan_view(open, &mut mirror);
        // A file whose bytes are not the ones recorded was edited while nobody was watching, and goes in first.
        let offline: Vec<(String, Vec<u8>)> = mirror
            .written
            .iter()
            .filter_map(|(address, recorded)| {
                let bytes = read_file(&mirror.root.join(&recorded.path)).ok()?;
                (hash(&bytes) != recorded.hash).then(|| (address.clone(), bytes))
            })
            .collect();
        if !offline.is_empty() {
            self.import(open, &mut mirror, offline);
            self.plan_view(open, &mut mirror);
        }
        if self.options.mount_virtual_rom && cfg!(target_os = "linux") {
            self.mount_virtual_rom(open, &mut mirror);
        }
        mirror.materialize(None)?;
        mirror.save_record(&open.directory);
        open.virtual_rom = Some(mirror);
        Ok(())
    }

    /// Mounts the folder for `mirror`, once a folder of plain files left from before is cleared away; where it cannot
    /// be mounted, the folder is written out as plain files instead.
    fn mount_virtual_rom(&self, open: &OpenProject, mirror: &mut VirtualRom) {
        if mirror.root.exists() {
            self.remove_virtual_rom(&open.directory, &mirror.written);
        }
        match mount::Mounted::start(&mirror.root, self.this.clone()) {
            Ok(mounted) => {
                mirror.written.clear();
                mirror.mount = Some(mounted);
            }
            Err(error) => {
                self.log(LogLevel::Warning, format!("Could not mount the ROM's folder ({error}); writing its files out instead"));
            }
        }
    }

    /// Asks NCPatcher for the ROM from the mirror's module down, and reads through that.
    fn plan_view(&self, open: &mut OpenProject, mirror: &mut VirtualRom) {
        // An insertion forgets what NCPatcher said; it is asked again.
        self.ensure_patcher(open);
        let scope = PlanScope::FromModule(mirror.module.key.clone());
        // Without a plan for the whole project NCPatcher is not there to ask, which the dock already says.
        let has_plan = open.patcher.as_ref().is_some_and(|(_, plan)| plan.is_some());
        mirror.plan = match has_plan.then(|| self.ncpatcher(open).plan_files(&open.directory, &mirror.variant, Some(&scope))) {
            Some(Ok(plan)) => Some(plan),
            Some(Err(error)) => {
                self.log(LogLevel::Warning, format!("The ROM's folder reads without NCPatcher's plan: {}", error.message));
                None
            }
            None => None,
        };
        mirror.view = EffectiveFilesystem::new();
        mirror.view.set_rom(layers::clean_rom(open));
        mirror.view.set_plan(&open.directory, mirror.plan.clone());
    }

    /// Inserts files edited in the folder into the active module, or drops the module's copy where an edit put a file
    /// back to what lies below, and says so. Returns whether any module file changed.
    fn import(&self, open: &mut OpenProject, mirror: &mut VirtualRom, edits: Vec<(String, Vec<u8>)>) -> bool {
        let mut changed = false;
        for (address, bytes) in edits {
            changed |= self.import_logged(open, mirror, &address, &bytes).unwrap_or(false);
        }
        changed
    }

    /// [`Self::import_one`], said in the log. Returns whether any module file changed.
    fn import_logged(&self, open: &mut OpenProject, mirror: &mut VirtualRom, address: &str, bytes: &[u8]) -> Result<bool> {
        let changed = match self.import_one(open, mirror, address, bytes) {
            Ok(Imported::Inserted) => {
                self.log(LogLevel::Info, format!("Inserted {address} into module {}", mirror.module.key));
                true
            }
            Ok(Imported::Dropped) => {
                self.log(LogLevel::Info, format!("{address} matches the layers below; moved module {}'s copy to {TRASH_DIR}", mirror.module.key));
                true
            }
            Ok(Imported::Unchanged) => false,
            Err(error) => {
                self.log(LogLevel::Warning, format!("Could not insert {address} into module {}: {}", mirror.module.key, error.message));
                return Err(error);
            }
        };
        if changed {
            modules_changed(open);
        }
        Ok(changed)
    }

    fn import_one(&self, open: &mut OpenProject, mirror: &mut VirtualRom, address: &str, bytes: &[u8]) -> Result<Imported> {
        let relative = archive_folder_path(address)?;
        let target = module_file_target(&mirror.module, &mirror.variant, address, &[])?;
        let outcome = if self.matches_below(open, &mirror.module.key, address, bytes) {
            match layers::drop_override(open, &mirror.module, address)? {
                Some(_) => Imported::Dropped,
                None => Imported::Unchanged,
            }
        } else if read_file(&target).ok().as_deref() == Some(bytes) {
            Imported::Unchanged
        } else {
            write_file_atomically(&target, bytes)?;
            Imported::Inserted
        };
        open.own_writes.insert(target);
        mirror.written.insert(address.to_owned(), Recorded { path: relative, hash: hash(bytes) });
        Ok(outcome)
    }

    /// A file deleted from the folder: the active module's copy of it goes to the trash, so it reads as the layers
    /// below give it, or is gone. Returns whether the module had one; without, there was nothing to delete.
    fn delete_one(&self, open: &mut OpenProject, mirror: &VirtualRom, address: &str) -> bool {
        match layers::drop_override(open, &mirror.module, address) {
            Ok(Some(target)) => {
                open.own_writes.insert(target);
                modules_changed(open);
                self.log(LogLevel::Info, format!("Moved {address} from module {} to {TRASH_DIR}", mirror.module.key));
                true
            }
            Ok(None) => {
                self.log(
                    LogLevel::Warning,
                    format!("{address} is not in module {}, so there is nothing to delete; the ROM's files stay", mirror.module.key),
                );
                false
            }
            Err(error) => {
                self.log(LogLevel::Warning, format!("Could not delete {address} from module {}: {}", mirror.module.key, error.message));
                false
            }
        }
    }

    /// The watcher's report of the folder: an edit is inserted, a deleted file is taken out of the module or comes
    /// back, and the host's own writes are told apart by their hash. Returns whether any module file changed.
    ///
    /// A mounted folder reports its own edits as they are made; the watcher only hears them again, and reading the
    /// folder here, with the state locked, would wait on the mount's threads waiting on that lock.
    pub(super) fn virtual_rom_changed(&self, open: &mut OpenProject, paths: &[PathBuf]) -> bool {
        if open.virtual_rom.as_ref().is_some_and(|mirror| mirror.mount.is_some()) {
            return false;
        }
        let Some(mut mirror) = open.virtual_rom.take() else { return false };
        let mut edits = Vec::new();
        let mut deleted = Vec::new();
        let mut seen = BTreeSet::new();
        for path in paths {
            let Some(relative) = project_relative(&open.directory, path) else { continue };
            let Some(relative) = relative.strip_prefix(&format!("{VIRTUAL_ROM_DIR}/")).map(str::to_owned) else { continue };
            if hidden(&relative) || !seen.insert(relative.clone()) || path.is_dir() {
                continue;
            }
            let address = mirror.address_of(&relative);
            match (address, read_file(&mirror.root.join(&relative)).ok()) {
                (Some(address), None) => deleted.push(address),
                (Some(address), Some(bytes)) => {
                    if mirror.written.get(&address).is_none_or(|recorded| recorded.hash != hash(&bytes)) {
                        edits.push((address, bytes));
                    }
                }
                (None, Some(bytes)) if new_file_address(&relative).is_some() => edits.push((relative, bytes)),
                (None, Some(_)) => self.log(
                    LogLevel::Warning,
                    format!("{VIRTUAL_ROM_DIR}/{relative} is not a file the ROM has; only new files under z_new/ are inserted"),
                ),
                (None, None) => {}
            }
        }
        let mut changed = !edits.is_empty() && self.import(open, &mut mirror, edits);
        // Each deleted file is written again as it now reads, or forgotten when nothing has it any more.
        let mut restore = BTreeSet::new();
        for address in deleted {
            changed |= self.delete_one(open, &mirror, &address);
            restore.insert(address);
        }
        if changed {
            self.plan_view(open, &mut mirror);
        }
        if let Err(error) = mirror.materialize(Some(&restore)) {
            self.log(LogLevel::Warning, format!("Could not restore the ROM's folder: {}", error.message));
        }
        mirror.save_record(&open.directory);
        open.virtual_rom = Some(mirror);
        changed
    }

    /// A file the mount `id` took: closed after being written to, or renamed over a ROM file. It is inserted, and the
    /// mount then holds what it reads as.
    pub(super) fn mount_wrote(&self, id: u64, address: String, bytes: Vec<u8>) -> Result<()> {
        self.with_mount(id, &address, |host, open, mirror| {
            let changed = host.import_logged(open, mirror, &address, &bytes)?;
            if changed {
                host.plan_view(open, mirror);
            }
            Ok((changed, ()))
        })
        .map(|((), _)| ())
    }

    /// A ROM file deleted from the mount `id`. Returns whether it was the active module's to delete.
    pub(super) fn mount_deleted(&self, id: u64, address: &str) -> Result<bool> {
        self.with_mount(id, address, |host, open, mirror| {
            let deleted = host.delete_one(open, mirror, address);
            if deleted {
                host.plan_view(open, mirror);
            }
            Ok((deleted, deleted))
        })
        .map(|(deleted, _)| deleted)
    }

    /// Runs `apply` on the mirror the mount `id` belongs to for `address`. When it says the modules changed, the
    /// mount is brought up to date -- `address`, and every address whose source the plan now names differently --
    /// and that is announced. The state lock is waited for only so long: the host may be holding it while something
    /// reads the mount.
    fn with_mount<T>(
        &self,
        id: u64,
        address: &str,
        apply: impl FnOnce(&Self, &mut OpenProject, &mut VirtualRom) -> Result<(bool, T)>,
    ) -> Result<(T, bool)> {
        let busy = || Error::new(ErrorCode::IoError, "the editor is busy; try again");
        let gone = || invalid("the ROM's folder is no longer exposed");
        let mut state = lock_within(&self.state, MOUNT_PATIENCE).ok_or_else(busy)?;
        let open = state.open.as_mut().ok_or_else(gone)?;
        let mut mirror = open.virtual_rom.take().ok_or_else(gone)?;
        if mirror.mount.as_ref().is_none_or(|mount| mount.id() != id) {
            open.virtual_rom = Some(mirror);
            return Err(gone());
        }
        let before = sources(mirror.plan.as_ref());
        let outcome = apply(self, open, &mut mirror);
        if let Ok((true, _)) = &outcome {
            let after = sources(mirror.plan.as_ref());
            let mut addresses = BTreeSet::from([address.to_owned()]);
            addresses.extend(before.keys().chain(after.keys()).filter(|address| before.get(*address) != after.get(*address)).cloned());
            if let Err(error) = mirror.materialize(Some(&addresses)) {
                self.log(LogLevel::Warning, format!("Could not refresh the ROM's folder: {}", error.message));
            }
        }
        open.virtual_rom = Some(mirror);
        drop(state);
        let (changed, value) = outcome?;
        if changed {
            let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
            self.emit(Event::GraphicsChanged { generation });
        }
        Ok((value, changed))
    }

    /// Brings the folder up to date with the modules: the addresses given, every address whose source the new plan
    /// names differently, and every address whose source is one of the files that changed.
    pub(super) fn refresh_virtual_rom(&self, open: &mut OpenProject, touched: BTreeSet<String>, changed_files: &[PathBuf]) {
        let Some(mut mirror) = open.virtual_rom.take() else { return };
        let before = sources(mirror.plan.as_ref());
        self.plan_view(open, &mut mirror);
        let after = sources(mirror.plan.as_ref());
        let changed: BTreeSet<String> = changed_files.iter().filter_map(|path| project_relative(&open.directory, path)).collect();
        let mut addresses = touched;
        addresses.extend(before.keys().chain(after.keys()).filter(|address| before.get(*address) != after.get(*address)).cloned());
        let relative_source = |source: &str| project_relative(&open.directory, &open.directory.join(source));
        addresses.extend(
            after
                .iter()
                .filter(|(_, (source, _))| !source.is_empty() && relative_source(source).is_some_and(|s| changed.contains(&s)))
                .map(|(a, _)| a.clone()),
        );
        if mirror.plan.is_none() {
            let by_target = |address: &String| {
                module_file_target(&mirror.module, &mirror.variant, address, &[]).ok().and_then(|target| project_relative(&open.directory, &target))
            };
            addresses.extend(mirror.written.keys().filter(|address| by_target(address).is_some_and(|target| changed.contains(&target))).cloned());
        }
        if let Err(error) = mirror.materialize(Some(&addresses)) {
            self.log(LogLevel::Warning, format!("Could not refresh the ROM's folder: {}", error.message));
        }
        mirror.save_record(&open.directory);
        open.virtual_rom = Some(mirror);
    }
}

pub(super) fn listing_path(open: &OpenProject) -> Option<String> {
    open.virtual_rom.as_ref().map(|mirror| mirror.root.display().to_string())
}
