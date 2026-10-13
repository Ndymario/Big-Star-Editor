//! The entity clipboard, as the Qt editor writes it: `BigStar-Entities/1`, a newline, then Base64 of the LZ10-compressed
//! JSON `{"entities":[...],"schema":"bigstar.entity-clipboard/1"}`. Each entity is `{"kind": n, "v": [fields...]}`, a
//! module sprite adds its `"hash"` as eight hex digits, and a path adds its `"nodes"`. Keys are written in sorted
//! order, which is the order `QJsonDocument` writes them in, so either editor reads what the other copied.

use bigstar_core::Fx32;
use bigstar_formats::{
    NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelSprite, NsmbLevelView, NsmbLevelZone, compress_nitro_lz10,
    decompress_nitro_lz,
};
use bigstar_level::{LevelEntityInsertions, SpriteInsertion};
use serde_json::{Map, Value, json};

use crate::level_view::{Kind, LevelView};

const HEADER: &str = "BigStar-Entities/1\n";
const SCHEMA: &str = "bigstar.entity-clipboard/1";
/// A paste bigger than this is not something anyone copied.
const PAYLOAD_LIMIT: usize = 24 * 1024 * 1024;
const ENTITY_LIMIT: usize = 10_000;

/// The kind numbers the Qt editor uses on the clipboard.
fn kind_number(kind: Kind) -> i64 {
    match kind {
        Kind::Object => 1,
        Kind::Sprite => 2,
        Kind::Entrance => 3,
        Kind::Zone => 4,
        Kind::Path => 5,
        Kind::ProgressPath => 6,
        Kind::View => 7,
    }
}

/// `{"hash"?, "kind", "nodes"?, "v"}`, keys in sorted order.
fn entity(kind: Kind, values: Vec<i64>, hash: Option<u32>, nodes: Option<Vec<Value>>) -> Value {
    let mut item = Map::new();
    if let Some(hash) = hash {
        item.insert("hash".to_owned(), Value::String(format!("{hash:08x}")));
    }
    item.insert("kind".to_owned(), json!(kind_number(kind)));
    if let Some(nodes) = nodes {
        item.insert("nodes".to_owned(), Value::Array(nodes));
    }
    item.insert("v".to_owned(), json!(values));
    Value::Object(item)
}

/// The selection as clipboard text. A path is copied whole, once, however many of its nodes are selected.
pub fn copy(view: &LevelView) -> Result<Option<String>, String> {
    if view.selection.is_empty() {
        return Ok(None);
    }
    let snapshot = &view.snapshot;
    let course = &snapshot.course;
    let mut seen = std::collections::BTreeSet::new();
    let mut entities = Vec::new();
    for item in &view.selection {
        if !seen.insert((item.kind, item.index)) {
            continue;
        }
        let index = item.index;
        let value = match item.kind {
            Kind::Object => snapshot.objects.get(index).map(|v| {
                let values =
                    [i64::from(v.object_id), i64::from(v.tileset_slot), i64::from(v.x), i64::from(v.y), i64::from(v.width), i64::from(v.height)];
                entity(Kind::Object, values.to_vec(), None, None)
            }),
            Kind::Sprite => course.sprites.get(index).map(|v| {
                let values = [i64::from(v.id), i64::from(v.x), i64::from(v.y), i64::from(v.events[0]), i64::from(v.events[1]), i64::from(v.settings)];
                entity(Kind::Sprite, values.to_vec(), view.sprite_hash(index), None)
            }),
            Kind::Entrance => course.entrances.get(index).map(|v| {
                let values = [
                    v.x.into(),
                    v.y.into(),
                    v.camera_x.into(),
                    v.camera_y.into(),
                    v.id.into(),
                    v.target_stage_area.into(),
                    v.target_stage_index.into(),
                    v.target_stage_group.into(),
                    v.target_entrance.into(),
                    v.camera_zoom.into(),
                    v.kind.into(),
                    v.flags.into(),
                    v.unknown_10.into(),
                    v.unknown_11.into(),
                    v.view_id.into(),
                    v.target_worldmap_node.into(),
                ];
                entity(Kind::Entrance, values.to_vec(), None, None)
            }),
            Kind::Zone => course.zones.get(index).map(|v| {
                let values = [
                    v.x.into(),
                    v.y.into(),
                    v.width.into(),
                    v.height.into(),
                    v.id.into(),
                    v.unknown_9.into(),
                    v.unknown_a.into(),
                    v.unknown_b.into(),
                ];
                entity(Kind::Zone, values.to_vec(), None, None)
            }),
            Kind::Path | Kind::ProgressPath => {
                let list = if item.kind == Kind::Path { &course.paths } else { &course.progress_paths };
                list.get(index).map(|v| {
                    let nodes = v
                        .nodes
                        .iter()
                        .map(|n| {
                            json!([
                                i64::from(n.x),
                                i64::from(n.y),
                                i64::from(n.speed.raw()),
                                i64::from(n.acceleration.raw()),
                                i64::from(n.delay),
                                i64::from(n.flags)
                            ])
                        })
                        .collect();
                    entity(item.kind, vec![v.id.into(), v.unknown.into()], None, Some(nodes))
                })
            }
            Kind::View => course.views.get(index).map(|v| {
                let values = [
                    v.x.into(),
                    v.y.into(),
                    v.width.into(),
                    v.height.into(),
                    v.id.into(),
                    v.camera_settings_id.into(),
                    v.bgm_id.into(),
                    v.background_id.into(),
                    v.tileset_id.into(),
                    v.foreground_id.into(),
                    v.light_type.into(),
                    v.progress_path_id.into(),
                ];
                entity(Kind::View, values.to_vec(), None, None)
            }),
        };
        entities.extend(value);
    }
    let mut root = Map::new();
    root.insert("entities".to_owned(), Value::Array(entities));
    root.insert("schema".to_owned(), Value::String(SCHEMA.to_owned()));
    let json = serde_json::to_vec(&Value::Object(root)).map_err(|e| e.to_string())?;
    let compressed = compress_nitro_lz10(&json).map_err(|e| e.message)?;
    Ok(Some(format!("{HEADER}{}", base64_encode(&compressed))))
}

/// What pasting `text` adds, each thing one tile (16 pixels) along from where it was copied, so a paste never lands
/// exactly on its original. Every value is range-checked; one bad value refuses the whole paste.
pub fn paste(text: &str) -> Result<LevelEntityInsertions, String> {
    let payload = text.strip_prefix(HEADER).ok_or("Clipboard does not contain Big Star entities")?;
    if payload.len() > PAYLOAD_LIMIT {
        return Err("Clipboard entity payload is too large".to_owned());
    }
    let encoded = base64_decode(payload).filter(|bytes| base64_encode(bytes) == payload).ok_or("Clipboard entity payload is not valid Base64")?;
    let json = decompress_nitro_lz(&encoded).map_err(|e| e.message)?;
    let invalid = || "Clipboard entity payload is invalid".to_owned();
    let root: Value = serde_json::from_slice(&json).map_err(|_| invalid())?;
    let entities = root.get("entities").and_then(Value::as_array).ok_or_else(invalid)?;
    if root.get("schema").and_then(Value::as_str) != Some(SCHEMA) || entities.len() > ENTITY_LIMIT {
        return Err(invalid());
    }
    let bad = || "Clipboard entity values are invalid".to_owned();
    let number = |values: &[Value], i: usize, low: i64, high: i64| values.get(i).and_then(Value::as_i64).filter(|n| (low..=high).contains(n));
    let mut items = LevelEntityInsertions::default();
    for entry in entities {
        let values: &[Value] = entry.get("v").and_then(Value::as_array).map_or(&[], Vec::as_slice);
        let kind = entry.get("kind").and_then(Value::as_i64).unwrap_or(-1);
        // The defaults of the Qt reader: anything from -32768 to 65535.
        let n = |i: usize| number(values, i, -32768, 65535);
        let r = |i: usize, low: i64, high: i64| number(values, i, low, high);
        let byte = |i: usize| r(i, 0, 255).map(|v| v as u8);
        let at = |i: usize| r(i, 0, 65519).map(|v| (v + 16) as u16);
        match (kind, values.len()) {
            (1, 6) => items.objects.push(NsmbLevelObject {
                object_id: r(0, 0, 4095).ok_or_else(bad)? as u16,
                tileset_slot: r(1, 0, 15).ok_or_else(bad)? as u8,
                x: (r(2, -32768, 32766).ok_or_else(bad)? + 1) as i16,
                y: (r(3, -32768, 32766).ok_or_else(bad)? + 1) as i16,
                width: r(4, 1, 65535).ok_or_else(bad)? as u16,
                height: r(5, 1, 65535).ok_or_else(bad)? as u16,
            }),
            (2, 6) => {
                let settings = r(5, 0, i64::from(u32::MAX)).ok_or_else(bad)? as u32;
                let value = NsmbLevelSprite {
                    id: r(0, 0, 65535).ok_or_else(bad)? as u16,
                    x: (r(1, 0, 65534).ok_or_else(bad)? + 1) as u16,
                    y: (r(2, 0, 65534).ok_or_else(bad)? + 1) as u16,
                    events: [byte(3).ok_or_else(bad)?, byte(4).ok_or_else(bad)?],
                    settings,
                };
                let hash = match entry.get("hash") {
                    None => None,
                    Some(Value::String(text)) if text.len() == 8 => Some(u32::from_str_radix(text, 16).map_err(|_| bad())?),
                    Some(_) => return Err(bad()),
                };
                items.sprites.push(SpriteInsertion { value, hash });
            }
            (3, 16) => items.entrances.push(NsmbLevelEntrance {
                x: at(0).ok_or_else(bad)?,
                y: at(1).ok_or_else(bad)?,
                camera_x: r(2, 0, 65535).ok_or_else(bad)? as u16,
                camera_y: r(3, 0, 65535).ok_or_else(bad)? as u16,
                id: byte(4).ok_or_else(bad)?,
                target_stage_area: byte(5).ok_or_else(bad)?,
                target_stage_index: byte(6).ok_or_else(bad)?,
                target_stage_group: byte(7).ok_or_else(bad)?,
                target_entrance: byte(8).ok_or_else(bad)?,
                camera_zoom: byte(9).ok_or_else(bad)?,
                kind: byte(10).ok_or_else(bad)?,
                flags: byte(11).ok_or_else(bad)?,
                unknown_10: byte(12).ok_or_else(bad)?,
                unknown_11: byte(13).ok_or_else(bad)?,
                view_id: byte(14).ok_or_else(bad)?,
                target_worldmap_node: byte(15).ok_or_else(bad)?,
            }),
            (4, 8) => items.zones.push(NsmbLevelZone {
                x: at(0).ok_or_else(bad)?,
                y: at(1).ok_or_else(bad)?,
                width: r(2, 1, 65535).ok_or_else(bad)? as u16,
                height: r(3, 1, 65535).ok_or_else(bad)? as u16,
                id: byte(4).ok_or_else(bad)?,
                unknown_9: byte(5).ok_or_else(bad)?,
                unknown_a: byte(6).ok_or_else(bad)?,
                unknown_b: byte(7).ok_or_else(bad)?,
            }),
            (5 | 6, 2) => {
                let nodes = entry.get("nodes").and_then(Value::as_array).filter(|n| n.len() <= ENTITY_LIMIT).ok_or_else(bad)?;
                let mut path =
                    NsmbLevelPath { id: byte(0).ok_or_else(bad)?, unknown: n(1).filter(|v| *v >= 0).ok_or_else(bad)? as u16, nodes: Vec::new() };
                for node in nodes {
                    let a: &[Value] = node.as_array().filter(|a| a.len() == 6).map(Vec::as_slice).ok_or_else(bad)?;
                    let full = (i64::from(i32::MIN), i64::from(i32::MAX));
                    path.nodes.push(NsmbLevelPathNode {
                        x: (number(a, 0, -32768, 32751).ok_or_else(bad)? + 16) as i16,
                        y: (number(a, 1, -32768, 32751).ok_or_else(bad)? + 16) as i16,
                        speed: Fx32::from_raw(number(a, 2, full.0, full.1).ok_or_else(bad)? as i32),
                        acceleration: Fx32::from_raw(number(a, 3, full.0, full.1).ok_or_else(bad)? as i32),
                        delay: number(a, 4, 0, 65535).ok_or_else(bad)? as u16,
                        flags: number(a, 5, 0, 65535).ok_or_else(bad)? as u16,
                    });
                }
                if kind == 6 { items.progress_paths.push(path) } else { items.paths.push(path) }
            }
            (7, 12) => items.views.push(NsmbLevelView {
                x: at(0).ok_or_else(bad)?,
                y: at(1).ok_or_else(bad)?,
                width: r(2, 1, 65535).ok_or_else(bad)? as u16,
                height: r(3, 1, 65535).ok_or_else(bad)? as u16,
                id: byte(4).ok_or_else(bad)?,
                camera_settings_id: byte(5).ok_or_else(bad)?,
                bgm_id: byte(6).ok_or_else(bad)?,
                background_id: byte(7).ok_or_else(bad)?,
                tileset_id: byte(8).ok_or_else(bad)?,
                foreground_id: byte(9).ok_or_else(bad)?,
                light_type: byte(10).ok_or_else(bad)?,
                progress_path_id: byte(11).ok_or_else(bad)?,
            }),
            _ => return Err(bad()),
        }
    }
    Ok(items)
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Standard Base64 with padding, which is what `QByteArray::toBase64` writes.
pub fn base64_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let symbol = |index: u32| ALPHABET.get((index & 63) as usize).map_or('=', |&c| char::from(c));
    for chunk in bytes.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|i| u32::from(chunk.get(i).copied().unwrap_or(0)));
        let group = (a << 16) | (b << 8) | c;
        out.push(symbol(group >> 18));
        out.push(symbol(group >> 12));
        out.push(if chunk.len() > 1 { symbol(group >> 6) } else { '=' });
        out.push(if chunk.len() > 2 { symbol(group) } else { '=' });
    }
    out
}

/// Strict: whole groups of four, the standard alphabet, padding only at the end.
pub fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let bytes = text.as_bytes();
    if !bytes.len().is_multiple_of(4) {
        return None;
    }
    let value = |c: u8| ALPHABET.iter().position(|&a| a == c).map(|p| p as u32);
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let groups = bytes.chunks(4).count();
    for (number, chunk) in bytes.chunks(4).enumerate() {
        let padding = chunk.iter().rev().take_while(|&&c| c == b'=').count();
        if padding > 2 || (padding > 0 && number + 1 != groups) {
            return None;
        }
        let mut group = 0u32;
        for &c in chunk.get(..4 - padding)? {
            group = (group << 6) | value(c)?;
        }
        group <<= 6 * padding as u32;
        let decoded = [(group >> 16) as u8, (group >> 8) as u8, group as u8];
        out.extend_from_slice(decoded.get(..3 - padding)?);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        for (bytes, text) in [(&b""[..], ""), (b"f", "Zg=="), (b"fo", "Zm8="), (b"foo", "Zm9v"), (b"foobar", "Zm9vYmFy"), (&[0xfb, 0xff][..], "+/8=")]
        {
            assert_eq!(base64_encode(bytes), text);
            assert_eq!(base64_decode(text).as_deref(), Some(bytes));
        }
        for bad in ["Zg=", "Z===", "Zg==Zg==", "Zm9-", "Zm 9"] {
            assert_eq!(base64_decode(bad), None, "{bad}");
        }
    }

    #[test]
    fn foreign_text_and_damaged_payloads_are_refused() {
        assert_eq!(paste("hello").unwrap_err(), "Clipboard does not contain Big Star entities");
        assert_eq!(paste("BigStar-Entities/1\n!!!!").unwrap_err(), "Clipboard entity payload is not valid Base64");
        let wrap = |json: &str| format!("{HEADER}{}", base64_encode(&compress_nitro_lz10(json.as_bytes()).unwrap()));
        assert_eq!(paste(&wrap(r#"{"entities":[],"schema":"other"}"#)).unwrap_err(), "Clipboard entity payload is invalid");
        assert_eq!(
            paste(&wrap(r#"{"entities":[{"kind":1,"v":[1,0,2,3,0,1]}],"schema":"bigstar.entity-clipboard/1"}"#)).unwrap_err(),
            "Clipboard entity values are invalid"
        );
        assert_eq!(
            paste(&wrap(r#"{"entities":[{"kind":9,"v":[]}],"schema":"bigstar.entity-clipboard/1"}"#)).unwrap_err(),
            "Clipboard entity values are invalid"
        );
        let ok = paste(&wrap(r#"{"entities":[{"kind":1,"v":[1,0,2,3,1,1]},{"hash":"0000abcd","kind":2,"v":[326,4,5,1,2,4294967295]}],"schema":"bigstar.entity-clipboard/1"}"#)).unwrap();
        assert_eq!((ok.objects[0].x, ok.objects[0].y), (3, 4));
        assert_eq!((ok.sprites[0].value.x, ok.sprites[0].value.settings, ok.sprites[0].hash), (5, u32::MAX, Some(0xabcd)));
    }
}
