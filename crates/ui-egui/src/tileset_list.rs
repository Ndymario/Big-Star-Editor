//! The Layers tab of the left panel: every tileset, background and foreground the game's tables hold, the ones the
//! project's `tilesets.yaml` adds, and the scenes the game builds outside a level. Edit (or a double-click) opens one in
//! the tileset workspace.

use bigstar_protocol::{EntryKey, GraphicsSubject, TilesetListEntry};
use egui::{RichText, Ui};

use crate::controls::{self, Button, Row, Width};
use crate::i18n::tr;
use crate::session::Session;

/// What the tab remembers between frames.
#[derive(Default)]
pub struct TilesetListState {
    pub filter: String,
}

/// An entry's name as the list shows it: a slot by its number, the two tilesets every level loads in the person's
/// language, and the rest as the list writes them.
fn label(entry: &TilesetListEntry) -> String {
    let name = match &entry.subject {
        GraphicsSubject::Tileset(EntryKey::Shared) => tr!("tileset_list.shared"),
        GraphicsSubject::Tileset(EntryKey::SubUnit) => tr!("tileset_list.sub_unit"),
        _ => entry.name.as_str(),
    };
    match entry.subject.table_id() {
        Some(id) => controls::numbered(i64::from(id), name, false),
        None => name.to_owned(),
    }
}

/// What a workspace on `subject` is called: a level by its title, an entry by its name in the list.
pub fn title(session: &Session, subject: &GraphicsSubject) -> String {
    if let GraphicsSubject::Level(level) = subject {
        return session.levels.get(level).map_or_else(String::new, |view| view.snapshot.title.clone());
    }
    let list = &session.tileset_list;
    let entries = list.tilesets.iter().chain(&list.backgrounds).chain(&list.foregrounds).chain(&list.scenes);
    entries.into_iter().find(|entry| entry.subject == *subject).map_or_else(String::new, label)
}

fn row(ui: &mut Ui, entry: &TilesetListEntry, open: &mut Option<GraphicsSubject>) {
    let label = label(entry);
    // Edit first, from the right, so the row takes what is left and its name is cut short to it.
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(entry.available, |ui| {
                if Button::new(tr!("level_list.edit")).small().quiet().show(ui).clicked() {
                    *open = Some(entry.subject.clone());
                }
                let text = if entry.available { label.clone() } else { tr!("tileset_list.empty_slot", name = label) };
                let response = Row::new(text).show(ui);
                if response.double_clicked() {
                    *open = Some(entry.subject.clone());
                }
            });
        });
    });
}

/// The list itself, filtered by what is typed above it.
pub fn show(ui: &mut Ui, session: &mut Session, state: &mut TilesetListState) {
    if let Some(problem) = &session.tileset_list.problem {
        controls::error_text(ui, problem);
    }
    controls::search(ui, &mut state.filter, tr!("tileset_list.filter"), Width::Fill);
    let filter = state.filter.trim().to_lowercase();
    let matches = |entry: &&TilesetListEntry| filter.is_empty() || label(entry).to_lowercase().contains(&filter);
    let list = &session.tileset_list;
    let groups = [
        ("tilesets", tr!("tileset_list.tilesets"), &list.tilesets),
        ("backgrounds", tr!("tileset_list.backgrounds"), &list.backgrounds),
        ("foregrounds", tr!("tileset_list.foregrounds"), &list.foregrounds),
        ("scenes", tr!("tileset_list.scenes"), &list.scenes),
    ];
    let mut open = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        // The rows end short of the scroll bar, which floats over the right edge where their Edit buttons are.
        let scroll = ui.spacing().scroll;
        ui.set_max_width(ui.available_width() - scroll.bar_width - scroll.bar_outer_margin);
        for (id, name, entries) in groups {
            let shown: Vec<&TilesetListEntry> = entries.iter().filter(matches).collect();
            if shown.is_empty() {
                continue;
            }
            // A filter opens every group it finds something in.
            let title = RichText::new(format!("{name} ({})", shown.len())).strong();
            let body = |ui: &mut Ui| shown.iter().for_each(|entry| row(ui, entry, &mut open));
            if filter.is_empty() {
                controls::group(ui, ("tileset-list", id), title, false, body);
            } else {
                controls::group(ui, ("tileset-list-filtered", id), title, true, body);
            }
        }
    });
    if let Some(subject) = open {
        session.request_tileset_resources(subject);
    }
}
