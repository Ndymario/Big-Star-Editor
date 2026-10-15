//! Draws the properties panel from `properties`' rows and turns what the person does into edits.

use bigstar_protocol::{LevelEdit, ObjectBankOverlays};
use egui::{RichText, Ui};

use crate::controls::{self, Button, Choice as _, TextField, Width};
use crate::level_view::{Item, Kind, LevelView};
use crate::properties::{
    Editor, PanelState, Row, Rows, Span, TABS, Tab, add_edit, add_node_edit, apply, apply_settings, common_settings, delete_edit, delete_node_edit,
    format_settings, format_value, item_count, line_label, list_lines, parse_settings, parse_value, rows, sprite_types,
};
use crate::{dialogs, icons};

const LABEL_WIDTH: f32 = 112.0;
/// The least a number's box is wide; a longer value widens its own.
const NUMBER_WIDTH: f32 = 52.0;
/// How much of its line a drop-down takes.
const CHOICE_SHARE: f32 = 0.55;
/// How many lines of the list show before it scrolls.
const LIST_ROWS: f32 = 7.0;
const NODE_INDENT: f32 = 16.0;

/// A field's editor. Returns the value to write when the person finished turning it.
fn editor(ui: &mut Ui, panel: &mut PanelState, index: usize, row: &Row) -> Option<i64> {
    let hex = panel.hex;
    match &row.editor {
        Editor::Number { min, max } => {
            // One below the range stands for "the selection disagrees", shown as a dash.
            let unset = min - 1;
            let shown = panel.dragging.filter(|(i, _)| *i == index).map(|(_, v)| v).or(row.value).unwrap_or(unset);
            let mut value = shown;
            // A spin box, laid out from the right: its arrows, then the number they step, close enough to read as one.
            let spacing = ui.spacing().clone();
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.spacing_mut().interact_size.x = NUMBER_WIDTH;
            let step = controls::steppers(ui);
            // The gap a widget leaves is set as it is placed: only the arrows keep the number close, and the number
            // stands off from the label before it as far as anything else does.
            ui.spacing_mut().item_spacing.x = spacing.item_spacing.x;
            let response = ui.add(
                controls::number(&mut value)
                    .range(unset..=*max)
                    .speed(0.25)
                    .update_while_editing(false)
                    .custom_formatter(move |n, _| if n as i64 == unset { "—".to_owned() } else { format_value(n as i64, hex) })
                    .custom_parser(move |text| parse_value(text, hex).map(|v| v as f64)),
            );
            *ui.spacing_mut() = spacing;
            if step != 0 {
                // An arrow steps from the value the selection agrees on; where it disagrees there is nothing to step from.
                return row.value.map(|v| (v + step).clamp(*min, *max)).filter(|v| Some(*v) != row.value);
            }
            if response.dragged() && value != shown {
                panel.dragging = Some((index, value));
            }
            if response.drag_stopped() {
                let dragged = panel.dragging.take().filter(|(i, _)| *i == index).map(|(_, v)| v);
                return dragged.filter(|v| *v >= *min && Some(*v) != row.value);
            }
            (response.changed() && !response.dragged() && value >= *min && Some(value) != row.value).then_some(value)
        }
        Editor::Choice(choices) => {
            let label = |value: i64| match choices.iter().find(|(v, _)| *v == value) {
                Some((v, text)) => format!("{} — {text}", format_value(*v, hex)),
                None => format!("{} — not listed", format_value(value, hex)),
            };
            let mut picked = None;
            // A share of the line, so the label and the note's icon keep theirs. A drop-down that took the whole
            // line would push them past the panel's edge, and the panel would widen to fit them every frame.
            let width = (ui.available_width() * CHOICE_SHARE).max(80.0);
            ui.allocate_ui(egui::vec2(width, ui.spacing().interact_size.y), |ui| {
                controls::drop_down(("property-choice", index))
                    .width(width)
                    .truncate()
                    .selected_text(row.value.map_or("—".to_owned(), label))
                    .show_ui(ui, |ui| {
                        for (value, text) in choices {
                            let entry = format!("{} — {text}", format_value(*value, hex));
                            if ui.choice(row.value == Some(*value), entry).on_hover_text(text).clicked() {
                                picked = Some(*value);
                            }
                        }
                    });
            });
            picked.filter(|v| Some(*v) != row.value)
        }
        Editor::Boolean => {
            let mut on = row.value == Some(1);
            // Undecided is something the data says -- a disagreeing selection, or bits that are neither off nor on --
            // not something the person can click back to.
            let undecided = !matches!(row.value, Some(0 | 1));
            let response = controls::checkbox_undecided(ui, &mut on, undecided, &row.label);
            response.clicked().then(|| if row.value == Some(1) { 0 } else { 1 })
        }
    }
}

/// One row: its label on the left (a checkbox is its own), its editor against the right edge, and its note between
/// them, so the editors of every row end on one line whatever their labels and notes.
fn row_ui(ui: &mut Ui, row: &Row, editor: impl FnOnce(&mut Ui) -> Option<i64>) -> Option<i64> {
    let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
    let leftwards = egui::Layout::left_to_right(egui::Align::Center);
    ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
        if row.editor == Editor::Boolean {
            return ui
                .with_layout(leftwards, |ui| {
                    let written = editor(ui);
                    if !row.note.is_empty() {
                        controls::hint(ui, &row.note);
                    }
                    written
                })
                .inner;
        }
        let written = editor(ui);
        if !row.note.is_empty() {
            controls::hint(ui, &row.note);
        }
        ui.with_layout(leftwards, |ui| ui.add(egui::Label::new(&row.label).truncate()).on_hover_text(&row.label));
        written
    })
    .inner
}

/// A sprite's Object ID as a drop-down of the actors by name, in place of a number nobody knows by heart. Returns
/// the type picked.
fn sprite_type_editor(ui: &mut Ui, row: &Row, types: &[(u16, String)]) -> Option<i64> {
    let selected = match row.value {
        Some(value) => match types.iter().find(|(t, _)| i64::from(*t) == value) {
            Some((t, name)) => format!("{t} — {name}"),
            None => format!("Actor {value}"),
        },
        None => "—".to_owned(),
    };
    let width = (ui.available_width() * CHOICE_SHARE).max(80.0);
    let filter_id = ui.id().with("sprite-picker-filter");
    let mut picked = None;
    ui.allocate_ui(egui::vec2(width, ui.spacing().interact_size.y), |ui| {
        controls::drop_down("sprite-picker").width(width).truncate().selected_text(selected).height(320.0).show_ui(ui, |ui| {
            // 300 actors are too many to address by number, so the list filters by what is typed.
            let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
            ui.add(TextField::new(&mut filter).hint("Filter actors…"));
            ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
            let needle = filter.to_lowercase();
            for (t, name) in types {
                let entry = format!("{t} — {name}");
                if !needle.is_empty() && !entry.to_lowercase().contains(&needle) {
                    continue;
                }
                let value = i64::from(*t);
                if ui.choice(row.value == Some(value), entry).clicked() && row.value != Some(value) {
                    picked = Some(value);
                }
            }
        });
    });
    picked
}

fn pairs_up(row: &Row) -> bool {
    row.span == Span::Half && !row.group.is_empty() && matches!(row.editor, Editor::Number { .. }) && row.note.is_empty()
}

/// The panel. Returns the edit the person made, if they made one.
pub fn show(ui: &mut Ui, view: &mut LevelView, overlays: Option<&ObjectBankOverlays>, busy: bool) -> Option<LevelEdit> {
    view.sync_panel();
    let reveal = std::mem::take(&mut view.panel.reveal);
    let mut edit = None;
    let mut bank_edit = None;
    ui.horizontal_wrapped(|ui| {
        for (tab, name) in TABS {
            if ui.choice(view.panel.tab == tab, name).clicked() && view.panel.tab != tab {
                view.panel_tab(tab);
            }
        }
    });
    ui.separator();
    let shown = rows(view, &view.panel, overlays);
    let tab = view.panel.tab;
    let count = item_count(&view.snapshot, tab);

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if tab == Tab::Entity(Kind::Sprite) {
            sprite_header(ui, view, &shown);
            bank_edit = missing_banks(ui, view, &shown, busy);
        }

        match tab {
            Tab::Entity(kind) => edit = item_list(ui, view, kind, &shown, reveal, busy),
            _ => {
                controls::section(ui, "Selection", |ui| {
                    ui.add_enabled_ui(tab != Tab::Course && count != 0, |ui| {
                        ui.horizontal(|ui| {
                            ui.add_sized([LABEL_WIDTH, 18.0], egui::Label::new("Item"));
                            let mut item = view.panel.item.min(count.saturating_sub(1));
                            if ui.add(controls::number(&mut item).range(0..=count.saturating_sub(1))).changed() {
                                view.panel_item(item);
                            }
                        });
                    });
                });
            }
        }

        controls::section(ui, "Properties", |ui| {
            if shown.rows.is_empty() {
                ui.weak(if count == 0 { "Nothing of this kind in the level." } else { "Nothing to edit here." });
            }
            let mut written = None;
            let types = if tab == Tab::Entity(Kind::Sprite) { sprite_types(view) } else { Vec::new() };
            ui.add_enabled_ui(!busy, |ui| {
                let mut index = 0;
                while let Some(row) = shown.rows.get(index) {
                    match shown.rows.get(index + 1).filter(|next| pairs_up(row) && pairs_up(next)) {
                        Some(next) => {
                            // Peers share a line under the name of what they are together: that on the left, and each
                            // one's own short label and box against the right edge, the second where every row's box is.
                            let size = egui::vec2(ui.available_width(), ui.spacing().interact_size.y);
                            ui.allocate_ui_with_layout(size, egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                for (offset, current) in [(1, next), (0, row)] {
                                    if let Some(value) = editor(ui, &mut view.panel, index + offset, current) {
                                        written = Some((index + offset, value));
                                    }
                                    ui.label(format!("{}:", current.label));
                                }
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                    ui.add(egui::Label::new(&row.group).truncate()).on_hover_text(&row.group)
                                });
                            });
                            index += 2;
                        }
                        None => {
                            // A sprite's first row is its type, picked by name wherever the actors are known.
                            let value = if tab == Tab::Entity(Kind::Sprite) && index == 0 && !types.is_empty() {
                                row_ui(ui, row, |ui| sprite_type_editor(ui, row, &types))
                            } else {
                                let panel = &mut view.panel;
                                row_ui(ui, row, |ui| editor(ui, panel, index, row))
                            };
                            if let Some(value) = value {
                                written = Some((index, value));
                            }
                            index += 1;
                        }
                    }
                }
            });
            controls::checkbox(ui, &mut view.panel.hex, "Display as hex");
            if let Some((row, value)) = written {
                edit = apply(view, &view.panel, &shown, row, value);
            }
        });

        // The raw value is the sprite; the declared fields are only a reading of it.
        if tab == Tab::Entity(Kind::Sprite) && count != 0 {
            controls::section(ui, "Raw Fields", |ui| {
                let current = common_settings(view, &shown).map(format_settings).unwrap_or_default();
                let mut text = view.panel.settings_text.clone().unwrap_or_else(|| current.clone());
                let response = ui
                    .add_enabled(!busy, TextField::new(&mut text).code().width(Width::Px(140.0)).hint("—"))
                    .on_hover_text("The settings word from its high byte down.");
                if response.changed() {
                    // A half-typed value is not a value: the box keeps it until it is whole.
                    if let Some(settings) = parse_settings(&text).filter(|_| text.trim() != current) {
                        edit = apply_settings(view, &shown, settings);
                    }
                    view.panel.settings_text = Some(text);
                } else if !response.has_focus() {
                    view.panel.settings_text = None;
                }
            });
        }
    });
    actor_info(ui.ctx(), view, &shown);
    edit.or(bank_edit)
}

/// Everything of the tab's kind, one line each, the selected ones lit. NSMB Editor's list: picking a line selects it
/// on the canvas and scrolls there, a path as a whole, and a modifier adds to what is selected. Miyamoto's as well: a
/// path opens onto its nodes, each one a line of its own. `reveal` scrolls the list to a selection made elsewhere.
/// What adds to the list and takes from it sits under it. Returns the edit one of those buttons made.
fn item_list(ui: &mut Ui, view: &mut LevelView, kind: Kind, shown: &Rows, reveal: bool, busy: bool) -> Option<LevelEdit> {
    let tab = Tab::Entity(kind);
    let lines = list_lines(view, kind);
    let mut edit = None;
    let line_of = |wanted: Item| lines.iter().position(|line| *line == wanted);
    let target = view.selection.first().filter(|_| reveal).and_then(|first| line_of(*first).or_else(|| line_of(Item::new(first.kind, first.index))));
    let mut picked = None;
    controls::section(ui, &format!("{}s", kind.name()), |ui| {
        let step = controls::HEIGHT_SM + ui.spacing().item_spacing.y;
        // The lines sit in a well of their own, as a list box does: its edges line up with the title above and the
        // buttons below, and the lines keep their padding inside it.
        let t = crate::theme::Tokens::get(ui.ctx());
        egui::Frame::NONE
            .fill(t.well)
            .stroke(egui::Stroke::new(1.0, t.border))
            .corner_radius(egui::CornerRadius::same(controls::RADIUS))
            .inner_margin(egui::Margin::same(3))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                if lines.is_empty() {
                    ui.weak("None in this level.");
                    return;
                }
                egui::ScrollArea::vertical().id_salt("item-list").max_height(LIST_ROWS * step).auto_shrink([false, true]).show_rows(
                    ui,
                    controls::HEIGHT_SM,
                    lines.len(),
                    |ui, range| {
                        if let Some(line) = target {
                            // Only the lines in view are laid out, so where the wanted one would be is worked out from them.
                            let top = ui.max_rect().top() + (line as f32 - range.start as f32) * step;
                            ui.scroll_to_rect(
                                egui::Rect::from_min_size(egui::pos2(ui.max_rect().left(), top), egui::vec2(1.0, controls::HEIGHT_SM)),
                                None,
                            );
                        }
                        for &item in lines.get(range).unwrap_or_default() {
                            let chosen = view.is_selected(item.kind, item.index, item.node);
                            let indent = if item.node.is_some() { NODE_INDENT } else { 0.0 };
                            if controls::Row::new(line_label(view, item)).selected(chosen).indent(indent).show(ui).clicked() {
                                picked = Some((item, ui.input(|i| i.modifiers.shift || i.modifiers.command)));
                            }
                        }
                    },
                );
            });
        ui.add_enabled_ui(!busy, |ui| {
            // Each pair shares the line evenly, whatever the panel's width.
            let half = egui::vec2(((ui.available_width() - ui.spacing().item_spacing.x) / 2.0).max(0.0), controls::HEIGHT);
            ui.horizontal_top(|ui| {
                if ui.add_sized(half, Button::new("Add")).clicked() {
                    view.expect_placement();
                    edit = add_edit(&view.snapshot, tab);
                }
                if ui.add_sized(half, Button::new("Delete").enabled(!lines.is_empty())).clicked() {
                    edit = delete_edit(tab, shown);
                }
            });
            if kind.path() && !shown.multi() && !lines.is_empty() {
                ui.horizontal_top(|ui| {
                    let nodes = shown.indices.first().and_then(|&i| view.path(kind, i)).map_or(0, Vec::len);
                    if ui.add_sized(half, Button::new("Add Node")).clicked() {
                        edit = add_node_edit(&view.snapshot, tab, shown);
                        view.panel_node(nodes);
                    }
                    if ui.add_sized(half, Button::new("Delete Node").enabled(nodes != 0)).clicked() {
                        edit = delete_node_edit(&view.snapshot, tab, shown);
                    }
                });
            }
        });
    });
    if let Some((item, add)) = picked {
        view.pick(item, add);
    }
    edit
}

/// The name of the actor the sprite resolved to, and the way into what it says about itself.
fn sprite_header(ui: &mut Ui, view: &mut LevelView, shown: &Rows) {
    let sprite_type = shown.indices.first().and_then(|&i| view.snapshot.course.sprites.get(i)).map(|s| s.id);
    ui.horizontal(|ui| {
        ui.vertical(|ui| match &shown.actor {
            Some(actor) => {
                ui.label(RichText::new(&actor.definition.name).strong());
                let number = sprite_type.map_or("—".to_owned(), |t| t.to_string());
                ui.weak(format!("{} · actor type {number}", actor.definition.category));
            }
            None => {
                let name = sprite_type.map_or("No sprite selected".to_owned(), |t| format!("Actor {t}"));
                ui.label(RichText::new(name).color(ui.visuals().weak_text_color()).strong());
                ui.weak(if shown.multi() { "Several sprites selected" } else { "Not in the actor catalog" });
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::icon(icons::INFO).ghost().name("About this actor").enabled(shown.actor.is_some()))
                .on_hover_text("What this actor is, and what each of its fields does")
                .clicked()
            {
                view.panel.show_actor_info = true;
            }
        });
    });
    ui.add_space(4.0);
}

/// A warning for each object bank the selected sprites need that the level does not load, with a button that loads
/// it. Loading one bank in a slot unloads the one there, so it says what that would leave out instead.
fn missing_banks(ui: &mut Ui, view: &LevelView, shown: &Rows, busy: bool) -> Option<LevelEdit> {
    let mut needed: Vec<(u8, u8)> = shown.indices.iter().filter_map(|&index| view.missing_bank(index)).collect();
    needed.sort_unstable();
    needed.dedup();
    let mut edit = None;
    for (slot, bank) in needed {
        let loaded = view.snapshot.course.object_banks.get(usize::from(slot)).copied().unwrap_or(0);
        let what = if shown.multi() { "A selected sprite" } else { "This sprite" };
        let mut text = if loaded == 0 {
            format!("{what} needs object bank {bank} in slot {slot}, which this level leaves empty, so the game leaves it out.")
        } else {
            format!("{what} needs object bank {bank} in slot {slot}, where this level loads bank {loaded}, so the game leaves it out.")
        };
        let displaced = view.sprites_needing_other_bank(slot, bank);
        if displaced > 0 {
            let sprites = if displaced == 1 { "sprite" } else { "sprites" };
            text.push_str(&format!(" Loading it would leave out the {displaced} {sprites} that need bank {loaded} instead."));
        }
        controls::warning_text(ui, text);
        let button = Button::new(format!("Load bank {bank} in slot {slot}")).enabled(!busy);
        if ui.add(button).on_hover_text("Changes the level's object bank for this slot, as the Banks tab does").clicked() {
            edit = Some(LevelEdit::SetObjectBank { slot: usize::from(slot), value: bank });
        }
        ui.add_space(4.0);
    }
    edit
}

/// What the actor is, and what all of its fields do, read off the same declarations the rows are built from.
fn actor_info(ctx: &egui::Context, view: &mut LevelView, shown: &Rows) {
    let Some(actor) = shown.actor.as_ref().filter(|_| view.panel.show_actor_info) else { return };
    let definition = &actor.definition;
    let mut open = true;
    dialogs::window(&definition.name).open(&mut open).default_size([520.0, 420.0]).show(ctx, |ui| {
        ui.weak(format!("{} · object {}", definition.category, definition.object_id));
        ui.label(if definition.notes.is_empty() { "This actor has no description." } else { &definition.notes });
        if let Some(error) = &actor.init_error {
            controls::warning_text(ui, format!("Its script could not start: {error}"));
        }
        ui.separator();
        egui::ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("actor-fields").num_columns(3).striped(true).show(ui, |ui| {
                ui.weak("Field");
                ui.weak("Bits");
                ui.weak("What it does");
                ui.end_row();
                for field in &definition.fields {
                    let bits = if field.bit_width == 1 {
                        field.bit_offset.to_string()
                    } else {
                        format!("{}–{}", field.bit_offset, u32::from(field.bit_offset) + u32::from(field.bit_width) - 1)
                    };
                    ui.label(&field.label);
                    ui.label(bits);
                    ui.label(&field.notes);
                    ui.end_row();
                    for choice in &field.choices {
                        ui.label(format!("    {}", choice.value));
                        ui.label("");
                        ui.label(&choice.label);
                        ui.end_row();
                    }
                    if field.checkbox_value != 0 {
                        ui.label(format!("    {}", field.checkbox_value));
                        ui.label("");
                        ui.label("On; anything else is off");
                        ui.end_row();
                    }
                }
            });
        });
    });
    if !open {
        view.panel.show_actor_info = false;
    }
}
