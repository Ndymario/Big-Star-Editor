//! The Filesystem dock: the ROM as NitroFS orders it, with what each module changes, and the actions that write into
//! a module. A port of `FilesystemDock`; the rows come from the host's filesystem view.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashSet};

use bigstar_protocol::{FileEntry, FileIcon, FileStatus, FilesystemListing};
use egui::{Color32, RichText, Ui};

use crate::editors::{EditorKind, ROM_USER_METADATA_ID, classify, editor_name};
use crate::services::Services;
use crate::session::{FileRead, Session};

const WARNING: Color32 = Color32::from_rgb(210, 140, 40);
const DANGER: Color32 = Color32::from_rgb(210, 70, 60);

/// Natural, case-insensitive order: `w2map` before `w10map`, and `bomthrow.narc` beside `Dat_2D.narc`.
pub fn compare_names(left: &str, right: &str) -> Ordering {
    let (l, r): (Vec<char>, Vec<char>) = (left.chars().collect(), right.chars().collect());
    let (mut i, mut j) = (0, 0);
    while i < l.len() && j < r.len() {
        let (a, b) = (l.get(i).copied().unwrap_or_default(), r.get(j).copied().unwrap_or_default());
        if a.is_ascii_digit() && b.is_ascii_digit() {
            let start = (i, j);
            while l.get(i).is_some_and(char::is_ascii_digit) {
                i += 1;
            }
            while r.get(j).is_some_and(char::is_ascii_digit) {
                j += 1;
            }
            let trim = |digits: &[char]| -> String {
                let text: String = digits.iter().collect();
                let trimmed = text.trim_start_matches('0');
                if trimmed.is_empty() { "0".to_owned() } else { trimmed.to_owned() }
            };
            let (x, y) = (trim(l.get(start.0..i).unwrap_or_default()), trim(r.get(start.1..j).unwrap_or_default()));
            match x.len().cmp(&y.len()).then_with(|| x.cmp(&y)) {
                Ordering::Equal => continue,
                other => return other,
            }
        }
        let (a, b) = (a.to_lowercase().next().unwrap_or(a), b.to_lowercase().next().unwrap_or(b));
        if a != b {
            return a.cmp(&b);
        }
        i += 1;
        j += 1;
    }
    (l.len() - i).cmp(&(r.len() - j)).then_with(|| left.cmp(right))
}

/// A ROM address as tree segments: `a/b.narc!c/d` is a, b.narc, c, d, keyed by the address up to each one.
pub fn split_address(path: &str) -> Vec<(String, String)> {
    let (outside, inside) = match path.split_once('!') {
        Some((outside, inside)) => (outside, Some(inside)),
        None => (path, None),
    };
    let mut segments = Vec::new();
    let mut key = String::new();
    for part in outside.split('/') {
        if !key.is_empty() {
            key.push('/');
        }
        key.push_str(part);
        segments.push((part.to_owned(), key.clone()));
    }
    if let Some(inside) = inside {
        for (index, part) in inside.split('/').enumerate() {
            key.push(if index == 0 { '!' } else { '/' });
            key.push_str(part);
            segments.push((part.to_owned(), key.clone()));
        }
    }
    segments
}

/// One row of the tree: a folder, a file, or an archive that is both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub name: String,
    pub key: String,
    /// The entry this row is a file for.
    pub entry: Option<usize>,
    pub children: Vec<Node>,
}

/// The tree NitroFS order gives: every folder owns a contiguous id range, so it sorts by the lowest id it holds (which
/// puts 00DUMMY above ARCHIVE); members sort by their archive's own ids; names break ties, naturally.
pub fn build_tree(entries: &[FileEntry]) -> Node {
    let addresses: Vec<Vec<(String, String)>> = entries.iter().map(|e| split_address(&e.path)).collect();
    let mut lowest: BTreeMap<String, i64> = BTreeMap::new();
    for (entry, segments) in entries.iter().zip(&addresses) {
        let mut note = |key: &String, id: i64| {
            let slot = lowest.entry(key.clone()).or_insert(id);
            *slot = (*slot).min(id);
        };
        if let Some(id) = entry.id {
            segments.iter().for_each(|(_, key)| note(key, id));
        }
        if let Some(id) = entry.member_id {
            let outside = entry.path.split('!').next().map_or(0, |o| o.split('/').count());
            segments.iter().skip(outside).for_each(|(_, key)| note(key, id));
        }
    }
    let mut order: Vec<usize> = (0..entries.len()).collect();
    let rank = |index: usize| -> Vec<(i64, &str)> {
        addresses.get(index).map_or_else(Vec::new, |segments| {
            segments.iter().map(|(name, key)| (lowest.get(key).copied().unwrap_or(i64::MAX), name.as_str())).collect()
        })
    };
    order.sort_by(|&a, &b| {
        let (x, y) = (rank(a), rank(b));
        for ((xi, xn), (yi, yn)) in x.iter().zip(&y) {
            let step = xi.cmp(yi).then_with(|| compare_names(xn, yn));
            if step != Ordering::Equal {
                return step;
            }
        }
        x.len().cmp(&y.len())
    });
    let mut root = Node { name: "root".to_owned(), key: String::new(), entry: None, children: Vec::new() };
    for index in order {
        let Some(segments) = addresses.get(index) else { continue };
        let mut node = &mut root;
        for (depth, (name, key)) in segments.iter().enumerate() {
            let position = match node.children.iter().position(|child| &child.key == key) {
                Some(position) => position,
                None => {
                    node.children.push(Node { name: name.clone(), key: key.clone(), entry: None, children: Vec::new() });
                    node.children.len() - 1
                }
            };
            let Some(child) = node.children.get_mut(position) else { break };
            if depth + 1 == segments.len() {
                child.entry = Some(index);
            }
            node = child;
        }
    }
    root
}

pub fn status_text(entry: &FileEntry) -> String {
    match entry.status {
        FileStatus::Pending if entry.id.is_some() && !entry.id_is_planned => "⏳ Pending rebuild".to_owned(),
        FileStatus::Pending => "⏳ Pending insertion".to_owned(),
        FileStatus::Orphaned => "Orphaned — reverting".to_owned(),
        FileStatus::Vanilla => "Vanilla".to_owned(),
        FileStatus::Replaced => "Replaced".to_owned(),
        FileStatus::Added => "Added".to_owned(),
    }
}

/// A member's id belongs to its archive's name table; a planned id is what the next build would assign.
pub fn id_text(entry: &FileEntry) -> String {
    match (entry.member_id, entry.id) {
        (Some(member), _) => member.to_string(),
        (None, None) => "—".to_owned(),
        (None, Some(id)) if entry.id_is_planned => format!("{id}?"),
        (None, Some(id)) => id.to_string(),
    }
}

pub fn provenance(entry: &FileEntry) -> String {
    match (entry.module.is_empty(), entry.component.is_empty()) {
        (true, _) if entry.from_module => "project".to_owned(),
        (true, _) => String::new(),
        (false, true) => entry.module.clone(),
        (false, false) => format!("{} / {}", entry.module, entry.component),
    }
}

/// Everything the hidden columns would say about a file, on one line.
pub fn details(entry: &FileEntry, editor: Option<EditorKind>) -> String {
    let id = match (entry.member_id, entry.id) {
        (Some(member), _) => format!("{member} (inside {})", entry.path.split('!').next().unwrap_or_default()),
        (None, None) => "unassigned".to_owned(),
        (None, Some(id)) if entry.id_is_planned => format!("{id} (planned — assigned by the next build)"),
        (None, Some(id)) => id.to_string(),
    };
    let mut fields = vec![entry.path.clone(), format!("File ID: {id}"), format!("Size: {} bytes", entry.size)];
    if let Some(kind) = editor.filter(|k| *k != EditorKind::None) {
        fields.push(format!("Opens in: {}", editor_name(kind)));
    }
    let from = provenance(entry);
    if !from.is_empty() {
        fields.push(format!("From: {from}"));
    }
    if !entry.variant.is_empty() {
        fields.push(format!("Variant: {}", entry.variant));
    }
    fields.join("   ·   ")
}

fn glyph(icon: FileIcon) -> &'static str {
    match icon {
        FileIcon::File => "📄",
        FileIcon::Archive => "📦",
        FileIcon::Image => "🖼",
        FileIcon::Palette => "🎨",
        FileIcon::ScreenMap => "🗺",
        FileIcon::Model => "🧊",
        FileIcon::Animation => "🎞",
        FileIcon::Audio => "🎵",
        FileIcon::Level => "🎮",
        FileIcon::Text => "📝",
        FileIcon::Particle => "✨",
        FileIcon::TouchHitbox => "👆",
        FileIcon::OamPositions => "📐",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Column {
    Status,
    Provenance,
    Variant,
    FileId,
    Size,
}

const COLUMNS: [(Column, &str); 5] = [
    (Column::Status, "Status"),
    (Column::Provenance, "Provenance"),
    (Column::Variant, "Variant"),
    (Column::FileId, "File ID"),
    (Column::Size, "Size"),
];

/// What the person is about to do that needs an answer first.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Prompt {
    /// A file chosen to add, waiting for where in the ROM it goes.
    Add {
        name: String,
        bytes: Vec<u8>,
        path: String,
    },
    /// The path already exists in these variants of the module.
    Overwrite {
        path: String,
        bytes: Vec<u8>,
        holding: Vec<String>,
    },
    Remove {
        path: String,
    },
}

/// What the dock asks the workspace to open.
pub enum OpenRequest {
    Hex {
        path: String,
        bytes: Vec<u8>,
    },
    /// `metadata` when the file is NitroFS file 131, which the metadata page reads.
    Editor {
        path: String,
        bytes: Vec<u8>,
        kind: EditorKind,
        metadata: bool,
    },
}

pub struct FilesDock {
    pub selected: Option<String>,
    pub expanded: HashSet<String>,
    columns: Vec<Column>,
    /// Which editor each file read so far opens in.
    editors: BTreeMap<String, EditorKind>,
    prompt: Option<Prompt>,
    /// A file written once the variants already holding it have been asked.
    waiting_targets: Option<(String, Vec<u8>)>,
    /// Scroll the selected row into view next frame.
    reveal: bool,
}

impl Default for FilesDock {
    fn default() -> Self {
        Self {
            selected: None,
            expanded: HashSet::from(["".to_owned()]),
            columns: vec![Column::Status],
            editors: BTreeMap::new(),
            prompt: None,
            waiting_targets: None,
            reveal: false,
        }
    }
}

impl FilesDock {
    /// Selects a ROM path and opens everything that holds it. False when the listing has no such file.
    pub fn reveal(&mut self, listing: Option<&FilesystemListing>, path: &str) -> bool {
        if path.is_empty() || !listing.is_some_and(|l| l.entries.iter().any(|e| e.path == path)) {
            return false;
        }
        for (_, key) in split_address(path) {
            self.expanded.insert(key);
        }
        self.selected = Some(path.to_owned());
        self.reveal = true;
        true
    }

    fn selected_entry<'a>(&self, listing: &'a FilesystemListing) -> Option<&'a FileEntry> {
        let path = self.selected.as_ref()?;
        listing.entries.iter().find(|e| &e.path == path)
    }

    /// Takes in file reads and answers the dock asked for, and returns what is to be opened.
    pub fn follow(&mut self, session: &mut Session, services: &dyn Services) -> Vec<OpenRequest> {
        let ids: BTreeMap<String, Option<i64>> =
            session.filesystem.as_ref().map(|l| l.entries.iter().map(|e| (e.path.clone(), e.id)).collect()).unwrap_or_default();
        let metadata = |path: &str| ids.get(path).copied().flatten() == Some(ROM_USER_METADATA_ID);
        for (path, bytes) in session.take_file_reads(FileRead::Classify) {
            let kind = classify(&bytes, &path, metadata(&path));
            self.editors.insert(path, kind);
        }
        let mut opened = Vec::new();
        for (path, bytes) in session.take_file_reads(FileRead::Open) {
            let metadata = metadata(&path);
            let kind = classify(&bytes, &path, metadata);
            self.editors.insert(path.clone(), kind);
            opened.push(if kind == EditorKind::None {
                OpenRequest::Hex { path, bytes }
            } else {
                OpenRequest::Editor { path, bytes, kind, metadata }
            });
        }
        for (path, bytes) in session.take_file_reads(FileRead::Hex) {
            opened.push(OpenRequest::Hex { path, bytes });
        }
        for (path, bytes) in session.take_file_reads(FileRead::Extract) {
            let name = path.rsplit(['/', '!']).next().unwrap_or(&path).to_owned();
            if let Err(error) = services.save_file("Extract file", &name, "", &bytes) {
                session.report(error);
            }
        }
        if let Some((path, holding)) = session.file_targets.take() {
            match self.waiting_targets.take() {
                Some((waiting, bytes)) if waiting == path && !holding.is_empty() => self.prompt = Some(Prompt::Overwrite { path, bytes, holding }),
                Some((waiting, bytes)) if waiting == path => session.write_file(&path, bytes),
                other => self.waiting_targets = other,
            }
        }
        opened
    }

    pub fn show(&mut self, ui: &mut Ui, session: &mut Session, services: &dyn Services) {
        session.refresh_filesystem_once();
        let Some(listing) = session.filesystem.clone() else {
            ui.spinner();
            return;
        };
        ui.horizontal(|ui| {
            // Where add and replace write, not what the tree shows.
            ui.label("Save to module");
            let current = listing.modules.iter().find(|m| m.key == listing.active_module).map_or("(none)".to_owned(), |m| m.name.clone());
            egui::ComboBox::from_id_salt("active-module").selected_text(current).show_ui(ui, |ui| {
                for module in &listing.modules {
                    if ui.selectable_label(module.key == listing.active_module, &module.name).clicked() {
                        session.set_active_module(&module.key);
                    }
                }
            });
            ui.menu_button("Columns", |ui| {
                for (column, name) in COLUMNS {
                    let mut shown = self.columns.contains(&column);
                    if ui.checkbox(&mut shown, name).changed() {
                        if shown {
                            self.columns.push(column);
                            self.columns.sort_by_key(|c| COLUMNS.iter().position(|(k, _)| k == c));
                        } else {
                            self.columns.retain(|c| *c != column);
                        }
                    }
                }
            });
        });
        if listing.pending {
            ui.colored_label(WARNING, "⏳ Module files differ from the last build. Run NCPatcher to assign IDs and rebuild the ROM.");
        }
        if let Some(error) = &listing.error {
            ui.weak(format!("NCPatcher: {error}"));
        }
        ui.separator();
        let tree = build_tree(&listing.entries);
        let footer = 96.0;
        egui::ScrollArea::both().max_height((ui.available_height() - footer).max(80.0)).auto_shrink([false, false]).show(ui, |ui| {
            egui::Grid::new("filesystem-tree").num_columns(1 + self.columns.len()).striped(true).spacing([12.0, 2.0]).show(ui, |ui| {
                ui.label(RichText::new("Path").weak());
                for (column, name) in COLUMNS {
                    if self.columns.contains(&column) {
                        ui.label(RichText::new(name).weak());
                    }
                }
                ui.end_row();
                self.rows(ui, session, &listing, &tree, 0);
            });
        });
        ui.separator();
        let entry = self.selected_entry(&listing).cloned();
        let editor = entry.as_ref().and_then(|e| self.editors.get(&e.path).copied());
        match &entry {
            Some(entry) => {
                ui.add(egui::Label::new(details(entry, editor)).wrap());
            }
            None => {
                ui.weak("No file selected.");
            }
        }
        ui.horizontal_wrapped(|ui| {
            let has_target = !listing.modules.is_empty();
            if ui.add_enabled(has_target, egui::Button::new("Add file")).clicked()
                && let Some(chosen) = services.open_file("Add file to module", None)
            {
                match chosen {
                    Ok((name, bytes)) => self.prompt = Some(Prompt::Add { path: format!("z_new/{name}"), name, bytes }),
                    Err(error) => session.report(error),
                }
            }
            let selected = entry.is_some();
            if ui.add_enabled(selected, egui::Button::new("Replace")).clicked()
                && let (Some(entry), Some(chosen)) = (&entry, services.open_file("Replacement file", None))
            {
                match chosen {
                    Ok((_, bytes)) => session.write_file(&entry.path, bytes),
                    Err(error) => session.report(error),
                }
            }
            // A member cannot be removed: dropping one renumbers every member after it, and the game addresses them by
            // number.
            let removable = entry.as_ref().is_some_and(|e| e.from_module && !e.module.is_empty() && !e.inside_archive());
            if ui.add_enabled(removable, egui::Button::new("Remove")).clicked()
                && let Some(entry) = &entry
            {
                self.prompt = Some(Prompt::Remove { path: entry.path.clone() });
            }
            if ui.add_enabled(selected, egui::Button::new("Extract")).clicked()
                && let Some(entry) = &entry
            {
                session.read_file(&entry.path, FileRead::Extract);
            }
            let opens = editor.is_some_and(|k| k != EditorKind::None);
            if ui.add_enabled(opens, egui::Button::new("Open")).clicked()
                && let Some(entry) = &entry
            {
                session.read_file(&entry.path, FileRead::Open);
            }
            if ui.add_enabled(selected, egui::Button::new("Hex edit")).clicked()
                && let Some(entry) = &entry
            {
                session.read_file(&entry.path, FileRead::Hex);
            }
        });
        self.prompt_window(ui.ctx(), session, &listing);
    }

    fn rows(&mut self, ui: &mut Ui, session: &mut Session, listing: &FilesystemListing, node: &Node, depth: usize) {
        let expandable = !node.children.is_empty()
            || node.entry.and_then(|i| listing.entries.get(i)).is_some_and(|e| e.icon == FileIcon::Archive && !e.inside_archive());
        let open = self.expanded.contains(&node.key);
        let entry = node.entry.and_then(|i| listing.entries.get(i));
        ui.horizontal(|ui| {
            ui.add_space(depth as f32 * 14.0);
            if expandable {
                if ui.add(egui::Button::new(if open { "⏷" } else { "⏵" }).small().frame(false)).clicked() {
                    self.toggle(session, node, entry);
                }
            } else {
                ui.add_space(16.0);
            }
            let icon = entry.map_or("📁", |e| glyph(e.icon));
            let chosen = entry.is_some() && self.selected.as_deref() == entry.map(|e| e.path.as_str());
            let response = ui.selectable_label(chosen, format!("{icon} {}", node.name));
            if chosen && std::mem::take(&mut self.reveal) {
                response.scroll_to_me(Some(egui::Align::Center));
            }
            if response.clicked()
                && let Some(entry) = entry
            {
                self.selected = Some(entry.path.clone());
                if !self.editors.contains_key(&entry.path) {
                    session.read_file(&entry.path, FileRead::Classify);
                }
            }
            if response.double_clicked() {
                match entry {
                    // Double-clicking a file opens it in whichever editor understands it, as NSMB Editor does.
                    Some(entry) => session.read_file(&entry.path, FileRead::Open),
                    None => self.toggle(session, node, entry),
                }
            }
        });
        for (column, _) in COLUMNS {
            if !self.columns.contains(&column) {
                continue;
            }
            let Some(entry) = entry else {
                ui.label("");
                continue;
            };
            match column {
                Column::Status => {
                    let color = match entry.status {
                        FileStatus::Pending => Some(WARNING),
                        FileStatus::Orphaned => Some(DANGER),
                        _ => None,
                    };
                    let text = RichText::new(status_text(entry));
                    ui.label(color.map_or(text.clone(), |c| text.color(c)));
                }
                Column::Provenance => {
                    ui.label(provenance(entry));
                }
                Column::Variant => {
                    ui.label(&entry.variant);
                }
                Column::FileId => {
                    let response = ui.label(id_text(entry));
                    if entry.inside_archive() {
                        let container = entry.path.split('!').next().unwrap_or_default();
                        response.on_hover_text(format!("Its file ID inside {container}, from that archive's own name table — not a NitroFS ID."));
                    } else if entry.id_is_planned {
                        response.on_hover_text("The ID the next build would assign. Nothing holds it yet.");
                    }
                }
                Column::Size => {
                    ui.label(entry.size.to_string());
                }
            }
        }
        ui.end_row();
        if open {
            for child in &node.children {
                self.rows(ui, session, listing, child, depth + 1);
            }
        }
    }

    fn toggle(&mut self, session: &mut Session, node: &Node, entry: Option<&FileEntry>) {
        if !self.expanded.remove(&node.key) {
            self.expanded.insert(node.key.clone());
            // An archive's other members are listed the first time it is opened.
            if let Some(entry) = entry.filter(|e| e.icon == FileIcon::Archive && !e.inside_archive()) {
                session.list_archive(&entry.path);
            }
        }
    }

    fn prompt_window(&mut self, ctx: &egui::Context, session: &mut Session, listing: &FilesystemListing) {
        let Some(prompt) = &mut self.prompt else { return };
        let mut close = false;
        let title = match prompt {
            Prompt::Add { .. } => "ROM destination",
            Prompt::Overwrite { .. } => "Replace existing module file",
            Prompt::Remove { .. } => "Remove module file",
        };
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).show(ctx, |ui| match prompt {
            Prompt::Add { name, bytes, path } => {
                ui.label(format!("Where in the ROM {name} goes. New files must be under z_new/."));
                ui.text_edit_singleline(path);
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        let target = path.trim().to_owned();
                        if !listing.entries.iter().any(|e| e.path == target) && !target.starts_with("z_new/") {
                            session.report("New NitroFS files must be under z_new/.".to_owned());
                        } else {
                            // The variants already holding the path are asked first, so the person can be warned.
                            session.ask_file_targets(&target);
                            self.waiting_targets = Some((target, std::mem::take(bytes)));
                            close = true;
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Prompt::Overwrite { path, bytes, holding } => {
                let names: Vec<&str> = holding.iter().map(|v| if v.is_empty() { "(base)" } else { v.as_str() }).collect();
                ui.label(format!("This path already exists in: {}. Replace the active variant?", names.join(", ")));
                ui.horizontal(|ui| {
                    if ui.button("Replace").clicked() {
                        session.write_file(path, std::mem::take(bytes));
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
            Prompt::Remove { path } => {
                ui.label(format!("Move this module file to the project's trash (.big-star-trash)?\n\n{path}"));
                ui.horizontal(|ui| {
                    if ui.button("Remove").clicked() {
                        session.remove_file(path);
                        close = true;
                    }
                    if ui.button("Cancel").clicked() {
                        close = true;
                    }
                });
            }
        });
        if close {
            self.prompt = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(path: &str, id: Option<i64>) -> FileEntry {
        FileEntry { path: path.to_owned(), id, ..FileEntry::default() }
    }

    #[test]
    fn names_sort_naturally_and_without_case() {
        let mut names = vec!["w10map", "w2map", "Dat_2D.narc", "bomthrow.narc", "a02", "a1"];
        names.sort_by(|a, b| compare_names(a, b));
        assert_eq!(names, ["a1", "a02", "bomthrow.narc", "Dat_2D.narc", "w2map", "w10map"]);
    }

    #[test]
    fn addresses_split_through_the_archive() {
        let segments = split_address("a/b.narc!c/d");
        assert_eq!(segments.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["a", "b.narc", "c", "d"]);
        assert_eq!(segments.iter().map(|(_, k)| k.as_str()).collect::<Vec<_>>(), ["a", "a/b.narc", "a/b.narc!c", "a/b.narc!c/d"]);
    }

    #[test]
    fn the_tree_follows_file_id_order_then_natural_names() {
        let entries = vec![
            entry("ARCHIVE/x.bin", Some(5)),
            entry("00DUMMY", Some(0)),
            entry("BG/b.bin", Some(3)),
            entry("BG/a.bin", Some(4)),
            entry("z_new/new2.bin", None),
            entry("z_new/new10.bin", None),
            FileEntry { path: "BG/a.bin!m1".to_owned(), member_id: Some(1), ..FileEntry::default() },
            FileEntry { path: "BG/a.bin!m0".to_owned(), member_id: Some(0), ..FileEntry::default() },
        ];
        let tree = build_tree(&entries);
        let names = |node: &Node| node.children.iter().map(|c| c.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&tree), ["00DUMMY", "BG", "ARCHIVE", "z_new"], "a folder sorts by the lowest id it holds");
        let bg = &tree.children[1];
        assert_eq!(names(bg), ["b.bin", "a.bin"]);
        assert_eq!(names(&bg.children[1]), ["m0", "m1"], "members by their archive's ids");
        assert_eq!(bg.children[1].entry, Some(3), "the archive is a file and a folder");
        assert_eq!(names(&tree.children[3]), ["new2.bin", "new10.bin"]);
    }

    #[test]
    fn rows_say_what_their_ids_and_statuses_are() {
        let planned = FileEntry { path: "z_new/a".into(), id: Some(9), id_is_planned: true, status: FileStatus::Pending, ..FileEntry::default() };
        assert_eq!((id_text(&planned), status_text(&planned)), ("9?".to_owned(), "⏳ Pending insertion".to_owned()));
        let rebuild = FileEntry { id: Some(9), status: FileStatus::Pending, ..FileEntry::default() };
        assert_eq!(status_text(&rebuild), "⏳ Pending rebuild");
        let member = FileEntry { path: "a.narc!b".into(), member_id: Some(2), size: 4, ..FileEntry::default() };
        assert_eq!(id_text(&member), "2");
        assert_eq!(
            details(&member, Some(EditorKind::Graphics)),
            "a.narc!b   ·   File ID: 2 (inside a.narc)   ·   Size: 4 bytes   ·   Opens in: Graphics"
        );
        let module = FileEntry { module: "coop".into(), component: "Vanilla".into(), ..FileEntry::default() };
        assert_eq!(provenance(&module), "coop / Vanilla");
    }
}
