//! Watches the open project and reports what changed once things go quiet, so a build that rewrites a hundred files
//! costs one refresh rather than a hundred.

use std::path::{Path, PathBuf};
use std::sync::Weak;
use std::sync::mpsc::{RecvTimeoutError, channel};
use std::time::Duration;

use notify::{RecursiveMode, Watcher};

use crate::host::Shared;

/// How long the tree has to stay still before a batch of changes is reported.
pub(crate) const DEBOUNCE: Duration = Duration::from_millis(250);

/// Dropping it stops the watch, and the debounce thread with it.
pub(crate) struct ProjectWatcher {
    _watcher: notify::RecommendedWatcher,
}

impl ProjectWatcher {
    pub fn start(directory: &Path, shared: Weak<Shared>) -> notify::Result<Self> {
        let (sender, receiver) = channel::<PathBuf>();
        let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = event
                && !matches!(event.kind, notify::EventKind::Access(_))
            {
                for path in event.paths {
                    let _ = sender.send(path);
                }
            }
        })?;
        watcher.watch(directory, RecursiveMode::Recursive)?;
        std::thread::Builder::new()
            .name("bigstar-watch".into())
            .spawn(move || {
                while let Ok(first) = receiver.recv() {
                    let mut batch = vec![first];
                    loop {
                        match receiver.recv_timeout(DEBOUNCE) {
                            Ok(path) => batch.push(path),
                            Err(RecvTimeoutError::Timeout) => break,
                            Err(RecvTimeoutError::Disconnected) => return,
                        }
                    }
                    let Some(shared) = shared.upgrade() else { return };
                    shared.files_changed(batch);
                }
            })
            .map_err(|e| notify::Error::generic(&e.to_string()))?;
        Ok(Self { _watcher: watcher })
    }
}
