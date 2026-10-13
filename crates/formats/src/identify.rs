use crate::{GraphicsFormatKind, identify_graphics_format, is_bmg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatKind {
    Unknown,
    NarcArchive,
    Graphics,
    MessageBundle,
    BgdatCandidate,
    LevelCandidate,
}

pub fn identify_format(data: &[u8], path_hint: &str) -> FormatKind {
    if data.starts_with(b"NARC") {
        FormatKind::NarcArchive
    } else if identify_graphics_format(data, path_hint) != GraphicsFormatKind::Unknown {
        FormatKind::Graphics
    } else if is_bmg(data, path_hint) {
        FormatKind::MessageBundle
    } else if path_hint.contains("bgdat") {
        FormatKind::BgdatCandidate
    } else if path_hint.contains("stage") || path_hint.contains("course") {
        FormatKind::LevelCandidate
    } else {
        FormatKind::Unknown
    }
}

pub fn format_name(kind: FormatKind) -> &'static str {
    match kind {
        FormatKind::NarcArchive => "NARC archive",
        FormatKind::Graphics => "graphics",
        FormatKind::MessageBundle => "BMG message bundle",
        FormatKind::BgdatCandidate => "NSMB bgdat candidate",
        FormatKind::LevelCandidate => "NSMB level candidate",
        FormatKind::Unknown => "unknown",
    }
}
