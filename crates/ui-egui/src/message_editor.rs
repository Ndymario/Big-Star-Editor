//! The BMG message editor as a file document: the message list with its filter, the text of the chosen message with
//! its `[type:HEXDATA]` escapes, its MID1 ID and INF1 attribute bytes, and the line-based text export and import. A
//! port of `MessageEditorPanel`; the document is pure, and `show` draws it.

use bigstar_formats::{Bmg, BmgMessage, bmg_encoding_name, bmg_message_plain_text, bmg_message_text, parse_bmg, parse_bmg_message_text, write_bmg};
use egui::{RichText, Ui};

use crate::services::Services;

/// One message file being edited. `bytes` are the plain BMG bytes; the workspace takes any LZ wrapper off before and
/// puts it back after.
pub struct MessageDoc {
    pub path: String,
    pub bmg: Bmg,
    /// Why the file could not be read as a BMG, in which case nothing else here means anything.
    pub error: Option<String>,
    pub dirty: bool,
    /// The message the text, ID and attribute fields belong to.
    pub active: Option<usize>,
    pub text: String,
    pub id: u32,
    pub attributes: String,
    pub filter: String,
}

/// Bytes as unbroken upper-case hex, as the attribute field shows them.
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

/// The attribute field's bytes; spaces are ignored, anything else that is not a whole hex byte is refused.
fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| *b != b' ').collect();
    if !digits.len().is_multiple_of(2) || !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let nibble = |d: u8| char::from(d).to_digit(16).and_then(|v| u8::try_from(v).ok()).unwrap_or(0);
    Some(digits.as_chunks::<2>().0.iter().map(|&[high, low]| (nibble(high) << 4) | nibble(low)).collect())
}

/// The text export is line-based, so a message's own newlines and backslashes survive as escapes.
pub fn escape_line(text: &str) -> String {
    text.replace('\\', "\\\\").replace('\n', "\\n").replace('\r', "\\r")
}

pub fn unescape_line(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// What an import did, for the person to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Imported {
    pub updated: usize,
    pub appended: usize,
    pub skipped: usize,
}

impl MessageDoc {
    pub fn open(path: &str, bytes: &[u8]) -> Self {
        let (bmg, error) = match parse_bmg(bytes) {
            Ok(bmg) => (bmg, None),
            Err(error) => (Bmg::default(), Some(error.message)),
        };
        let mut doc = Self {
            path: path.to_owned(),
            bmg,
            error,
            dirty: false,
            active: None,
            text: String::new(),
            id: 0,
            attributes: String::new(),
            filter: String::new(),
        };
        if !doc.bmg.messages.is_empty() {
            doc.load(Some(0));
        }
        doc
    }

    /// Whether this editor reads a file: a BMG that parses.
    pub fn supports(bytes: &[u8]) -> bool {
        parse_bmg(bytes).is_ok()
    }

    pub fn info(&self) -> String {
        format!(
            "{} message(s) · {} · file ID {} · {} attribute byte(s)",
            self.bmg.messages.len(),
            bmg_encoding_name(self.bmg.encoding),
            self.bmg.file_id,
            self.bmg.attribute_size
        )
    }

    /// A row of the list: its index, its MID1 ID when the file has them, and its text without escapes.
    pub fn row(&self, index: usize) -> Option<(String, String, String)> {
        let message = self.bmg.messages.get(index)?;
        let preview = bmg_message_plain_text(message);
        Some((
            index.to_string(),
            message.id.map(|id| id.to_string()).unwrap_or_default(),
            if preview.is_empty() { "(empty)".to_owned() } else { preview },
        ))
    }

    /// Whether the filter lets a row through: its text contains the filter, ignoring case, or its ID or index is it.
    pub fn visible(&self, index: usize) -> bool {
        let needle = self.filter.trim();
        if needle.is_empty() {
            return true;
        }
        let Some((number, id, text)) = self.row(index) else { return false };
        text.to_lowercase().contains(&needle.to_lowercase()) || id == needle || number == needle
    }

    /// Writes the fields back into the message they were loaded from.
    pub fn commit(&mut self) {
        let Some(message) = self.active.and_then(|row| self.bmg.messages.get_mut(row)) else { return };
        let before = message.clone();
        message.parts = parse_bmg_message_text(&self.text);
        if self.bmg.has_mid1 {
            message.id = Some(self.id);
        }
        if self.bmg.attribute_size > 0
            && let Some(mut decoded) = unhex(&self.attributes)
        {
            decoded.resize(usize::from(self.bmg.attribute_size), 0);
            message.attributes = decoded;
        }
        if *message != before {
            self.dirty = true;
        }
    }

    /// Commits the fields and loads another message into them.
    pub fn select(&mut self, row: Option<usize>) {
        self.commit();
        self.load(row);
    }

    fn load(&mut self, row: Option<usize>) {
        self.active = row.filter(|row| *row < self.bmg.messages.len());
        match self.active.and_then(|row| self.bmg.messages.get(row).map(|m| (row, m))) {
            Some((row, message)) => {
                self.text = bmg_message_text(message);
                self.id = message.id.unwrap_or(u32::try_from(row).unwrap_or(u32::MAX));
                self.attributes = hex(&message.attributes);
            }
            None => {
                self.text.clear();
                self.attributes.clear();
            }
        }
    }

    pub fn stats(&self) -> String {
        if self.active.is_none() {
            return "No message selected.".to_owned();
        }
        let parts = parse_bmg_message_text(&self.text);
        let escapes = parts.iter().filter(|p| p.escape).count();
        let plain: String = parts.iter().filter(|p| !p.escape).map(|p| p.text.as_str()).collect();
        format!("{} character(s) · {} line(s) · {} escape sequence(s)", plain.chars().count(), plain.matches('\n').count() + 1, escapes)
    }

    /// Adds a blank message, or a copy of the selected one, at the end and selects it.
    pub fn add(&mut self, duplicate: bool) {
        self.commit();
        let mut message = match self.active.and_then(|row| self.bmg.messages.get(row)) {
            Some(active) if duplicate => active.clone(),
            _ => BmgMessage::default(),
        };
        message.attributes.resize(usize::from(self.bmg.attribute_size), 0);
        // A duplicate must not clone the ID: two entries answering to the same ID would make the game's lookup order
        // decide which text it shows.
        message.id = self.bmg.has_mid1.then(|| self.bmg.messages.iter().map(|m| m.id.unwrap_or(0).saturating_add(1)).max().unwrap_or(0));
        self.bmg.messages.push(message);
        self.dirty = true;
        self.load(Some(self.bmg.messages.len() - 1));
    }

    /// Removes the selected message; INF1 entries are addressed by index, so everything after it moves down one.
    pub fn remove(&mut self) {
        let Some(row) = self.active.filter(|row| *row < self.bmg.messages.len()) else { return };
        self.active = None;
        self.bmg.messages.remove(row);
        self.dirty = true;
        let next = row.min(self.bmg.messages.len().saturating_sub(1));
        self.load((!self.bmg.messages.is_empty()).then_some(next));
    }

    /// `[type:hexdata]` for an escape typed as `type:hexdata`, or why it is not one.
    pub fn escape(typed: &str) -> Result<String, String> {
        let candidate = format!("[{}]", typed.trim());
        let parts = parse_bmg_message_text(&candidate);
        match parts.as_slice() {
            [part] if part.escape => Ok(candidate),
            _ => Err("Escapes are a decimal type, a colon, and an even number of hexadecimal digits.".to_owned()),
        }
    }

    /// The messages as the NDS BMG Editor's text export.
    pub fn export_text(&mut self) -> String {
        self.commit();
        let mut out = String::new();
        out.push_str("# BMG Text Export\n");
        out.push_str(&format!("# Total messages: {}\n", self.bmg.messages.len()));
        out.push_str(&format!("# Encoding: {}\n", bmg_encoding_name(self.bmg.encoding)));
        out.push_str("# Format: [index] message text with [TYPE:HEXDATA] for escape sequences\n");
        out.push_str("# Line breaks inside a message are written as \\n, a literal backslash as \\\\\n");
        out.push_str(&format!("#{}\n\n", "=".repeat(70)));
        for (i, message) in self.bmg.messages.iter().enumerate() {
            let text = escape_line(&bmg_message_text(message));
            out.push_str(&format!("[{i:03}] {}\n", if text.is_empty() { "[EMPTY]" } else { &text }));
        }
        out
    }

    /// Reads a text export back. Existing entries are updated in place so their INF1 attributes and MID1 IDs survive;
    /// only indices past the end of the file are appended.
    pub fn import_text(&mut self, text: &str) -> Imported {
        self.commit();
        let mut result = Imported::default();
        for line in text.lines() {
            if line.starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let Some(close) = line.find(']').filter(|_| line.starts_with('[')) else {
                result.skipped += 1;
                continue;
            };
            let Some(index) = line.get(1..close).and_then(|n| n.parse::<usize>().ok()).filter(|i| *i <= 0xffff) else {
                result.skipped += 1;
                continue;
            };
            let mut body = line.get(close + 1..).unwrap_or_default().trim();
            if body == "[EMPTY]" || body == "[NULL]" {
                body = "";
            }
            while self.bmg.messages.len() <= index {
                let mut blank = BmgMessage::default();
                blank.attributes.resize(usize::from(self.bmg.attribute_size), 0);
                if self.bmg.has_mid1 {
                    blank.id = u32::try_from(self.bmg.messages.len()).ok();
                }
                self.bmg.messages.push(blank);
                result.appended += 1;
            }
            if let Some(message) = self.bmg.messages.get_mut(index) {
                message.parts = parse_bmg_message_text(&unescape_line(body));
            }
            result.updated += 1;
        }
        if result.updated > 0 {
            self.dirty = true;
        }
        self.active = None;
        self.load(Some(0));
        result
    }

    /// The edited file, with whatever is in the fields.
    pub fn output(&mut self) -> Result<Vec<u8>, String> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        self.commit();
        write_bmg(&self.bmg).map_err(|e| e.message)
    }
}

/// What the document asks the workspace to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MessageAction {
    Save,
}

/// The parts of the view that are not the document: the open escape prompt, the removal being confirmed, where the
/// text cursor was, and what the last import or problem was.
#[derive(Default)]
pub struct MessageView {
    escape: Option<String>,
    confirm_remove: Option<usize>,
    cursor: Option<usize>,
    notice: Option<String>,
}

pub fn show(ui: &mut Ui, doc: &mut MessageDoc, view: &mut MessageView, services: &dyn Services) -> Vec<MessageAction> {
    let mut actions = Vec::new();
    ui.horizontal(|ui| {
        ui.label(RichText::new(&doc.path).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(doc.error.is_none() && doc.dirty, egui::Button::new("Save"))
                .on_hover_text("Writes the file into the module it comes from, or the active one")
                .clicked()
            {
                actions.push(MessageAction::Save);
            }
        });
    });
    if let Some(error) = &doc.error {
        ui.colored_label(ui.visuals().error_fg_color, error);
        return actions;
    }
    ui.label(doc.info());
    ui.add(egui::TextEdit::singleline(&mut doc.filter).hint_text("Filter messages by text or ID").desired_width(f32::INFINITY));

    let list_height = (ui.available_height() * 0.55).max(80.0);
    let mut chosen = None;
    egui::ScrollArea::vertical().id_salt("messages").max_height(list_height).auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("message-list").num_columns(if doc.bmg.has_mid1 { 3 } else { 2 }).striped(true).show(ui, |ui| {
            ui.strong("#");
            if doc.bmg.has_mid1 {
                ui.strong("ID");
            }
            ui.strong("Message");
            ui.end_row();
            for index in 0..doc.bmg.messages.len() {
                if !doc.visible(index) {
                    continue;
                }
                let Some((number, id, text)) = doc.row(index) else { continue };
                let selected = doc.active == Some(index);
                if ui.selectable_label(selected, number).clicked() {
                    chosen = Some(index);
                }
                if doc.bmg.has_mid1 && ui.selectable_label(selected, id).clicked() {
                    chosen = Some(index);
                }
                if ui.selectable_label(selected, text).clicked() {
                    chosen = Some(index);
                }
                ui.end_row();
            }
        });
    });
    if let Some(index) = chosen.filter(|i| doc.active != Some(*i)) {
        doc.select(Some(index));
        view.cursor = None;
    }

    ui.separator();
    let editing = doc.active.is_some();
    let editor = ui.add_enabled(
        editing,
        egui::TextEdit::multiline(&mut doc.text).code_editor().desired_width(f32::INFINITY).desired_rows(5).id_salt("message-text"),
    );
    if let Some(state) = egui::TextEdit::load_state(ui.ctx(), editor.id)
        && let Some(range) = state.cursor.char_range()
    {
        view.cursor = Some(range.primary.index.0);
    }
    if editor.changed() {
        doc.commit();
    }
    egui::Grid::new("message-fields").num_columns(2).show(ui, |ui| {
        if doc.bmg.has_mid1 {
            ui.label("Message ID");
            if ui.add_enabled(editing, egui::DragValue::new(&mut doc.id).range(0..=i32::MAX)).changed() {
                doc.commit();
            }
            ui.end_row();
        }
        if doc.bmg.attribute_size > 0 {
            ui.label("Attributes");
            if ui.add_enabled(editing, egui::TextEdit::singleline(&mut doc.attributes).hint_text("INF1 attribute bytes, hexadecimal")).changed() {
                doc.commit();
            }
            ui.end_row();
        }
    });
    ui.weak(doc.stats());

    ui.horizontal_wrapped(|ui| {
        if ui.button("Add").clicked() {
            doc.add(false);
        }
        if ui.button("Duplicate").clicked() {
            doc.add(true);
        }
        if ui.add_enabled(editing, egui::Button::new("Remove")).clicked() {
            view.confirm_remove = doc.active;
        }
        if ui.add_enabled(editing, egui::Button::new("Insert escape…")).clicked() {
            view.escape = Some("255:00000000".to_owned());
        }
        if ui.button("Export text…").clicked() {
            let text = doc.export_text();
            let name = doc.path.rsplit(['/', '!']).next().unwrap_or("messages").to_owned();
            let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
            if let Err(error) = services.save_file("Export messages", &format!("{stem}.txt"), "txt", text.as_bytes()) {
                view.notice = Some(error);
            }
        }
        if ui.button("Import text…").clicked() {
            match services.open_file("Import messages", Some("txt")) {
                Some(Ok((_, bytes))) => {
                    let done = doc.import_text(&String::from_utf8_lossy(&bytes));
                    view.notice = Some(format!(
                        "Updated {} message(s), appended {}, skipped {} unreadable line(s).",
                        done.updated, done.appended, done.skipped
                    ));
                }
                Some(Err(error)) => view.notice = Some(error),
                None => {}
            }
        }
    });
    if let Some(notice) = &view.notice {
        ui.weak(notice);
    }

    prompts(ui, doc, view);
    actions
}

fn prompts(ui: &mut Ui, doc: &mut MessageDoc, view: &mut MessageView) {
    if let Some(row) = view.confirm_remove {
        let mut answer = None;
        egui::Modal::new(egui::Id::new(("message-remove", &doc.path))).show(ui.ctx(), |ui| {
            ui.heading("Remove message");
            ui.label(format!(
                "Remove message {row}? Every later message shifts down one index, which breaks any code that indexes this file directly."
            ));
            ui.horizontal(|ui| {
                if ui.button("Remove").clicked() {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(remove) = answer {
            if remove {
                doc.remove();
            }
            view.confirm_remove = None;
        }
    }
    if let Some(typed) = &mut view.escape {
        let mut answer = None;
        egui::Modal::new(egui::Id::new(("message-escape", &doc.path))).show(ui.ctx(), |ui| {
            ui.heading("Insert escape sequence");
            ui.label("Escape as type:hexdata, for example 255:00000100");
            ui.text_edit_singleline(typed);
            ui.horizontal(|ui| {
                if ui.button("Insert").clicked() {
                    answer = Some(true);
                }
                if ui.button("Cancel").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => match MessageDoc::escape(typed) {
                Ok(escape) => {
                    let at = view.cursor.unwrap_or(usize::MAX).min(doc.text.chars().count());
                    let byte = doc.text.char_indices().nth(at).map_or(doc.text.len(), |(i, _)| i);
                    doc.text.insert_str(byte, &escape);
                    doc.commit();
                    view.escape = None;
                }
                Err(error) => view.notice = Some(error),
            },
            Some(false) => view.escape = None,
            None => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use bigstar_formats::BmgPart;

    pub(crate) fn sample_bmg() -> Vec<u8> {
        let bmg = Bmg {
            attribute_size: 4,
            has_mid1: true,
            messages: vec![
                BmgMessage {
                    parts: vec![BmgPart::escape(255, vec![0, 0, 1, 0]), BmgPart::text(" Luigi has appeared!")],
                    attributes: vec![1, 0, 0, 0],
                    id: Some(40),
                },
                BmgMessage { parts: vec![BmgPart::text("Welcome!")], attributes: vec![2, 0, 0, 0], id: Some(41) },
            ],
            ..Default::default()
        };
        write_bmg(&bmg).unwrap()
    }

    #[test]
    fn message_editor_round_trips_and_edits_bmg_messages() {
        let bytes = sample_bmg();
        assert!(MessageDoc::supports(&bytes));
        let mut doc = MessageDoc::open("script/course.bmg", &bytes);
        assert_eq!(doc.bmg.messages.len(), 2);
        // The list previews plain text; escapes belong in the editor, spelled the way the NDS BMG Editor spells them.
        let (_, id, text) = doc.row(0).unwrap();
        assert_eq!(text, " Luigi has appeared!");
        assert_eq!(id, "40");
        assert_eq!(doc.text, "[255:00000100] Luigi has appeared!");

        // Untouched in, untouched out.
        assert_eq!(doc.output().unwrap(), bytes);
        assert!(!doc.dirty);

        doc.filter = "Welcome".to_owned();
        assert!(!doc.visible(0));
        assert!(doc.visible(1));
        doc.filter.clear();

        doc.select(Some(1));
        doc.text = "Bye![1:0A]".to_owned();
        let edited = doc.output().unwrap();
        assert!(doc.dirty);
        let parsed = parse_bmg(&edited).unwrap();
        assert_eq!(parsed.messages.len(), 2);
        assert_eq!(bmg_message_text(&parsed.messages[1]), "Bye![1:0A]");
        // The edit must not disturb the other message, its attributes, or its MID1 ID.
        assert_eq!(parsed.messages[0], parse_bmg(&bytes).unwrap().messages[0]);
        assert_eq!(parsed.messages[1].attributes, [2, 0, 0, 0]);
        assert_eq!(parsed.messages[1].id, Some(41));
    }

    #[test]
    fn a_duplicate_gets_a_fresh_id_and_removal_renumbers() {
        let mut doc = MessageDoc::open("script/course.bmg", &sample_bmg());
        doc.add(true);
        assert_eq!(doc.active, Some(2));
        assert_eq!(doc.bmg.messages[2].id, Some(42), "never the ID it was copied from");
        assert_eq!(doc.bmg.messages[2].parts, doc.bmg.messages[0].parts);
        doc.add(false);
        assert_eq!(doc.bmg.messages[3].attributes, [0, 0, 0, 0]);
        assert_eq!(doc.bmg.messages[3].id, Some(43));
        doc.select(Some(0));
        doc.remove();
        assert_eq!(doc.bmg.messages.len(), 3);
        assert_eq!(doc.active, Some(0));
        assert_eq!(doc.bmg.messages[0].id, Some(41));
    }

    #[test]
    fn escapes_are_checked_before_they_are_inserted() {
        assert_eq!(MessageDoc::escape("255:00000100").unwrap(), "[255:00000100]");
        assert!(MessageDoc::escape("hello").is_err());
        assert!(MessageDoc::escape("300:00").is_err(), "the type is a byte");
    }

    #[test]
    fn the_text_export_reads_back_into_the_same_messages() {
        let mut doc = MessageDoc::open("script/course.bmg", &sample_bmg());
        doc.select(Some(1));
        doc.text = "Two\nlines \\ here".to_owned();
        let exported = doc.export_text();
        assert!(exported.starts_with("# BMG Text Export\n# Total messages: 2\n"));
        assert!(exported.contains("[000] [255:00000100] Luigi has appeared!\n"));
        assert!(exported.contains("[001] Two\\nlines \\\\ here\n"));

        let mut other = MessageDoc::open("script/course.bmg", &sample_bmg());
        let done = other.import_text(&format!("{exported}[003] [EMPTY]\nnot a line\n[x] nope\n"));
        assert_eq!(done, Imported { updated: 3, appended: 2, skipped: 2 });
        assert_eq!(bmg_message_text(&other.bmg.messages[1]), "Two\nlines \\ here");
        // Imported text keeps the attributes and IDs already there.
        assert_eq!(other.bmg.messages[1].id, Some(41));
        assert_eq!(other.bmg.messages[1].attributes, [2, 0, 0, 0]);
        assert_eq!(other.bmg.messages[3].id, Some(3));
        assert!(other.bmg.messages[3].parts.is_empty());
    }

    #[test]
    fn line_escapes_round_trip() {
        for text in ["", "a\\b", "x\ny\rz", "\\n literally"] {
            assert_eq!(unescape_line(&escape_line(text)), text);
        }
        assert_eq!(unescape_line("\\q"), "\\q");
    }

    #[test]
    fn a_file_without_ids_or_attributes_has_no_fields_for_them() {
        let bare = Bmg { messages: vec![BmgMessage { parts: vec![BmgPart::text("Hello!")], ..Default::default() }], ..Default::default() };
        let bytes = write_bmg(&bare).unwrap();
        let mut doc = MessageDoc::open("script/error.bmg", &bytes);
        assert!(!doc.bmg.has_mid1);
        assert_eq!(doc.bmg.attribute_size, 0);
        assert_eq!(doc.row(0).unwrap().1, "");
        assert_eq!(doc.output().unwrap(), bytes);
    }

    #[test]
    fn a_wrapped_message_file_is_not_read_as_one() {
        // The dispatcher takes the wrapper off; the editor itself only reads plain BMG.
        let compressed = bigstar_formats::compress_nitro_lz10(&sample_bmg()).unwrap();
        assert!(!MessageDoc::supports(&compressed));
        assert_eq!(crate::editors::classify(&compressed, "script/course.bmg", false), crate::editors::EditorKind::Messages);
    }
}
