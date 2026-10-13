//! One entry point for "open this file in whichever editor understands it", so nothing asks the person what kind of
//! resource they are about to edit; and the hex document every file can fall back to. A port of `FileEditors`.

use bigstar_formats::{compress_nitro_lz10, compress_nitro_lz11, decompress_nitro_lz, is_bmg};

use crate::graphics_editor::GraphicsDoc;
use crate::message_editor::MessageDoc;
use crate::resource_editor::ResourceDoc;

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
    unwrap_with(bytes).1
}

/// The wrapper (`0x10` LZ10, `0x11` LZ11, `0` none) and the plain bytes, so what an editor hands back can go to disk
/// compressed the way it arrived.
pub fn unwrap_with(bytes: &[u8]) -> (u8, Vec<u8>) {
    match bytes.first() {
        Some(&compression @ (0x10 | 0x11)) => match decompress_nitro_lz(bytes) {
            Ok(body) => (compression, body),
            Err(_) => (0, bytes.to_vec()),
        },
        _ => (0, bytes.to_vec()),
    }
}

/// `edited` wrapped the way `original` was.
pub fn rewrap(original: &[u8], edited: Vec<u8>) -> Result<Vec<u8>, String> {
    wrap(unwrap_with(original).0, edited)
}

/// `body` in the wrapper `unwrap_with` reported.
pub fn wrap(compression: u8, body: Vec<u8>) -> Result<Vec<u8>, String> {
    match compression {
        0x10 => compress_nitro_lz10(&body).map_err(|e| e.message),
        0x11 => compress_nitro_lz11(&body).map_err(|e| e.message),
        _ => Ok(body),
    }
}

/// Which editor claims a file. Ordered by how certain each claim is: a BMG is settled by its magic and a parse, while
/// the graphics editor also accepts raw files on a name hint alone, so it goes last.
pub fn classify(bytes: &[u8], path: &str, rom_user_metadata: bool) -> EditorKind {
    let plain = unwrap(bytes);
    if is_bmg(&plain, path) && MessageDoc::supports(&plain) {
        return EditorKind::Messages;
    }
    // The uiStudio resources, the banner and the ROM metadata are pages of the same graphics editor.
    if ResourceDoc::supports(&plain, path, rom_user_metadata) || GraphicsDoc::supports(bytes, path) {
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
        let nclr = bigstar_formats::write_nclr(&bigstar_formats::Nclr { palette: vec![0; 32], ..Default::default() }).unwrap();
        assert_eq!(classify(&nclr, "a.nclr", false), EditorKind::Graphics);
        assert_eq!(classify(b"RLCN\xff\xfe\x00\x01", "b.nclr", false), EditorKind::None, "a header that does not parse claims nothing");
        assert_eq!(classify(b"hello", "notes.txt", false), EditorKind::None);
        assert_eq!(classify(b"MESGbmg1", "x.bmg", false), EditorKind::None, "a magic that does not parse claims nothing");
        assert_eq!(editor_name(EditorKind::Graphics), "Graphics");
    }

    #[test]
    fn opening_a_file_chooses_one_editor_through_any_lz_wrapper() {
        let bmg = crate::message_editor::tests::sample_bmg();
        assert_eq!(classify(&bmg, "script/course.bmg", false), EditorKind::Messages);
        // A uiStudio layout is a page of the one graphics editor, not an editor of its own.
        assert_eq!(classify(b"JNBL\0\0\x01\0\x01\0\x02\0\x10\x0c", "uiStudio/a.bnbl", false), EditorKind::Graphics);
        assert_eq!(classify(&[0; 64], "BG_ncg/a_ncg.bin", false), EditorKind::Graphics);
        assert_eq!(classify(b"nothing here at all", "notes.txt", false), EditorKind::None);
        // ROM metadata is only metadata as file 131.
        assert_eq!(classify(b"[Music]\n0=Overworld\n", "NSMBeUD", true), EditorKind::Graphics);
        assert_eq!(classify(b"[Music]\n0=Overworld\n", "NSMBeUD", false), EditorKind::None);
        // Compression is the dispatcher's business: a wrapped file reaches the same editor as the plain one.
        let compressed = compress_nitro_lz10(&bmg).unwrap();
        assert_eq!(classify(&compressed, "script/course.bmg", false), EditorKind::Messages);
        assert_eq!(wrap(0x10, bmg.clone()).unwrap(), compressed);
        assert_eq!(wrap(0, bmg.clone()).unwrap(), bmg);
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
