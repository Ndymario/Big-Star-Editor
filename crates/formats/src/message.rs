//! BMG message bundles (`MESGbmg1`): INF1/DAT1/MID1, inline escapes and the `[type:HEXDATA]` text form. Port of
//! `libs/nsmb-formats/src/message.cpp`.
//!
//! Message text is a Rust `String`, so the two byte sequences C++ could carry through untouched -- invalid UTF-8 in a
//! UTF-8 file and unpaired surrogates in a UTF-16 one -- are replaced with U+FFFD on load.

use std::collections::BTreeMap;

use bigstar_core::{Error, ErrorCode, LittleEndianWriter, Result};
use serde::{Deserialize, Serialize};

const HEADER_SIZE: usize = 0x20;
const SECTION_HEADER_SIZE: usize = 8;
/// Nintendo's own tooling rounds every section up to 32 bytes, and readers take the size from the section header, so
/// the padding is invisible to the game either way.
const SECTION_ALIGNMENT: usize = 32;
/// The escape marker is 0x1A in every encoding; only the unit width around it changes.
const ESCAPE_MARKER: u8 = 0x1a;

/// The encoding byte NDS BMG files carry at header offset 0x10.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum BmgEncoding {
    Unknown = 0,
    Cp1252 = 1,
    #[default]
    Utf16 = 2,
    ShiftJis = 3,
    Utf8 = 4,
}

/// One run of a message: either literal text or one of the game's inline escape sequences.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BmgPart {
    pub escape: bool,
    /// Meaningful only when `escape` is false.
    pub text: String,
    pub escape_type: u8,
    pub escape_data: Vec<u8>,
}

impl BmgPart {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), ..Self::default() }
    }

    pub fn escape(escape_type: u8, escape_data: Vec<u8>) -> Self {
        Self { escape: true, text: String::new(), escape_type, escape_data }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BmgMessage {
    pub parts: Vec<BmgPart>,
    /// INF1 entry bytes after the 4-byte DAT1 offset.
    pub attributes: Vec<u8>,
    /// MID1 message ID, when the file carries that section.
    pub id: Option<u32>,
}

/// A section this model does not interpret (STR1, FLW1, FLI1, ...), kept so saves stay lossless.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BmgSection {
    pub magic: String,
    /// Bytes after the 8-byte section header, padding included.
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bmg {
    pub encoding: BmgEncoding,
    pub file_id: u16,
    pub default_color: u8,
    /// The INF1 byte after the default colour.
    pub inf1_reserved: u8,
    /// INF1 entry size minus the 4-byte offset.
    pub attribute_size: u16,
    pub has_mid1: bool,
    /// MID1 bytes after its entry count.
    pub mid1_reserved: [u8; 6],
    /// Header bytes 0x11..0x1F.
    pub header_reserved: [u8; 15],
    pub messages: Vec<BmgMessage>,
    pub extra_sections: Vec<BmgSection>,
    /// Section magics in the order they were read.
    pub section_order: Vec<String>,
}

fn err(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

fn read16(bytes: &[u8], offset: usize) -> u16 {
    crate::g2d::u16_at(bytes, offset)
}

fn read32(bytes: &[u8], offset: usize) -> u32 {
    crate::g2d::u32_at(bytes, offset)
}

fn byte(bytes: &[u8], offset: usize) -> u8 {
    bytes.get(offset).copied().unwrap_or(0)
}

/// CP1252 differs from Latin-1 only across 0x80..0x9F. Undefined slots keep their byte value so a file that happens
/// to contain one still survives a load/save cycle unchanged.
const CP1252_HIGH: [u32; 32] = [
    0x20ac, 0x0081, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160, 0x2039, 0x0152, 0x008d, 0x017d, 0x008f, 0x0090, 0x2018,
    0x2019, 0x201c, 0x201d, 0x2022, 0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x009d, 0x017e, 0x0178,
];

fn cp1252_char(byte: u8) -> char {
    let code = if (0x80..0xa0).contains(&byte) { CP1252_HIGH.get(usize::from(byte - 0x80)).copied().unwrap_or(0) } else { u32::from(byte) };
    char::from_u32(code).unwrap_or(char::REPLACEMENT_CHARACTER)
}

fn encode_text(text: &str, encoding: BmgEncoding) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    if encoding == BmgEncoding::Utf8 {
        out.extend_from_slice(text.as_bytes());
        return Ok(out);
    }
    for c in text.chars() {
        let code = u32::from(c);
        match encoding {
            BmgEncoding::Utf16 => {
                let mut units = [0u16; 2];
                for unit in c.encode_utf16(&mut units) {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
            }
            BmgEncoding::Cp1252 => {
                if code < 0x80 || (0xa0..0x100).contains(&code) {
                    out.push(code as u8);
                } else if let Some(index) = CP1252_HIGH.iter().position(|&high| high == code) {
                    out.push(0x80 + index as u8);
                } else {
                    return Err(err(ErrorCode::InvalidArgument, "character cannot be written in this file's CP1252 encoding"));
                }
            }
            _ => return Err(err(ErrorCode::Unsupported, "unsupported BMG text encoding")),
        }
    }
    Ok(out)
}

fn unit_width(encoding: BmgEncoding) -> usize {
    if encoding == BmgEncoding::Utf16 { 2 } else { 1 }
}

/// One DAT1 string, split into literal runs and escape sequences. Escapes carry their own total byte length, so an
/// unrecognised escape type still passes through a load/save cycle intact.
fn decode_message(pool: &[u8], offset: usize, encoding: BmgEncoding) -> Result<Vec<BmgPart>> {
    if offset > pool.len() {
        return Err(err(ErrorCode::OutOfBounds, "message offset points outside the DAT1 pool"));
    }
    let width = unit_width(encoding);
    // The smallest escape is marker + length + type, plus the marker's second unit under UTF-16.
    let minimum_escape = width + 2;
    let mut parts = Vec::new();
    let mut wide: Vec<u16> = Vec::new();
    let mut narrow: Vec<u8> = Vec::new();

    let flush = |parts: &mut Vec<BmgPart>, wide: &mut Vec<u16>, narrow: &mut Vec<u8>| {
        if encoding == BmgEncoding::Utf16 {
            if !wide.is_empty() {
                parts.push(BmgPart::text(char::decode_utf16(wide.drain(..)).map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER)).collect::<String>()));
            }
            return;
        }
        if narrow.is_empty() {
            return;
        }
        let text = if encoding == BmgEncoding::Utf8 {
            String::from_utf8_lossy(narrow).into_owned()
        } else {
            narrow.iter().map(|&b| cp1252_char(b)).collect()
        };
        parts.push(BmgPart::text(text));
        narrow.clear();
    };

    let mut p = offset;
    while p + width <= pool.len() {
        let unit = if encoding == BmgEncoding::Utf16 { read16(pool, p) } else { u16::from(byte(pool, p)) };
        if unit == 0 {
            flush(&mut parts, &mut wide, &mut narrow);
            return Ok(parts);
        }
        if unit == u16::from(ESCAPE_MARKER) {
            if p + minimum_escape > pool.len() {
                return Err(err(ErrorCode::ParseError, "escape sequence runs past the DAT1 pool"));
            }
            let length = usize::from(byte(pool, p + width));
            if length < minimum_escape || p + length > pool.len() {
                return Err(err(ErrorCode::ParseError, "escape sequence declares an impossible length"));
            }
            flush(&mut parts, &mut wide, &mut narrow);
            let escape_data = pool.get(p + width + 2..p + length).unwrap_or_default().to_vec();
            parts.push(BmgPart::escape(byte(pool, p + width + 1), escape_data));
            p += length;
            continue;
        }
        if encoding == BmgEncoding::Utf16 {
            wide.push(unit);
        } else {
            narrow.push(unit as u8);
        }
        p += width;
    }
    Err(err(ErrorCode::ParseError, "message is not terminated inside the DAT1 pool"))
}

fn encode_message(message: &BmgMessage, encoding: BmgEncoding) -> Result<Vec<u8>> {
    let width = unit_width(encoding);
    let mut out = Vec::new();
    for part in &message.parts {
        if !part.escape {
            out.extend(encode_text(&part.text, encoding)?);
            continue;
        }
        let length = width + 2 + part.escape_data.len();
        let Ok(length) = u8::try_from(length) else {
            return Err(err(ErrorCode::InvalidArgument, "escape sequence payload is too long to store"));
        };
        out.push(ESCAPE_MARKER);
        if width == 2 {
            out.push(0);
        }
        out.push(length);
        out.push(part.escape_type);
        out.extend_from_slice(&part.escape_data);
    }
    out.resize(out.len() + width, 0);
    Ok(out)
}

fn pad_section(body: &mut Vec<u8>) {
    while !(body.len() + SECTION_HEADER_SIZE).is_multiple_of(SECTION_ALIGNMENT) {
        body.push(0);
    }
}

fn append_section(out: &mut Vec<u8>, magic: &str, body: &[u8]) {
    out.extend_from_slice(magic.as_bytes());
    out.extend_from_slice(&((SECTION_HEADER_SIZE + body.len()) as u32).to_le_bytes());
    out.extend_from_slice(body);
}

/// The extension of the last path component, `std::filesystem::path::extension` style: a leading dot does not start
/// one.
pub(crate) fn path_extension(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    if name == "." || name == ".." {
        return "";
    }
    match name.rfind('.') {
        Some(dot) if dot > 0 => name.get(dot..).unwrap_or(""),
        _ => "",
    }
}

pub fn is_bmg(bytes: &[u8], path: &str) -> bool {
    if bytes.starts_with(b"MESGbmg1") {
        return true;
    }
    bytes.is_empty() && path_extension(path).eq_ignore_ascii_case(".bmg")
}

pub fn bmg_encoding_name(encoding: BmgEncoding) -> &'static str {
    match encoding {
        BmgEncoding::Cp1252 => "CP1252",
        BmgEncoding::Utf16 => "UTF-16",
        BmgEncoding::ShiftJis => "Shift-JIS",
        BmgEncoding::Utf8 => "UTF-8",
        BmgEncoding::Unknown => "unspecified",
    }
}

pub fn parse_bmg(bytes: &[u8]) -> Result<Bmg> {
    if bytes.len() < HEADER_SIZE || !bytes.starts_with(b"MESGbmg1") {
        return Err(err(ErrorCode::ParseError, "file does not start with the MESGbmg1 header"));
    }
    let encoding = match byte(bytes, 0x10) {
        // What the DS games assume.
        0 | 2 => BmgEncoding::Utf16,
        1 => BmgEncoding::Cp1252,
        3 => return Err(err(ErrorCode::Unsupported, "Shift-JIS BMG files are not supported yet")),
        4 => BmgEncoding::Utf8,
        _ => return Err(err(ErrorCode::Unsupported, "unrecognised BMG text encoding")),
    };
    let mut value = Bmg { encoding, ..Bmg::default() };
    value.header_reserved.copy_from_slice(bytes.get(0x11..0x20).unwrap_or(&[0; 15]));

    let mut offsets = Vec::new();
    let mut attributes = Vec::new();
    let mut ids = Vec::new();
    let mut pool: &[u8] = &[];
    let (mut seen_inf1, mut seen_dat1) = (false, false);

    let mut p = HEADER_SIZE;
    while p + SECTION_HEADER_SIZE <= bytes.len() {
        let raw_magic = bytes.get(p..p + 4).unwrap_or_default();
        if !raw_magic.iter().all(u8::is_ascii_alphanumeric) {
            break;
        }
        let magic = String::from_utf8_lossy(raw_magic).into_owned();
        let mut size = read32(bytes, p + 4) as usize;
        // A truncated final section is common in ripped files; take what is actually there.
        if size < SECTION_HEADER_SIZE || p + size > bytes.len() {
            size = bytes.len() - p;
        }
        let body = bytes.get(p + SECTION_HEADER_SIZE..p + size).unwrap_or_default();
        value.section_order.push(magic.clone());

        if magic == "INF1" && !seen_inf1 {
            seen_inf1 = true;
            if body.len() < 8 {
                return Err(err(ErrorCode::ParseError, "INF1 section is too small"));
            }
            let count = usize::from(read16(body, 0));
            let entry_size = usize::from(read16(body, 2));
            value.file_id = read16(body, 4);
            value.default_color = byte(body, 6);
            value.inf1_reserved = byte(body, 7);
            if entry_size < 4 {
                return Err(err(ErrorCode::ParseError, "INF1 entries are too small to hold a DAT1 offset"));
            }
            value.attribute_size = (entry_size - 4) as u16;
            if 8 + count * entry_size > body.len() {
                return Err(err(ErrorCode::ParseError, "INF1 declares more entries than the section holds"));
            }
            for i in 0..count {
                let entry = 8 + i * entry_size;
                offsets.push(read32(body, entry) as usize);
                attributes.push(body.get(entry + 4..entry + entry_size).unwrap_or_default().to_vec());
            }
        } else if magic == "DAT1" && !seen_dat1 {
            seen_dat1 = true;
            pool = body;
        } else if magic == "MID1" && !value.has_mid1 {
            value.has_mid1 = true;
            if body.len() < 8 {
                return Err(err(ErrorCode::ParseError, "MID1 section is too small"));
            }
            let count = usize::from(read16(body, 0));
            value.mid1_reserved.copy_from_slice(body.get(2..8).unwrap_or(&[0; 6]));
            if 8 + count * 4 > body.len() {
                return Err(err(ErrorCode::ParseError, "MID1 declares more IDs than the section holds"));
            }
            ids = (0..count).map(|i| read32(body, 8 + i * 4)).collect();
        } else {
            value.extra_sections.push(BmgSection { magic, body: body.to_vec() });
        }
        p += size;
    }

    if !seen_inf1 || !seen_dat1 {
        return Err(err(ErrorCode::ParseError, "BMG file is missing its INF1 or DAT1 section"));
    }
    value.messages.reserve(offsets.len());
    for (i, (offset, attributes)) in offsets.into_iter().zip(attributes).enumerate() {
        let parts = decode_message(pool, offset, value.encoding)?;
        value.messages.push(BmgMessage { parts, attributes, id: ids.get(i).copied() });
    }
    Ok(value)
}

pub fn write_bmg(value: &Bmg) -> Result<Vec<u8>> {
    if value.messages.len() > 0xffff {
        return Err(err(ErrorCode::InvalidArgument, "a BMG file cannot hold more than 65535 messages"));
    }
    let width = unit_width(value.encoding);
    let entry_size = 4 + usize::from(value.attribute_size);

    // Offset 0 is the shared empty string.
    let mut pool = vec![0u8; width];
    let mut seen: BTreeMap<Vec<u8>, u32> = BTreeMap::from([(vec![0u8; width], 0)]);
    let mut inf1 = Vec::new();
    inf1.extend_from_slice(&(value.messages.len() as u16).to_le_bytes());
    inf1.extend_from_slice(&(entry_size as u16).to_le_bytes());
    inf1.extend_from_slice(&value.file_id.to_le_bytes());
    inf1.push(value.default_color);
    inf1.push(value.inf1_reserved);

    for message in &value.messages {
        let encoded = encode_message(message, value.encoding)?;
        let offset = match seen.get(&encoded) {
            Some(&offset) => offset,
            None => {
                let offset = pool.len() as u32;
                pool.extend_from_slice(&encoded);
                seen.insert(encoded, offset);
                offset
            }
        };
        inf1.extend_from_slice(&offset.to_le_bytes());
        let mut attributes = message.attributes.clone();
        attributes.resize(usize::from(value.attribute_size), 0);
        inf1.extend_from_slice(&attributes);
    }

    let mut mid1 = Vec::new();
    if value.has_mid1 {
        mid1.extend_from_slice(&(value.messages.len() as u16).to_le_bytes());
        mid1.extend_from_slice(&value.mid1_reserved);
        for (i, message) in value.messages.iter().enumerate() {
            mid1.extend_from_slice(&message.id.unwrap_or(i as u32).to_le_bytes());
        }
    }

    pad_section(&mut inf1);
    pad_section(&mut pool);
    pad_section(&mut mid1);

    let mut out = vec![0u8; HEADER_SIZE];
    if let Some(slot) = out.get_mut(..8) {
        slot.copy_from_slice(b"MESGbmg1");
    }
    if let Some(slot) = out.get_mut(0x10) {
        *slot = value.encoding as u8;
    }
    if let Some(slot) = out.get_mut(0x11..0x20) {
        slot.copy_from_slice(&value.header_reserved);
    }

    let mut emitted: Vec<String> = Vec::new();
    let mut emit = |out: &mut Vec<u8>, name: &str| {
        if emitted.iter().any(|e| e == name) {
            return;
        }
        match name {
            "INF1" => append_section(out, name, &inf1),
            "DAT1" => append_section(out, name, &pool),
            "MID1" => {
                if !value.has_mid1 {
                    return;
                }
                append_section(out, name, &mid1);
            }
            _ => {
                let Some(extra) = value.extra_sections.iter().find(|s| s.magic == name) else { return };
                append_section(out, name, &extra.body);
            }
        }
        emitted.push(name.to_owned());
    };
    for name in &value.section_order {
        emit(&mut out, name);
    }
    for name in ["INF1", "DAT1", "MID1"] {
        emit(&mut out, name);
    }
    for extra in &value.extra_sections {
        emit(&mut out, &extra.magic);
    }
    let sections = emitted.len() as u32;

    let size = out.len() as u32;
    let mut writer = LittleEndianWriter::new(&mut out);
    writer.u32(0x08, size)?;
    writer.u32(0x0c, sections)?;
    Ok(out)
}

/// Messages round-trip through the `[type:HEXDATA]` convention the NDS BMG Editor uses, so text exported from one
/// tool can be pasted into the other.
pub fn bmg_message_text(message: &BmgMessage) -> String {
    let mut out = String::new();
    for part in &message.parts {
        if !part.escape {
            out.push_str(&part.text);
            continue;
        }
        out.push('[');
        out.push_str(&part.escape_type.to_string());
        out.push(':');
        for byte in &part.escape_data {
            out.push_str(&format!("{byte:02X}"));
        }
        out.push(']');
    }
    out
}

pub fn bmg_message_plain_text(message: &BmgMessage) -> String {
    message.parts.iter().filter(|p| !p.escape).map(|p| p.text.as_str()).collect::<String>().replace(['\n', '\r'], " ")
}

pub fn parse_bmg_message_text(text: &str) -> Vec<BmgPart> {
    let bytes = text.as_bytes();
    let mut parts = Vec::new();
    let mut literal: Vec<u8> = Vec::new();
    let flush = |parts: &mut Vec<BmgPart>, literal: &mut Vec<u8>| {
        if !literal.is_empty() {
            parts.push(BmgPart::text(String::from_utf8_lossy(literal).into_owned()));
            literal.clear();
        }
    };

    let mut i = 0;
    while let Some(&c) = bytes.get(i) {
        if c != b'[' {
            literal.push(c);
            i += 1;
            continue;
        }
        let find = |needle: u8| bytes.get(i..).and_then(|rest| rest.iter().position(|&b| b == needle)).map(|n| n + i);
        let (Some(close), Some(colon)) = (find(b']'), find(b':')) else {
            literal.push(c);
            i += 1;
            continue;
        };
        if colon > close {
            literal.push(c);
            i += 1;
            continue;
        }
        let type_text = bytes.get(i + 1..colon).unwrap_or_default();
        let hex_text = bytes.get(colon + 1..close).unwrap_or_default();
        let escape_type = std::str::from_utf8(type_text)
            .ok()
            .filter(|t| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|t| t.parse::<u32>().ok())
            .and_then(|t| u8::try_from(t).ok());
        let Some(escape_type) = escape_type.filter(|_| hex_text.iter().all(u8::is_ascii_hexdigit)) else {
            literal.push(c);
            i += 1;
            continue;
        };
        flush(&mut parts, &mut literal);
        let mut padded = hex_text.to_vec();
        if padded.len() % 2 != 0 {
            padded.insert(0, b'0');
        }
        let nibble = |d: u8| char::from(d).to_digit(16).unwrap_or(0) as u8;
        let escape_data = padded.as_chunks::<2>().0.iter().map(|&[high, low]| (nibble(high) << 4) | nibble(low)).collect();
        parts.push(BmgPart::escape(escape_type, escape_data));
        i = close + 1;
    }
    flush(&mut parts, &mut literal);
    parts
}
