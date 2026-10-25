use std::path::{Component, Path, PathBuf};

/// Collapses `.` and `..` without touching the filesystem. An empty result means the path named its own base.
pub fn lexically_normal(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                let climbs = matches!(out.components().next_back(), Some(Component::Normal(_)));
                if climbs {
                    out.pop();
                } else if !matches!(out.components().next_back(), Some(Component::RootDir | Component::Prefix(_))) {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
