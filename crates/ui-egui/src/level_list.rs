//! The Levels tab of the left panel: the project's levels by world. Picking an area shows it in the Preview tab; Edit
//! (or a double-click) opens it as a document.

use bigstar_protocol::{AreaSummary, LevelGroup, LevelId};
use egui::{RichText, Ui};

use crate::controls::{self, Button, Row};
use crate::i18n::{self, tr};
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

/// A name with each run of digits written `{1}`, `{2}` and so on in order, and the runs themselves: "W4 Tower" is
/// `W{1} Tower` and `["4"]`, so every world's tower is one text.
pub(crate) fn numbered(name: &str) -> (String, Vec<String>) {
    let mut template = String::new();
    let mut numbers: Vec<String> = Vec::new();
    let mut run = String::new();
    for c in name.chars().map(Some).chain([None]) {
        match c {
            Some(c) if c.is_ascii_digit() => run.push(c),
            other => {
                if !run.is_empty() {
                    numbers.push(std::mem::take(&mut run));
                    template.push_str(&format!("{{{}}}", numbers.len()));
                }
                template.extend(other);
            }
        }
    }
    (template, numbers)
}

/// The key a catalog name's translation is written under: `level_names.` and the name's [`numbered`] template in
/// lowercase, each run of other characters one `_` (`W4 Tower` is `level_names.w_1_tower`). The text under it is the
/// whole template, in the language, with its `{1}`, `{2}` where the numbers go. A name with no key (a project's own, or
/// a number like "1-1") is shown as it is.
pub(crate) fn name_key(template: &str) -> String {
    let mut key = String::from("level_names.");
    for word in template.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()) {
        if !key.ends_with('.') {
            key.push('_');
        }
        key.push_str(word);
    }
    key
}

/// A level, group or area's name as the list shows it, in the person's language: "World 4 (secret)", "W2 Tower" and
/// "Area 3" by their numbers, other catalog names by their keys. A project's own name is shown as written.
fn shown_name(name: &str) -> String {
    let (template, numbers) = numbered(name);
    let Some(text) = i18n::find(&name_key(&template)) else { return name.to_owned() };
    let names: Vec<String> = (1..=numbers.len()).map(|n| n.to_string()).collect();
    let args: Vec<(&str, &dyn std::fmt::Display)> =
        names.iter().zip(&numbers).map(|(name, number)| (name.as_str(), number as &dyn std::fmt::Display)).collect();
    i18n::format(text, &args)
}

fn area_row(ui: &mut Ui, label: &str, area: &AreaSummary, shown: Option<&str>, picked: &mut Picked) {
    // Edit first, from the right, so the row takes what is left and its name is cut short to it rather than holding the
    // panel open.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(area.available, |ui| {
                if Button::new(tr!("level_list.edit")).small().quiet().show(ui).clicked() {
                    picked.open = Some(area.token.clone());
                }
                let tip = tr!(
                    "level_list.area_tooltip",
                    label,
                    token = area.token,
                    course = if area.course_path.is_empty() { tr!("level_list.missing") } else { area.course_path.as_str() },
                    bgdat = if area.bgdat_path.is_empty() { tr!("level_list.missing") } else { area.bgdat_path.as_str() }
                );
                let response = Row::new(label).selected(shown == Some(area.token.as_str())).show(ui).on_hover_text(tip);
                if response.double_clicked() {
                    picked.open = Some(area.token.clone());
                } else if response.clicked() {
                    picked.preview = Some(area.token.clone());
                }
                controls::context_menu(&response, |ui| {
                    if Button::menu(tr!("common.open")).show(ui).clicked() {
                        picked.open = Some(area.token.clone());
                        ui.close();
                    }
                    for (name, path) in [
                        (tr!("level_list.show_course_in_filesystem"), &area.course_path),
                        (tr!("level_list.show_bgdat_in_filesystem"), &area.bgdat_path),
                    ] {
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

fn group_ui(ui: &mut Ui, group: &LevelGroup, path: &str, shown: Option<&str>, picked: &mut Picked) {
    for child in &group.groups {
        let id = format!("{path}>{}", child.name);
        controls::group(ui, &id, RichText::new(shown_name(&child.name)).strong(), false, |ui| {
            group_ui(ui, child, &id, shown, picked);
        });
    }
    for level in &group.levels {
        match level.areas.as_slice() {
            [only] => area_row(ui, &shown_name(&level.display_name), only, shown, picked),
            areas => {
                controls::group(ui, format!("{path}/{}", level.display_name), shown_name(&level.display_name), true, |ui| {
                    for area in areas {
                        area_row(ui, &shown_name(&area.display_name), area, shown, picked);
                    }
                });
            }
        }
    }
}

/// The list itself; `previewed` is the level in the Preview tab, whose row is lit. A level of one area is one row.
/// Returns a file to show in the Filesystem dock, when asked for.
pub fn show(ui: &mut Ui, session: &mut Session, previewed: Option<LevelId>) -> Option<String> {
    // A ROM edited directly is its own ROM, though an uploaded one has no path to give.
    if let Some(project) = &session.project
        && !project.mode.is_rom()
        && project.rom_path.is_none()
    {
        controls::error_text(ui, tr!("level_list.this_project_has_no_rom_so_no_level_can_be_opened"));
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
        group_ui(ui, &session.level_list.root, "", shown.as_deref(), &mut picked)
    });
    if let Some(area) = picked.open {
        session.open_level(&area);
    } else if let Some(area) = picked.preview {
        session.preview_level(&area);
    }
    picked.reveal
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{self, Language};

    #[test]
    fn names_are_keyed_by_their_words_with_the_numbers_left_out() {
        assert_eq!(numbered("W4 Tower"), ("W{1} Tower".to_string(), vec!["4".to_string()]));
        assert_eq!(numbered("Cannon (W1 to W5)").0, "Cannon (W{1} to W{2})");
        assert_eq!(numbered("Toad Houses").1.len(), 0);
        assert_eq!(name_key("W{1} Tower"), "level_names.w_1_tower");
        assert_eq!(name_key("World {1} (secret)"), "level_names.world_1_secret");
    }

    #[test]
    fn the_catalogs_names_read_in_the_current_language_and_a_projects_own_stay() {
        i18n::set(Language::from_code("pt"));
        assert_eq!(shown_name("W4 Tower"), "Torre M4");
        assert_eq!(shown_name("W6 Tower 2"), "Torre M6 2");
        assert_eq!(shown_name("Cannon (W3 to W6)"), "Canhão (M3 a M6)");
        assert_eq!(shown_name("World 4 (secret)"), "Mundo 4 (secreto)");
        assert_eq!(shown_name("World 12"), "Mundo 12");
        assert_eq!(shown_name("Area 2"), "Área 2");
        assert_eq!(shown_name("Toad Houses"), "Casas dos Toads");
        assert_eq!(shown_name("1-1"), "1-1");
        assert_eq!(shown_name("My Own Level 3"), "My Own Level 3");
        i18n::set(Language::ENGLISH);
        assert_eq!(shown_name("W4 Tower"), "W4 Tower");
        assert_eq!(shown_name("World 4 (secret)"), "World 4 (secret)");
    }
}
