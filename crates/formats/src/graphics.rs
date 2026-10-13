//! Graphics format identification. The decoders arrive with phase 7.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsFormatKind {
    Unknown,
    NitroCharacterGraphics,
    NitroPalette,
    NitroScreenMap,
    NitroCellLayout,
    NitroCellAnimation,
    NitroTexture,
    NitroModel,
    RawCharacterGraphicsCandidate,
    RawPaletteCandidate,
    RawScreenMapCandidate,
}

pub fn identify_graphics_format(data: &[u8], path_hint: &str) -> GraphicsFormatKind {
    // BNCL (JNCL) and BNCD (JNCD) are not NitroSystem formats; they are UI resources and are not claimed here.
    for (magic, kind) in [
        (b"RGCN", GraphicsFormatKind::NitroCharacterGraphics),
        (b"RLCN", GraphicsFormatKind::NitroPalette),
        (b"RCSN", GraphicsFormatKind::NitroScreenMap),
        (b"RECN", GraphicsFormatKind::NitroCellLayout),
        (b"RNAN", GraphicsFormatKind::NitroCellAnimation),
        (b"BTX0", GraphicsFormatKind::NitroTexture),
        (b"BMD0", GraphicsFormatKind::NitroModel),
    ] {
        if data.starts_with(magic) {
            return kind;
        }
    }
    let hint = path_hint.to_ascii_lowercase();
    if hint.ends_with(".ncgr") || hint.ends_with("_ncg.bin") {
        GraphicsFormatKind::RawCharacterGraphicsCandidate
    } else if hint.ends_with(".nclr") || hint.ends_with("_ncl.bin") {
        GraphicsFormatKind::RawPaletteCandidate
    } else if hint.ends_with(".nscr") || hint.ends_with("_nsc.bin") {
        GraphicsFormatKind::RawScreenMapCandidate
    } else if hint.ends_with(".nsbtx") {
        GraphicsFormatKind::NitroTexture
    } else if hint.ends_with(".nsbmd") {
        GraphicsFormatKind::NitroModel
    } else {
        GraphicsFormatKind::Unknown
    }
}

pub fn graphics_format_name(kind: GraphicsFormatKind) -> &'static str {
    match kind {
        GraphicsFormatKind::NitroCharacterGraphics => "Nitro character graphics",
        GraphicsFormatKind::NitroPalette => "Nitro palette",
        GraphicsFormatKind::NitroScreenMap => "Nitro screen map",
        GraphicsFormatKind::NitroCellLayout => "Nitro cell layout",
        GraphicsFormatKind::NitroCellAnimation => "Nitro cell animation",
        GraphicsFormatKind::NitroTexture => "Nitro texture",
        GraphicsFormatKind::NitroModel => "Nitro model",
        GraphicsFormatKind::RawCharacterGraphicsCandidate => "raw character graphics candidate",
        GraphicsFormatKind::RawPaletteCandidate => "raw palette candidate",
        GraphicsFormatKind::RawScreenMapCandidate => "raw screen map candidate",
        GraphicsFormatKind::Unknown => "unknown",
    }
}
