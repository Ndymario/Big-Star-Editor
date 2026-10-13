//! What the Filesystem dock is served: the project's filesystem view as protocol rows, each with the icon its first
//! bytes call for. A port of the C++ dock's `classify_entry` and of `MainWindow::reloadFilesystemView`'s module
//! localisation; the view itself is `bigstar_project::build_filesystem_view`.

use std::path::Path;

use bigstar_formats::{FormatKind, GraphicsFormatKind, UiResourceKind, identify_format, identify_graphics_format, identify_ui_resource};
use bigstar_patching::ModuleGraph;
use bigstar_project::{FilesystemEntry, FilesystemStatus};
use bigstar_protocol::{FileEntry, FileIcon, FileStatus};

/// How much of a file is read to tell what it is.
pub(crate) const HEAD: usize = 32;

/// A module whose directory the project carries itself is read from there, whatever NCPatcher reported.
pub(crate) fn localize_modules(graph: &mut ModuleGraph, project: &Path) {
    let local = project.join("modules");
    if !local.is_dir() {
        return;
    }
    graph.dir.clone_from(&local);
    for module in &mut graph.modules {
        let dir = local.join(&module.key);
        if dir.is_dir() {
            module.file = dir.join("module.yaml");
            if let Some(nitrofs) = &mut module.nitrofs {
                nitrofs.dir = dir.join("nitrofs");
            }
            module.dir = dir;
        }
    }
}

pub(crate) fn status(status: FilesystemStatus) -> FileStatus {
    match status {
        FilesystemStatus::Vanilla => FileStatus::Vanilla,
        FilesystemStatus::Replaced => FileStatus::Replaced,
        FilesystemStatus::Added => FileStatus::Added,
        FilesystemStatus::Pending => FileStatus::Pending,
        FilesystemStatus::Orphaned => FileStatus::Orphaned,
    }
}

pub(crate) fn entry(entry: &FilesystemEntry, icon: FileIcon) -> FileEntry {
    FileEntry {
        path: entry.path.clone(),
        status: status(entry.status),
        module: entry.module.clone(),
        component: entry.component.clone(),
        variant: entry.variant.clone(),
        id: entry.id,
        id_is_planned: entry.id_is_planned,
        member_id: entry.member_id,
        size: entry.size,
        from_module: !entry.source.as_os_str().is_empty() && !entry.module.is_empty(),
        icon,
    }
}

fn has_magic(head: &[u8], magic: &[u8], offset: usize) -> bool {
    head.get(offset..offset + magic.len()) == Some(magic)
}

/// Where a magic can still be read in a compressed file: NSMB stores plenty of files LZ-compressed, and when the first
/// block flag is zero the header survives verbatim right after it -- offset 5 for a bare LZ stream, 9 behind an LZ77
/// header.
fn magic_offsets(head: &[u8]) -> Vec<usize> {
    let mut offsets = vec![0];
    if head.first() == Some(&0x10) {
        offsets.push(5);
    }
    if has_magic(head, b"LZ77", 0) {
        offsets.push(9);
    }
    offsets
}

fn graphics_icon(kind: GraphicsFormatKind) -> Option<FileIcon> {
    use GraphicsFormatKind::*;
    match kind {
        NitroPalette | RawPaletteCandidate => Some(FileIcon::Palette),
        NitroScreenMap | RawScreenMapCandidate => Some(FileIcon::ScreenMap),
        NitroModel => Some(FileIcon::Model),
        NitroCellAnimation => Some(FileIcon::Animation),
        NitroCharacterGraphics | RawCharacterGraphicsCandidate | NitroCellLayout | NitroTexture => Some(FileIcon::Image),
        Unknown => None,
    }
}

/// What a file is, for its icon: its head first, its name only when the bytes say nothing. Magic wins throughout,
/// because the name hints treat any path containing "course" as a level.
pub(crate) fn icon(head: &[u8], path: &str) -> FileIcon {
    for offset in magic_offsets(head) {
        let body = head.get(offset.min(head.len())..).unwrap_or_default();
        if identify_format(body, "") == FormatKind::NarcArchive {
            return FileIcon::Archive;
        }
        match identify_ui_resource(body, path) {
            UiResourceKind::Bnbl => return FileIcon::TouchHitbox,
            UiResourceKind::Bncl => return FileIcon::OamPositions,
            UiResourceKind::Bncd | UiResourceKind::Enpg | UiResourceKind::Banner => return FileIcon::Image,
            UiResourceKind::Unknown => {}
        }
        // BNLL is undocumented; recognised only so its name cannot fall through to the level hint.
        if has_magic(body, b"JNLL", 0) {
            return FileIcon::File;
        }
        if let Some(icon) = graphics_icon(identify_graphics_format(body, "")) {
            return icon;
        }
        if has_magic(body, b"SDAT", 0) {
            return FileIcon::Audio;
        }
        if has_magic(body, b"MESG", 0) {
            return FileIcon::Text;
        }
        if has_magic(body, b" APS22_1", 0) {
            return FileIcon::Particle;
        }
        if [b"BCA0", b"BTP0", b"BTA0", b"BMA0", b"BVA0"].iter().any(|magic| has_magic(body, *magic, 0)) {
            return FileIcon::Animation;
        }
    }
    if path.to_ascii_lowercase().ends_with(".enpg") {
        return FileIcon::Image;
    }
    if let Some(icon) = graphics_icon(identify_graphics_format(&[], path)) {
        return icon;
    }
    match identify_format(&[], path) {
        FormatKind::BgdatCandidate | FormatKind::LevelCandidate => FileIcon::Level,
        _ => FileIcon::File,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_read_the_head_before_the_name() {
        assert_eq!(icon(b"NARC\xfe\xff", "x.bin"), FileIcon::Archive);
        assert_eq!(icon(b"RLCN\xff\xfe\x00\x01", "x.bin"), FileIcon::Palette);
        // An LZ stream whose first block is uncompressed still shows its magic at offset 5.
        assert_eq!(icon(b"\x10\x40\x00\x00\x00NARC\xfe\xff", "x.bin"), FileIcon::Archive);
        assert_eq!(icon(b"MESGbmg1", "course/x.bmg"), FileIcon::Text, "a message file under course/ is not a level");
        assert_eq!(icon(b"", "course/A01_1.bin"), FileIcon::Level);
        assert_eq!(icon(b"", "readme"), FileIcon::File);
    }
}
