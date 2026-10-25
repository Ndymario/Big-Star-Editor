//! The level tools of the Level menu: the hex editor, `.nml` export and import, the object and sprite finders and the
//! sprite-event viewer. Ports of `MainWindow::editLevelHex`, `exportLevelPackage`, `importLevelPackage`, `findEntity`
//! and `showSpriteEvents`.

use crate::controls::{self, Button, TextField};
use crate::{dialogs, icons};
use bigstar_level::{LevelPackage, decode_nml, encode_nml};
use bigstar_protocol::{LevelBytes, LevelEdit, LevelId};
use egui::{RichText, Ui};

use crate::level_view::{Item, Kind};
use crate::services::Services;
use crate::session::{BytesFor, Session};
use crate::workspace::Workspace;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HexPage {
    #[default]
    Course,
    Bgdat,
}

pub struct HexEditor {
    pub level: LevelId,
    pub title: String,
    pub course: String,
    pub bgdat: String,
    pub page: HexPage,
}

pub struct Finder {
    pub sprites: bool,
    pub id: u16,
}

#[derive(Default)]
pub struct Tools {
    pub hex: Option<HexEditor>,
    pub finder: Option<Finder>,
    pub events: bool,
    /// ROM User Metadata was asked for before the filesystem listing that says which file it is had arrived.
    pub metadata: bool,
}

/// Bytes as hex, sixteen to a line.
pub fn hex_text(bytes: &[u8]) -> String {
    bytes.chunks(16).map(|line| line.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")).collect::<Vec<_>>().join("\n")
}

/// Hex digits, any whitespace between them; `None` for anything else or an odd count.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect()
}

/// The level as an `.nml`, carrying the ROM file ids when the files have them.
pub fn nml(bytes: LevelBytes) -> Result<Vec<u8>, String> {
    let package = LevelPackage {
        course_file_id: bytes.course_file_id.unwrap_or(0),
        bgdat_file_id: bytes.bgdat_file_id.unwrap_or(0),
        course: bytes.course,
        bgdat: bytes.bgdat,
    };
    encode_nml(&package).map_err(|e| e.message)
}

/// The edit importing an `.nml` makes.
pub fn import_edit(bytes: &[u8]) -> Result<LevelEdit, String> {
    let package = decode_nml(bytes).map_err(|e| e.message)?;
    Ok(LevelEdit::ReplaceSerialized { course: package.course, bgdat: package.bgdat, name: "Import level".to_owned() })
}

impl Tools {
    /// Takes the bytes a tool asked for, once they arrive.
    /// Opens NitroFS file 131, the ROM user metadata, once the listing says where it is.
    pub fn open_metadata(&mut self, session: &mut Session) {
        if !self.metadata {
            return;
        }
        let Some(listing) = &session.filesystem else { return };
        self.metadata = false;
        match listing.entries.iter().find(|entry| entry.id == Some(crate::editors::ROM_USER_METADATA_ID)).map(|entry| entry.path.clone()) {
            Some(path) => session.read_file(&path, crate::session::FileRead::Open),
            None => session.report("This ROM has no file 131, where its metadata is kept.".to_owned()),
        }
    }

    pub fn take_bytes(&mut self, session: &mut Session, services: &dyn Services) {
        let Some((level, purpose, bytes)) = session.level_bytes.take() else { return };
        let title = session.levels.get(&level).map_or_else(|| "level".to_owned(), |v| v.snapshot.title.clone());
        match purpose {
            BytesFor::HexEditor => {
                self.hex = Some(HexEditor { level, title, course: hex_text(&bytes.course), bgdat: hex_text(&bytes.bgdat), page: HexPage::Course });
            }
            BytesFor::Export => {
                let written = nml(bytes).and_then(|encoded| services.save_file("Export Level", &format!("{title}.nml"), "nml", &encoded));
                if let Err(error) = written {
                    session.report(error);
                }
            }
        }
    }

    /// Import Level: an `.nml` replaces the active level, as one undo step.
    pub fn import(&mut self, session: &mut Session, services: &dyn Services) {
        if session.active.is_none() {
            return;
        }
        match services.open_file("Import Level", Some("nml")) {
            Some(Ok((_, bytes))) => match import_edit(&bytes) {
                Ok(edit) => session.edit(edit),
                Err(error) => session.report(error),
            },
            Some(Err(error)) => session.report(error),
            None => {}
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, session: &mut Session, workspace: &mut Workspace) {
        self.hex_window(ctx, session);
        self.finder_window(ctx, session, workspace);
        self.events_window(ctx, session);
    }

    fn hex_window(&mut self, ctx: &egui::Context, session: &mut Session) {
        let Some(editor) = &mut self.hex else { return };
        let mut open = true;
        let mut done = false;
        dialogs::window(format!("Level Hex Editor — {}", editor.title)).open(&mut open).default_size([900.0, 600.0]).show(ctx, |ui| {
            controls::tabs(ui, "hex-page", &mut editor.page, &[(HexPage::Course, "Course"), (HexPage::Bgdat, "BGDat")]);
            let text = match editor.page {
                HexPage::Course => &mut editor.course,
                HexPage::Bgdat => &mut editor.bgdat,
            };
            egui::ScrollArea::both().max_height(ui.available_height() - 48.0).auto_shrink([false, false]).show(ui, |ui| {
                ui.add(TextField::multiline(text).code().lock_focus());
            });
            dialogs::actions(ui, |ui| {
                if Button::new("Save").primary().show(ui).clicked() {
                    match (unhex(&editor.course), unhex(&editor.bgdat)) {
                        (Some(course), Some(bgdat)) => {
                            session.edit_level(editor.level, LevelEdit::ReplaceSerialized { course, bgdat, name: "Hex edit level".to_owned() });
                            done = true;
                        }
                        _ => session.report("Hex data contains an invalid or incomplete byte.".to_owned()),
                    }
                }
                if Button::new("Cancel").show(ui).clicked() {
                    done = true;
                }
            });
        });
        if done || !open {
            self.hex = None;
        }
    }

    fn finder_window(&mut self, ctx: &egui::Context, session: &mut Session, workspace: &mut Workspace) {
        let Some(finder) = &mut self.finder else { return };
        let mut open = true;
        let title = if finder.sprites { "Sprite Finder" } else { "Object Finder" };
        dialogs::window(title).open(&mut open).default_size([640.0, 420.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(if finder.sprites { "Placement type" } else { "Object ID" });
                let maximum = if finder.sprites { u16::MAX } else { 4095 };
                ui.add(controls::number(&mut finder.id).range(0..=maximum));
                if Button::new("Find").with_icon(icons::SEARCH).primary().show(ui).clicked() {
                    session.find(finder.sprites, finder.id);
                }
            });
            ui.separator();
            let Some(found) = session.found.clone().filter(|f| f.sprites == finder.sprites) else {
                ui.weak("Searches every level in the project, including unsaved edits.");
                return;
            };
            if found.entities.is_empty() {
                ui.weak(format!("Nothing uses {} {}.", if found.sprites { "sprite type" } else { "object" }, found.id));
                return;
            }
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("found").num_columns(5).striped(true).show(ui, |ui| {
                    for heading in ["Level", "Index", "X", "Y", "Settings"] {
                        ui.label(RichText::new(heading).weak());
                    }
                    ui.end_row();
                    for entity in &found.entities {
                        // Picking a row opens its level and selects what was found.
                        if controls::link(ui, &entity.title).on_hover_text(&entity.area).clicked() {
                            let kind = if found.sprites { Kind::Sprite } else { Kind::Object };
                            workspace.to_select = Some((entity.area.clone(), kind, entity.index));
                            session.open_level(&entity.area);
                        }
                        ui.label(entity.index.to_string());
                        ui.label(entity.x.to_string());
                        ui.label(entity.y.to_string());
                        ui.label(entity.settings.map(|s| format!("0x{s:08x}")).unwrap_or_default());
                        ui.end_row();
                    }
                });
            });
        });
        if !open {
            self.finder = None;
            session.found = None;
        }
    }

    fn events_window(&mut self, ctx: &egui::Context, session: &mut Session) {
        if !self.events {
            return;
        }
        let mut open = true;
        dialogs::window("Sprite Event Viewer").open(&mut open).default_size([520.0, 380.0]).show(ctx, |ui| {
            let Some(view) = session.level_mut() else {
                ui.weak("Open an editable level first.");
                return;
            };
            ui.weak(format!("Events of {}", view.snapshot.title));
            let mut rows: Vec<(u8, usize, u16, String)> = Vec::new();
            for (index, sprite) in view.snapshot.course.sprites.iter().enumerate() {
                let name = view
                    .art
                    .actors
                    .actor(sprite.id, view.sprite_hash(index))
                    .map_or_else(|| format!("Actor {}", sprite.id), |a| a.definition.name.clone());
                rows.extend(sprite.events.iter().filter(|&&event| event != 0).map(|&event| (event, index, sprite.id, name.clone())));
            }
            rows.sort_by_key(|row| row.0);
            if rows.is_empty() {
                ui.weak("No sprite in this level names an event.");
                return;
            }
            let mut picked = None;
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("events").num_columns(4).striped(true).show(ui, |ui| {
                    for heading in ["Event", "Sprite", "Type", "Name"] {
                        ui.label(RichText::new(heading).weak());
                    }
                    ui.end_row();
                    for (event, index, sprite_type, name) in &rows {
                        ui.label(event.to_string());
                        if controls::link(ui, index.to_string()).clicked() {
                            picked = Some(*index);
                        }
                        ui.label(sprite_type.to_string());
                        ui.label(name);
                        ui.end_row();
                    }
                });
            });
            if let Some(index) = picked {
                view.selection = vec![Item::new(Kind::Sprite, index)];
            }
        });
        if !open {
            self.events = false;
        }
    }
}

/// The Tools menu: the editors and the ROM's own details.
pub fn menu(ui: &mut Ui, tools: &mut Tools, session: &mut Session, language: crate::i18n::Language) {
    let editing = session.active.is_some();
    let project = session.project.is_some();
    if Button::menu("Tilesets and Backgrounds…").enabled(editing).show(ui).clicked() {
        if let Some(level) = session.active {
            session.request_tileset_resources(level);
        }
        ui.close();
    }
    if Button::menu(format!("{}…", language.tr("Hex Editor"))).enabled(editing).show(ui).clicked() {
        session.request_level_bytes(BytesFor::HexEditor);
        ui.close();
    }
    ui.separator();
    if Button::menu("ROM Banner…").enabled(project).show(ui).clicked() {
        session.request_banner();
        ui.close();
    }
    if Button::menu("ROM User Metadata…").enabled(project).show(ui).clicked() {
        tools.metadata = true;
        session.refresh_filesystem_once();
        ui.close();
    }
}

/// The File menu's lines for the open level: writing it out to a file and reading one in over it.
pub fn level_file_items(ui: &mut Ui, tools: &mut Tools, session: &mut Session, services: &dyn Services) {
    let editing = session.active.is_some();
    if Button::menu("Import Level (.nml)…").enabled(editing).show(ui).clicked() {
        tools.import(session, services);
        ui.close();
    }
    if Button::menu("Export Level (.nml)…").enabled(editing).show(ui).clicked() {
        session.request_level_bytes(BytesFor::Export);
        ui.close();
    }
}

/// The Edit menu's lines for finding things across the levels and within the open one.
pub fn find_items(ui: &mut Ui, tools: &mut Tools, session: &mut Session) {
    let project = session.project.is_some();
    if Button::menu("Find Object…").enabled(project).show(ui).clicked() {
        tools.finder = Some(Finder { sprites: false, id: 0 });
        session.found = None;
        ui.close();
    }
    if Button::menu("Find Sprite…").enabled(project).show(ui).clicked() {
        tools.finder = Some(Finder { sprites: true, id: 0 });
        session.found = None;
        ui.close();
    }
    if Button::menu("Sprite Events…").enabled(session.active.is_some()).show(ui).clicked() {
        tools.events = true;
        ui.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_reads_back_what_it_writes_and_refuses_half_bytes() {
        let bytes: Vec<u8> = (0..40).collect();
        let text = hex_text(&bytes);
        assert_eq!(text.lines().count(), 3);
        assert!(text.starts_with("00 01 02"));
        assert_eq!(unhex(&text), Some(bytes));
        assert_eq!(unhex("0A 1b\\n\\tFF"), None, "escaped text is not hex");
        assert_eq!(unhex("0A 1b\n\tFF"), Some(vec![0x0a, 0x1b, 0xff]));
        assert_eq!(unhex("0A 1"), None);
        assert_eq!(unhex("0G"), None);
    }

    #[test]
    fn an_exported_level_imports_as_one_named_step() {
        let bytes = LevelBytes { course: vec![1, 2, 3], bgdat: vec![0xff, 0xff], course_file_id: Some(7), bgdat_file_id: None };
        let encoded = nml(bytes).unwrap();
        let Ok(LevelEdit::ReplaceSerialized { course, bgdat, name }) = import_edit(&encoded) else { panic!() };
        assert_eq!((course, bgdat, name.as_str()), (vec![1, 2, 3], vec![0xff, 0xff], "Import level"));
        assert!(import_edit(b"not a level").is_err());
    }
}
