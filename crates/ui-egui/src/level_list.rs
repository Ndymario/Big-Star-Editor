//! The Levels tab of the left panel: the project's levels by world. Picking an area shows it in the Preview tab; Edit
//! (or a double-click) opens it as a document.

use bigstar_protocol::{AreaSummary, LevelGroup, LevelId};
use egui::{RichText, Ui};

use crate::controls::{self, Button, Row};
use crate::i18n::Language;
use crate::session::Session;

/// What the person asked of the list this frame.
#[derive(Default)]
struct Picked {
    /// An area to show in the Preview tab.
    preview: Option<String>,
    /// An area to open as a document.
    open: Option<String>,
    /// A file to show in the Filesystem dock.
    reveal: Option<String>,
}

fn area_row(ui: &mut Ui, label: &str, area: &AreaSummary, shown: Option<&str>, picked: &mut Picked) {
    // Edit first, from the right, so the row takes what is left and its name is cut short to it rather than holding the
    // panel open.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(area.available, |ui| {
                if Button::new("Edit").small().quiet().show(ui).clicked() {
                    picked.open = Some(area.token.clone());
                }
                let tip = format!(
                    "{label}\nToken: {}\nCourse: {}\nBGDat: {}",
                    area.token,
                    if area.course_path.is_empty() { "missing" } else { &area.course_path },
                    if area.bgdat_path.is_empty() { "missing" } else { &area.bgdat_path }
                );
                let response = Row::new(label).selected(shown == Some(area.token.as_str())).show(ui).on_hover_text(tip);
                if response.double_clicked() {
                    picked.open = Some(area.token.clone());
                } else if response.clicked() {
                    picked.preview = Some(area.token.clone());
                }
                controls::context_menu(&response, |ui| {
                    if Button::menu("Open").show(ui).clicked() {
                        picked.open = Some(area.token.clone());
                        ui.close();
                    }
                    for (name, path) in [("Show Course in Filesystem", &area.course_path), ("Show BGDat in Filesystem", &area.bgdat_path)] {
                        if Button::menu(name).enabled(!path.is_empty()).show(ui).clicked() {
                            picked.reveal = Some(path.clone());
                            ui.close();
                        }
                    }
                });
            });
        });
    });
}

fn group_ui(ui: &mut Ui, group: &LevelGroup, path: &str, shown: Option<&str>, language: Language, picked: &mut Picked) {
    for child in &group.groups {
        let id = format!("{path}>{}", child.name);
        // `${world}` is the list's word for "World", in the person's language.
        let name = child.name.replace("${world}", language.tr("World"));
        controls::group(ui, &id, RichText::new(name).strong(), false, |ui| {
            group_ui(ui, child, &id, shown, language, picked);
        });
    }
    for level in &group.levels {
        match level.areas.as_slice() {
            [only] => area_row(ui, &level.display_name, only, shown, picked),
            areas => {
                controls::group(ui, format!("{path}/{}", level.display_name), &level.display_name, true, |ui| {
                    for area in areas {
                        area_row(ui, &area.display_name, area, shown, picked);
                    }
                });
            }
        }
    }
}

/// The list itself; `previewed` is the level in the Preview tab, whose row is lit. A level of one area is one row.
/// Returns a file to show in the Filesystem dock, when asked for.
pub fn show(ui: &mut Ui, session: &mut Session, previewed: Option<LevelId>, language: Language) -> Option<String> {
    // A ROM edited directly is its own ROM, though an uploaded one has no path to give.
    if let Some(project) = &session.project
        && !project.mode.is_rom()
        && project.rom_path.is_none()
    {
        controls::error_text(ui, "This project has no ROM, so no level can be opened.");
    }
    if let Some(problem) = &session.level_list.problem {
        controls::error_text(ui, problem);
    }
    let shown = previewed.and_then(|level| session.levels.get(&level)).map(|view| view.snapshot.area.clone());
    let mut picked = Picked::default();
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // The rows end short of the scroll bar, which floats over the right edge where their Edit buttons are.
        let scroll = ui.spacing().scroll;
        ui.set_max_width(ui.available_width() - scroll.bar_width - scroll.bar_outer_margin);
        group_ui(ui, &session.level_list.root, "", shown.as_deref(), language, &mut picked)
    });
    if let Some(area) = picked.open {
        session.open_level(&area);
    } else if let Some(area) = picked.preview {
        session.preview_level(&area);
    }
    picked.reveal
}
