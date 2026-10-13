//! What the desktop gives the frontend: a settings file, a folder dialog, and a host in this process.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use bigstar_client::{Connection, HostInfo, InProcess, Waker};
use bigstar_host::Host;
use bigstar_ui_egui::Services;

pub struct NativeServices {
    file: Option<PathBuf>,
    values: Mutex<BTreeMap<String, String>>,
}

/// `$XDG_CONFIG_HOME/bigstar/settings.json`, `%APPDATA%\bigstar\settings.json`, or the like.
fn settings_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .or_else(|| std::env::var_os("APPDATA"))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))?;
    Some(base.join("bigstar").join("settings.json"))
}

impl NativeServices {
    pub fn load() -> Self {
        let file = settings_path();
        let values =
            file.as_ref().and_then(|path| std::fs::read(path).ok()).and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
        Self { file, values: Mutex::new(values) }
    }
}

impl Services for NativeServices {
    fn load(&self, key: &str) -> Option<String> {
        self.values.lock().ok()?.get(key).cloned()
    }

    fn store(&self, key: &str, value: &str) {
        let Ok(mut values) = self.values.lock() else { return };
        values.insert(key.to_owned(), value.to_owned());
        let Some(path) = &self.file else { return };
        let written = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| serde_json::to_vec_pretty(&*values).map_err(std::io::Error::other))
            .and_then(|bytes| std::fs::write(path, bytes));
        if let Err(error) = written {
            log::warn!("could not save settings to {}: {error}", path.display());
        }
    }

    fn can_embed(&self) -> bool {
        true
    }

    fn connect_embedded(&self, root: &str, waker: Waker) -> Result<Box<dyn Connection>, String> {
        let host = Host::new(std::path::Path::new(root)).map_err(|e| e.message)?;
        let info = HostInfo { host_version: env!("CARGO_PKG_VERSION").to_owned(), workspace_name: host.workspace_name() };
        Ok(Box::new(InProcess::spawn(host.session(), info, Some(waker))))
    }

    fn pick_folder(&self) -> Option<String> {
        rfd::FileDialog::new().set_title("Workspace folder").pick_folder().map(|path| path.to_string_lossy().into_owned())
    }

    fn save_file(&self, title: &str, suggested: &str, extension: &str, bytes: &[u8]) -> Result<bool, String> {
        let Some(path) = rfd::FileDialog::new().set_title(title).set_file_name(suggested).add_filter(extension, &[extension]).save_file() else {
            return Ok(false);
        };
        bigstar_core::write_file_atomically(&path, bytes).map(|()| true).map_err(|e| e.message)
    }

    fn open_file(&self, title: &str, extension: Option<&str>) -> Option<Result<(String, Vec<u8>), String>> {
        let dialog = rfd::FileDialog::new().set_title(title);
        let dialog = match extension {
            Some(extension) => dialog.add_filter(extension, &[extension]),
            None => dialog,
        };
        let path = dialog.pick_file()?;
        let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
        Some(std::fs::read(&path).map(|bytes| (name, bytes)).map_err(|e| format!("could not read {}: {e}", path.display())))
    }
}
