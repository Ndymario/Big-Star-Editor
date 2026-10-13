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

pub(crate) fn contains_parent_reference(path: &Path) -> bool {
    path.components().any(|part| matches!(part, Component::ParentDir))
}

/// The `/`-separated spelling a manifest stores, whatever the platform.
pub(crate) fn to_generic_string(path: &Path) -> String {
    path.components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().into_owned()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
