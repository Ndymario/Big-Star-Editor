//! Draws the properties panel from `properties`' rows and turns what the person does into edits.

use bigstar_protocol::{LevelEdit, ObjectBankOverlays};
use egui::{RichText, Ui};

use crate::controls::{self, Button, Choice as _, TextField, Width};
use crate::level_view::{Kind, LevelView};
use crate::properties::{
    Editor, PanelState, Row, Rows, Span, TABS, Tab, add_edit, add_node_edit, apply, apply_settings, common_settings, delete_edit, delete_node_edit,
    format_settings, format_value, item_count, parse_settings, parse_value, rows, sprite_types,
};
use crate::{dialogs, icons};

const LABEL_WIDTH: f32 = 112.0;

/// A field's editor. Returns the value to write when the person finished turning it.
fn editor(ui: &mut Ui, panel: &mut PanelState, index: usize, row: &Row) -> Option<i64> {
    let hex = panel.hex;
    match &row.editor {
        Editor::Number { min, max } => {
            // One below the range stands for "the selection disagrees", shown as a dash.
            let unset = min - 1;
            let shown = panel.dragging.filter(|(i, _)| *i == index).map(|(_, v)| v).or(row.value).unwrap_or(unset);
            let mut value = shown;
            let response = ui.add(
                controls::number(&mut value)
                    .range(unset..=*max)
                    .speed(0.25)
                    .update_while_editing(false)
                    .custom_formatter(move |n, _| if n as i64 == unset { "—".to_owned() } else { format_value(n as i64, hex) })
                    .custom_parser(move |text| parse_value(text, hex).map(|v| v as f64)),
            );
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
            // The note's icon follows on the same line. A drop-down that took the whole line would push it past the
            // panel's edge, and the panel would widen to fit it every frame.
            let note = if row.note.is_empty() { 0.0 } else { icons::SIZE_SM + ui.spacing().item_spacing.x };
            let width = (ui.available_width() - note).max(80.0);
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

/// One row: its label (a checkbox is its own), its editor and its note.
fn row_ui(ui: &mut Ui, panel: &mut PanelState, index: usize, row: &Row, label_width: f32) -> Option<i64> {
    ui.horizontal(|ui| {
        if row.editor != Editor::Boolean {
            ui.add_sized([label_width, 18.0], egui::Label::new(&row.label).truncate()).on_hover_text(&row.label);
        }
        let written = editor(ui, panel, index, row);
        if !row.note.is_empty() {
            controls::hint(ui, &row.note);
        }
        written
    })
    .inner
}

fn pairs_up(row: &Row) -> bool {
    row.span == Span::Half && matches!(row.editor, Editor::Number { .. }) && row.note.is_empty()
}

/// The panel. Returns the edit the person made, if they made one.
pub fn show(ui: &mut Ui, view: &mut LevelView, overlays: Option<&ObjectBankOverlays>, busy: bool) -> Option<LevelEdit> {
    view.sync_panel();
    let mut edit = None;
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
            sprite_header(ui, view, &shown, busy, &mut edit);
        }

        controls::section(ui, "Selection", |ui| {
            ui.add_enabled_ui(tab != Tab::Course && count != 0 && !shown.multi(), |ui| {
                ui.horizontal(|ui| {
                    ui.add_sized([LABEL_WIDTH, 18.0], egui::Label::new("Item"));
                    let mut item = view.panel.item.min(count.saturating_sub(1));
                    if ui.add(controls::number(&mut item).range(0..=count.saturating_sub(1))).changed() {
                        view.panel_item(item);
                    }
                    if shown.multi() {
                        ui.weak(format!("{} selected", shown.indices.len()));
                    }
                });
            });
            if let Tab::Entity(kind @ (Kind::Path | Kind::ProgressPath)) = tab
                && !shown.multi()
            {
                let nodes = shown.indices.first().and_then(|&i| view.path(kind, i)).map_or(0, Vec::len);
                ui.add_enabled_ui(nodes != 0, |ui| {
                    ui.horizontal(|ui| {
                        ui.add_sized([LABEL_WIDTH, 18.0], egui::Label::new("Node"));
                        let mut node = shown.node;
                        if ui.add(controls::number(&mut node).range(0..=nodes.saturating_sub(1))).changed() {
                            view.panel_node(node);
                        }
                    });
                });
            }
        });

        controls::section(ui, "Properties", |ui| {
            if shown.rows.is_empty() {
                ui.weak(if count == 0 { "Nothing of this kind in the level." } else { "Nothing to edit here." });
            }
            let mut written = None;
            ui.add_enabled_ui(!busy, |ui| {
                let mut index = 0;
                while let Some(row) = shown.rows.get(index) {
                    match shown.rows.get(index + 1).filter(|next| pairs_up(row) && pairs_up(next)) {
                        Some(next) => {
                            // Peers share a line, and their labels are as wide as the longer one so the editors line up.
                            let half = (ui.available_width() / 2.0 - 8.0).max(60.0);
                            ui.horizontal(|ui| {
                                for (offset, current) in [(0, row), (1, next)] {
                                    ui.allocate_ui(egui::vec2(half, 22.0), |ui| {
                                        if let Some(value) = row_ui(ui, &mut view.panel, index + offset, current, 72.0) {
                                            written = Some((index + offset, value));
                                        }
                                    });
                                }
                            });
                            index += 2;
                        }
                        None => {
                            if let Some(value) = row_ui(ui, &mut view.panel, index, row, LABEL_WIDTH) {
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

        if let Tab::Entity(kind) = tab {
            ui.add_enabled_ui(!busy, |ui| {
                ui.horizontal(|ui| {
                    if Button::new("Add").show(ui).clicked() {
                        view.expect_placement();
                        edit = add_edit(&view.snapshot, tab);
                    }
                    if Button::new("Delete").enabled(count != 0).show(ui).clicked() {
                        edit = delete_edit(tab, &shown);
                    }
                });
                if kind.path() && !shown.multi() && count != 0 {
                    ui.horizontal(|ui| {
                        let nodes = shown.indices.first().and_then(|&i| view.path(kind, i)).map_or(0, Vec::len);
                        if Button::new("Add Node").show(ui).clicked() {
                            edit = add_node_edit(&view.snapshot, tab, &shown);
                            view.panel_node(nodes);
                        }
                        if Button::new("Delete Node").enabled(nodes != 0).show(ui).clicked() {
                            edit = delete_node_edit(&view.snapshot, tab, &shown);
                        }
                    });
                }
            });
        }
    });
    actor_info(ui.ctx(), view, &shown);
    edit
}

/// The name of the actor the sprite resolved to, and a picker that retypes it by name.
fn sprite_header(ui: &mut Ui, view: &mut LevelView, shown: &Rows, busy: bool, edit: &mut Option<LevelEdit>) {
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
    let entries = sprite_types(view);
    let enabled = !entries.is_empty() && !shown.multi() && sprite_type.is_some() && !busy;
    let current = sprite_type.and_then(|t| entries.iter().find(|(e, _)| *e == t));
    let selected = match (current, sprite_type) {
        (Some((t, name)), _) => format!("{t} — {name}"),
        (None, Some(t)) => format!("Actor {t}"),
        (None, None) => String::new(),
    };
    let filter_id = ui.id().with("sprite-picker-filter");
    ui.add_enabled_ui(enabled, |ui| {
        controls::drop_down("sprite-picker").width(ui.available_width()).truncate().selected_text(selected).height(320.0).show_ui(ui, |ui| {
            // 300 actors are too many to address by number, so the list filters by what is typed.
            let mut filter: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
            ui.add(TextField::new(&mut filter).hint("Filter actors…"));
            ui.data_mut(|d| d.insert_temp(filter_id, filter.clone()));
            let needle = filter.to_lowercase();
            for (t, name) in &entries {
                let entry = format!("{t} — {name}");
                if !needle.is_empty() && !entry.to_lowercase().contains(&needle) {
                    continue;
                }
                if ui.choice(Some(*t) == sprite_type, entry).clicked() && Some(*t) != sprite_type {
                    // The picker turns the Object ID row, so there is one way the type is written.
                    *edit = apply(view, &view.panel, shown, 0, i64::from(*t));
                }
            }
        });
    });
    ui.add_space(4.0);
}

/// What the actor is, and what all of its fields do, read off the same declarations the rows are built from.
fn actor_info(ctx: &egui::Context, view: &mut LevelView, shown: &Rows) {
    let Some(actor) = shown.actor.as_ref().filter(|_| view.panel.show_actor_info) else { return };
    let definition = &actor.definition;
    let mut open = true;
    dialogs::window(&definition.name).open(&mut open).default_size([520.0, 420.0]).show(ctx, |ui| {
        ui.weak(format!("{} · object {}", definition.category, definition.object_id));
        ui.label(if definition.notes.is_empty() { "This actor documents nothing about itself." } else { &definition.notes });
        if let Some(error) = &actor.init_error {
            controls::warning_text(ui, format!("Its script failed to start: {error}"));
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
