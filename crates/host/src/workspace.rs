use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use bigstar_core::{Error, ErrorCode, Result};

/// The directory a host serves. Nothing outside it is reachable: every path a client names goes through `resolve`,
/// which refuses anything that is not a plain relative path and anything whose real location, symlinks followed,
/// is not under the root.
#[derive(Debug, Clone)]
pub struct Workspace {
    root: PathBuf,
}

fn refuse(message: &str) -> Error {
    Error::new(ErrorCode::InvalidArgument, message)
}

impl Workspace {
    pub fn new(root: &Path) -> Result<Self> {
        let root =
            std::fs::canonicalize(root).map_err(|_| Error::new(ErrorCode::NotFound, format!("workspace root {} does not exist", root.display())))?;
        if !root.is_dir() {
            return Err(refuse("the workspace root is not a directory"));
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn name(&self) -> String {
        self.root.file_name().map_or_else(|| self.root.display().to_string(), |name| name.to_string_lossy().into_owned())
    }

    /// The absolute location of a root-relative `/`-separated path. The path need not exist, so it can name something
    /// about to be created; the part that does exist is resolved through symlinks and must stay under the root.
    pub fn resolve(&self, client_path: &str) -> Result<PathBuf> {
        let parts = split_client_path(client_path)?;
        let joined = parts.iter().fold(self.root.clone(), |path, part| path.join(part));

        for ancestor in joined.ancestors() {
            match std::fs::symlink_metadata(ancestor) {
                Ok(_) => {
                    // A link that cannot be resolved is refused rather than skipped: writing through a dangling one
                    // creates its target, wherever that is.
                    let real = std::fs::canonicalize(ancestor).map_err(|_| refuse("path cannot be resolved"))?;
                    if !real.starts_with(&self.root) {
                        return Err(refuse("path leaves the workspace"));
                    }
                    let tail = joined.strip_prefix(ancestor).map_err(|_| refuse("path cannot be resolved"))?;
                    return Ok(if tail.as_os_str().is_empty() { real } else { real.join(tail) });
                }
                Err(e) if e.kind() == ErrorKind::NotFound => {}
                Err(_) => return Err(refuse("path cannot be inspected")),
            }
        }
        Err(refuse("path leaves the workspace"))
    }

    /// `path` as a root-relative `/`-separated string, or `None` when it is not under the root.
    pub fn relative(&self, path: &Path) -> Option<String> {
        let relative = path.strip_prefix(&self.root).ok()?;
        let parts: Vec<String> = relative
            .components()
            .map(|component| match component {
                Component::Normal(part) => Some(part.to_string_lossy().into_owned()),
                _ => None,
            })
            .collect::<Option<_>>()?;
        Some(parts.join("/"))
    }
}

/// The components of a client path, or why it is not a plain relative path.
fn split_client_path(client_path: &str) -> Result<Vec<&str>> {
    if client_path.contains('\\') {
        return Err(refuse("paths use / as the separator"));
    }
    if client_path.contains('\0') {
        return Err(refuse("path contains a NUL byte"));
    }
    if client_path.starts_with('/') {
        return Err(refuse("paths are relative to the workspace root"));
    }
    let mut parts = Vec::new();
    for part in client_path.split('/') {
        match part {
            "" | "." => {}
            ".." => return Err(refuse("paths may not contain ..")),
            _ => {
                let mut chars = part.chars();
                if matches!((chars.next(), chars.next()), (Some(letter), Some(':')) if letter.is_ascii_alphabetic()) {
                    return Err(refuse("paths may not carry a drive prefix"));
                }
                parts.push(part);
            }
        }
    }
    Ok(parts)
}
