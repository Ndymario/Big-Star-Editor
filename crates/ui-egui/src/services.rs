//! What the frontend needs from the machine it runs on. The desktop app and the browser each supply one; the frontend
//! itself never touches a file, a socket library or a window system.

use bigstar_client::{Connection, Waker};

/// What a DS ROM file is called: `.nds`, or `.ids` for the iQue DS, which is the same format.
pub const ROM_EXTENSIONS: &[&str] = &["nds", "ids"];

fn own_rom_extension(name: &str) -> Option<&'static str> {
    let own = name.rsplit_once('.')?.1.to_ascii_lowercase();
    ROM_EXTENSIONS.iter().copied().find(|extension| *extension == own)
}

/// Whether a file named `name` is called a ROM.
pub fn is_rom_name(name: &str) -> bool {
    own_rom_extension(name).is_some()
}

/// The extension a ROM named `name` keeps when it is saved: its own, or `nds` when it has none of `ROM_EXTENSIONS`.
pub fn rom_extension(name: &str) -> &'static str {
    own_rom_extension(name).unwrap_or("nds")
}

pub trait Services {
    /// A value stored under `key` by an earlier run, if any.
    fn load(&self, key: &str) -> Option<String>;

    fn store(&self, key: &str, value: &str);

    /// Whether this build can run a host in its own process (desktop only).
    fn can_embed(&self) -> bool {
        false
    }

    /// Starts a host for the workspace at `root` and connects to it.
    fn connect_embedded(&self, _root: &str, _waker: Waker) -> Result<Box<dyn Connection>, String> {
        Err("this build cannot run a host of its own".to_owned())
    }

    /// The accent colour the person chose for their system, as red, green and blue, where the system has one. Asked
    /// every frame, so it has to be cheap.
    fn accent_color(&self) -> Option<[u8; 3]> {
        None
    }

    /// Asks the person for a folder, or `None` when they cancel or there is no dialog.
    fn pick_folder(&self) -> Option<String> {
        None
    }

    /// Starts a host in this process for ROMs on this machine, with no workspace, and connects to it (desktop only).
    fn connect_local_roms(&self, _waker: Waker) -> Result<Box<dyn Connection>, String> {
        Err("this build cannot open a ROM on this machine".to_owned())
    }

    /// Asks the person for a ROM (`ROM_EXTENSIONS`) on this machine, in a dialog titled `title`: its full path, or `None` when they cancel
    /// or there is no dialog.
    fn pick_rom(&self, _title: &str) -> Option<String> {
        None
    }

    /// Asks the person where on this machine to write a file named like `suggested`, of type `extension`: the full
    /// path, or `None` when they cancel or there is no dialog.
    fn pick_save_path(&self, _title: &str, _suggested: &str, _extension: &str) -> Option<String> {
        None
    }

    /// The address offered when connecting to a host; the browser offers the host that served the page.
    fn default_host(&self) -> String {
        "ws://127.0.0.1:7878".to_owned()
    }

    /// A token the page was opened with, as the host's printed link carries it; it also means the host to connect to is
    /// the default one.
    fn default_token(&self) -> Option<String> {
        None
    }

    /// Asks the person where to save `bytes` (a file named like `suggested`, of type `extension`) and writes them.
    /// `Ok(false)` when they cancelled.
    fn save_file(&self, _title: &str, _suggested: &str, _extension: &str, _bytes: &[u8]) -> Result<bool, String> {
        Err("this build cannot save files".to_owned())
    }

    /// Asks the person for a file (of one of the types `extensions`, or any when there are none) and reads it: its name
    /// and its bytes. `None` when they cancelled.
    fn open_file(&self, _title: &str, _extensions: &[&str]) -> Option<Result<(String, Vec<u8>), String>> {
        None
    }

    /// Starts asking the person for a file whose bytes arrive later, from `take_picked_file`: a browser's file picker
    /// answers only after the frame that opened it. `false` where there is no such picker.
    fn pick_file_later(&self, _extensions: &[&str]) -> bool {
        false
    }

    /// A file `pick_file_later` asked for, once it has been read: its name and its bytes.
    fn take_picked_file(&self) -> Option<Result<(String, Vec<u8>), String>> {
        None
    }
}

/// Files a test hands the frontend, and the ones it saved.
#[derive(Debug, Default)]
pub struct FileShelf {
    /// What the next "open" dialogs return, first first: a name and the bytes.
    pub to_open: std::collections::VecDeque<(String, Vec<u8>)>,
    /// Every file saved, by suggested name.
    pub saved: Vec<(String, Vec<u8>)>,
    /// What the next "pick a ROM" and "save to" dialogs return, first first.
    pub paths: std::collections::VecDeque<String>,
}

/// Keeps values and files in memory; for tests and screenshots.
#[derive(Default)]
pub struct MemoryServices {
    values: std::sync::Arc<std::sync::Mutex<std::collections::BTreeMap<String, String>>>,
    pub shelf: std::sync::Arc<std::sync::Mutex<FileShelf>>,
    accent: Option<[u8; 3]>,
}

impl MemoryServices {
    /// Services on a system whose accent colour is `accent`.
    pub fn with_accent(accent: [u8; 3]) -> Self {
        Self { accent: Some(accent), ..Self::default() }
    }

    /// Services sharing this one's stored values, as a later run of the program would find them.
    pub fn sharing_values(&self) -> Self {
        Self { values: self.values.clone(), shelf: std::sync::Arc::default(), accent: self.accent }
    }
}

impl Services for MemoryServices {
    fn load(&self, key: &str) -> Option<String> {
        self.values.lock().ok()?.get(key).cloned()
    }

    fn store(&self, key: &str, value: &str) {
        if let Ok(mut values) = self.values.lock() {
            values.insert(key.to_owned(), value.to_owned());
        }
    }

    fn accent_color(&self) -> Option<[u8; 3]> {
        self.accent
    }

    fn save_file(&self, _title: &str, suggested: &str, _extension: &str, bytes: &[u8]) -> Result<bool, String> {
        let mut shelf = self.shelf.lock().map_err(|_| "the shelf is poisoned".to_owned())?;
        shelf.saved.push((suggested.to_owned(), bytes.to_vec()));
        Ok(true)
    }

    fn open_file(&self, _title: &str, _extensions: &[&str]) -> Option<Result<(String, Vec<u8>), String>> {
        self.shelf.lock().ok()?.to_open.pop_front().map(Ok)
    }

    fn pick_rom(&self, _title: &str) -> Option<String> {
        self.shelf.lock().ok()?.paths.pop_front()
    }

    fn pick_save_path(&self, _title: &str, _suggested: &str, _extension: &str) -> Option<String> {
        self.shelf.lock().ok()?.paths.pop_front()
    }
}
