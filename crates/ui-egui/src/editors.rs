//! One entry point for "open this file in whichever editor understands it", so nothing asks the person what kind of
//! resource they are about to edit; and the hex document every file can fall back to. A port of `FileEditors`.

use bigstar_formats::{
    GraphicsFormatKind, UiResourceKind, decompress_nitro_lz, identify_graphics_format, identify_ui_resource, is_bmg, parse_bmg,
    parse_rom_user_metadata,
};

/// NSMB keeps its ROM user metadata in NitroFS file 131, which the metadata reader understands rather than uiStudio's.
pub const ROM_USER_METADATA_ID: i64 = 131;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorKind {
    None,
    Messages,
    Graphics,
}

pub fn editor_name(kind: EditorKind) -> &'static str {
    match kind {
        EditorKind::Messages => "Messages",
        EditorKind::Graphics => "Graphics",
        EditorKind::None => "None",
    }
}

/// A file with its LZ wrapper, if it had a real one, taken off: an editor sees plain bytes. A file that only looks
/// compressed stays exactly as it was read.
pub fn unwrap(bytes: &[u8]) -> Vec<u8> {
    match bytes.first() {
        Some(0x10 | 0x11) => decompress_nitro_lz(bytes).unwrap_or_else(|_| bytes.to_vec()),
        _ => bytes.to_vec(),
    }
}

/// Which editor claims a file. Ordered by how certain each claim is: a BMG is settled by its magic and a parse, while
/// the graphics editor also accepts raw files on a name hint alone, so it goes last.
pub fn classify(bytes: &[u8], path: &str, rom_user_metadata: bool) -> EditorKind {
    let plain = unwrap(bytes);
    if is_bmg(&plain, path) && parse_bmg(&plain).is_ok() {
        return EditorKind::Messages;
    }
    if rom_user_metadata && parse_rom_user_metadata(&plain).is_ok() {
        return EditorKind::Graphics;
    }
    if identify_ui_resource(&plain, path) != UiResourceKind::Unknown || identify_graphics_format(&plain, path) != GraphicsFormatKind::Unknown {
        return EditorKind::Graphics;
    }
    EditorKind::None
}

/// Bytes as hex, sixteen to a line.
pub fn hex_text(bytes: &[u8]) -> String {
    crate::tools::hex_text(bytes)
}

/// The hex document's bytes, or why there are none.
pub fn hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    crate::tools::unhex(text).ok_or_else(|| "hex data contains an invalid or incomplete byte".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_go_to_the_editor_that_understands_them() {
        assert_eq!(classify(b"RLCN\xff\xfe\x00\x01\x00\x00\x00\x00\x10\x00\x01\x00", "a.nclr", false), EditorKind::Graphics);
        assert_eq!(classify(b"hello", "notes.txt", false), EditorKind::None);
        assert_eq!(classify(b"MESGbmg1", "x.bmg", false), EditorKind::None, "a magic that does not parse claims nothing");
        assert_eq!(editor_name(EditorKind::Graphics), "Graphics");
    }

    #[test]
    fn a_wrapper_comes_off_before_a_file_is_classified() {
        let body: Vec<u8> = (0..200u8).map(|i| i % 7).collect();
        let wrapped = bigstar_formats::compress_nitro_lz10(&body).unwrap();
        assert_eq!(unwrap(&wrapped), body);
        // Something that only looks compressed is left alone.
        assert_eq!(unwrap(&[0x10, 0xff]), [0x10, 0xff]);
        assert_eq!(hex_bytes("0A 1").unwrap_err(), "hex data contains an invalid or incomplete byte");
    }
}
