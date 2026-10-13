//! What opens over the screen: a titled box that must be answered, and the floating windows tools open in.

use egui::{Align, Layout, Ui};

use crate::{controls, theme::Tokens};

/// A dialog titled `title`, `width` points wide, over everything else until it is answered. Returns what `body`
/// returned, and whether the person backed out of it with Escape or a click outside.
pub fn modal<R>(ctx: &egui::Context, id: impl egui::AsIdSalt, title: &str, width: f32, body: impl FnOnce(&mut Ui) -> R) -> (R, bool) {
    let t = Tokens::get(ctx);
    let response = egui::Modal::new(egui::Id::new("modal").with(id))
        .frame(
            egui::Frame::NONE
                .fill(t.overlay)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(10))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            // Never wider than the window it covers.
            ui.set_width(width.min(ui.ctx().content_rect().width() - 48.0));
            controls::title(ui, title);
            ui.add_space(6.0);
            body(ui)
        });
    let dismissed = response.should_close();
    (response.inner, dismissed)
}

/// A floating window titled `title`: a tool that stays open beside the documents. Finish it with `.open(..)` and
/// `.show(ctx, |ui| ..)`.
pub fn window<'open>(title: impl Into<String>) -> Window<'open> {
    let title = title.into();
    Window { window: egui::Window::new(title.as_str()).title_bar(false).collapsible(false), title, open: None }
}

/// What [`window`] makes. Its heading is drawn like a [`modal`]'s, in place of egui's title bar; it is dragged by any
/// part of it that is not a control.
pub struct Window<'open> {
    window: egui::Window<'open>,
    title: String,
    open: Option<&'open mut bool>,
}

impl<'open> Window<'open> {
    /// Shown while `open` is set, with a close button that clears it.
    pub fn open(mut self, open: &'open mut bool) -> Self {
        self.open = Some(open);
        self
    }

    pub fn resizable(mut self, resizable: bool) -> Self {
        self.window = self.window.resizable(resizable);
        self
    }

    pub fn default_width(mut self, width: f32) -> Self {
        self.window = self.window.default_width(width);
        self
    }

    pub fn default_size(mut self, size: impl Into<egui::Vec2>) -> Self {
        self.window = self.window.default_size(size);
        self
    }

    pub fn show<R>(self, ctx: &egui::Context, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
        let Self { window, title, open } = self;
        if open.as_deref() == Some(&false) {
            return None;
        }
        let t = Tokens::get(ctx);
        // The editor's own background, so the fields and cards in it stand out as they do in a panel.
        let frame = egui::Frame::window(&ctx.global_style()).fill(t.base).inner_margin(egui::Margin::same(14));
        // Opens in the middle of the screen, and stays wherever it is then dragged.
        let window = window.pivot(egui::Align2::CENTER_CENTER).default_pos(ctx.content_rect().center());
        let mut closed = false;
        let shown = window.frame(frame).show(ctx, |ui| {
            ui.horizontal(|ui| {
                controls::title(ui, &title);
                if open.is_some() {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        closed = controls::icon_button(ui, crate::icons::CLOSE, "Close").clicked();
                    });
                }
            });
            ui.add_space(6.0);
            body(ui)
        });
        if let Some(open) = open.filter(|_| closed) {
            *open = false;
        }
        shown.and_then(|shown| shown.inner)
    }
}

/// The buttons a dialog ends with, from the right: the one that goes ahead first.
pub fn actions<R>(ui: &mut Ui, buttons: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(10.0);
    // A row of its own height: a layout given the rest of the window would stretch the window to fill.
    let size = egui::vec2(ui.available_width(), controls::HEIGHT);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), buttons).inner
}
