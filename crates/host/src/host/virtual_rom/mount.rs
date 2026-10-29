//! The ROM's folder as a filesystem of its own, mounted through FUSE on Linux. Nothing is written to the disk: the
//! files live in memory, as the mirror puts them there, and what a tool does to them reaches the host as it happens.
//!
//! A file written to and closed -- its last descriptor -- is inserted into the active module, and a ROM file renamed
//! over -- how most editors save -- is too. Deleting a file takes the module's copy away, which leaves the file as the
//! layers below give it, or gone when nothing below has it; a file of the ROM's own cannot be deleted or moved. Files
//! and folders made anywhere else are kept in memory for as long as the folder is mounted, so that a tool's temporary
//! files work; new files under `z_new/` are inserted like any other edit, and moving one (`notes.txt.part` renamed to
//! `notes.txt` once it is complete) moves the module's copy with it.
//!
//! The host calls in with its state locked and never touches the mount while it does, and the mount's threads lock
//! the tree only for as long as it takes to read or change it: reads never wait on the host. A write waits for the
//! host's lock only so long, and fails with `EIO` after, so neither side can hang on the other.

use std::collections::{BTreeMap, HashMap};
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, SystemTime};

use fuser::{
    AccessFlags, BackgroundSession, Config, Errno, FileAttr, FileHandle, FileType, Filesystem, FopenFlags, Generation, INodeNo, LockOwner,
    MountOption, OpenFlags, RenameFlags, ReplyAttr, ReplyCreate, ReplyData, ReplyDirectory, ReplyEmpty, ReplyEntry, ReplyOpen, ReplyStatfs,
    ReplyWrite, ReplyXattr, Request, TimeOrNow, WriteFlags,
};

use super::{Files, Shared, hidden, lock, new_file_address};

/// How long the kernel may keep what it was told about a name or a file. The host changes files under the kernel's
/// feet, so this is short.
const TTL: Duration = Duration::from_secs(1);
const ROOT: u64 = 1;

/// Tells mounts apart, so a mount the host has replaced does not write through the new one.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
/// Where this process has a folder mounted, for [`unmount_all`].
static MOUNTED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

enum Kind {
    /// `made` when somebody made it in the mount, rather than the mirror for the files it holds; only those stay once
    /// they are empty.
    Dir { children: BTreeMap<String, u64>, made: bool },
    /// `address` is the ROM file it holds; a file without one was made in the mount. `dirty` while it holds bytes the
    /// host has not been given.
    File { bytes: Vec<u8>, address: Option<String>, dirty: bool },
}

struct Node {
    parent: u64,
    name: String,
    kind: Kind,
    mtime: SystemTime,
}

struct Tree {
    nodes: HashMap<u64, Node>,
    next: u64,
}

impl Tree {
    fn new() -> Self {
        let root = Node { parent: ROOT, name: String::new(), kind: Kind::Dir { children: BTreeMap::new(), made: true }, mtime: SystemTime::now() };
        Self { nodes: HashMap::from([(ROOT, root)]), next: ROOT + 1 }
    }

    /// Where `ino` is under the folder, `/`-separated.
    fn path(&self, mut ino: u64) -> String {
        let mut parts = Vec::new();
        while ino != ROOT {
            let Some(node) = self.nodes.get(&ino) else { break };
            parts.push(node.name.as_str());
            ino = node.parent;
        }
        parts.reverse();
        parts.join("/")
    }

    fn child(&self, parent: u64, name: &str) -> Option<u64> {
        match &self.nodes.get(&parent)?.kind {
            Kind::Dir { children, .. } => children.get(name).copied(),
            Kind::File { .. } => None,
        }
    }

    fn find(&self, relative: &str) -> Option<u64> {
        relative.split('/').filter(|part| !part.is_empty()).try_fold(ROOT, |ino, part| self.child(ino, part))
    }

    fn insert(&mut self, parent: u64, name: &str, kind: Kind) -> Option<u64> {
        let ino = self.next;
        let Some(Node { kind: Kind::Dir { children, .. }, mtime, .. }) = self.nodes.get_mut(&parent) else { return None };
        children.insert(name.to_owned(), ino);
        *mtime = SystemTime::now();
        self.next += 1;
        self.nodes.insert(ino, Node { parent, name: name.to_owned(), kind, mtime: SystemTime::now() });
        Some(ino)
    }

    /// Takes `ino` and everything under it out of the tree.
    fn detach(&mut self, ino: u64) {
        let Some(node) = self.nodes.remove(&ino) else { return };
        if let Some(Node { kind: Kind::Dir { children, .. }, mtime, .. }) = self.nodes.get_mut(&node.parent) {
            children.remove(&node.name);
            *mtime = SystemTime::now();
        }
        let mut pending: Vec<u64> = match node.kind {
            Kind::Dir { children, .. } => children.into_values().collect(),
            Kind::File { .. } => Vec::new(),
        };
        while let Some(ino) = pending.pop() {
            if let Some(Node { kind: Kind::Dir { children, .. }, .. }) = self.nodes.remove(&ino) {
                pending.extend(children.into_values());
            }
        }
    }

    /// Drops the folders above `ino` that the mirror made and nothing is left in.
    fn prune(&mut self, mut ino: u64) {
        while ino != ROOT {
            let Some(Node { parent, kind: Kind::Dir { children, made: false }, .. }) = self.nodes.get(&ino) else { return };
            if !children.is_empty() {
                return;
            }
            let parent = *parent;
            self.detach(ino);
            ino = parent;
        }
    }

    /// The files at or under `ino`.
    fn files_in(&self, ino: u64) -> Vec<u64> {
        match self.nodes.get(&ino).map(|node| &node.kind) {
            Some(Kind::File { .. }) => vec![ino],
            Some(Kind::Dir { children, .. }) => children.values().flat_map(|child| self.files_in(*child)).collect(),
            None => Vec::new(),
        }
    }

    /// Whether `ino` holds a file of the ROM's own, or is one: a ROM file that is not a new one under `z_new/`.
    fn holds_rom_files(&self, ino: u64) -> bool {
        self.files_in(ino).into_iter().any(|file| {
            matches!(self.nodes.get(&file).map(|node| &node.kind), Some(Kind::File { address: Some(address), .. }) if !address.starts_with("z_new/"))
        })
    }

    fn put(&mut self, relative: &str, address: &str, bytes: &[u8]) {
        let mut parent = ROOT;
        let mut parts: Vec<&str> = relative.split('/').filter(|part| !part.is_empty()).collect();
        let Some(name) = parts.pop() else { return };
        for part in parts {
            parent = match self.child(parent, part) {
                Some(ino) if matches!(self.nodes.get(&ino).map(|node| &node.kind), Some(Kind::Dir { .. })) => ino,
                // A file somebody made where the ROM has a folder gives way to the folder.
                Some(ino) => {
                    self.detach(ino);
                    let dir = Kind::Dir { children: BTreeMap::new(), made: false };
                    let Some(ino) = self.insert(parent, part, dir) else { return };
                    ino
                }
                None => {
                    let Some(ino) = self.insert(parent, part, Kind::Dir { children: BTreeMap::new(), made: false }) else { return };
                    ino
                }
            };
        }
        if let Some(ino) = self.child(parent, name)
            && let Some(node) = self.nodes.get_mut(&ino)
        {
            match &mut node.kind {
                // A file being written to keeps what was written until it is closed.
                Kind::File { dirty: true, address: own, .. } => *own = Some(address.to_owned()),
                Kind::File { bytes: own, address: own_address, .. } => {
                    *own_address = Some(address.to_owned());
                    if own.as_slice() != bytes {
                        *own = bytes.to_vec();
                        node.mtime = SystemTime::now();
                    }
                }
                Kind::Dir { .. } => {
                    self.detach(ino);
                    self.insert(parent, name, Kind::File { bytes: bytes.to_vec(), address: Some(address.to_owned()), dirty: false });
                }
            }
            return;
        }
        self.insert(parent, name, Kind::File { bytes: bytes.to_vec(), address: Some(address.to_owned()), dirty: false });
    }

    fn remove(&mut self, relative: &str) {
        let Some(ino) = self.find(relative) else { return };
        let Some(node) = self.nodes.get(&ino) else { return };
        let parent = node.parent;
        match node.kind {
            Kind::File { .. } => {
                self.detach(ino);
                self.prune(parent);
            }
            // What was a folder of somebody's is left to them.
            Kind::Dir { .. } => {}
        }
    }

    fn files(&self) -> impl Iterator<Item = (u64, &Node)> {
        self.nodes.iter().filter(|(_, node)| matches!(node.kind, Kind::File { .. })).map(|(ino, node)| (*ino, node))
    }
}

/// The ROM's folder while it is mounted. Dropping it unmounts it.
pub(in crate::host) struct Mounted {
    id: u64,
    root: PathBuf,
    tree: Arc<Mutex<Tree>>,
    /// Taken when it is dropped, which unmounts it.
    session: Option<BackgroundSession>,
}

/// The folder goes with the mount, so that nothing of the ROM is left in the project; it is made again when the
/// folder is next mounted.
impl Drop for Mounted {
    fn drop(&mut self) {
        lock(&MOUNTED).retain(|root| *root != self.root);
        drop(self.session.take());
        let _ = std::fs::remove_dir(&self.root);
    }
}

/// Unmounts every folder this process mounted. A process that exits with one mounted leaves it dead, unreadable until
/// it is unmounted, and an app that does not wind its host down on the way out calls this instead.
pub fn unmount_all() {
    for root in std::mem::take(&mut *lock(&MOUNTED)) {
        unmount(&root);
        let _ = std::fs::remove_dir(&root);
    }
}

impl Mounted {
    /// Mounts an empty tree over `root`, which must be an empty folder, or one a mount that died left behind.
    pub(in crate::host) fn start(root: &Path, host: Weak<Shared>) -> std::io::Result<Self> {
        clear_dead_mount(root);
        std::fs::create_dir_all(root)?;
        let owner = std::fs::metadata(root)?;
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let tree = Arc::new(Mutex::new(Tree::new()));
        let filesystem = RomFs { id, tree: Arc::clone(&tree), host_calls: Mutex::new(()), host, uid: owner.uid(), gid: owner.gid() };
        let mut config = Config::default();
        config.mount_options = vec![MountOption::FSName("bigstar".into()), MountOption::Subtype("virtual-rom".into()), MountOption::NoAtime];
        // More than one, so that reading does not wait behind a write waiting for the host.
        config.n_threads = Some(4);
        let session = fuser::spawn_mount(filesystem, root, &config)?;
        lock(&MOUNTED).push(root.to_path_buf());
        Ok(Self { id, root: root.to_path_buf(), tree, session: Some(session) })
    }

    pub(in crate::host) fn id(&self) -> u64 {
        self.id
    }

    pub(in crate::host) fn put(&self, relative: &str, address: &str, bytes: &[u8]) {
        lock(&self.tree).put(relative, address, bytes);
    }

    pub(in crate::host) fn remove(&self, relative: &str) {
        lock(&self.tree).remove(relative);
    }

    /// What the mount holds that the host has not taken: edits still being written, by ROM address, and the files
    /// made in it that are no file of the ROM's, by where they are. Dot files are neither.
    pub(in crate::host) fn unsaved(&self) -> (Files, Files) {
        let tree = lock(&self.tree);
        let (mut edits, mut strays) = (Vec::new(), Vec::new());
        for (ino, node) in tree.files() {
            let Kind::File { bytes, address, dirty } = &node.kind else { continue };
            let relative = tree.path(ino);
            match (address, new_file_address(&relative)) {
                (Some(address), _) if *dirty => edits.push((address.clone(), bytes.clone())),
                (Some(_), _) => {}
                // A new file the host never took: one inserted holds its address.
                (None, Some(address)) => edits.push((address, bytes.clone())),
                (None, None) if !hidden(&relative) => strays.push((relative, bytes.clone())),
                (None, None) => {}
            }
        }
        (edits, strays)
    }
}

/// A mount whose editor died leaves its folder unreadable ("Transport endpoint is not connected") until it is
/// unmounted.
fn clear_dead_mount(root: &Path) {
    const ENOTCONN: i32 = 107;
    if std::fs::metadata(root).err().and_then(|error| error.raw_os_error()) == Some(ENOTCONN) {
        unmount(root);
    }
}

/// Detaches the mount at `root` at once, even while something has a file of it open.
fn unmount(root: &Path) {
    for program in ["fusermount3", "fusermount"] {
        let unmounted = std::process::Command::new(program).arg("-u").arg("-z").arg(root).status();
        if unmounted.is_ok_and(|status| status.success()) {
            return;
        }
    }
}

struct RomFs {
    id: u64,
    tree: Arc<Mutex<Tree>>,
    /// Held across whatever hands the host a file or takes one from it, so that each sees the last one done: a file
    /// deleted while its save is on the way to the host is deleted once it is there, not before, to come back after.
    host_calls: Mutex<()>,
    host: Weak<Shared>,
    uid: u32,
    gid: u32,
}

impl RomFs {
    fn attr(&self, ino: u64, node: &Node) -> FileAttr {
        let (kind, size, perm, nlink) = match &node.kind {
            Kind::Dir { .. } => (FileType::Directory, 0, 0o755, 2),
            Kind::File { bytes, .. } => (FileType::RegularFile, bytes.len() as u64, 0o644, 1),
        };
        FileAttr {
            ino: INodeNo(ino),
            size,
            blocks: size.div_ceil(512),
            atime: node.mtime,
            mtime: node.mtime,
            ctime: node.mtime,
            crtime: node.mtime,
            kind,
            perm,
            nlink,
            uid: self.uid,
            gid: self.gid,
            rdev: 0,
            blksize: 4096,
            flags: 0,
        }
    }

    fn attr_of(&self, ino: u64) -> Option<FileAttr> {
        let tree = lock(&self.tree);
        tree.nodes.get(&ino).map(|node| self.attr(ino, node))
    }

    fn host(&self) -> Result<Arc<Shared>, Errno> {
        self.host.upgrade().ok_or(Errno::EIO)
    }

    /// Hands the host what `ino` holds when it is dirty and is a ROM file or a new one. A file of no address stays
    /// here, as a tool's own.
    fn save(&self, ino: u64) -> Result<(), Errno> {
        let _calls = lock(&self.host_calls);
        self.save_holding(ino)
    }

    /// [`Self::save`], with `host_calls` already held.
    fn save_holding(&self, ino: u64) -> Result<(), Errno> {
        let (address, bytes) = {
            let mut tree = lock(&self.tree);
            let relative = tree.path(ino);
            let Some(Node { kind: Kind::File { bytes, address, dirty }, .. }) = tree.nodes.get_mut(&ino) else { return Ok(()) };
            if !*dirty {
                return Ok(());
            }
            let Some(address) = address.clone().or_else(|| new_file_address(&relative)) else { return Ok(()) };
            *dirty = false;
            (address, bytes.clone())
        };
        let saved = self.host().and_then(|host| host.mount_wrote(self.id, address, bytes).map_err(|_| Errno::EIO));
        if saved.is_err()
            && let Some(Node { kind: Kind::File { dirty, .. }, .. }) = lock(&self.tree).nodes.get_mut(&ino)
        {
            *dirty = true;
        }
        saved
    }

    fn name(name: &OsStr) -> Result<&str, Errno> {
        name.to_str().filter(|name| !name.is_empty() && !name.contains('/')).ok_or(Errno::EINVAL)
    }

    fn reply_entry(&self, ino: Option<u64>, reply: ReplyEntry) {
        match ino.and_then(|ino| self.attr_of(ino)) {
            Some(attr) => reply.entry(&TTL, &attr, Generation(0)),
            None => reply.error(Errno::ENOENT),
        }
    }
}

impl Filesystem for RomFs {
    fn init(&mut self, _req: &Request, config: &mut fuser::KernelConfig) -> std::io::Result<()> {
        // `O_TRUNC` then comes with the open, rather than as a truncation by name that would be inserted on its own: an
        // empty file, a moment before what is written to it.
        let _ = config.add_capabilities(fuser::InitFlags::FUSE_ATOMIC_O_TRUNC);
        Ok(())
    }

    fn lookup(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEntry) {
        let ino = name.to_str().and_then(|name| lock(&self.tree).child(parent.0, name));
        self.reply_entry(ino, reply);
    }

    fn getattr(&self, _req: &Request, ino: INodeNo, _fh: Option<FileHandle>, reply: ReplyAttr) {
        match self.attr_of(ino.0) {
            Some(attr) => reply.attr(&TTL, &attr),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn setattr(
        &self,
        _req: &Request,
        ino: INodeNo,
        _mode: Option<u32>,
        _uid: Option<u32>,
        _gid: Option<u32>,
        size: Option<u64>,
        _atime: Option<TimeOrNow>,
        mtime: Option<TimeOrNow>,
        _ctime: Option<SystemTime>,
        fh: Option<FileHandle>,
        _crtime: Option<SystemTime>,
        _chgtime: Option<SystemTime>,
        _bkuptime: Option<SystemTime>,
        _flags: Option<fuser::BsdFileFlags>,
        reply: ReplyAttr,
    ) {
        // Modes and owners are the mount's own; a tool that copies them over is not told no.
        let truncated = {
            let mut tree = lock(&self.tree);
            let Some(node) = tree.nodes.get_mut(&ino.0) else { return reply.error(Errno::ENOENT) };
            if let Some(TimeOrNow::SpecificTime(time)) = mtime {
                node.mtime = time;
            }
            match (&mut node.kind, size) {
                (Kind::File { bytes, dirty, .. }, Some(size)) => {
                    bytes.resize(usize::try_from(size).unwrap_or(usize::MAX), 0);
                    *dirty = true;
                    node.mtime = SystemTime::now();
                    true
                }
                (Kind::Dir { .. }, Some(_)) => return reply.error(Errno::EISDIR),
                _ => false,
            }
        };
        // Truncated by name, there is no close to wait for.
        if truncated
            && fh.is_none()
            && let Err(error) = self.save(ino.0)
        {
            return reply.error(error);
        }
        match self.attr_of(ino.0) {
            Some(attr) => reply.attr(&TTL, &attr),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn mkdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32, reply: ReplyEntry) {
        let Ok(name) = Self::name(name) else { return reply.error(Errno::EINVAL) };
        let made = {
            let mut tree = lock(&self.tree);
            if tree.child(parent.0, name).is_some() {
                return reply.error(Errno::EEXIST);
            }
            tree.insert(parent.0, name, Kind::Dir { children: BTreeMap::new(), made: true })
        };
        match made {
            Some(ino) => self.reply_entry(Some(ino), reply),
            None => reply.error(Errno::ENOTDIR),
        }
    }

    fn unlink(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let _calls = lock(&self.host_calls);
        let address = {
            let mut tree = lock(&self.tree);
            let Some(ino) = name.to_str().and_then(|name| tree.child(parent.0, name)) else { return reply.error(Errno::ENOENT) };
            match tree.nodes.get(&ino).map(|node| &node.kind) {
                Some(Kind::Dir { .. }) => return reply.error(Errno::EISDIR),
                Some(Kind::File { address: Some(address), .. }) => address.clone(),
                _ => {
                    tree.detach(ino);
                    return reply.ok();
                }
            }
        };
        // A ROM file goes out of the module; the host then puts back what lies below, or takes the file away.
        match self.host().and_then(|host| host.mount_deleted(self.id, &address).map_err(|_| Errno::EIO)) {
            Ok(true) => reply.ok(),
            Ok(false) => reply.error(Errno::EPERM),
            Err(error) => reply.error(error),
        }
    }

    fn rmdir(&self, _req: &Request, parent: INodeNo, name: &OsStr, reply: ReplyEmpty) {
        let mut tree = lock(&self.tree);
        let Some(ino) = name.to_str().and_then(|name| tree.child(parent.0, name)) else { return reply.error(Errno::ENOENT) };
        match tree.nodes.get(&ino).map(|node| &node.kind) {
            Some(Kind::Dir { children, .. }) if children.is_empty() => {
                tree.detach(ino);
                reply.ok();
            }
            Some(Kind::Dir { .. }) => reply.error(Errno::ENOTEMPTY),
            _ => reply.error(Errno::ENOTDIR),
        }
    }

    fn rename(&self, _req: &Request, parent: INodeNo, name: &OsStr, newparent: INodeNo, newname: &OsStr, flags: RenameFlags, reply: ReplyEmpty) {
        if flags.contains(RenameFlags::RENAME_EXCHANGE) || flags.contains(RenameFlags::RENAME_WHITEOUT) {
            return reply.error(Errno::EINVAL);
        }
        let Ok(newname) = Self::name(newname) else { return reply.error(Errno::EINVAL) };
        let _calls = lock(&self.host_calls);
        let moved = {
            let mut tree = lock(&self.tree);
            let Some(source) = name.to_str().and_then(|name| tree.child(parent.0, name)) else { return reply.error(Errno::ENOENT) };
            // The ROM's own files stay where the ROM has them; new ones move, out of the module where they were and
            // into it where they go.
            if tree.holds_rom_files(source) {
                return reply.error(Errno::EPERM);
            }
            let target = tree.child(newparent.0, newname);
            if target == Some(source) {
                return reply.ok();
            }
            if target.is_some() && flags.contains(RenameFlags::RENAME_NOREPLACE) {
                return reply.error(Errno::EEXIST);
            }
            let source_is_dir = matches!(tree.nodes.get(&source).map(|node| &node.kind), Some(Kind::Dir { .. }));
            // Renamed over a ROM file -- how most editors save -- the file takes its place as that ROM file.
            let mut takes_over = None;
            match target.and_then(|target| tree.nodes.get(&target).map(|node| (target, &node.kind))) {
                Some((_, Kind::Dir { .. })) if !source_is_dir => return reply.error(Errno::EISDIR),
                Some((_, Kind::Dir { children, .. })) if !children.is_empty() => return reply.error(Errno::ENOTEMPTY),
                Some((_, Kind::File { .. })) if source_is_dir => return reply.error(Errno::ENOTDIR),
                Some((target, Kind::File { address, .. })) => {
                    takes_over = address.clone();
                    tree.detach(target);
                }
                Some((target, Kind::Dir { .. })) => tree.detach(target),
                None => {}
            }
            let moved: Vec<(u64, Option<String>)> = tree
                .files_in(source)
                .into_iter()
                .map(|file| match tree.nodes.get(&file).map(|node| &node.kind) {
                    Some(Kind::File { address, .. }) => (file, address.clone()),
                    _ => (file, None),
                })
                .collect();
            // The kernel holds the new name to the source's inode, so that is the node that moves.
            let Some(node) = tree.nodes.remove(&source) else { return reply.error(Errno::ENOENT) };
            if let Some(Node { kind: Kind::Dir { children, .. }, mtime, .. }) = tree.nodes.get_mut(&node.parent) {
                children.remove(&node.name);
                *mtime = SystemTime::now();
            }
            let Some(Node { kind: Kind::Dir { children, .. }, mtime, .. }) = tree.nodes.get_mut(&newparent.0) else {
                return reply.error(Errno::ENOTDIR);
            };
            children.insert(newname.to_owned(), source);
            *mtime = SystemTime::now();
            tree.nodes.insert(source, Node { parent: newparent.0, name: newname.to_owned(), ..node });
            // Each file is what its new place makes it: the ROM file it took the place of, a new one, or a tool's own.
            for (file, _) in &moved {
                if let Some(Node { kind: Kind::File { address, dirty, .. }, .. }) = tree.nodes.get_mut(file) {
                    *address = if *file == source { takes_over.clone() } else { None };
                    *dirty = true;
                }
            }
            moved
        };
        // The files are inserted where they now are, and the module's copies where they were go. The rename happened
        // either way: a file the host could not take stays dirty, and is tried again when the folder goes.
        for (file, _) in &moved {
            let _ = self.save_holding(*file);
        }
        let host = self.host.upgrade();
        for (_, old) in moved {
            if let (Some(host), Some(old)) = (&host, old) {
                let _ = host.mount_deleted(self.id, &old);
            }
        }
        reply.ok();
    }

    fn open(&self, _req: &Request, ino: INodeNo, flags: OpenFlags, reply: ReplyOpen) {
        let mut tree = lock(&self.tree);
        match tree.nodes.get_mut(&ino.0) {
            Some(Node { kind: Kind::File { bytes, dirty, .. }, mtime, .. }) => {
                if flags.0 & libc::O_TRUNC != 0 && !bytes.is_empty() {
                    bytes.clear();
                    *dirty = true;
                    *mtime = SystemTime::now();
                }
                reply.opened(FileHandle(0), FopenFlags::empty());
            }
            Some(Node { kind: Kind::Dir { .. }, .. }) => reply.error(Errno::EISDIR),
            None => reply.error(Errno::ENOENT),
        }
    }

    fn read(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        size: u32,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyData,
    ) {
        let tree = lock(&self.tree);
        let Some(Node { kind: Kind::File { bytes, .. }, .. }) = tree.nodes.get(&ino.0) else { return reply.error(Errno::ENOENT) };
        let start = usize::try_from(offset).unwrap_or(usize::MAX).min(bytes.len());
        let end = start.saturating_add(size as usize).min(bytes.len());
        reply.data(bytes.get(start..end).unwrap_or_default());
    }

    fn write(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        offset: u64,
        data: &[u8],
        _write_flags: WriteFlags,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        reply: ReplyWrite,
    ) {
        let mut tree = lock(&self.tree);
        let Some(node) = tree.nodes.get_mut(&ino.0) else { return reply.error(Errno::ENOENT) };
        let Kind::File { bytes, dirty, .. } = &mut node.kind else { return reply.error(Errno::EISDIR) };
        let Ok(start) = usize::try_from(offset) else { return reply.error(Errno::EINVAL) };
        let end = start.saturating_add(data.len());
        if bytes.len() < end {
            bytes.resize(end, 0);
        }
        if let Some(range) = bytes.get_mut(start..end) {
            range.copy_from_slice(data);
        }
        *dirty = true;
        node.mtime = SystemTime::now();
        reply.written(u32::try_from(data.len()).unwrap_or(u32::MAX));
    }

    /// Every `close` flushes, including one of a descriptor that was only duplicated (a shell's `>` closes the one it
    /// opened before anything is written), so the file waits for its release.
    fn flush(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _lock_owner: LockOwner, reply: ReplyEmpty) {
        reply.ok();
    }

    fn release(
        &self,
        _req: &Request,
        ino: INodeNo,
        _fh: FileHandle,
        _flags: OpenFlags,
        _lock_owner: Option<LockOwner>,
        _flush: bool,
        reply: ReplyEmpty,
    ) {
        // The last descriptor of the open file is closed. Nobody waits for this answer, so a failure is only logged by
        // the host, and the file stays dirty for the next try.
        let _ = self.save(ino.0);
        reply.ok();
    }

    fn fsync(&self, _req: &Request, ino: INodeNo, _fh: FileHandle, _datasync: bool, reply: ReplyEmpty) {
        match self.save(ino.0) {
            Ok(()) => reply.ok(),
            Err(error) => reply.error(error),
        }
    }

    fn readdir(&self, _req: &Request, ino: INodeNo, _fh: FileHandle, offset: u64, mut reply: ReplyDirectory) {
        let tree = lock(&self.tree);
        let Some(node) = tree.nodes.get(&ino.0) else { return reply.error(Errno::ENOENT) };
        let Kind::Dir { children, .. } = &node.kind else { return reply.error(Errno::ENOTDIR) };
        let mut entries = vec![(ino.0, FileType::Directory, "."), (node.parent, FileType::Directory, "..")];
        for (name, child) in children {
            let kind = match tree.nodes.get(child).map(|node| &node.kind) {
                Some(Kind::Dir { .. }) => FileType::Directory,
                _ => FileType::RegularFile,
            };
            entries.push((*child, kind, name.as_str()));
        }
        for (index, (ino, kind, name)) in entries.into_iter().enumerate().skip(usize::try_from(offset).unwrap_or(usize::MAX)) {
            if reply.add(INodeNo(ino), index as u64 + 1, kind, name) {
                break;
            }
        }
        reply.ok();
    }

    fn statfs(&self, _req: &Request, _ino: INodeNo, reply: ReplyStatfs) {
        let tree = lock(&self.tree);
        let size = |node: &Node| match &node.kind {
            Kind::File { bytes, .. } => (bytes.len() as u64).div_ceil(4096),
            Kind::Dir { .. } => 0,
        };
        let blocks: u64 = tree.nodes.values().map(size).sum();
        // Room to spare, so a tool that checks before copying in does not stop.
        let free = 1 << 20;
        reply.statfs(blocks + free, free, free, tree.nodes.len() as u64 + free, free, 4096, 255, 4096);
    }

    fn getxattr(&self, _req: &Request, _ino: INodeNo, _name: &OsStr, _size: u32, reply: ReplyXattr) {
        reply.error(Errno::NO_XATTR);
    }

    fn listxattr(&self, _req: &Request, _ino: INodeNo, size: u32, reply: ReplyXattr) {
        if size == 0 { reply.size(0) } else { reply.data(&[]) }
    }

    // What the mount does not do, answered as a filesystem that does not do it answers, so that a tool falls back to
    // what it does without fuser logging a warning each time.

    /// File managers ask for the filesystem's label, among others; there is none to give.
    fn ioctl(
        &self,
        _req: &Request,
        _ino: INodeNo,
        _fh: FileHandle,
        _flags: fuser::IoctlFlags,
        _cmd: u32,
        _in_data: &[u8],
        _out_size: u32,
        reply: fuser::ReplyIoctl,
    ) {
        reply.error(Errno::ENOTTY);
    }

    fn setxattr(&self, _req: &Request, _ino: INodeNo, _name: &OsStr, _value: &[u8], _flags: i32, _position: u32, reply: ReplyEmpty) {
        reply.error(Errno::ENOTSUP);
    }

    fn removexattr(&self, _req: &Request, _ino: INodeNo, _name: &OsStr, reply: ReplyEmpty) {
        reply.error(Errno::NO_XATTR);
    }

    fn symlink(&self, _req: &Request, _parent: INodeNo, _link_name: &OsStr, _target: &Path, reply: ReplyEntry) {
        reply.error(Errno::EPERM);
    }

    fn link(&self, _req: &Request, _ino: INodeNo, _newparent: INodeNo, _newname: &OsStr, reply: ReplyEntry) {
        reply.error(Errno::EPERM);
    }

    fn readlink(&self, _req: &Request, _ino: INodeNo, reply: ReplyData) {
        reply.error(Errno::EINVAL);
    }

    /// Only regular files, and those come through `create`.
    fn mknod(&self, _req: &Request, _parent: INodeNo, _name: &OsStr, _mode: u32, _umask: u32, _rdev: u32, reply: ReplyEntry) {
        reply.error(Errno::EPERM);
    }

    fn fsyncdir(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _datasync: bool, reply: ReplyEmpty) {
        reply.ok();
    }

    fn fallocate(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _offset: u64, _length: u64, _mode: i32, reply: ReplyEmpty) {
        reply.error(Errno::EOPNOTSUPP);
    }

    /// The kernel then seeks by itself, and stops asking.
    fn lseek(&self, _req: &Request, _ino: INodeNo, _fh: FileHandle, _offset: i64, _whence: i32, reply: fuser::ReplyLseek) {
        reply.error(Errno::ENOSYS);
    }

    /// The kernel then copies by reading and writing, and stops asking.
    fn copy_file_range(
        &self,
        _req: &Request,
        _ino_in: INodeNo,
        _fh_in: FileHandle,
        _offset_in: u64,
        _ino_out: INodeNo,
        _fh_out: FileHandle,
        _offset_out: u64,
        _len: u64,
        _flags: fuser::CopyFileRangeFlags,
        reply: ReplyWrite,
    ) {
        reply.error(Errno::ENOSYS);
    }

    fn access(&self, _req: &Request, ino: INodeNo, _mask: AccessFlags, reply: ReplyEmpty) {
        match lock(&self.tree).nodes.contains_key(&ino.0) {
            true => reply.ok(),
            false => reply.error(Errno::ENOENT),
        }
    }

    fn create(&self, _req: &Request, parent: INodeNo, name: &OsStr, _mode: u32, _umask: u32, _flags: i32, reply: ReplyCreate) {
        let Ok(name) = Self::name(name) else { return reply.error(Errno::EINVAL) };
        let attr = {
            let mut tree = lock(&self.tree);
            if tree.child(parent.0, name).is_some() {
                return reply.error(Errno::EEXIST);
            }
            // Dirty from the start, so that an empty new file is inserted too.
            let Some(ino) = tree.insert(parent.0, name, Kind::File { bytes: Vec::new(), address: None, dirty: true }) else {
                return reply.error(Errno::ENOTDIR);
            };
            tree.nodes.get(&ino).map(|node| self.attr(ino, node))
        };
        match attr {
            Some(attr) => reply.created(&TTL, &attr, Generation(0), FileHandle(0), FopenFlags::empty()),
            None => reply.error(Errno::EIO),
        }
    }
}
