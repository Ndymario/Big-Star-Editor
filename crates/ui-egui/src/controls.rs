//! The controls every screen is built from: buttons, fields, drop-downs, tabs, rows and the text that reports a
//! state, in one set of sizes and colours. Screens ask for a control here rather than building egui's own, so two of
//! a kind never differ; a test below holds them to it.

use egui::{Color32, CornerRadius, Rect, Response, RichText, Sense, Stroke, Ui, Vec2, WidgetText, pos2, vec2};

use crate::{
    icons::{self, Icon},
    motion,
    theme::{self, Tokens},
};

/// How tall a button, a one-line field and a drop-down are.
pub const HEIGHT: f32 = 28.0;
/// How tall a small button and a row of a list are.
pub const HEIGHT_SM: f32 = 24.0;
/// The space around a button's text.
pub const PADDING: Vec2 = vec2(10.0, 5.0);
const PADDING_SM: Vec2 = vec2(8.0, 3.0);
const TEXT_SM: f32 = 12.5;
/// The border of a button that has none to show. egui takes the width of a border off the padding, so one of no width
/// would make the button a pixel bigger than a framed one: this is as wide, and clear.
const UNSEEN_STROKE: Stroke = Stroke { width: 1.0, color: Color32::TRANSPARENT };
/// The space around a field's text.
pub const FIELD_MARGIN: egui::Margin = egui::Margin::symmetric(8, 5);
/// The corners of a control.
pub const RADIUS: u8 = 6;
/// The narrowest a field that shares its row gets.
const FIELD_MIN: f32 = 120.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Plain,
    /// The main action of a form, in the accent colour.
    Primary,
    /// An action that destroys something.
    Danger,
    /// No frame until hovered: for a symbol beside something.
    Ghost,
}

/// A button. `ui.add(Button::new("Save").primary())`, or [`Button::show`].
#[must_use = "a button does nothing until it is added to a Ui"]
pub struct Button<'a> {
    text: WidgetText,
    icon: Option<Icon>,
    kind: Kind,
    small: bool,
    enabled: bool,
    selected: bool,
    quiet: bool,
    name: Option<&'a str>,
    shortcut: Option<String>,
}

impl<'a> Button<'a> {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        Button {
            text: text.into(),
            icon: None,
            kind: Kind::Plain,
            small: false,
            enabled: true,
            selected: false,
            quiet: false,
            name: None,
            shortcut: None,
        }
    }

    /// A line of a menu: as wide as the menu, framed only under the pointer.
    pub fn menu(text: impl Into<WidgetText>) -> Self {
        Button::new(text).quiet()
    }

    /// A button that is an icon alone. Give it a [`Self::name`].
    pub fn icon(icon: Icon) -> Self {
        Button::new("").with_icon(icon)
    }

    /// An icon before the text.
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Filled in the accent colour, for the main action of a form.
    pub fn primary(mut self) -> Self {
        self.kind = Kind::Primary;
        self
    }

    /// Filled red, for an action that destroys something.
    pub fn danger(mut self) -> Self {
        self.kind = Kind::Danger;
        self
    }

    /// Without a frame.
    pub fn ghost(mut self) -> Self {
        self.kind = Kind::Ghost;
        self
    }

    /// The small size, for a row of a list or a crowded toolbar.
    pub fn small(mut self) -> Self {
        self.small = true;
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Drawn as the one of several that is chosen, or as a switch that is on.
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// Framed only while the pointer is on it.
    pub fn quiet(mut self) -> Self {
        self.quiet = true;
        self
    }

    /// What assistive technology is told, when the text alone does not say what the button does.
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    /// The keys that do the same, shown faint at the right of a menu line.
    pub fn shortcut(mut self, ctx: &egui::Context, shortcut: egui::KeyboardShortcut) -> Self {
        self.shortcut = Some(ctx.format_shortcut(&shortcut));
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        ui.add(self)
    }
}

impl egui::Widget for Button<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let t = Tokens::get(ui.ctx());
        let dense = self.small || ui.spacing().interact_size.y < HEIGHT;
        let height = if dense { HEIGHT_SM } else { HEIGHT };
        let text = match self.kind {
            Kind::Primary => self.text.strong().color(t.inverted),
            Kind::Danger => self.text.strong().color(Color32::WHITE),
            Kind::Plain | Kind::Ghost if self.selected => self.text.color(t.primary_text),
            Kind::Plain | Kind::Ghost => self.text,
        };
        let size = vec2(0.0, height);
        // An icon takes the colour of the text beside it.
        let tint = match self.kind {
            Kind::Primary => Some(t.inverted),
            Kind::Danger => Some(Color32::WHITE),
            Kind::Plain | Kind::Ghost if self.selected => Some(t.primary_text),
            Kind::Plain | Kind::Ghost => None,
        };
        let icon_size = if dense { icons::SIZE_SM } else { icons::SIZE };
        let image = |icon: Icon| {
            let image = icon.image(ui.ctx(), icon_size);
            tint.map_or(image.clone(), |tint| image.tint(tint))
        };
        let mut atoms: Vec<egui::Atom<'_>> = Vec::new();
        if let Some(icon) = self.icon {
            atoms.push(image(icon).into());
        }
        // The keys are no part of the button's name, so it is taken before they join the text.
        let named = self.shortcut.is_some().then(|| text.text().to_owned());
        if !text.is_empty() {
            atoms.push(text.into());
        }
        if let Some(keys) = self.shortcut {
            atoms.push(egui::Atom::grow());
            atoms.push(RichText::new(keys).color(t.muted).into());
        }
        let mut button = egui::Button::new(egui::Atoms::from(atoms))
            .image_tint_follows_text_color(tint.is_none())
            .min_size(size)
            .wrap_mode(egui::TextWrapMode::Extend)
            .frame_when_inactive(true);
        // The fill changes over a moment under the pointer. Where the pointer was is known from the frame before, kept
        // under the place this button takes in its `Ui`.
        let key = ui.next_auto_id().with("button-hover");
        let state: Pointer = ui.data(|data| data.get_temp(key)).unwrap_or_default();
        let hover = motion::toward(ui.ctx(), key, state != Pointer::Away, motion::COLOR);
        match self.kind {
            Kind::Primary => button = button.fill(motion::mix(t.primary, t.primary_hover, hover)),
            Kind::Danger => {
                let hovered = motion::mix(t.danger, Color32::BLACK, 0.12);
                button = button.fill(motion::mix(t.danger, hovered, hover));
            }
            // The chosen one of several, or a switch that is on: tinted in the accent colour.
            Kind::Plain | Kind::Ghost if self.selected => {
                button = button.fill(t.primary_subtle).stroke(Stroke::new(1.0, if self.quiet { Color32::TRANSPARENT } else { t.primary }));
            }
            // No frame at rest; a plain wash comes in under the pointer so an icon button reads as something to press.
            Kind::Ghost => {
                let wash = motion::mix(Color32::TRANSPARENT, t.border, hover);
                button = button.fill(wash).stroke(UNSEEN_STROKE);
            }
            // Always framed, so the size is the same under the pointer as away from it; the frame is just not drawn
            // until hovered.
            Kind::Plain if self.quiet => {
                let hovered = ui.visuals().widgets.hovered.weak_bg_fill;
                button = button.fill(motion::mix(Color32::TRANSPARENT, hovered, hover)).stroke(UNSEEN_STROKE);
            }
            // A pressed button keeps the look egui gives it.
            Kind::Plain if state != Pointer::Down => {
                let widgets = &ui.visuals().widgets;
                let (rest, hovered) = (widgets.inactive.weak_bg_fill, widgets.hovered.weak_bg_fill);
                button = button.fill(motion::mix(rest, hovered, hover));
            }
            Kind::Plain => {}
        }
        // Not in a child `Ui`: a row that wraps can move only what is placed in it directly to its next line.
        let response = if self.small {
            let style = ui.style().clone();
            compact(ui);
            let response = ui.add_enabled(self.enabled, button);
            ui.set_style(style);
            response
        } else {
            ui.add_enabled(self.enabled, button)
        };
        let state = if response.is_pointer_button_down_on() {
            Pointer::Down
        } else if response.hovered() {
            Pointer::Over
        } else {
            Pointer::Away
        };
        ui.data_mut(|data| data.insert_temp(key, state));
        if let Some(name) = self.name.or(named.as_deref()) {
            let enabled = self.enabled;
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
        }
        response
    }
}

/// Makes every control added to `ui` from here on the small size: a toolbar with much on it, the foot of a panel.
pub fn compact(ui: &mut Ui) {
    let spacing = ui.spacing_mut();
    spacing.button_padding = PADDING_SM;
    spacing.interact_size.y = HEIGHT_SM;
    spacing.item_spacing.x = 6.0;
    ui.style_mut().override_font_id = Some(theme::body(TEXT_SM));
}

/// Where the pointer was on a button the frame before.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Pointer {
    #[default]
    Away,
    Over,
    Down,
}

/// A choice among several, the chosen one marked, in place of egui's `selectable_label` and `selectable_value`, which
/// change size under the pointer.
pub trait Choice {
    fn choice(&mut self, chosen: bool, text: impl Into<WidgetText>) -> Response;

    fn choice_value<T: PartialEq>(&mut self, current: &mut T, value: T, text: impl Into<WidgetText>) -> Response {
        let mut response = self.choice(*current == value, text);
        if response.clicked() && *current != value {
            *current = value;
            response.mark_changed();
        }
        response
    }
}

impl Choice for Ui {
    fn choice(&mut self, chosen: bool, text: impl Into<WidgetText>) -> Response {
        let text = text.into();
        let name = text.text().to_owned();
        let response = Button::new(text).selected(chosen).quiet().show(self);
        let enabled = self.is_enabled();
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, chosen, &name));
        response
    }
}

/// A switch drawn as a button: lit while `on`.
pub fn toggle(ui: &mut Ui, on: &mut bool, text: impl Into<WidgetText>) -> Response {
    let text = text.into();
    let name = text.text().to_owned();
    let mut response = Button::new(text).selected(*on).show(ui);
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    let (enabled, on) = (ui.is_enabled(), *on);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, enabled, on, &name));
    response
}

/// An icon as a button without a frame; `name` is what it does.
pub fn icon_button(ui: &mut Ui, icon: Icon, name: &str) -> Response {
    Button::icon(icon).ghost().name(name).show(ui).on_hover_text(name)
}

/// The menu a right-click on `response` opens, in the same look as the menu bar's.
pub fn context_menu<R>(response: &Response, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    egui::Popup::context_menu(response).style(menu_style).show(body).map(|inner| inner.inner)
}

/// egui's look for a menu, with room either side of each line's text: egui's leaves it against the edges.
fn menu_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = PADDING_SM;
}

/// egui's look for a menu bar, with a little room either side of each title.
pub fn bar_style(style: &mut egui::Style) {
    egui::containers::menu::menu_style(style);
    style.spacing.button_padding = vec2(5.0, 2.0);
}

fn menu_config() -> egui::containers::menu::MenuConfig {
    egui::containers::menu::MenuConfig::new().style(menu_style)
}

/// A menu of the window's menu bar. It opens as the pointer comes onto its button, and the next along takes its place as
/// the pointer moves across the bar, so none needs a press; it closes once the pointer has left both it and its button.
pub fn bar_menu<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    use egui::containers::menu::MenuConfig;
    let id = ui.next_auto_id().with("bar-menu");
    let open = egui::Popup::is_id_open(ui.ctx(), id);
    // egui's flat button of a menu bar, keeping the wash of the pointer while its menu is open.
    let mut button = egui::Button::new(title);
    if open {
        button = button.fill(ui.visuals().widgets.hovered.weak_bg_fill);
    }
    let response = ui.add(button);
    // Only coming onto the button opens it, so a menu closed with Escape stays closed under a pointer that has not moved.
    let was_hovered = ui.data_mut(|data| std::mem::replace(data.get_temp_mut_or_default::<bool>(id.with("hovered")), response.hovered()));
    let pointer = ui.ctx().pointer_hover_pos();
    // Where the menu was drawn the frame before, with a little give so a pointer on its edge is still in it.
    let menu_rect = ui.ctx().read_response(id).map(|menu| menu.rect.expand(4.0));
    // egui counts a pointer a few points short of the button as on it, so that counts as inside too: a slow pointer
    // would otherwise open the menu there and close it again the next frame.
    let inside = response.hovered() || pointer.is_some_and(|p| response.rect.contains(p) || menu_rect.is_some_and(|rect| rect.contains(p)));
    let set = if ui.is_enabled() && ((response.hovered() && !was_hovered) || response.clicked()) {
        Some(true)
    } else if open && pointer.is_some() && !inside {
        Some(false)
    } else {
        None
    };
    let config = menu_config();
    egui::Popup::menu(&response)
        .id(id)
        .open_memory(set.map(egui::SetOpenCommand::Bool))
        // A press on the button that opened it is no click outside it: the menu stays.
        .close_behavior(if response.clicked() { egui::PopupCloseBehavior::IgnoreClicks } else { config.close_behavior })
        .style(config.style.clone())
        .info(egui::UiStackInfo::new(egui::UiKind::Menu).with_tag_value(MenuConfig::MENU_CONFIG_TAG, config))
        .show(body)
        .map(|inner| inner.inner)
}

/// The arrow that opens and closes a node of a tree.
pub fn disclosure(ui: &mut Ui, open: bool) -> Response {
    let icon = if open { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(icons::SIZE), Sense::click());
    let t = Tokens::get(ui.ctx());
    let hover = motion::hover(ui, &response, motion::COLOR);
    icon.paint(ui.painter(), rect.shrink(2.0), motion::mix(t.muted, t.foreground, hover));
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, if open { "Collapse" } else { "Expand" }));
    response
}

/// A titled group that folds away, its arrow the one [`disclosure`] draws. The title is cut short to the width there is:
/// egui's own collapsing header keeps its whole title, holding a panel open past where it is dragged.
pub fn group<R>(
    ui: &mut Ui,
    id: impl egui::AsIdSalt,
    title: impl Into<WidgetText>,
    default_open: bool,
    body: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), ui.make_persistent_id(id), default_open);
    let icon = if state.is_open() { icons::CHEVRON_DOWN } else { icons::CHEVRON_RIGHT };
    let header = Row::new(title).icon(icon).show(ui);
    if header.clicked() {
        state.toggle(ui);
    }
    state.show_body_indented(&header, ui, body).map(|body| body.inner)
}

/// One line of a list or a tree that can be the chosen one. As wide as the space it is put in unless [`Row::fit`].
#[must_use = "a row does nothing until it is added to a Ui"]
pub struct Row {
    text: WidgetText,
    selected: bool,
    icon: Option<Icon>,
    fit: bool,
    min_width: f32,
    indent: f32,
}

impl Row {
    pub fn new(text: impl Into<WidgetText>) -> Self {
        Row { text: text.into(), selected: false, icon: None, fit: false, min_width: 0.0, indent: 0.0 }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    /// An icon before the text.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = Some(icon);
        self
    }

    /// Only as wide as its text: for a row that shares its line.
    pub fn fit(mut self) -> Self {
        self.fit = true;
        self
    }

    /// At least this wide, for a [`Row::fit`] row that should still reach the end of its line.
    pub fn min_width(mut self, width: f32) -> Self {
        self.min_width = width;
        self
    }

    /// Set in from the left by this much: for a line that belongs to the one above it.
    pub fn indent(mut self, indent: f32) -> Self {
        self.indent = indent;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        ui.add(self)
    }
}

impl egui::Widget for Row {
    fn ui(self, ui: &mut Ui) -> Response {
        const PAD: f32 = 6.0;
        const GAP: f32 = 6.0;
        let t = Tokens::get(ui.ctx());
        let name = self.text.text().to_owned();
        let icon_room = if self.icon.is_some() { icons::SIZE + GAP } else { 0.0 };
        let room = (ui.available_width() - 2.0 * PAD - icon_room - self.indent).max(0.0);
        let galley = if self.fit {
            self.text.into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, egui::TextStyle::Button)
        } else {
            self.text.into_galley(ui, Some(egui::TextWrapMode::Truncate), room, egui::TextStyle::Button)
        };
        let width = if self.fit { (galley.size().x + 2.0 * PAD + icon_room + self.indent).max(self.min_width) } else { ui.available_width() };
        let (rect, response) = ui.allocate_exact_size(vec2(width, HEIGHT_SM.max(galley.size().y + 4.0)), Sense::click());
        let (enabled, selected) = (ui.is_enabled(), self.selected);
        response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, &name));
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let hover = motion::hover(ui, &response, motion::COLOR);
        let fill = if self.selected { t.primary_subtle } else { motion::mix(Color32::TRANSPARENT, t.border, hover) };
        let ink = if !enabled {
            t.subtle
        } else if self.selected {
            t.primary_text
        } else {
            t.foreground
        };
        let painter = ui.painter();
        painter.rect_filled(rect, CornerRadius::same(RADIUS - 2), fill);
        let mut left = rect.left() + PAD + self.indent;
        if let Some(icon) = self.icon {
            let at = Rect::from_min_size(pos2(left, rect.center().y - icons::SIZE / 2.0), Vec2::splat(icons::SIZE));
            icon.paint(painter, at, if self.selected { t.primary_text } else { t.muted });
            left += icons::SIZE + GAP;
        }
        painter.galley(pos2(left, rect.center().y - galley.size().y / 2.0), galley, ink);
        response
    }
}

/// How wide a field is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Width {
    /// All the width there is.
    Fill,
    /// This many points, or what there is when that is less.
    Px(f32),
    /// What is left of the row after this much is kept for the controls that follow the field.
    Rest(f32),
}

impl Width {
    fn resolve(self, available: f32) -> f32 {
        match self {
            Width::Fill => available,
            Width::Px(width) => width.min(available),
            Width::Rest(reserve) => (available - reserve).max(FIELD_MIN.min(available)),
        }
    }
}

/// A text field. One line unless [`TextField::rows`] says otherwise.
#[must_use = "a field does nothing until it is added to a Ui"]
pub struct TextField<'a> {
    value: &'a mut dyn egui::TextBuffer,
    hint: &'a str,
    name: Option<&'a str>,
    width: Width,
    rows: Option<usize>,
    password: bool,
    code: bool,
    id: Option<egui::Id>,
    lock_focus: bool,
}

impl<'a> TextField<'a> {
    pub fn new(value: &'a mut dyn egui::TextBuffer) -> Self {
        TextField { value, hint: "", name: None, width: Width::Fill, rows: None, password: false, code: false, id: None, lock_focus: false }
    }

    /// A field of several lines.
    pub fn multiline(value: &'a mut dyn egui::TextBuffer) -> Self {
        TextField::new(value).rows(4)
    }

    /// What the empty field says it is for.
    pub fn hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }

    /// What assistive technology (and the test driver) calls the field.
    pub fn name(mut self, name: &'a str) -> Self {
        self.name = Some(name);
        self
    }

    pub fn width(mut self, width: Width) -> Self {
        self.width = width;
        self
    }

    /// Several lines, this many shown.
    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = Some(rows);
        self
    }

    pub fn password(mut self) -> Self {
        self.password = true;
        self
    }

    /// In the fixed-width font, for text that is data.
    pub fn code(mut self) -> Self {
        self.code = true;
        self
    }

    /// An id of its own, for a caller that reads the field's cursor.
    pub fn id_salt(mut self, salt: impl egui::AsIdSalt) -> Self {
        self.id = Some(egui::Id::new(salt));
        self
    }

    /// Tab types a tab rather than leaving the field.
    pub fn lock_focus(mut self) -> Self {
        self.lock_focus = true;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        ui.add(self)
    }
}

impl egui::Widget for TextField<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let width = self.width.resolve(ui.available_width());
        let mut edit = match self.rows {
            Some(rows) => egui::TextEdit::multiline(self.value).desired_rows(rows),
            None => egui::TextEdit::singleline(self.value).password(self.password).vertical_align(egui::Align::Center).min_size(vec2(0.0, HEIGHT)),
        };
        edit = edit.hint_text(self.hint.to_owned()).margin(FIELD_MARGIN).desired_width(width).lock_focus(self.lock_focus);
        if self.code {
            edit = edit.font(egui::TextStyle::Monospace);
        }
        if let Some(id) = self.id {
            edit = edit.id_salt(id);
        }
        let response = ui.add(edit);
        if let Some(name) = self.name {
            let enabled = response.enabled();
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::TextEdit, enabled, name));
        }
        response
    }
}

/// A search field with a clear button. Returns the field's response; `changed()` also when it was cleared.
pub fn search(ui: &mut Ui, text: &mut String, hint: &str, width: Width) -> Response {
    /// The room the clear button takes after the field.
    const CLEAR: f32 = 30.0;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        let t = Tokens::get(ui.ctx());
        icons::SEARCH.show(ui, icons::SIZE_SM, t.subtle);
        let room = width.resolve(ui.available_width()) - CLEAR - icons::SIZE_SM - 4.0;
        let mut response = TextField::new(text).hint(hint).width(Width::Px(room.max(FIELD_MIN))).show(ui);
        if !text.is_empty() && icon_button(ui, icons::CLOSE, "Clear search").clicked() {
            text.clear();
            response.mark_changed();
        }
        response
    })
    .inner
}

/// A number dragged or typed: egui's own, in this set's sizes. Finish it with `.range(..)` and the rest.
pub fn number<N: egui::emath::Numeric>(value: &mut N) -> egui::DragValue<'_> {
    egui::DragValue::new(value)
}

/// How wide the arrows of a spin box are.
const STEPPER_WIDTH: f32 = 14.0;

/// The arrows of a spin box, one over the other, for the [`number`] they sit beside. Returns the step a click asked
/// for: one up, one down, or none.
pub fn steppers(ui: &mut Ui) -> i64 {
    let t = Tokens::get(ui.ctx());
    let (rect, whole) = ui.allocate_exact_size(vec2(STEPPER_WIDTH, ui.spacing().interact_size.y), Sense::hover());
    let enabled = ui.is_enabled();
    let mut step = 0;
    for (by, name) in [(1, "Increase"), (-1, "Decrease")] {
        let half = if by > 0 { rect.split_top_bottom_at_fraction(0.5).0 } else { rect.split_top_bottom_at_fraction(0.5).1 };
        let response = ui.interact(half, whole.id.with(by), Sense::click());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name));
        let hover = motion::hover(ui, &response, motion::COLOR);
        ui.painter().rect_filled(half.shrink(1.0), CornerRadius::same(3), motion::mix(Color32::TRANSPARENT, t.border, hover));
        // A small triangle pointing the way it steps.
        let (centre, reach) = (half.center(), vec2(3.5, 2.0 * by as f32));
        let points =
            vec![pos2(centre.x - reach.x, centre.y + reach.y), pos2(centre.x + reach.x, centre.y + reach.y), pos2(centre.x, centre.y - reach.y)];
        let ink = if enabled { motion::mix(t.muted, t.foreground, hover) } else { t.subtle };
        ui.painter().add(egui::Shape::convex_polygon(points, ink, Stroke::NONE));
        if response.clicked() {
            step = by;
        }
    }
    step
}

/// The side of a checkbox's box.
const MARK: f32 = 16.0;
/// The corners of a checkbox's box.
const MARK_RADIUS: u8 = 4;
/// Between the box and its text.
const MARK_GAP: f32 = 8.0;

/// A checkbox: a box that fills with the accent colour when ticked. One with no text is named by its caller.
pub fn checkbox(ui: &mut Ui, checked: &mut bool, text: impl Into<WidgetText>) -> Response {
    checkbox_undecided(ui, checked, false, text)
}

/// A [`checkbox`] that shows a dash while `undecided`: what it stands for is neither on nor off.
pub fn checkbox_undecided(ui: &mut Ui, checked: &mut bool, undecided: bool, text: impl Into<WidgetText>) -> Response {
    let t = Tokens::get(ui.ctx());
    let text = text.into();
    let name = text.text().to_owned();
    let galley = (!text.is_empty()).then(|| {
        let room = ui.available_width() - MARK - MARK_GAP;
        text.into_galley(ui, None, room, egui::TextStyle::Button)
    });
    let mut size = vec2(MARK, ui.spacing().interact_size.y);
    if let Some(galley) = &galley {
        size.x += MARK_GAP + galley.size().x;
        size.y = size.y.max(galley.size().y);
    }
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    if response.clicked() {
        *checked = !*checked;
        response.mark_changed();
    }
    let (enabled, on) = (ui.is_enabled(), *checked);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Checkbox, enabled, on, &name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hover = motion::hover(ui, &response, motion::COLOR);
    let filled = motion::toward(ui.ctx(), response.id.with("on"), on || undecided, motion::COLOR);
    let frame = Rect::from_center_size(pos2(rect.left() + MARK / 2.0, rect.center().y), Vec2::splat(MARK));
    let radius = CornerRadius::same(MARK_RADIUS);
    let accent = motion::mix(t.primary, t.primary_hover, hover);
    let edge = motion::mix(t.border_strong, t.muted, hover);
    let fade = if enabled { 1.0 } else { 0.5 };
    let painter = ui.painter();
    painter.rect(
        frame,
        radius,
        motion::mix(t.base, accent, filled).gamma_multiply(fade),
        Stroke::new(1.0, motion::mix(edge, accent, filled).gamma_multiply(fade)),
        egui::StrokeKind::Inside,
    );
    if filled > 0.0 {
        let ink = t.inverted.gamma_multiply(filled);
        if undecided {
            painter.line_segment([frame.left_center() + vec2(4.0, 0.0), frame.right_center() - vec2(4.0, 0.0)], Stroke::new(2.0, ink));
        } else {
            icons::CHECK.bold().paint(painter, frame.shrink(2.0), ink);
        }
    }
    if response.has_focus() {
        painter.rect_stroke(frame.expand(2.0), radius + CornerRadius::same(2), Stroke::new(1.0, t.ring), egui::StrokeKind::Outside);
    }
    if let Some(galley) = galley {
        let at = pos2(frame.right() + MARK_GAP, rect.center().y - galley.size().y / 2.0);
        painter.galley(at, galley, t.foreground.gamma_multiply(fade));
    }
    response
}

/// A small mark that explains what is beside it to whoever rests the pointer on it.
pub fn hint(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    icons::INFO.show(ui, icons::SIZE_SM, t.subtle).on_hover_text(text)
}

/// A warning sign that says, on hover, what is wrong.
pub fn warning_hint(ui: &mut Ui, text: &str) -> Response {
    let t = Tokens::get(ui.ctx());
    icons::WARNING.show(ui, icons::SIZE_SM, t.warning).on_hover_text(text)
}

/// A drop-down whose choices the caller draws with [`Choice`]: finish it with `.selected_text(..).show_ui(ui, |ui| ..)`.
pub fn drop_down(id: impl egui::AsIdSalt) -> egui::ComboBox {
    egui::ComboBox::from_id_salt(id)
}

/// One reading of a value, so a choice entry and the box beside it never disagree about the base.
pub fn format_value(value: i64, hex: bool) -> String {
    match (hex, value < 0) {
        (false, _) => value.to_string(),
        (true, false) => format!("0x{value:X}"),
        (true, true) => format!("-0x{:X}", value.unsigned_abs()),
    }
}

/// A number with its name, as every numbered choice shows it: `1 – Castle`.
pub fn numbered(value: i64, name: &str, hex: bool) -> String {
    format!("{} – {name}", format_value(value, hex))
}

/// A number picked by its name, from a drop-down of `choices` that each show as [`numbered`]. A value no choice
/// names shows as not listed, and none, a selection that disagrees, as a dash. Finish it with [`NumberedChoice::show`].
pub fn numbered_choice<'a, T, S>(id: impl std::hash::Hash + std::fmt::Debug, choices: &'a [(T, S)]) -> NumberedChoice<'a, T, S> {
    NumberedChoice { id: egui::Id::new(id), choices, hex: false, width: None, height: None, filter: None }
}

pub struct NumberedChoice<'a, T, S> {
    id: egui::Id,
    choices: &'a [(T, S)],
    hex: bool,
    width: Option<f32>,
    height: Option<f32>,
    filter: Option<&'a str>,
}

impl<'a, T: Copy + Into<i64>, S: AsRef<str>> NumberedChoice<'a, T, S> {
    /// Shows the numbers in hexadecimal.
    pub fn hex(mut self, hex: bool) -> Self {
        self.hex = hex;
        self
    }

    /// How wide the closed drop-down is, its text cut short to fit. It takes the width it is given and no more, so a
    /// long name cannot widen the panel it sits in.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// How tall the open list gets before it scrolls.
    pub fn height(mut self, height: f32) -> Self {
        self.height = Some(height);
        self
    }

    /// A box above the list that keeps only the choices whose number or name holds what is typed, for a list too long
    /// to scroll through. `hint` fills the empty box.
    pub fn filter(mut self, hint: &'a str) -> Self {
        self.filter = Some(hint);
        self
    }

    /// Returns the choice picked, when it is not `current` already.
    pub fn show(self, ui: &mut Ui, current: Option<i64>) -> Option<T> {
        let Self { id, choices, hex, width, height, filter } = self;
        let selected = match current {
            Some(value) => match choices.iter().find(|(v, _)| (*v).into() == value) {
                Some((_, name)) => numbered(value, name.as_ref(), hex),
                None => numbered(value, "not listed", hex),
            },
            None => "—".to_owned(),
        };
        let mut picked = None;
        let list = |ui: &mut Ui| {
            let mut combo = drop_down(id).selected_text(selected);
            if let Some(width) = width {
                combo = combo.width(width).truncate();
            }
            if let Some(height) = height {
                combo = combo.height(height);
            }
            combo.show_ui(ui, |ui| {
                let filter_id = id.with("filter");
                let needle = match filter {
                    Some(hint) => {
                        let mut text: String = ui.data(|d| d.get_temp(filter_id)).unwrap_or_default();
                        ui.add(TextField::new(&mut text).hint(hint));
                        ui.data_mut(|d| d.insert_temp(filter_id, text.clone()));
                        text.to_lowercase()
                    }
                    None => String::new(),
                };
                for (value, name) in choices {
                    let value_i64 = (*value).into();
                    let entry = numbered(value_i64, name.as_ref(), hex);
                    if !needle.is_empty() && !entry.to_lowercase().contains(&needle) {
                        continue;
                    }
                    if ui.choice(current == Some(value_i64), entry).on_hover_text(name.as_ref()).clicked() && current != Some(value_i64) {
                        picked = Some(*value);
                    }
                }
            });
        };
        match width {
            Some(width) => {
                ui.allocate_ui(vec2(width, ui.spacing().interact_size.y), list);
            }
            None => list(ui),
        }
        picked
    }
}

/// A row of tabs over a line, the chosen one underlined; the underline slides to the tab chosen next. Returns whether
/// another tab was chosen. A row wider than its place scrolls sideways: claiming its whole width would make a panel
/// dragged narrower than the row keep that width while drawing only what it was dragged to.
pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, id: impl egui::AsIdSalt, current: &mut T, tabs: &[(T, &str)]) -> bool {
    let t = Tokens::get(ui.ctx());
    let before = *current;
    let base = egui::Id::new("tabs").with(id);
    let width = ui.available_width();
    egui::ScrollArea::horizontal().id_salt(base).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
        let mut chosen: Option<Rect> = None;
        let row = ui
            .horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (value, label) in tabs {
                    let active = *value == *current;
                    let font = theme::body(13.5);
                    let size = ui.painter().layout_no_wrap((*label).to_owned(), font.clone(), t.muted).size();
                    let (rect, response) = ui.allocate_exact_size(size + vec2(20.0, 12.0), Sense::click());
                    let enabled = ui.is_enabled();
                    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, active, *label));
                    let color = if active {
                        t.primary_text
                    } else {
                        let hover = motion::hover(ui, &response, motion::COLOR);
                        motion::mix(t.muted, t.foreground, hover)
                    };
                    ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, *label, font, color);
                    if response.clicked() && !active {
                        *current = *value;
                    }
                    if *value == *current {
                        chosen = Some(rect);
                    }
                }
            })
            .response
            .rect;
        let bottom = row.bottom();
        ui.painter().line_segment([pos2(row.left(), bottom), pos2(row.left() + width.max(row.width()), bottom)], Stroke::new(1.0, t.border));
        if let Some(chosen) = chosen {
            // Measured from the row, so scrolling moves nothing.
            let left = motion::slide(ui.ctx(), base.with("left"), chosen.left() - row.left(), motion::SLOW);
            let width = motion::slide(ui.ctx(), base.with("width"), chosen.width(), motion::SLOW);
            let line = Rect::from_min_size(pos2(row.left() + left, bottom - 1.0), vec2(width, 2.0));
            ui.painter().rect_filled(line, 0.0, t.primary);
        }
    });
    *current != before
}

/// Something is loading: a ring in a faint colour with a quarter of it in a strong one, turning once a second.
#[derive(Debug, Clone, Copy)]
pub struct Spinner {
    size: f32,
}

impl Default for Spinner {
    fn default() -> Self {
        Spinner::new()
    }
}

impl Spinner {
    /// The size that sits in a line of text.
    pub const INLINE: f32 = 16.0;
    /// The size that stands for a whole screen.
    pub const BLOCK: f32 = 32.0;

    pub fn new() -> Self {
        Spinner { size: Self::INLINE }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl egui::Widget for Spinner {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(self.size), Sense::hover());
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::ProgressIndicator, true, "Loading"));
        if !ui.is_rect_visible(rect) {
            return response;
        }
        let t = Tokens::get(ui.ctx());
        let width = if self.size > 20.0 { 4.0 } else { 2.0 };
        let radius = (self.size - width) / 2.0;
        let center = rect.center();
        let painter = ui.painter();
        painter.circle_stroke(center, radius, Stroke::new(width, t.primary_subtle));
        let start = (ui.input(|i| i.time) % 1.0) as f32 * std::f32::consts::TAU;
        const STEPS: usize = 16;
        let points = (0..=STEPS)
            .map(|step| {
                let angle = start + std::f32::consts::FRAC_PI_2 * step as f32 / STEPS as f32;
                center + radius * vec2(angle.cos(), angle.sin())
            })
            .collect();
        painter.add(egui::Shape::line(points, Stroke::new(width, t.primary)));
        ui.ctx().request_repaint();
        response
    }
}

/// A spinner with a line of text, centred in the width.
pub fn loading(ui: &mut Ui, message: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(64.0);
        ui.add(Spinner::new().size(Spinner::BLOCK));
        ui.add_space(8.0);
        ui.weak(message);
    });
}

/// Text that goes somewhere when clicked.
pub fn link(ui: &mut Ui, text: impl Into<String>) -> Response {
    let t = Tokens::get(ui.ctx());
    let label = egui::Label::new(egui::RichText::new(text).color(t.primary_text)).sense(Sense::click());
    let response = ui.add(label).on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.hovered() {
        let y = response.rect.bottom() - 1.0;
        ui.painter().line_segment([pos2(response.rect.left(), y), pos2(response.rect.right(), y)], Stroke::new(1.0, t.primary_text));
    }
    response
}

/// A card that groups part of a form or a panel, under a title when given one.
pub fn section<R>(ui: &mut Ui, title: &str, body: impl FnOnce(&mut Ui) -> R) -> R {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE
        .fill(t.surface)
        .stroke(Stroke::new(1.0, t.border))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                ui.label(egui::RichText::new(title).strong());
            }
            body(ui)
        })
        .inner
}

/// A theme to choose: a small picture of an interface in `palette` over the theme's name, ringed when it is the one
/// chosen.
pub fn theme_card(ui: &mut Ui, name: &str, palette: &Tokens, selected: bool) -> Response {
    let t = Tokens::get(ui.ctx());
    let (rect, response) = ui.allocate_exact_size(vec2(124.0, 86.0), Sense::click());
    let response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, name));
    if !ui.is_rect_visible(rect) {
        return response;
    }
    let hover = motion::hover(ui, &response, motion::COLOR);
    let edge = if selected { t.primary } else { motion::mix(t.border, t.border_strong, hover) };
    let painter = ui.painter();
    let picture = egui::Rect::from_min_max(rect.min, egui::pos2(rect.max.x, rect.max.y - 24.0));
    let radius = CornerRadius::same(8);
    painter.rect_filled(picture, radius, palette.base);
    // A side panel, two lines of text, and the main button.
    let panel = egui::Rect::from_min_size(picture.min + vec2(8.0, 8.0), vec2(34.0, picture.height() - 16.0));
    painter.rect(panel, CornerRadius::same(4), palette.surface, Stroke::new(1.0, palette.border), egui::StrokeKind::Inside);
    let line = |y: f32, width: f32, color| {
        let at = egui::pos2(panel.right() + 10.0, picture.top() + y);
        painter.rect_filled(egui::Rect::from_min_size(at, vec2(width, 4.0)), CornerRadius::same(2), color);
    };
    line(12.0, 44.0, palette.foreground);
    line(22.0, 30.0, palette.muted);
    let button = egui::Rect::from_min_size(egui::pos2(panel.right() + 10.0, picture.bottom() - 22.0), vec2(36.0, 12.0));
    painter.rect_filled(button, CornerRadius::same(4), palette.primary);
    let stroke = Stroke::new(if selected { 2.0 } else { 1.0 }, edge);
    painter.rect_stroke(picture, radius, stroke, egui::StrokeKind::Inside);
    let color = if selected { t.primary_text } else { t.foreground };
    painter.text(egui::pos2(rect.center().x, rect.max.y - 11.0), egui::Align2::CENTER_CENTER, name, crate::theme::body(12.5), color);
    response
}

/// The title of a dialog or of a screen's part.
pub fn title(ui: &mut Ui, text: &str) -> Response {
    ui.label(egui::RichText::new(text).size(16.0).strong())
}

/// The line a document opens with: what it is, a remark about it, and Save at the far end, lit while there is
/// something to save. Returns whether Save was pressed.
pub fn document_header(ui: &mut Ui, path: &str, remark: &str, can_save: bool) -> bool {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(path).strong());
        if !remark.is_empty() {
            ui.weak(remark);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let save = Button::new("Save").with_icon(icons::SAVE).enabled(can_save);
            (if can_save { save.primary() } else { save })
                .show(ui)
                .on_hover_text("Writes the file into the module it comes from, or the active one")
                .clicked()
        })
        .inner
    })
    .inner
}

/// A side panel inside a document: `default` wide at first but no more than a third of the document, and never dragged
/// past half of it, so the editor in the middle keeps room for its toolbar however narrow the document is.
pub fn side_panel(ui: &Ui, panel: egui::Panel, default: f32) -> egui::Panel {
    let width = ui.available_width();
    panel.resizable(true).default_size(default.min(width / 3.0)).max_size(width / 2.0)
}

/// What sits under a part that fills the height down to it, such as a list with its buttons below: its height is
/// known only once it is laid out, so the part above takes last frame's measure off what is left, and a change is
/// drawn again at once. Call [`Footer::start`] where the footer begins and [`Footer::end`] after it.
pub struct Footer {
    id: egui::Id,
    height: f32,
    top: f32,
}

impl Footer {
    /// The footer under `id_salt`, `guess` tall until it has been measured.
    pub fn new(ui: &Ui, id_salt: impl egui::AsIdSalt, guess: f32) -> Self {
        let id = ui.id().with(("footer", id_salt));
        let height = ui.data(|data| data.get_temp(id)).unwrap_or(guess);
        Footer { id, height, top: 0.0 }
    }

    /// The height left above the footer, but never less than `min`.
    pub fn room_above(&self, ui: &Ui, min: f32) -> f32 {
        (ui.available_height() - self.height).max(min)
    }

    pub fn start(&mut self, ui: &Ui) {
        self.top = ui.cursor().top();
    }

    pub fn end(self, ui: &Ui) {
        let measured = ui.cursor().top() - self.top;
        if (measured - self.height).abs() > 0.5 {
            ui.data_mut(|data| data.insert_temp(self.id, measured));
            ui.ctx().request_repaint();
        }
    }
}

/// Text kept to one line of a row, cut short with an ellipsis (the whole of it on hover) instead of running past the
/// row's end or growing it, leaving `reserve` points after it for what follows.
pub fn one_line(ui: &mut Ui, text: impl Into<WidgetText>, reserve: f32) -> Response {
    let width = (ui.available_width() - reserve).max(0.0);
    ui.scope(|ui| {
        ui.set_max_width(width);
        ui.add(egui::Label::new(text).truncate())
    })
    .inner
}

/// Something went wrong.
pub fn error_text(ui: &mut Ui, text: impl Into<String>) -> Response {
    let t = Tokens::get(ui.ctx());
    ui.colored_label(t.danger, text.into())
}

/// Something the person should know before going on.
pub fn warning_text(ui: &mut Ui, text: impl Into<String>) -> Response {
    let t = Tokens::get(ui.ctx());
    ui.colored_label(t.warning, text.into())
}

/// What a document's last import, export or check had to say, on a line under its header until the person dismisses
/// it. `warning` colours it as something that went wrong; otherwise it reads as a remark.
pub fn notice_line(ui: &mut Ui, notice: &mut Option<String>, warning: bool) {
    let Some(text) = notice.as_deref() else { return };
    let dismissed = ui
        .horizontal_wrapped(|ui| {
            if warning {
                warning_text(ui, text);
            } else {
                ui.weak(text);
            }
            Button::icon(icons::CLOSE).ghost().small().name("Dismiss").show(ui).on_hover_text("Dismiss").clicked()
        })
        .inner;
    if dismissed {
        *notice = None;
    }
}

/// A heading with a sentence under it, for a screen with nothing else to show.
pub fn notice(ui: &mut Ui, icon: Icon, heading: &str, message: &str) {
    let t = Tokens::get(ui.ctx());
    ui.vertical_centered(|ui| {
        ui.add_space(56.0);
        icon.thin().show(ui, 44.0, t.subtle);
        ui.add_space(10.0);
        title(ui, heading);
        ui.add_space(4.0);
        ui.weak(message);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_field_never_takes_more_than_there_is() {
        assert_eq!(Width::Fill.resolve(400.0), 400.0);
        assert_eq!(Width::Px(180.0).resolve(400.0), 180.0);
        assert_eq!(Width::Px(180.0).resolve(90.0), 90.0);
        assert_eq!(Width::Rest(140.0).resolve(400.0), 260.0);
        // A short row keeps a usable field rather than a sliver.
        assert_eq!(Width::Rest(140.0).resolve(200.0), 120.0);
        assert_eq!(Width::Rest(140.0).resolve(80.0), 80.0);
    }

    /// Screens build their buttons, fields and dialogs from this set; egui's own, used directly, are how sizes and
    /// colours drift apart.
    #[test]
    fn screens_use_these_controls_not_eguis() {
        const BANNED: [&str; 20] = [
            ".selectable_label(",
            ".selectable_value(",
            ".toggle_value(",
            "ui.button(",
            "egui::Button",
            "TextEdit::",
            "text_edit_singleline(",
            "text_edit_multiline(",
            "small_button(",
            "ComboBox::",
            ".checkbox(",
            ".radio",
            "DragValue::new",
            "egui::Window",
            "egui::Modal",
            "egui::Checkbox",
            "ui.link(",
            "ui.group(",
            ".spinner()",
            "CollapsingHeader",
        ];
        /// The set itself, and this crate's tests, which name what they ban.
        const EXEMPT: [&str; 3] = ["controls.rs", "dialogs.rs", "tests.rs"];
        fn walk(dir: &std::path::Path, found: &mut Vec<String>) {
            for entry in std::fs::read_dir(dir).expect("read src") {
                let path = entry.expect("dir entry").path();
                if path.is_dir() {
                    walk(&path, found);
                } else if path.extension().is_some_and(|e| e == "rs") && path.file_name().is_some_and(|n| !EXEMPT.iter().any(|exempt| n == *exempt)) {
                    let text = std::fs::read_to_string(&path).expect("read source");
                    for (number, line) in text.lines().enumerate() {
                        if BANNED.iter().any(|banned| line.contains(banned)) {
                            found.push(format!("{}:{}: {}", path.display(), number + 1, line.trim()));
                        }
                    }
                }
            }
        }
        let mut found = Vec::new();
        walk(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"), &mut found);
        assert!(found.is_empty(), "use crate::controls or crate::dialogs instead:\n{}", found.join("\n"));
    }
}
