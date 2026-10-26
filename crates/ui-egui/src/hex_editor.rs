//! A hex editor in the manner of HxD: offsets, the bytes in hex and as text side by side, a caret and a selection
//! that both columns share, overwrite and insert modes, undo, find, go to, and a data inspector that reads and writes
//! the bytes at the caret as numbers. The file editor's fallback and the Level Hex Editor are both this.

use std::ops::Range;

use egui::{Event, EventFilter, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, Ui, pos2, vec2};

use crate::controls::{self, Button, Choice, TextField, Width};
use crate::theme::Tokens;

/// Hex digits, any whitespace between them; `None` for anything else or an odd count.
pub fn unhex(text: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = text.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if !digits.len().is_multiple_of(2) {
        return None;
    }
    digits.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect()
}

/// How a byte reads in the text column: printable ASCII as itself, anything else as a dot.
fn shown_char(byte: u8) -> char {
    if byte.is_ascii_graphic() || byte == b' ' { char::from(byte) } else { '.' }
}

/// Text as bytes, a character to a byte: Latin-1, with anything past it left out.
fn text_bytes(text: &str) -> Vec<u8> {
    text.chars().filter_map(|c| u8::try_from(u32::from(c)).ok()).collect()
}

/// An offset typed into Go To: hex, with or without `0x`, or decimal after a `#`.
fn parse_offset(text: &str) -> Option<usize> {
    let text = text.trim();
    if let Some(decimal) = text.strip_prefix('#') {
        return decimal.trim().parse().ok();
    }
    let digits = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")).unwrap_or(text);
    usize::from_str_radix(digits, 16).ok()
}

/// A change: the bytes at `at` that were `removed` became `inserted`. Undoing is the same change the other way.
#[derive(Debug, Clone, PartialEq)]
struct Splice {
    at: usize,
    removed: Vec<u8>,
    removed_marks: Vec<bool>,
    inserted: Vec<u8>,
    /// Where the caret was before, put back on undo.
    caret: usize,
    /// Typing that a next key can add to, so a run of typing undoes at once.
    typing: bool,
}

/// The column the caret types into.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pane {
    #[default]
    Hex,
    Text,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Endian {
    #[default]
    Little,
    Big,
}

/// What the Find field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FindAs {
    #[default]
    Hex,
    Text,
}

/// The bytes being edited and where the caret is in them.
pub struct HexDoc {
    pub bytes: Vec<u8>,
    /// The bytes as last read or saved, so a change shows.
    saved: Vec<u8>,
    /// Which bytes were changed since then, drawn in the warning colour.
    marks: Vec<bool>,
    undo: Vec<Splice>,
    redo: Vec<Splice>,
    /// Where typing goes: the byte the caret is on, which may be one past the end.
    pub caret: usize,
    /// The other end of the selection; the selection is the bytes between it and the caret.
    pub anchor: usize,
    /// The low digit of the caret's byte is next.
    low_nibble: bool,
    pub pane: Pane,
    /// Typing inserts rather than writing over.
    pub insert: bool,
    pub columns: usize,
    pub endian: Endian,
    pub inspector: bool,
    goto: String,
    find: String,
    find_as: FindAs,
    /// What the last find or go to said, when it did not land.
    message: Option<String>,
    /// The inspector line being typed into, and what it holds.
    editing: Option<(usize, String)>,
    /// Scroll the caret into view on the next frame.
    reveal: bool,
}

impl HexDoc {
    pub fn new(bytes: Vec<u8>) -> Self {
        HexDoc {
            marks: vec![false; bytes.len()],
            saved: bytes.clone(),
            bytes,
            undo: Vec::new(),
            redo: Vec::new(),
            caret: 0,
            anchor: 0,
            low_nibble: false,
            pane: Pane::Hex,
            insert: false,
            columns: 16,
            endian: Endian::Little,
            inspector: true,
            goto: String::new(),
            find: String::new(),
            find_as: FindAs::Hex,
            message: None,
            editing: None,
            reveal: false,
        }
    }

    pub fn dirty(&self) -> bool {
        self.bytes != self.saved
    }

    /// The bytes are what was saved now.
    pub fn mark_saved(&mut self) {
        self.saved.clone_from(&self.bytes);
        self.marks.iter_mut().for_each(|m| *m = false);
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The selected bytes; empty when nothing is.
    pub fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    /// Where the inspector and a selection's value read from: the selection's start, or the caret.
    fn focus(&self) -> usize {
        self.selection().start
    }

    pub fn select(&mut self, range: Range<usize>) {
        let len = self.bytes.len();
        self.anchor = range.start.min(len);
        self.caret = range.end.min(len);
        self.low_nibble = false;
        self.reveal = true;
    }

    fn move_caret(&mut self, to: usize, extend: bool) {
        self.caret = to.min(self.bytes.len());
        if !extend {
            self.anchor = self.caret;
        }
        self.low_nibble = false;
        self.reveal = true;
        self.end_typing();
    }

    fn end_typing(&mut self) {
        if let Some(last) = self.undo.last_mut() {
            last.typing = false;
        }
    }

    /// Replaces `range` with `inserted`, as one undo step unless it carries on the typing before it.
    fn splice(&mut self, range: Range<usize>, inserted: Vec<u8>, typing: bool) {
        let range = range.start.min(self.bytes.len())..range.end.min(self.bytes.len());
        if range.is_empty() && inserted.is_empty() {
            return;
        }
        let removed: Vec<u8> = self.bytes.splice(range.clone(), inserted.iter().copied()).collect();
        let removed_marks: Vec<bool> = self.marks.splice(range.clone(), std::iter::repeat_n(true, inserted.len())).collect();
        self.redo.clear();
        let step = Splice { at: range.start, removed, removed_marks, inserted, caret: self.caret, typing };
        match self.undo.last_mut() {
            // Writing over the byte after the last one typed, or inserting after it, is the same step.
            Some(last) if typing && last.typing && step.at == last.at + last.inserted.len() => {
                last.removed.extend(step.removed);
                last.removed_marks.extend(step.removed_marks);
                last.inserted.extend(step.inserted);
            }
            // The second digit of a byte only changes the byte the first one wrote.
            Some(last) if typing && last.typing && step.at + 1 == last.at + last.inserted.len() && step.inserted.len() == 1 => {
                if let (Some(byte), Some(&typed)) = (last.inserted.last_mut(), step.inserted.first()) {
                    *byte = typed;
                }
            }
            _ => self.undo.push(step),
        }
    }

    fn apply_undo(&mut self, undo: bool) {
        let (from, to) = if undo { (&mut self.undo, &mut self.redo) } else { (&mut self.redo, &mut self.undo) };
        let Some(mut step) = from.pop() else { return };
        let range = step.at..step.at + step.inserted.len();
        let back = std::mem::take(&mut step.removed);
        let back_marks = std::mem::take(&mut step.removed_marks);
        step.removed = self.bytes.splice(range.clone(), back.iter().copied()).collect();
        step.removed_marks = self.marks.splice(range, back_marks).collect();
        step.inserted = back;
        step.typing = false;
        let caret = step.caret;
        step.caret = self.caret;
        to.push(step);
        self.caret = caret.min(self.bytes.len());
        self.anchor = self.caret;
        self.low_nibble = false;
        self.reveal = true;
    }

    pub fn undo(&mut self) {
        self.apply_undo(true);
    }

    pub fn redo(&mut self) {
        self.apply_undo(false);
    }

    /// A hex digit typed into the hex column.
    pub fn type_digit(&mut self, digit: u8) {
        self.clear_for_typing();
        let at = self.caret;
        if self.low_nibble
            && let Some(&high) = self.bytes.get(at)
        {
            let byte = (high & 0xf0) | digit;
            self.splice(at..at + 1, vec![byte], true);
            self.caret = at + 1;
            self.low_nibble = false;
        } else {
            let replace = if self.insert || at == self.bytes.len() { at..at } else { at..at + 1 };
            let low = self.bytes.get(at).filter(|_| !self.insert).map_or(0, |b| b & 0x0f);
            self.splice(replace, vec![(digit << 4) | low], true);
            self.low_nibble = true;
        }
        self.anchor = self.caret;
        self.reveal = true;
    }

    /// Characters typed into the text column, a byte each.
    pub fn type_text(&mut self, text: &str) {
        let bytes = text_bytes(text);
        if bytes.is_empty() {
            return;
        }
        self.clear_for_typing();
        self.write(bytes, true);
    }

    /// Writes `bytes` at the caret, over what is there or before it, and moves past them.
    fn write(&mut self, bytes: Vec<u8>, typing: bool) {
        let at = self.caret;
        let count = bytes.len();
        let end = if self.insert { at } else { (at + count).min(self.bytes.len()) };
        self.splice(at..end, bytes, typing);
        self.caret = at + count;
        self.anchor = self.caret;
        self.low_nibble = false;
        self.reveal = true;
    }

    /// Before typing: a selection is typed over from its start, or, inserting, taken out first.
    fn clear_for_typing(&mut self) {
        let selection = self.selection();
        if selection.is_empty() {
            return;
        }
        self.end_typing();
        if self.insert {
            self.splice(selection.clone(), Vec::new(), false);
        }
        self.caret = selection.start;
        self.anchor = selection.start;
        self.low_nibble = false;
    }

    /// Pasted text: hex when it all reads as hex and the hex column has the caret, its characters otherwise. It
    /// replaces a selection; with none, it is written over or inserted as typing would be.
    pub fn paste(&mut self, text: &str) {
        let bytes = match self.pane {
            Pane::Hex => unhex(text).unwrap_or_else(|| text_bytes(text)),
            Pane::Text => text_bytes(text),
        };
        if bytes.is_empty() {
            return;
        }
        self.end_typing();
        let selection = self.selection();
        if selection.is_empty() {
            self.write(bytes, false);
        } else {
            let count = bytes.len();
            self.splice(selection.clone(), bytes, false);
            self.select(selection.start..selection.start + count);
        }
    }

    /// Delete: the selection, or the byte at the caret. Backspace: the selection, or the byte before it.
    pub fn delete(&mut self, before: bool) {
        self.end_typing();
        let selection = self.selection();
        let range = match () {
            () if !selection.is_empty() => selection,
            () if before => self.caret.saturating_sub(1)..self.caret,
            () => self.caret..(self.caret + 1).min(self.bytes.len()),
        };
        if range.is_empty() {
            return;
        }
        self.splice(range.clone(), Vec::new(), false);
        self.move_caret(range.start, false);
    }

    /// Every selected byte becomes `value`.
    pub fn fill(&mut self, value: u8) {
        let selection = self.selection();
        if selection.is_empty() {
            return;
        }
        self.end_typing();
        self.splice(selection.clone(), vec![value; selection.len()], false);
        self.select(selection);
    }

    /// The selection, or the byte at the caret, as text for the clipboard.
    pub fn copied(&self, form: CopyAs) -> String {
        let selection = self.selection();
        let range = if selection.is_empty() { self.caret..(self.caret + 1).min(self.bytes.len()) } else { selection };
        let bytes = self.bytes.get(range).unwrap_or_default();
        match form {
            CopyAs::Hex => bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" "),
            CopyAs::Text => bytes.iter().map(|&b| shown_char(b)).collect(),
            CopyAs::Array => {
                let lines: Vec<String> =
                    bytes.chunks(16).map(|line| line.iter().map(|b| format!("0x{b:02X}")).collect::<Vec<_>>().join(", ")).collect();
                format!("{{\n    {}\n}}", lines.join(",\n    "))
            }
        }
    }

    /// Finds the next (or previous) place `needle` is, from the caret, around the end and back to the start.
    pub fn find_next(&mut self, needle: &[u8], backwards: bool) -> bool {
        let len = self.bytes.len();
        if needle.is_empty() || needle.len() > len {
            return false;
        }
        let at = |start: usize| self.bytes.get(start..).is_some_and(|rest| rest.starts_with(needle));
        let last = len - needle.len();
        let selection = self.selection();
        let found = if backwards {
            let from = selection.start.min(last + 1);
            (0..from).rev().find(|&i| at(i)).or_else(|| (from..=last).rev().find(|&i| at(i)))
        } else {
            let from = if selection.is_empty() { self.caret } else { selection.start + 1 };
            (from.min(last + 1)..=last).find(|&i| at(i)).or_else(|| (0..from.min(last + 1)).find(|&i| at(i)))
        };
        match found {
            Some(start) => {
                self.end_typing();
                self.select(start..start + needle.len());
                true
            }
            None => false,
        }
    }

    /// The bytes at the caret, as many as there are of the `size` asked for.
    fn read(&self, at: usize, size: usize) -> Option<u64> {
        let bytes = self.bytes.get(at..at + size)?;
        let fold = |value: u64, byte: &u8| (value << 8) | u64::from(*byte);
        Some(match self.endian {
            Endian::Little => bytes.iter().rev().fold(0, fold),
            Endian::Big => bytes.iter().fold(0, fold),
        })
    }

    /// Writes `value` as `size` bytes at `at`, over what is there.
    fn write_value(&mut self, at: usize, size: usize, value: u64) {
        if at + size > self.bytes.len() {
            return;
        }
        let mut bytes: Vec<u8> = (0..size).map(|i| (value >> (8 * i)) as u8).collect();
        if self.endian == Endian::Big {
            bytes.reverse();
        }
        self.end_typing();
        self.splice(at..at + size, bytes, false);
    }

    /// What the selection is worth as a number, when it is short enough to be one: (unsigned, signed).
    pub fn selection_value(&self) -> Option<(u64, i64)> {
        let selection = self.selection();
        let size = selection.len();
        if !(1..=8).contains(&size) {
            return None;
        }
        let value = self.read(selection.start, size)?;
        let shift = 64 - 8 * size as u32;
        Some((value, ((value << shift) as i64) >> shift))
    }
}

/// What the clipboard gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyAs {
    Hex,
    Text,
    /// A C array: `{ 0x01, 0x02 }`.
    Array,
}

/// A line of the data inspector: what it reads the bytes at the caret as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Binary,
    Signed(usize),
    Unsigned(usize),
    Float32,
    Float64,
    /// The DS's fixed point: 20.12 in four bytes, 4.12 in two.
    Fixed(usize),
}

const KINDS: &[(Kind, &str)] = &[
    (Kind::Binary, "Binary (8 bit)"),
    (Kind::Signed(1), "Int8"),
    (Kind::Unsigned(1), "UInt8"),
    (Kind::Signed(2), "Int16"),
    (Kind::Unsigned(2), "UInt16"),
    (Kind::Signed(3), "Int24"),
    (Kind::Unsigned(3), "UInt24"),
    (Kind::Signed(4), "Int32"),
    (Kind::Unsigned(4), "UInt32"),
    (Kind::Signed(8), "Int64"),
    (Kind::Unsigned(8), "UInt64"),
    (Kind::Float32, "Float32"),
    (Kind::Float64, "Float64"),
    (Kind::Fixed(2), "Fixed 4.12"),
    (Kind::Fixed(4), "Fixed 20.12"),
];

impl Kind {
    fn size(self) -> usize {
        match self {
            Kind::Binary => 1,
            Kind::Signed(size) | Kind::Unsigned(size) | Kind::Fixed(size) => size,
            Kind::Float32 => 4,
            Kind::Float64 => 8,
        }
    }

    fn signed(raw: u64, size: usize) -> i64 {
        let shift = 64 - 8 * size as u32;
        ((raw << shift) as i64) >> shift
    }

    fn show(self, raw: u64) -> String {
        match self {
            Kind::Binary => format!("{raw:08b}"),
            Kind::Signed(size) => Kind::signed(raw, size).to_string(),
            Kind::Unsigned(_) => raw.to_string(),
            Kind::Float32 => format!("{}", f32::from_bits(raw as u32)),
            Kind::Float64 => format!("{}", f64::from_bits(raw)),
            Kind::Fixed(size) => format!("{}", Kind::signed(raw, size) as f64 / 4096.0),
        }
    }

    /// What typed text is as this kind's bytes, as a number; `None` when it is not one or does not fit.
    fn parse(self, text: &str) -> Option<u64> {
        let text = text.trim();
        let size = self.size();
        let bits = 8 * size as u32;
        let mask = if bits == 64 { u64::MAX } else { (1u64 << bits) - 1 };
        let hex = text.strip_prefix("0x").or_else(|| text.strip_prefix("0X"));
        match self {
            Kind::Binary => u8::from_str_radix(text, 2).ok().map(u64::from),
            Kind::Signed(_) | Kind::Unsigned(_) if hex.is_some() => u64::from_str_radix(hex?, 16).ok().filter(|v| *v <= mask),
            Kind::Signed(_) => {
                let value: i64 = text.parse().ok()?;
                let (low, high) = if bits == 64 { (i64::MIN, i64::MAX) } else { (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1) };
                (low..=high).contains(&value).then_some(value as u64 & mask)
            }
            Kind::Unsigned(_) => text.parse().ok().filter(|v| *v <= mask),
            Kind::Float32 => text.parse::<f32>().ok().map(|v| u64::from(v.to_bits())),
            Kind::Float64 => text.parse::<f64>().ok().map(f64::to_bits),
            Kind::Fixed(_) => {
                let raw = (text.parse::<f64>().ok()? * 4096.0).round();
                let (low, high) = (-(2f64.powi(bits as i32 - 1)), 2f64.powi(bits as i32 - 1) - 1.0);
                (low..=high).contains(&raw).then_some(raw as i64 as u64 & mask)
            }
        }
    }
}

/// What the person asked of the editor this frame that its owner carries out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HexAction {
    Save,
}

/// Where the parts of a line are, in characters from its left.
struct Layout {
    char_width: f32,
    row_height: f32,
    offset_digits: usize,
    columns: usize,
}

impl Layout {
    /// Between the offsets and the bytes, and the bytes and the text.
    const GAP: usize = 2;

    fn hex_x(&self, column: usize) -> f32 {
        // A space more between each group of eight.
        let chars = self.offset_digits + Self::GAP + column * 3 + column / 8;
        chars as f32 * self.char_width
    }

    fn hex_width(&self) -> usize {
        self.columns * 3 - 1 + (self.columns - 1) / 8
    }

    fn text_x(&self, column: usize) -> f32 {
        (self.offset_digits + Self::GAP + self.hex_width() + Self::GAP + column) as f32 * self.char_width
    }

    fn width(&self) -> f32 {
        self.text_x(self.columns)
    }

    /// The pane and byte column under `x`, nearest when between them.
    fn hit(&self, x: f32) -> (Pane, usize) {
        let text_start = self.text_x(0) - self.char_width;
        if x >= text_start {
            let column = ((x - self.text_x(0)) / self.char_width).floor().max(0.0) as usize;
            return (Pane::Text, column.min(self.columns - 1));
        }
        let column = (0..self.columns).rev().find(|&c| x >= self.hex_x(c) - self.char_width * 0.5).unwrap_or(0);
        (Pane::Hex, column)
    }
}

/// Which hex editor last had the keyboard, so the app's own shortcuts leave its keys to it.
fn focus_key() -> egui::Id {
    egui::Id::new("hex-editor-focus")
}

/// Whether a hex editor has the keyboard, so Undo, Redo and the like are its own.
pub fn has_keyboard(ctx: &egui::Context) -> bool {
    let mine: Option<egui::Id> = ctx.data(|d| d.get_temp(focus_key()));
    mine.is_some_and(|id| ctx.memory(|m| m.has_focus(id)))
}

/// The editor: a toolbar over the bytes, the data inspector beside them and a status line under them. `save` puts a
/// Save button on the toolbar, for an owner without one of its own.
pub fn show(ui: &mut Ui, doc: &mut HexDoc, id: impl egui::AsIdSalt, save: bool) -> Option<HexAction> {
    let id = ui.make_persistent_id("hex-editor").with(id);
    let mut action = None;
    toolbar(ui, doc, save, &mut action);
    ui.add_space(4.0);
    let status_height = ui.spacing().interact_size.y + 6.0;
    let height = (ui.available_height() - status_height).max(80.0);
    // Laid out in exact rectangles: in a window, panes that fill what they are given would widen it every frame.
    let width = ui.available_width();
    let (area, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    let inspector_width = if doc.inspector { 270f32.min(width * 0.4) } else { 0.0 };
    let grid_rect = Rect::from_min_size(area.min, vec2(width - inspector_width, height));
    ui.scope_builder(egui::UiBuilder::new().max_rect(grid_rect), |ui| grid(ui, doc, id));
    if doc.inspector {
        let t = Tokens::get(ui.ctx());
        let x = grid_rect.right() + 6.0;
        ui.painter().vline(x, area.y_range(), Stroke::new(1.0, t.border));
        let rect = Rect::from_min_max(pos2(x + 8.0, area.top()), area.max);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            egui::ScrollArea::vertical().id_salt(id.with("inspector")).auto_shrink([false, false]).show(ui, |ui| inspector(ui, doc));
        });
    }
    status(ui, doc);
    action
}

fn toolbar(ui: &mut Ui, doc: &mut HexDoc, save: bool, action: &mut Option<HexAction>) {
    ui.horizontal_wrapped(|ui| {
        if save {
            let button = Button::new("Save").with_icon(crate::icons::SAVE).enabled(doc.dirty());
            if (if doc.dirty() { button.primary() } else { button }).show(ui).clicked() {
                *action = Some(HexAction::Save);
            }
            ui.separator();
        }
        if controls::icon_button(ui, crate::icons::UNDO, "Undo").on_hover_text("Undo (Ctrl+Z)").clicked() {
            doc.undo();
        }
        if controls::icon_button(ui, crate::icons::REDO, "Redo").on_hover_text("Redo (Ctrl+Y)").clicked() {
            doc.redo();
        }
        ui.separator();
        ui.label("Go to");
        let goto = TextField::new(&mut doc.goto).hint("Offset").name("Go to offset").code().width(Width::Px(90.0)).show(ui);
        let entered = goto.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        if entered || Button::new("Go").small().show(ui).on_hover_text("Hex, or decimal after a #").clicked() {
            match parse_offset(&doc.goto) {
                Some(offset) if offset <= doc.bytes.len() => {
                    doc.message = None;
                    doc.move_caret(offset, false);
                }
                _ => doc.message = Some(format!("No offset {} in this file.", doc.goto.trim())),
            }
        }
        ui.separator();
        ui.label("Find");
        let hint = if doc.find_as == FindAs::Hex { "Hex bytes" } else { "Text" };
        let find = TextField::new(&mut doc.find).hint(hint).name("Find").code().width(Width::Px(150.0)).show(ui);
        controls::drop_down("hex-find-as").selected_text(if doc.find_as == FindAs::Hex { "Hex" } else { "Text" }).width(60.0).show_ui(ui, |ui| {
            ui.choice_value(&mut doc.find_as, FindAs::Hex, "Hex");
            ui.choice_value(&mut doc.find_as, FindAs::Text, "Text");
        });
        let entered = find.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
        let previous = Button::new("Previous").small().show(ui).clicked();
        let next = Button::new("Next").small().show(ui).clicked();
        if entered || previous || next {
            let needle = match doc.find_as {
                FindAs::Hex => unhex(&doc.find),
                FindAs::Text => Some(text_bytes(&doc.find)),
            };
            doc.message = match needle {
                None => Some("Find takes whole bytes in hex.".to_owned()),
                Some(needle) if doc.find_next(&needle, previous) => None,
                Some(_) => Some("Not found.".to_owned()),
            };
        }
        ui.separator();
        ui.label("Columns");
        controls::drop_down("hex-columns").selected_text(doc.columns.to_string()).width(48.0).show_ui(ui, |ui| {
            for columns in [8, 16, 32] {
                ui.choice_value(&mut doc.columns, columns, columns.to_string());
            }
        });
        controls::toggle(ui, &mut doc.inspector, "Inspector");
        if let Some(message) = &doc.message {
            controls::warning_text(ui, message);
        }
    });
}

fn grid(ui: &mut Ui, doc: &mut HexDoc, id: egui::Id) {
    let t = Tokens::get(ui.ctx());
    let font = FontId::monospace(13.0);
    let char_width = ui.painter().layout_no_wrap("0".to_owned(), font.clone(), t.foreground).size().x;
    let row_height = ui.text_style_height(&egui::TextStyle::Monospace).max(font.size) + 4.0;
    let len = doc.bytes.len();
    let digits = format!("{:X}", len.max(1)).len().max(8);
    let layout = Layout { char_width, row_height, offset_digits: digits, columns: doc.columns };

    // The column numbers over the bytes.
    let (header, _) = ui.allocate_exact_size(vec2(ui.available_width(), row_height), Sense::hover());
    let painter = ui.painter_at(header);
    let left = header.left() + 6.0;
    painter.text(pos2(left, header.center().y), egui::Align2::LEFT_CENTER, "Offset", font.clone(), t.muted);
    for column in 0..doc.columns {
        let x = left + layout.hex_x(column);
        painter.text(pos2(x, header.center().y), egui::Align2::LEFT_CENTER, format!("{column:02X}"), font.clone(), t.muted);
    }
    painter.text(pos2(left + layout.text_x(0), header.center().y), egui::Align2::LEFT_CENTER, "Decoded text", font.clone(), t.muted);
    painter.line_segment([header.left_bottom(), header.right_bottom()], Stroke::new(1.0, t.border));

    let rows = len / doc.columns + 1;
    let caret_row = doc.caret / doc.columns;
    let mut scroll = egui::ScrollArea::both().id_salt(id.with("rows")).auto_shrink([false, false]);
    // Keeps the caret in view after a key moves it.
    let viewport: Option<(f32, f32)> = ui.data(|d| d.get_temp(id.with("viewport")));
    if std::mem::take(&mut doc.reveal)
        && let Some((top, height)) = viewport
    {
        let y = caret_row as f32 * row_height;
        if y < top {
            scroll = scroll.vertical_scroll_offset(y);
        } else if y + row_height > top + height {
            scroll = scroll.vertical_scroll_offset(y + row_height - height);
        }
    }
    let output = scroll.show_rows(ui, row_height, rows, |ui, visible| {
        let size = vec2((layout.width() + 12.0).max(ui.available_width()), row_height * visible.len() as f32);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        let origin = pos2(rect.left() + 6.0, rect.top() - visible.start as f32 * row_height);
        interact(ui, doc, &layout, &response, origin, id);
        paint(ui, doc, &layout, rect, origin, visible, &font, &t, response.has_focus());
        response
    });
    ui.data_mut(|d| d.insert_temp(id.with("viewport"), (output.state.offset.y, output.inner_rect.height())));
    let response = output.inner;
    if response.has_focus() {
        ui.data_mut(|d| d.insert_temp(focus_key(), response.id));
        ui.memory_mut(|m| {
            m.set_focus_lock_filter(response.id, EventFilter { tab: true, horizontal_arrows: true, vertical_arrows: true, escape: false })
        });
        let page = viewport.map_or(16, |(_, height)| (height / row_height).max(1.0) as usize) * doc.columns;
        keys(ui, doc, page);
    }
    context_menu(&response, doc);
}

/// The byte and pane under a point, and the place a drag reaches: one past the byte when the drag runs forward.
fn byte_at(doc: &HexDoc, layout: &Layout, origin: Pos2, at: Pos2) -> (Pane, usize) {
    let row = ((at.y - origin.y) / layout.row_height).floor().max(0.0) as usize;
    let (pane, column) = layout.hit(at.x - origin.x);
    (pane, (row * doc.columns + column).min(doc.bytes.len()))
}

fn interact(ui: &Ui, doc: &mut HexDoc, layout: &Layout, response: &egui::Response, origin: Pos2, id: egui::Id) {
    let pointer = ui.input(|i| i.pointer.interact_pos());
    let press = id.with("press");
    if (response.drag_started() || response.clicked() || response.secondary_clicked())
        && let Some(at) = pointer
    {
        response.request_focus();
        let (pane, byte) = byte_at(doc, layout, origin, at);
        doc.pane = pane;
        let shift = ui.input(|i| i.modifiers.shift);
        // A right click inside the selection keeps it for the menu.
        let keep = response.secondary_clicked() && doc.selection().contains(&byte);
        if !keep {
            doc.end_typing();
            doc.low_nibble = false;
            if shift {
                doc.caret = byte;
            } else {
                doc.caret = byte;
                doc.anchor = byte;
            }
        }
        ui.data_mut(|d| d.insert_temp(press, if shift { doc.anchor } else { byte }));
    }
    if response.dragged()
        && let Some(at) = pointer
    {
        let start: usize = ui.data(|d| d.get_temp(press)).unwrap_or(doc.anchor);
        let (_, byte) = byte_at(doc, layout, origin, at);
        // The bytes from the one pressed to the one under the pointer, both included.
        if byte >= start {
            doc.anchor = start;
            doc.caret = (byte + 1).min(doc.bytes.len());
        } else {
            doc.anchor = (start + 1).min(doc.bytes.len());
            doc.caret = byte;
        }
        doc.reveal = true;
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
    }
}

#[allow(clippy::too_many_arguments)]
fn paint(ui: &Ui, doc: &HexDoc, layout: &Layout, rect: Rect, origin: Pos2, visible: Range<usize>, font: &FontId, t: &Tokens, focused: bool) {
    let painter = ui.painter_at(rect);
    let selection = doc.selection();
    let cw = layout.char_width;
    let rh = layout.row_height;
    let selected_fill = if focused { t.primary_subtle } else { t.border };
    for row in visible {
        let y = origin.y + row as f32 * rh;
        let mid = y + rh * 0.5;
        let start = row * doc.columns;
        if start > doc.bytes.len() {
            break;
        }
        let offset_color = if (start..start + doc.columns).contains(&doc.caret) { t.primary_text } else { t.muted };
        painter.text(
            pos2(origin.x, mid),
            egui::Align2::LEFT_CENTER,
            format!("{start:0width$X}", width = layout.offset_digits),
            font.clone(),
            offset_color,
        );
        for column in 0..doc.columns {
            let index = start + column;
            let hex = Rect::from_min_size(pos2(origin.x + layout.hex_x(column), y), vec2(cw * 2.0, rh));
            let text = Rect::from_min_size(pos2(origin.x + layout.text_x(column), y), vec2(cw, rh));
            if selection.contains(&index) {
                // The selection runs on across the space after a byte unless the row or the selection ends there.
                let spread = if column + 1 < doc.columns && selection.contains(&(index + 1)) {
                    cw * (1.0 + f32::from(u8::from((column + 1) % 8 == 0)))
                } else {
                    0.0
                };
                painter.rect_filled(hex.with_max_x(hex.max.x + spread), 0.0, selected_fill);
                painter.rect_filled(text, 0.0, selected_fill);
            }
            if index == doc.caret {
                let (active, other) = if doc.pane == Pane::Hex { (hex, text) } else { (text, hex) };
                let ink = if focused { t.primary } else { t.muted };
                if doc.pane == Pane::Hex && doc.low_nibble {
                    let digit = Rect::from_min_size(pos2(hex.left() + cw, y), vec2(cw, rh));
                    painter.rect_stroke(digit.shrink(0.5), 1.0, Stroke::new(1.5, ink), egui::StrokeKind::Inside);
                } else if doc.insert {
                    painter.line_segment([active.left_top(), active.left_bottom()], Stroke::new(2.0, ink));
                } else {
                    painter.rect_stroke(active.shrink(0.5), 1.0, Stroke::new(1.5, ink), egui::StrokeKind::Inside);
                }
                painter.rect_stroke(other.shrink(0.5), 1.0, Stroke::new(1.0, t.border_strong), egui::StrokeKind::Inside);
            }
            let Some(&byte) = doc.bytes.get(index) else { continue };
            let color = if doc.marks.get(index).copied().unwrap_or(false) {
                t.danger
            } else if byte == 0 {
                t.subtle
            } else {
                t.foreground
            };
            painter.text(pos2(hex.left(), mid), egui::Align2::LEFT_CENTER, format!("{byte:02X}"), font.clone(), color);
            let shown = shown_char(byte);
            let text_color = if shown == '.' && byte != b'.' { t.subtle } else { color };
            painter.text(pos2(text.left(), mid), egui::Align2::LEFT_CENTER, shown, font.clone(), text_color);
        }
    }
}

fn keys(ui: &Ui, doc: &mut HexDoc, page: usize) {
    let events = ui.input(|i| i.events.clone());
    for event in events {
        match event {
            Event::Copy => ui.ctx().copy_text(doc.copied(if doc.pane == Pane::Hex { CopyAs::Hex } else { CopyAs::Text })),
            Event::Cut => {
                ui.ctx().copy_text(doc.copied(if doc.pane == Pane::Hex { CopyAs::Hex } else { CopyAs::Text }));
                if !doc.selection().is_empty() {
                    doc.delete(false);
                }
            }
            Event::Paste(text) => doc.paste(&text),
            Event::Text(text) => match doc.pane {
                Pane::Hex => {
                    for digit in text.chars().filter_map(|c| c.to_digit(16)) {
                        doc.type_digit(digit as u8);
                    }
                }
                Pane::Text => doc.type_text(&text),
            },
            Event::Key { key, pressed: true, modifiers, .. } => key_press(doc, key, modifiers, page),
            _ => {}
        }
    }
}

fn key_press(doc: &mut HexDoc, key: Key, modifiers: Modifiers, page: usize) {
    let extend = modifiers.shift;
    let command = modifiers.command;
    let columns = doc.columns;
    let caret = doc.caret;
    let row_start = caret - caret % columns;
    match key {
        Key::ArrowLeft => doc.move_caret(caret.saturating_sub(1), extend),
        Key::ArrowRight => doc.move_caret(caret + 1, extend),
        Key::ArrowUp => doc.move_caret(caret.checked_sub(columns).unwrap_or(caret), extend),
        Key::ArrowDown if caret + columns <= doc.bytes.len() => doc.move_caret(caret + columns, extend),
        Key::PageUp => doc.move_caret(caret.saturating_sub(page), extend),
        Key::PageDown => doc.move_caret(caret + page, extend),
        Key::Home if command => doc.move_caret(0, extend),
        Key::End if command => doc.move_caret(doc.bytes.len(), extend),
        Key::Home => doc.move_caret(row_start, extend),
        Key::End => doc.move_caret((row_start + columns - 1).min(doc.bytes.len()), extend),
        Key::Tab => {
            doc.pane = if doc.pane == Pane::Hex { Pane::Text } else { Pane::Hex };
            doc.low_nibble = false;
            doc.end_typing();
        }
        Key::Insert => doc.insert = !doc.insert,
        Key::Delete => doc.delete(false),
        Key::Backspace => doc.delete(true),
        Key::A if command => doc.select(0..doc.bytes.len()),
        // Redo first: Cmd+Shift+Z would otherwise also match Cmd+Z.
        Key::Z if command && modifiers.shift => doc.redo(),
        Key::Y if command => doc.redo(),
        Key::Z if command => doc.undo(),
        _ => {}
    }
}

fn context_menu(response: &egui::Response, doc: &mut HexDoc) {
    controls::context_menu(response, |ui| {
        let some = !doc.bytes.is_empty();
        let selected = !doc.selection().is_empty();
        for (form, label) in [(CopyAs::Hex, "Copy as Hex"), (CopyAs::Text, "Copy as Text"), (CopyAs::Array, "Copy as C Array")] {
            if Button::menu(label).enabled(some).show(ui).clicked() {
                ui.ctx().copy_text(doc.copied(form));
                ui.close();
            }
        }
        if Button::menu("Copy Offset").show(ui).clicked() {
            ui.ctx().copy_text(format!("0x{:X}", doc.focus()));
            ui.close();
        }
        ui.separator();
        for (value, label) in [(0x00, "Fill Selection with 00"), (0xff, "Fill Selection with FF")] {
            if Button::menu(label).enabled(selected).show(ui).clicked() {
                doc.fill(value);
                ui.close();
            }
        }
        if Button::menu("Delete Selection").enabled(selected).show(ui).clicked() {
            doc.delete(false);
            ui.close();
        }
        if Button::menu("Select All").enabled(some).show(ui).clicked() {
            doc.select(0..doc.bytes.len());
            ui.close();
        }
        ui.separator();
        if Button::menu("Undo").enabled(doc.can_undo()).show(ui).clicked() {
            doc.undo();
            ui.close();
        }
        if Button::menu("Redo").enabled(doc.can_redo()).show(ui).clicked() {
            doc.redo();
            ui.close();
        }
    });
}

fn inspector(ui: &mut Ui, doc: &mut HexDoc) {
    let at = doc.focus();
    ui.horizontal(|ui| {
        ui.strong("Data Inspector");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            controls::drop_down("hex-endian").selected_text(if doc.endian == Endian::Little { "Little-endian" } else { "Big-endian" }).show_ui(
                ui,
                |ui| {
                    ui.choice_value(&mut doc.endian, Endian::Little, "Little-endian");
                    ui.choice_value(&mut doc.endian, Endian::Big, "Big-endian");
                },
            );
        });
    });
    ui.weak(format!("At 0x{at:X} ({at}). Type a value and press Enter to write it."));
    ui.add_space(4.0);
    let mut write = None;
    egui::Grid::new("hex-inspector").num_columns(2).striped(true).spacing([8.0, 2.0]).show(ui, |ui| {
        for (row, (kind, name)) in KINDS.iter().enumerate() {
            ui.label(egui::RichText::new(*name).weak());
            let Some(raw) = doc.read(at, kind.size()) else {
                ui.weak("—");
                ui.end_row();
                continue;
            };
            let mut text = match &doc.editing {
                Some((editing, text)) if *editing == row => text.clone(),
                _ => kind.show(raw),
            };
            let field = TextField::new(&mut text).code().name(name).id_salt(("hex-inspector", row)).width(Width::Px(150.0)).show(ui);
            if field.gained_focus() || field.changed() {
                doc.editing = Some((row, text.clone()));
            }
            if field.lost_focus() {
                if ui.input(|i| i.key_pressed(Key::Enter)) {
                    match kind.parse(&text) {
                        Some(value) => write = Some((kind.size(), value)),
                        None => doc.message = Some(format!("{} is not a {name}.", text.trim())),
                    }
                }
                doc.editing = None;
            }
            ui.end_row();
        }
    });
    if let Some((size, value)) = write {
        doc.message = None;
        doc.write_value(at, size, value);
    }
}

fn status(ui: &mut Ui, doc: &mut HexDoc) {
    let t = Tokens::get(ui.ctx());
    ui.painter().hline(ui.max_rect().x_range(), ui.cursor().top(), Stroke::new(1.0, t.border));
    ui.add_space(3.0);
    ui.horizontal_wrapped(|ui| {
        let mode = if doc.insert { "Insert" } else { "Overwrite" };
        if Button::new(mode).small().ghost().show(ui).on_hover_text("Insert key: whether typing inserts bytes or writes over them").clicked() {
            doc.insert = !doc.insert;
        }
        ui.separator();
        ui.monospace(format!("Offset: 0x{:X}", doc.caret));
        ui.separator();
        ui.monospace(format!("Size: 0x{:X} ({})", doc.bytes.len(), doc.bytes.len()));
        let selection = doc.selection();
        if selection.is_empty() {
            return;
        }
        ui.separator();
        let (start, end, count) = (selection.start, selection.end - 1, selection.len());
        ui.monospace(format!("Selection: 0x{start:X}–0x{end:X} ({count} byte{})", if count == 1 { "" } else { "s" }));
        if let Some((unsigned, signed)) = doc.selection_value() {
            ui.separator();
            let hex = format!("{unsigned:0width$X}", width = selection.len() * 2);
            let value = if signed == unsigned as i64 {
                format!("Value: 0x{hex} = {unsigned}")
            } else {
                format!("Value: 0x{hex} = {unsigned} ({signed} signed)")
            };
            ui.monospace(value).on_hover_text("The selected bytes as one integer, in the inspector's byte order");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(bytes: &[u8]) -> HexDoc {
        HexDoc::new(bytes.to_vec())
    }

    #[test]
    fn hex_reads_back_what_it_writes_and_refuses_half_bytes() {
        let bytes: Vec<u8> = (0..40).collect();
        let mut d = doc(&bytes);
        d.select(0..40);
        assert!(d.copied(CopyAs::Hex).starts_with("00 01 02"));
        assert_eq!(unhex(&d.copied(CopyAs::Hex)), Some(bytes));
        assert_eq!(unhex("0A 1b\\n\\tFF"), None, "escaped text is not hex");
        assert_eq!(unhex("0A 1b\n\tFF"), Some(vec![0x0a, 0x1b, 0xff]));
        assert_eq!(unhex("0A 1"), None);
        assert_eq!(unhex("0G"), None);
    }

    #[test]
    fn typing_two_digits_writes_one_byte_and_undoes_as_one_run() {
        let mut d = doc(&[0x11, 0x22, 0x33]);
        for digit in [0xa, 0xb, 0xc, 0xd] {
            d.type_digit(digit);
        }
        assert_eq!((d.bytes.as_slice(), d.caret), (&[0xab, 0xcd, 0x33][..], 2));
        assert!(d.dirty() && d.marks[0] && d.marks[1] && !d.marks[2]);
        d.undo();
        assert_eq!((d.bytes.as_slice(), d.caret, d.dirty()), (&[0x11, 0x22, 0x33][..], 0, false));
        assert!(!d.marks[0]);
        d.redo();
        assert_eq!(d.bytes, [0xab, 0xcd, 0x33]);
    }

    #[test]
    fn inserting_grows_the_file_and_typing_at_the_end_appends() {
        let mut d = doc(&[0x11]);
        d.insert = true;
        d.type_digit(0xf);
        d.type_digit(0x0);
        assert_eq!(d.bytes, [0xf0, 0x11]);
        d.insert = false;
        d.move_caret(2, false);
        d.type_text("A");
        assert_eq!(d.bytes, [0xf0, 0x11, b'A']);
        d.undo();
        d.undo();
        assert_eq!(d.bytes, [0x11]);
    }

    #[test]
    fn text_overwrites_and_a_selection_is_typed_over_from_its_start() {
        let mut d = doc(b"hello world");
        d.pane = Pane::Text;
        d.select(6..11);
        d.type_text("W");
        assert_eq!(d.bytes, b"hello World");
        d.insert = true;
        d.select(0..5);
        d.type_text("bye");
        assert_eq!(d.bytes, b"bye World");
    }

    #[test]
    fn delete_backspace_and_paste() {
        let mut d = doc(&[1, 2, 3, 4, 5]);
        d.move_caret(2, false);
        d.delete(false);
        assert_eq!(d.bytes, [1, 2, 4, 5]);
        d.delete(true);
        assert_eq!((d.bytes.as_slice(), d.caret), (&[1, 4, 5][..], 1));
        d.select(1..3);
        d.paste("AA BB CC");
        assert_eq!(d.bytes, [1, 0xaa, 0xbb, 0xcc]);
        assert_eq!(d.selection(), 1..4, "what was pasted is selected");
        d.pane = Pane::Text;
        d.move_caret(0, false);
        d.paste("hi");
        assert_eq!(d.bytes, [b'h', b'i', 0xbb, 0xcc], "overwriting, a paste writes over");
    }

    #[test]
    fn find_wraps_around_and_goes_backwards() {
        let mut d = doc(b"abcabcabc");
        assert!(d.find_next(b"bc", false));
        assert_eq!(d.selection(), 1..3);
        assert!(d.find_next(b"bc", false));
        assert_eq!(d.selection(), 4..6);
        assert!(d.find_next(b"bc", true));
        assert_eq!(d.selection(), 1..3);
        assert!(d.find_next(b"bc", true));
        assert_eq!(d.selection(), 7..9, "back past the start comes round from the end");
        assert!(!d.find_next(b"zz", false));
    }

    #[test]
    fn a_selection_reads_as_an_integer_in_either_byte_order() {
        let mut d = doc(&[0x34, 0x12, 0xff, 0xff]);
        d.select(0..2);
        assert_eq!(d.selection_value(), Some((0x1234, 0x1234)));
        d.endian = Endian::Big;
        assert_eq!(d.selection_value(), Some((0x3412, 0x3412)));
        d.select(2..4);
        assert_eq!(d.selection_value(), Some((0xffff, -1)));
        d.select(0..0);
        assert_eq!(d.selection_value(), None);
    }

    #[test]
    fn the_inspector_reads_and_writes_numbers() {
        assert_eq!(Kind::Signed(2).show(0xfffe), "-2");
        assert_eq!(Kind::Unsigned(3).parse("0x123456"), Some(0x12_3456));
        assert_eq!(Kind::Signed(1).parse("-129"), None);
        assert_eq!(Kind::Signed(1).parse("-128"), Some(0x80));
        assert_eq!(Kind::Unsigned(1).parse("256"), None);
        assert_eq!(Kind::Fixed(4).parse("1.5"), Some(0x1800));
        assert_eq!(Kind::Fixed(4).show(0xffff_f000), "-1");
        assert_eq!(Kind::Binary.parse("10000001"), Some(0x81));
        assert_eq!(Kind::Float32.parse("1"), Some(0x3f80_0000));
        let mut d = doc(&[0; 4]);
        d.write_value(0, 4, 0x1234_5678);
        assert_eq!(d.bytes, [0x78, 0x56, 0x34, 0x12]);
        d.endian = Endian::Big;
        d.write_value(0, 2, 0xabcd);
        assert_eq!(d.bytes, [0xab, 0xcd, 0x34, 0x12]);
    }

    #[test]
    fn offsets_and_copies() {
        assert_eq!(parse_offset("0x1F"), Some(31));
        assert_eq!(parse_offset("1f"), Some(31));
        assert_eq!(parse_offset("#31"), Some(31));
        assert_eq!(parse_offset("zz"), None);
        let mut d = doc(b"Hi\x00");
        d.select(0..3);
        assert_eq!(d.copied(CopyAs::Hex), "48 69 00");
        assert_eq!(d.copied(CopyAs::Text), "Hi.");
        assert_eq!(d.copied(CopyAs::Array), "{\n    0x48, 0x69, 0x00\n}");
    }

    #[test]
    fn saving_clears_the_marks() {
        let mut d = doc(&[0]);
        d.fill(1);
        assert!(!d.dirty(), "nothing selected, nothing filled");
        d.select(0..1);
        d.fill(0xff);
        assert!(d.dirty() && d.marks[0]);
        d.mark_saved();
        assert!(!d.dirty() && !d.marks[0]);
    }
}
