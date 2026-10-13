//! What the frontend needs from the machine it runs on. The desktop app and the browser each supply one; the frontend
//! itself never touches a file, a socket library or a window system.

use bigstar_client::{Connection, Waker};

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

    /// Asks the person for a folder, or `None` when they cancel or there is no dialog.
    fn pick_folder(&self) -> Option<String> {
        None
    }

    /// The address offered when connecting to a host; the browser offers the host that served the page.
    fn default_host(&self) -> String {
        "ws://127.0.0.1:7878".to_owned()
    }
}

/// Keeps values in memory; for tests and screenshots.
#[derive(Default)]
pub struct MemoryServices {
    values: std::sync::Mutex<std::collections::BTreeMap<String, String>>,
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
}
