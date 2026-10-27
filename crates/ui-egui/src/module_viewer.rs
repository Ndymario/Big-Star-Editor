//! NCPatcher runtime modules (`.ncrt`) as a read-only file document: the header and how the image is laid out, the
//! patches the module writes into the game when it is loaded, its exports object, and its code as assembly. The image
//! is built by NCPatcher, so nothing here is edited.

use crate::controls::{self, Button, Choice, Row};
use crate::icons;
use crate::services::Services;
use crate::theme::Tokens;
use bigstar_formats::{AsmLine, CodeMode, ListingMode, NcrtModule, NcrtPatchKind, disassemble_ncrt, parse_ncrt};
use egui::{FontId, Sense, Ui, pos2, vec2};

pub struct ModuleDoc {
    pub path: String,
    pub module: Result<NcrtModule, String>,
    mode: ListingMode,
    listing: Vec<ListingRow>,
}

/// A row of the listing: a label line before an instruction, or the instruction itself.
enum ListingRow {
    Label(String),
    Line(AsmLine),
}

impl ModuleDoc {
    pub fn open(path: &str, bytes: &[u8]) -> Self {
        let mut doc = Self { path: path.to_owned(), module: parse_ncrt(bytes).map_err(|e| e.message), mode: ListingMode::Auto, listing: Vec::new() };
        doc.relist();
        doc
    }

    /// Whether this viewer reads a file: a runtime module that parses.
    pub fn supports(bytes: &[u8]) -> bool {
        parse_ncrt(bytes).is_ok()
    }

    fn relist(&mut self) {
        let Ok(module) = &self.module else { return };
        self.listing = disassemble_ncrt(module, self.mode)
            .into_iter()
            .flat_map(|line| line.labels.clone().into_iter().map(ListingRow::Label).chain([ListingRow::Line(line)]))
            .collect();
    }

    /// The listing's row for an offset: its first label when it has one.
    fn row_of(&self, offset: u32) -> Option<usize> {
        self.listing
            .iter()
            .position(|row| match row {
                ListingRow::Label(_) => false,
                ListingRow::Line(line) => line.offset >= offset,
            })
            .map(|row| {
                let mut first = row;
                while first > 0 && matches!(self.listing.get(first - 1), Some(ListingRow::Label(_))) {
                    first -= 1;
                }
                first
            })
    }

    /// The listing as an assembly source, for saving.
    pub fn listing_text(&self) -> String {
        let mut text = format!("@ {}\n", self.path);
        for row in &self.listing {
            match row {
                ListingRow::Label(name) => text.push_str(&format!("{name}:\n")),
                ListingRow::Line(line) => {
                    let comment = if line.comment.is_empty() { String::new() } else { format!(" @ {}", line.comment) };
                    text.push_str(&format!("    {:<40} @ +0x{:04X}: {}{comment}\n", line.text, line.offset, line.bytes));
                }
            }
        }
        text
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModuleTab {
    #[default]
    Overview,
    Patches,
    Exports,
    Assembly,
}

/// The tab in front, and where in the listing the person has been sent and came from.
#[derive(Default)]
pub struct ModuleView {
    tab: ModuleTab,
    /// The offset to bring into view and mark on the next frame.
    reveal: Option<u32>,
    marked: Option<u32>,
    back: Vec<u32>,
    notice: Option<String>,
}

impl ModuleView {
    fn go(&mut self, offset: u32) {
        if let Some(from) = self.marked {
            self.back.push(from);
        }
        self.tab = ModuleTab::Assembly;
        self.reveal = Some(offset);
        self.marked = Some(offset);
    }
}

pub fn show(ui: &mut Ui, doc: &mut ModuleDoc, view: &mut ModuleView, services: &dyn Services) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&doc.path).strong());
        ui.weak("· NCPatcher runtime module, read only");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if Button::new("Export listing…").enabled(doc.module.is_ok()).show(ui).clicked() {
                let name = doc.path.rsplit(['/', '!']).next().unwrap_or("module").to_owned();
                let stem = name.rsplit_once('.').map_or(name.as_str(), |(stem, _)| stem);
                if let Err(error) = services.save_file("Export listing", &format!("{stem}.s"), "s", doc.listing_text().as_bytes()) {
                    view.notice = Some(error);
                }
            }
        });
    });
    let module = match &doc.module {
        Ok(module) => module,
        Err(error) => {
            controls::error_text(ui, error);
            return;
        }
    };
    if let Some(notice) = &view.notice {
        controls::warning_text(ui, notice);
    }
    controls::tabs(
        ui,
        ("module-tabs", &doc.path),
        &mut view.tab,
        &[(ModuleTab::Overview, "Overview"), (ModuleTab::Patches, "Patches"), (ModuleTab::Exports, "Exports"), (ModuleTab::Assembly, "Assembly")],
    );
    ui.add_space(6.0);
    match view.tab {
        ModuleTab::Overview => overview(ui, module, view),
        ModuleTab::Patches => patches(ui, module, view),
        ModuleTab::Exports => exports(ui, module, view),
        ModuleTab::Assembly => assembly(ui, doc, view),
    }
}

fn size(bytes: u32) -> String {
    format!("0x{bytes:X} ({bytes} bytes)")
}

fn overview(ui: &mut Ui, module: &NcrtModule, view: &mut ModuleView) {
    egui::ScrollArea::vertical().id_salt("module-overview").auto_shrink([false, false]).show(ui, |ui| {
        controls::section(ui, "Header", |ui| {
            egui::Grid::new("module-header").num_columns(2).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
                let mut field = |name: &str, value: String| {
                    ui.label(name);
                    ui.monospace(value);
                    ui.end_row();
                };
                field("Format version", module.version.to_string());
                field("Image size", size(module.size()));
                field("Linked for", format!("0x{:08X}", module.base));
                field("Load alignment", format!("{} bytes", 1u32 << module.align_log2.min(31)));
                field("Patch count", module.patches.len().to_string());
                field("Relocations", module.relocs.len().to_string());
                field("Exports object", if module.exports.is_some() { "yes".to_owned() } else { "none".to_owned() });
                field("Static destructor room", format!("{} object(s)", module.atexit_capacity));
            });
        });
        ui.add_space(8.0);
        controls::section(ui, "Layout", |ui| {
            egui::Grid::new("module-regions").num_columns(3).striped(true).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
                ui.strong("Part");
                ui.strong("Offsets");
                ui.strong("Size");
                ui.end_row();
                for region in module.regions() {
                    ui.label(region.name);
                    ui.monospace(format!("+0x{:04X} – +0x{:04X}", region.start, region.end));
                    ui.monospace(size(region.end - region.start));
                    ui.end_row();
                }
            });
        });
        ui.add_space(8.0);
        controls::section(ui, "Constructors and destructors", |ui| {
            if module.constructors.is_empty() && module.destructors.is_empty() {
                ui.weak("This module has none.");
            }
            let functions = module.constructors.iter().map(|a| ("Constructor", a)).chain(module.destructors.iter().map(|a| ("Destructor", a)));
            for (kind, address) in functions {
                let set = if address & 1 != 0 { "THUMB" } else { "ARM" };
                let Some(offset) = module.offset_of(address & !1) else { continue };
                if Row::new(format!("{kind} at +0x{offset:04X} ({set})")).show(ui).on_hover_text("Show it in the listing").clicked() {
                    view.go(offset);
                }
            }
        });
    });
}

fn patches(ui: &mut Ui, module: &NcrtModule, view: &mut ModuleView) {
    if module.patches.is_empty() {
        controls::notice(ui, icons::INFO, "No patches", "This module writes nothing into the game when it is loaded.");
        return;
    }
    ui.weak("Written into the game when the module is attached, in this order, and reverted last first when it is detached.");
    egui::ScrollArea::vertical().id_salt("module-patches").auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("module-patch-list").num_columns(5).striped(true).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
            for heading in ["#", "Game address", "Kind", "Size", "Writes"] {
                ui.strong(heading);
            }
            ui.end_row();
            for (index, patch) in module.patches.iter().enumerate() {
                ui.monospace(index.to_string());
                ui.monospace(format!("0x{:08X}", patch.address));
                ui.label(patch.kind.name());
                ui.monospace(patch.size.to_string());
                let offset = module.offset_of(patch.value & !1);
                let what = match (patch.kind, offset) {
                    (NcrtPatchKind::Copy, Some(offset)) => {
                        let bytes = module.image.get(offset as usize..offset as usize + usize::from(patch.size)).unwrap_or_default();
                        let shown: Vec<String> = bytes.iter().take(16).map(|b| format!("{b:02X}")).collect();
                        format!("{}{}  (from +0x{offset:04X})", shown.join(" "), if bytes.len() > 16 { " …" } else { "" })
                    }
                    (_, Some(offset)) => {
                        let set = match patch.kind.destination_mode() {
                            Some(CodeMode::Thumb) => " (THUMB)",
                            _ => "",
                        };
                        format!("a branch to +0x{offset:04X}{set}")
                    }
                    (_, None) => format!("a branch to 0x{:08X}", patch.value),
                };
                match offset {
                    Some(offset) => {
                        if Row::new(egui::RichText::new(what).monospace()).fit().show(ui).on_hover_text("Show it in the listing").clicked() {
                            view.go(offset);
                        }
                    }
                    None => {
                        ui.monospace(what);
                    }
                }
                ui.end_row();
            }
        });
    });
}

fn exports(ui: &mut Ui, module: &NcrtModule, view: &mut ModuleView) {
    let words = module.export_words();
    if words.is_empty() {
        controls::notice(ui, icons::INFO, "No exports", "This module gives its loader nothing through ncprt_exports.");
        return;
    }
    ui.weak("The object ncprt_exports_of returns. Its type is the module's own, so it is shown a word at a time.");
    egui::ScrollArea::vertical().id_salt("module-exports").auto_shrink([false, false]).show(ui, |ui| {
        egui::Grid::new("module-export-list").num_columns(4).striped(true).spacing(vec2(16.0, 4.0)).show(ui, |ui| {
            for heading in ["Field offset", "Word", "As an int", "Points at"] {
                ui.strong(heading);
            }
            ui.end_row();
            let start = module.exports.unwrap_or(0);
            for word in words {
                ui.monospace(format!("+0x{:X}", word.offset - start));
                ui.monospace(format!("0x{:08X}", word.value));
                ui.monospace((word.value as i32).to_string());
                match word.target {
                    Some(target) => {
                        let code = (bigstar_formats::NCRT_HEADER_SIZE..module.code_end()).contains(&target);
                        let what = match (code, word.value & 1 != 0) {
                            (true, true) => format!("THUMB function at +0x{target:04X}"),
                            (true, false) => format!("function or data at +0x{target:04X}"),
                            (false, _) => format!("data at +0x{target:04X}"),
                        };
                        if Row::new(egui::RichText::new(what).monospace()).fit().show(ui).on_hover_text("Show it in the listing").clicked() {
                            view.go(target);
                        }
                    }
                    None => {
                        ui.weak("not an address in the module");
                    }
                }
                ui.end_row();
            }
        });
    });
}

fn assembly(ui: &mut Ui, doc: &mut ModuleDoc, view: &mut ModuleView) {
    ui.horizontal(|ui| {
        ui.label("Instruction set");
        let before = doc.mode;
        let name = |mode: ListingMode| match mode {
            ListingMode::Auto => "Follow the code",
            ListingMode::Arm => "All ARM",
            ListingMode::Thumb => "All THUMB",
        };
        controls::drop_down(("module-mode", &doc.path)).selected_text(name(doc.mode)).show_ui(ui, |ui| {
            for mode in [ListingMode::Auto, ListingMode::Arm, ListingMode::Thumb] {
                ui.choice_value(&mut doc.mode, mode, name(mode));
            }
        });
        if doc.mode != before {
            doc.relist();
        }
        controls::hint(
            ui,
            "A module keeps no symbols. The listing names what the header, the patches and the exports point at, reads each \
             as the instruction set they say, and follows the branches and loads from there.",
        );
        if Button::new("Back").enabled(!view.back.is_empty()).show(ui).clicked()
            && let Some(offset) = view.back.pop()
        {
            view.reveal = Some(offset);
            view.marked = Some(offset);
        }
    });
    ui.add_space(4.0);

    let t = Tokens::get(ui.ctx());
    let font = FontId::monospace(13.0);
    let char_width = ui.painter().layout_no_wrap("0".to_owned(), font.clone(), t.foreground).size().x;
    let row_height = font.size + 5.0;
    let mut scroll = egui::ScrollArea::both().id_salt(("module-listing", &doc.path)).auto_shrink([false, false]);
    if let Some(row) = view.reveal.take().and_then(|offset| doc.row_of(offset)) {
        scroll = scroll.vertical_scroll_offset((row as f32 * row_height - 3.0 * row_height).max(0.0));
    }
    let columns = [0.0, 8.0, 19.0, 61.0].map(|c| c * char_width + 6.0);
    let mut follow = None;
    scroll.show_rows(ui, row_height, doc.listing.len(), |ui, visible| {
        for row in doc.listing.get(visible).unwrap_or_default() {
            let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width().max(columns[3] + 40.0 * char_width), row_height), Sense::click());
            let painter = ui.painter_at(rect);
            let y = rect.center().y;
            let text = |column: usize, text: &str, color| {
                painter.text(
                    pos2(rect.left() + columns.get(column).copied().unwrap_or(0.0), y),
                    egui::Align2::LEFT_CENTER,
                    text,
                    font.clone(),
                    color,
                );
            };
            match row {
                ListingRow::Label(name) => text(0, &format!("{name}:"), t.primary_text),
                ListingRow::Line(line) => {
                    if view.marked == Some(line.offset) {
                        painter.rect_filled(rect, 2.0, t.primary_subtle);
                    } else if line.target.is_some() && response.hovered() {
                        painter.rect_filled(rect, 2.0, t.surface_raised);
                    }
                    text(0, &format!("+{:04X}", line.offset), t.muted);
                    text(1, &line.bytes, t.subtle);
                    text(2, &line.text, t.foreground);
                    if !line.comment.is_empty() {
                        text(3, &format!("; {}", line.comment), t.muted);
                    }
                    if let Some(target) = line.target
                        && response.on_hover_text(format!("Go to +0x{target:04X}")).clicked()
                    {
                        follow = Some(target);
                    }
                }
            }
        }
    });
    if let Some(target) = follow {
        view.go(target);
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// The smallest module there is: a header, and nothing in it.
    pub fn empty_module() -> Vec<u8> {
        let base = 0x1000_0000u32;
        let fields = [0x5452_434E, 1, base, 60, 0, base + 60, 0, base + 60, 0, base + 60, 0, base + 60, 0, base + 60, 0];
        fields.iter().flat_map(|f: &u32| f.to_le_bytes()).collect()
    }

    #[test]
    fn a_module_that_does_not_parse_is_not_claimed() {
        assert!(ModuleDoc::supports(&empty_module()));
        assert!(!ModuleDoc::supports(b"NCRT"));
        assert!(!ModuleDoc::supports(b"hello"));
    }

    #[test]
    fn an_empty_module_lists_nothing_and_says_so() {
        let doc = ModuleDoc::open("z_new/ncprt/empty.ncrt", &empty_module());
        assert!(doc.module.is_ok());
        assert!(doc.listing.is_empty());
        assert_eq!(doc.listing_text(), "@ z_new/ncprt/empty.ncrt\n");
    }
}
