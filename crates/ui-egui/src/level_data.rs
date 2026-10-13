//! The Level Data tab: the glue block's flags and values, named by the modules that declare them, and the stage-object
//! registration the sprites imply. A port of `LevelDataWidget`.

use bigstar_formats::{GlueValue, fnv1a_32};
use bigstar_level::{LevelDataKind, ModuleDataCatalog};
use bigstar_protocol::{LevelEdit, LevelSnapshot};
use egui::{Color32, RichText, Ui};

/// The registration lives in the glue block too, but it is derived from the sprites; it is listed, not edited.
const REGISTRATION_HASH: u32 = fnv1a_32("glue.stageObjects");
const EDIT_NAME: &str = "Edit level data";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRow {
    pub hash: u32,
    pub key: String,
    pub flag: bool,
    /// False for a key a module declares that the level does not hold yet.
    pub present: bool,
    pub payload: Vec<u8>,
}

fn key_name(hash: u32, modules: Option<&ModuleDataCatalog>) -> String {
    modules.and_then(|m| m.level_data_by_hash(hash)).map_or_else(|| format!("0x{hash:08x}"), |d| d.key.clone())
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
}

/// A non-empty, even number of hex digits, with any whitespace between them.
pub fn parse_hex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) {
        return None;
    }
    digits.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect()
}

/// The level's flags, then its values, then every key a module declares that the level does not hold.
pub fn rows(snapshot: &LevelSnapshot, modules: Option<&ModuleDataCatalog>) -> Vec<DataRow> {
    let glue = &snapshot.glue;
    let mut rows: Vec<DataRow> =
        glue.flags.iter().map(|&hash| DataRow { hash, key: key_name(hash, modules), flag: true, present: true, payload: Vec::new() }).collect();
    rows.extend(glue.values.iter().filter(|v| v.hash != REGISTRATION_HASH).map(|v| DataRow {
        hash: v.hash,
        key: key_name(v.hash, modules),
        flag: false,
        present: true,
        payload: v.payload.clone(),
    }));
    for declaration in modules.map(|m| m.level_data.as_slice()).unwrap_or_default() {
        if declaration.hash == REGISTRATION_HASH {
            continue;
        }
        let flag = declaration.kind == LevelDataKind::Flag;
        if !rows.iter().any(|row| row.hash == declaration.hash && row.flag == flag) {
            rows.push(DataRow { hash: declaration.hash, key: declaration.key.clone(), flag, present: false, payload: Vec::new() });
        }
    }
    rows
}

/// Turns a flag on or off.
pub fn set_flag(snapshot: &LevelSnapshot, hash: u32, on: bool) -> Option<LevelEdit> {
    let mut glue = snapshot.glue.clone();
    let found = glue.flags.iter().position(|&h| h == hash);
    match (on, found) {
        (true, None) => glue.flags.push(hash),
        (false, Some(at)) => {
            glue.flags.remove(at);
        }
        _ => return None,
    }
    Some(LevelEdit::SetGlue { glue, name: EDIT_NAME.to_owned() })
}

/// Sets a value's bytes from hex text, adding the value after the others when the level does not hold it yet.
pub fn set_value(snapshot: &LevelSnapshot, hash: u32, text: &str) -> Result<LevelEdit, String> {
    let bytes = parse_hex(text).ok_or("Values must contain a non-empty, even number of hexadecimal digits.")?;
    let mut glue = snapshot.glue.clone();
    match glue.values.iter_mut().find(|v| v.hash == hash) {
        Some(value) => value.payload = bytes,
        None => {
            let payload_order = glue.values.iter().map(|v| v.payload_order + 1).max().unwrap_or(0);
            glue.values.push(GlueValue { hash, payload: bytes, payload_order });
        }
    }
    Ok(LevelEdit::SetGlue { glue, name: EDIT_NAME.to_owned() })
}

/// The stage id each registered module sprite takes, and its name.
pub fn registration(snapshot: &LevelSnapshot, modules: Option<&ModuleDataCatalog>) -> Vec<(u32, String)> {
    snapshot
        .registration_hashes
        .iter()
        .enumerate()
        .map(|(i, &hash)| {
            let name = modules.and_then(|m| m.stage_object_by_hash(hash)).map_or_else(|| format!("0x{hash:08x} (undeclared)"), |o| o.name.clone());
            (u32::from(snapshot.stage_object_id_base) + i as u32, name)
        })
        .collect()
}

/// What the person did in the tab this frame.
pub enum Outcome {
    Edit(LevelEdit),
    Error(String),
}

/// The value cells being typed in, by hash, until Enter or leaving the cell writes them.
pub type Drafts = std::collections::HashMap<u32, String>;

pub fn show(ui: &mut Ui, snapshot: &LevelSnapshot, modules: Option<&ModuleDataCatalog>, drafts: &mut Drafts, busy: bool) -> Option<Outcome> {
    let mut outcome = None;
    let read_only = snapshot.glue_error.is_some() || busy;
    if let Some(error) = &snapshot.glue_error {
        ui.colored_label(
            Color32::from_rgb(210, 70, 60),
            format!("Malformed glue data: {error}\nThis block is read-only and will be preserved byte-for-byte."),
        );
    }
    ui.label(RichText::new("Level data").strong());
    let rows = rows(snapshot, modules);
    if rows.is_empty() {
        ui.weak("This level holds no level data, and no module declares any.");
    }
    egui::Grid::new("level-data").num_columns(3).striped(true).show(ui, |ui| {
        ui.weak("Key");
        ui.weak("Shape");
        ui.weak("Value / raw bytes");
        ui.end_row();
        for row in &rows {
            ui.label(&row.key).on_hover_text(format!("0x{:08x}", row.hash));
            ui.label(if row.flag { "flag" } else { "value" });
            if row.flag {
                let mut on = row.present;
                if ui.add_enabled(!read_only, egui::Checkbox::without_text(&mut on)).changed() {
                    outcome = set_flag(snapshot, row.hash, on).map(Outcome::Edit);
                }
            } else {
                let shown = if row.present { hex(&row.payload) } else { String::new() };
                let draft = drafts.entry(row.hash).or_insert_with(|| shown.clone());
                let hint = if row.present { "" } else { "absent — enter bytes to add" };
                let edit = ui
                    .add_enabled(!read_only, egui::TextEdit::singleline(draft).hint_text(hint).font(egui::TextStyle::Monospace).desired_width(220.0));
                if edit.lost_focus() && *draft != shown {
                    outcome = Some(match set_value(snapshot, row.hash, draft) {
                        Ok(edit) => Outcome::Edit(edit),
                        Err(message) => Outcome::Error(message),
                    });
                    drafts.remove(&row.hash);
                } else if !edit.has_focus() && *draft != shown {
                    *draft = shown;
                }
            }
            ui.end_row();
        }
    });
    ui.add_space(8.0);
    ui.label(RichText::new("Derived registration (read-only)").strong());
    let registered = registration(snapshot, modules);
    if registered.is_empty() {
        ui.weak("No module sprites are placed in this level.");
    }
    egui::Grid::new("registration").num_columns(2).striped(true).show(ui, |ui| {
        for (id, name) in registered {
            ui.label(id.to_string());
            ui.label(name);
            ui.end_row();
        }
    });
    outcome
}

#[cfg(test)]
mod tests {
    use bigstar_formats::GlueBlock;
    use bigstar_level::{LevelDataDeclaration, StageObjectDeclaration};

    use super::*;

    fn snapshot(glue: GlueBlock) -> LevelSnapshot {
        let mut snapshot = crate::level_view::tests_support::snapshot();
        snapshot.glue = glue;
        snapshot
    }

    #[test]
    fn rows_name_what_modules_declare_and_offer_what_is_missing() {
        let declared = |key: &str, kind| LevelDataDeclaration { key: key.to_owned(), kind, hash: fnv1a_32(key), ..Default::default() };
        let modules = ModuleDataCatalog {
            level_data: vec![declared("coop.lives", LevelDataKind::Value), declared("coop.hard", LevelDataKind::Flag)],
            stage_objects: vec![StageObjectDeclaration { name: "Signboard".to_owned(), hash: 7, ..Default::default() }],
            ..Default::default()
        };
        let glue = GlueBlock {
            flags: vec![0x1234],
            values: vec![
                GlueValue { hash: fnv1a_32("coop.lives"), payload: vec![5], payload_order: 0 },
                GlueValue { hash: REGISTRATION_HASH, payload: vec![1, 2, 3, 4], payload_order: 1 },
            ],
        };
        let mut level = snapshot(glue);
        let listed = rows(&level, Some(&modules));
        let keys: Vec<(&str, bool, bool)> = listed.iter().map(|r| (r.key.as_str(), r.flag, r.present)).collect();
        assert_eq!(keys, [("0x00001234", true, true), ("coop.lives", false, true), ("coop.hard", true, false)]);
        assert_eq!(hex(&listed[1].payload), "05");

        level.registration_hashes = vec![7, 9];
        assert_eq!(registration(&level, Some(&modules)), [(326, "Signboard".to_owned()), (327, "0x00000009 (undeclared)".to_owned())]);
    }

    #[test]
    fn flags_and_values_are_one_named_edit_each() {
        let level = snapshot(GlueBlock { flags: vec![1], values: vec![GlueValue { hash: 2, payload: vec![0], payload_order: 3 }] });
        let Some(LevelEdit::SetGlue { glue, name }) = set_flag(&level, 1, false) else { panic!() };
        assert_eq!((glue.flags.len(), name.as_str()), (0, "Edit level data"));
        assert_eq!(set_flag(&level, 1, true), None, "nothing to change");
        let Ok(LevelEdit::SetGlue { glue, .. }) = set_value(&level, 2, "de ad") else { panic!() };
        assert_eq!(glue.values[0].payload, [0xde, 0xad]);
        // A value the level lacks goes after the others.
        let Ok(LevelEdit::SetGlue { glue, .. }) = set_value(&level, 9, "01") else { panic!() };
        assert_eq!((glue.values[1].hash, glue.values[1].payload_order), (9, 4));
        for bad in ["", "abc", "zz"] {
            assert_eq!(set_value(&level, 2, bad).unwrap_err(), "Values must contain a non-empty, even number of hexadecimal digits.");
        }
    }
}
