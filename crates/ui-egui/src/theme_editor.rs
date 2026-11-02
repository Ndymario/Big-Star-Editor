//! The window a theme's colours are edited in, beside Preferences. The interface shows the colours as they change, and
//! nothing is kept until the theme is saved.

use egui::{Color32, Ui};

use crate::controls::{self, Button, Choice as _, TextField};
use crate::dialogs;
use crate::i18n::{self, tr};
use crate::theme::{self, AnyTheme, Theme, Tokens};

const WIDTH: f32 = 340.0;
const LABEL_WIDTH: f32 = 170.0;

/// A theme being made or changed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ThemeDraft {
    /// The custom theme being changed; `None` for a new one.
    pub(crate) editing: Option<String>,
    pub(crate) name: String,
    pub(crate) light: Tokens,
    pub(crate) dark: Tokens,
    /// The mode Preferences was showing, to return to: the editor can show either.
    pub(crate) shown_dark: bool,
}

impl ThemeDraft {
    /// A new theme, starting from `from` as it looks now, with the system's accent colour if it is in use.
    pub(crate) fn copy_of(from: AnyTheme<'_>, theme: &Theme, accent: Option<Color32>, shown_dark: bool) -> Self {
        let from_name = from.display_name();
        let name = if from.built_in { tr!("theme_editor.name_custom", name = from_name) } else { tr!("theme_editor.name_copy", name = from_name) };
        let (light, dark) = (theme.accented(from.light.clone(), accent), theme.accented(from.dark.clone(), accent));
        Self { editing: None, name, light, dark, shown_dark }
    }

    /// A change to the custom theme `from`, as it was saved.
    pub(crate) fn of(from: AnyTheme<'_>, shown_dark: bool) -> Self {
        Self { editing: Some(from.id.to_owned()), name: from.name.to_owned(), light: from.light.clone(), dark: from.dark.clone(), shown_dark }
    }

    pub(crate) fn palette(&self, dark: bool) -> &Tokens {
        if dark { &self.dark } else { &self.light }
    }

    fn palette_mut(&mut self, dark: bool) -> &mut Tokens {
        if dark { &mut self.dark } else { &mut self.light }
    }
}

fn picker(ui: &mut Ui, color: &mut Color32) -> bool {
    let mut rgb = [color.r(), color.g(), color.b()];
    let changed = ui.color_edit_button_srgb(&mut rgb).changed();
    if changed {
        let [r, g, b] = rgb;
        *color = Color32::from_rgb(r, g, b);
    }
    changed
}

/// The editor, opening beside `preferences`. Which mode's colours it edits is `showing_dark`, the mode the interface
/// shows. Returns `Some(true)` when the theme is to be saved and `Some(false)` when it is to be dropped.
pub(crate) fn window(ctx: &egui::Context, draft: &mut ThemeDraft, showing_dark: &mut bool, preferences: Option<egui::Rect>) -> Option<bool> {
    let mut open = true;
    let mut answer = None;
    let mut window = dialogs::window(tr!("theme_editor.title")).open(&mut open).resizable(false);
    if let Some(rect) = preferences {
        window = window.beside(rect, WIDTH);
    }
    window.show(ctx, |ui| {
        ui.set_width(WIDTH);
        ui.label(tr!("theme_editor.theme_name"));
        TextField::new(&mut draft.name).name(tr!("theme_editor.theme_name")).show(ui);
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.choice_value(showing_dark, false, tr!("theme_editor.light_colours"));
            ui.choice_value(showing_dark, true, tr!("theme_editor.dark_colours"));
        });
        ui.weak(tr!("theme_editor.each_mode"));
        ui.add_space(4.0);
        let palette = draft.palette_mut(*showing_dark);
        ui.horizontal(|ui| {
            let mut accent = palette.primary;
            if picker(ui, &mut accent) {
                *palette = palette.clone().with_accent(accent);
            }
            ui.label(tr!("theme_editor.accent_colour")).on_hover_text(tr!("theme_editor.accent_hint"));
        });
        ui.add_space(4.0);
        let height = (ctx.content_rect().height() - 320.0).clamp(160.0, 480.0);
        egui::ScrollArea::vertical().id_salt("theme-colours").max_height(height).min_scrolled_height(height).auto_shrink(false).show(ui, |ui| {
            for (group, tokens) in &theme::TOKEN_GROUPS {
                ui.label(egui::RichText::new(i18n::text(group)).strong());
                for token in *tokens {
                    let color = (token.color)(palette);
                    ui.horizontal(|ui| {
                        picker(ui, color);
                        ui.allocate_ui_with_layout(
                            egui::vec2(LABEL_WIDTH, controls::HEIGHT),
                            egui::Layout::left_to_right(egui::Align::Center),
                            |ui| {
                                ui.set_min_width(LABEL_WIDTH);
                                ui.label(i18n::text(token.label));
                            },
                        );
                        ui.weak(egui::RichText::new(theme::channels(*color)).monospace());
                    });
                }
                ui.add_space(6.0);
            }
        });
        let save = if draft.editing.is_some() { tr!("theme_editor.save_changes") } else { tr!("theme_editor.save_theme") };
        dialogs::actions(ui, |ui| {
            if Button::new(save).primary().enabled(!draft.name.trim().is_empty()).show(ui).clicked() {
                answer = Some(true);
            }
            if Button::new(tr!("common.cancel")).show(ui).clicked() {
                answer = Some(false);
            }
        });
    });
    if open { answer } else { Some(false) }
}
