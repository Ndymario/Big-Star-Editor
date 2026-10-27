//! The Level Data tab: the glue block's flags and values, named by the modules that declare them, and the stage-object
//! registration the sprites imply. A port of `LevelDataWidget`.

use crate::controls::{self, Button, TextField, Width};
use bigstar_formats::{GlueValue, fnv1a_32};
use bigstar_level::{LevelDataDeclaration, LevelDataEditor, LevelDataKind, ModuleDataCatalog};
use bigstar_protocol::{LevelEdit, LevelSnapshot};
use egui::{RichText, Ui};

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
    /// The mask's width in bytes, for a view mask that is edited on the views rather than as bytes.
    pub view_mask: Option<usize>,
}

/// The width in bytes of a key declared as a view mask, or `None` when it is anything else.
pub fn view_mask_width(declaration: &LevelDataDeclaration) -> Option<usize> {
    if declaration.editor != Some(LevelDataEditor::ViewMask) || declaration.kind != LevelDataKind::Value {
        return None;
    }
    match declaration.type_name.as_str() {
        "u8" => Some(1),
        "u16" => Some(2),
        "u32" => Some(4),
        _ => None,
    }
}

/// The view masks the modules declare, with their widths.
pub fn view_masks(modules: Option<&ModuleDataCatalog>) -> Vec<(&LevelDataDeclaration, usize)> {
    modules.map(|m| m.level_data.as_slice()).unwrap_or_default().iter().filter_map(|d| Some((d, view_mask_width(d)?))).collect()
}

/// The level's mask for a key, little-endian like every payload; an absent key has no bits set. `None` when the stored
/// payload is not the declared width, which is then only shown as bytes.
pub fn stored_mask(snapshot: &LevelSnapshot, hash: u32, width: usize) -> Option<u32> {
    match snapshot.glue.values.iter().find(|v| v.hash == hash) {
        None => Some(0),
        Some(value) if value.payload.len() == width => Some(value.payload.iter().rev().fold(0, |mask, &b| mask << 8 | u32::from(b))),
        Some(_) => None,
    }
}

/// The view ids the area holds, in order.
pub fn area_views(snapshot: &LevelSnapshot) -> Vec<u8> {
    let mut ids: Vec<u8> = snapshot.course.views.iter().map(|v| v.id).collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn mask_bytes(mask: u32, width: usize) -> Vec<u8> {
    mask.to_le_bytes().into_iter().take(width).collect()
}

/// Ids as runs, `1–7, 9`.
pub fn id_ranges(mask: u32) -> String {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for bit in (0..32).filter(|b| mask & (1 << b) != 0) {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == bit => *end = bit,
            _ => runs.push((bit, bit)),
        }
    }
    runs.iter().map(|&(start, end)| if start == end { start.to_string() } else { format!("{start}–{end}") }).collect::<Vec<_>>().join(", ")
}

/// Sets or clears the bits of `views` in a view mask, adding the key when the level does not hold it yet.
pub fn set_views(snapshot: &LevelSnapshot, hash: u32, width: usize, views: &[u8], on: bool) -> Option<LevelEdit> {
    let mask = stored_mask(snapshot, hash, width)?;
    let bits = views.iter().filter(|&&v| usize::from(v) < width * 8).fold(0u32, |bits, &v| bits | 1 << v);
    let next = if on { mask | bits } else { mask & !bits };
    let present = snapshot.glue.values.iter().any(|v| v.hash == hash);
    (next != mask || (on && !present)).then(|| set_payload(snapshot, hash, mask_bytes(next, width)))
}

/// The view masks as checkboxes for the selected views: on where every selected view is set, a dash where they
/// disagree. Shown in the properties of a view.
pub fn custom_view_data(ui: &mut Ui, snapshot: &LevelSnapshot, modules: Option<&ModuleDataCatalog>, views: &[u8], busy: bool) -> Option<LevelEdit> {
    let masks = view_masks(modules);
    if masks.is_empty() || views.is_empty() {
        return None;
    }
    let mut edit = None;
    controls::section(ui, "Custom data", |ui| {
        let read_only = snapshot.glue_error.is_some() || busy;
        for (declaration, width) in masks {
            let Some(mask) = stored_mask(snapshot, declaration.hash, width) else {
                ui.add_enabled(false, egui::Label::new(&declaration.key))
                    .on_disabled_hover_text("The level stores this key at a different size; edit its bytes in the Level Data tab.");
                continue;
            };
            let fits = views.iter().all(|&v| usize::from(v) < width * 8);
            let set = views.iter().filter(|&&v| usize::from(v) < width * 8 && mask & (1 << v) != 0).count();
            let mut on = set == views.len();
            let response = ui
                .add_enabled_ui(!read_only && fits, |ui| controls::checkbox_undecided(ui, &mut on, set != 0 && set != views.len(), &declaration.key))
                .inner;
            let response =
                if fits { response } else { response.on_disabled_hover_text(format!("A {}-bit mask has no bit for a view this high.", width * 8)) };
            if response.changed() {
                edit = set_views(snapshot, declaration.hash, width, views, on);
            }
        }
    });
    edit
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
    let width = |hash: u32| modules.and_then(|m| m.level_data_by_hash(hash)).and_then(view_mask_width);
    let mut rows: Vec<DataRow> = glue
        .flags
        .iter()
        .map(|&hash| DataRow { hash, key: key_name(hash, modules), flag: true, present: true, payload: Vec::new(), view_mask: None })
        .collect();
    rows.extend(glue.values.iter().filter(|v| v.hash != REGISTRATION_HASH).map(|v| DataRow {
        hash: v.hash,
        key: key_name(v.hash, modules),
        flag: false,
        present: true,
        payload: v.payload.clone(),
        view_mask: width(v.hash).filter(|&w| w == v.payload.len()),
    }));
    for declaration in modules.map(|m| m.level_data.as_slice()).unwrap_or_default() {
        if declaration.hash == REGISTRATION_HASH {
            continue;
        }
        let flag = declaration.kind == LevelDataKind::Flag;
        if !rows.iter().any(|row| row.hash == declaration.hash && row.flag == flag) {
            rows.push(DataRow {
                hash: declaration.hash,
                key: declaration.key.clone(),
                flag,
                present: false,
                payload: Vec::new(),
                view_mask: view_mask_width(declaration),
            });
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
    let bytes = parse_hex(text).ok_or("Enter an even number of hexadecimal digits, at least two.")?;
    Ok(set_payload(snapshot, hash, bytes))
}

fn set_payload(snapshot: &LevelSnapshot, hash: u32, bytes: Vec<u8>) -> LevelEdit {
    let mut glue = snapshot.glue.clone();
    match glue.values.iter_mut().find(|v| v.hash == hash) {
        Some(value) => value.payload = bytes,
        None => {
            let payload_order = glue.values.iter().map(|v| v.payload_order + 1).max().unwrap_or(0);
            glue.values.push(GlueValue { hash, payload: bytes, payload_order });
        }
    }
    LevelEdit::SetGlue { glue, name: EDIT_NAME.to_owned() }
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
        controls::error_text(ui, format!("This glue data cannot be read: {error}\nIt is read-only and is saved back exactly as it was."));
    }
    ui.label(RichText::new("Level data").strong());
    let rows = rows(snapshot, modules);
    if rows.is_empty() {
        ui.weak("This level has no level data, and no module defines any.");
    }
    egui::Grid::new("level-data").num_columns(3).striped(true).show(ui, |ui| {
        ui.weak("Key");
        ui.weak("Shape");
        ui.weak("Value / raw bytes");
        ui.end_row();
        for row in &rows {
            ui.label(&row.key).on_hover_text(format!("0x{:08x}", row.hash));
            ui.label(if row.flag {
                "flag"
            } else if row.view_mask.is_some() {
                "view mask"
            } else {
                "value"
            });
            if let Some(width) = row.view_mask {
                // Edited on the views themselves; here it is a summary, plus the bits no view of this area can show.
                let mask = if row.present { stored_mask(snapshot, row.hash, width).unwrap_or(0) } else { 0 };
                let area = area_views(snapshot).iter().filter(|&&v| usize::from(v) < width * 8).fold(0u32, |bits, &v| bits | 1 << v);
                ui.horizontal_wrapped(|ui| {
                    let set = mask & area;
                    ui.label(if set == 0 { "No views".to_owned() } else { format!("Views {}", id_ranges(set)) })
                        .on_hover_text("Set it on each view, under Custom data in its properties.");
                    let stray = mask & !area;
                    if stray != 0 {
                        ui.weak(format!("and {}, which this area lacks", id_ranges(stray)));
                        if ui.add(Button::new("Clear").small().enabled(!read_only)).on_hover_text("Clear the bits no view here has.").clicked() {
                            outcome = Some(Outcome::Edit(set_payload(snapshot, row.hash, mask_bytes(mask & area, width))));
                        }
                    }
                });
            } else if row.flag {
                let mut on = row.present;
                if ui.add_enabled_ui(!read_only, |ui| controls::checkbox(ui, &mut on, "")).inner.changed() {
                    outcome = set_flag(snapshot, row.hash, on).map(Outcome::Edit);
                }
            } else {
                let shown = if row.present { hex(&row.payload) } else { String::new() };
                let draft = drafts.entry(row.hash).or_insert_with(|| shown.clone());
                let hint = if row.present { "" } else { "absent — enter bytes to add" };
                let edit = ui.add_enabled(!read_only, TextField::new(draft).hint(hint).code().width(Width::Px(220.0)));
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
    fn view_masks_are_set_per_view_and_keep_bits_no_view_has() {
        use bigstar_formats::NsmbLevelView;
        let declared = |key: &str, type_name: &str, editor| LevelDataDeclaration {
            key: key.to_owned(),
            kind: LevelDataKind::Value,
            type_name: type_name.to_owned(),
            hash: fnv1a_32(key),
            editor,
            ..Default::default()
        };
        let mask = declared("coop.noLiquidReloadViews", "u16", Some(LevelDataEditor::ViewMask));
        let modules = ModuleDataCatalog {
            level_data: vec![mask.clone(), declared("coop.wide", "s32", Some(LevelDataEditor::ViewMask)), declared("coop.plain", "u32", None)],
            ..Default::default()
        };
        // Only an unsigned value of a width an editor can shift into is a mask.
        assert_eq!(view_masks(Some(&modules)).iter().map(|(d, w)| (d.key.as_str(), *w)).collect::<Vec<_>>(), [("coop.noLiquidReloadViews", 2)]);

        let mut level = snapshot(GlueBlock::default());
        level.course.views = [3, 0, 1].map(|id| NsmbLevelView { id, ..Default::default() }).to_vec();
        assert_eq!(area_views(&level), [0, 1, 3]);
        assert_eq!(stored_mask(&level, mask.hash, 2), Some(0), "an absent key has no bits set");
        assert_eq!(rows(&level, Some(&modules))[0].view_mask, Some(2));

        // Ticking a view adds the key; ticking it again changes nothing.
        let Some(LevelEdit::SetGlue { glue, .. }) = set_views(&level, mask.hash, 2, &[1, 3], true) else { panic!() };
        assert_eq!(glue.values[0].payload, [0x0a, 0x00]);
        level.glue = glue;
        assert_eq!(set_views(&level, mask.hash, 2, &[1], true), None);
        // A view too high for the mask is left alone rather than wrapping onto another bit.
        let Some(LevelEdit::SetGlue { glue, .. }) = set_views(&level, mask.hash, 2, &[3, 20], false) else { panic!() };
        assert_eq!(glue.values[0].payload, [0x02, 0x00]);

        // A payload of the wrong size is only bytes.
        level.glue.values[0].payload = vec![1, 2, 3, 4];
        assert_eq!(stored_mask(&level, mask.hash, 2), None);
        assert_eq!(rows(&level, Some(&modules))[0].view_mask, None);
        assert_eq!(set_views(&level, mask.hash, 2, &[0], true), None);

        assert_eq!(id_ranges(0xFFFF_FFFE), "1–31");
        assert_eq!(id_ranges(0b1011_0001), "0, 4–5, 7");
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
            assert_eq!(set_value(&level, 2, bad).unwrap_err(), "Enter an even number of hexadecimal digits, at least two.");
        }
    }
}
