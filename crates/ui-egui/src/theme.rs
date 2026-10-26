//! Colours and the egui style. One palette in a light and a dark mode; every colour the interface draws its chrome in
//! is a token here, and [`apply`] turns the tokens into the style egui's own widgets take.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Visuals};

/// One palette, light or dark.
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    /// Window background.
    pub base: Color32,
    /// Card, panel and field background.
    pub surface: Color32,
    pub surface_raised: Color32,
    /// What is typed into or drawn on: a field, the level canvas.
    pub well: Color32,
    /// Dialog and menu background.
    pub overlay: Color32,
    pub foreground: Color32,
    /// Supporting text.
    pub muted: Color32,
    /// Placeholder text.
    pub subtle: Color32,
    /// Text on a coloured background.
    pub inverted: Color32,
    pub primary: Color32,
    pub primary_hover: Color32,
    pub primary_subtle: Color32,
    pub primary_text: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub ring: Color32,
    pub success: Color32,
    pub warning: Color32,
    pub warning_subtle: Color32,
    pub danger: Color32,
    pub danger_subtle: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// A theme: one palette in its light and its dark mode.
#[derive(Debug)]
pub struct Family {
    /// What a stored choice names it by.
    pub id: &'static str,
    pub name: &'static str,
    pub light: Tokens,
    pub dark: Tokens,
}

impl Family {
    pub fn variant(&self, dark: bool) -> &Tokens {
        if dark { &self.dark } else { &self.light }
    }
}

/// The themes, the default one first.
pub static FAMILIES: [Family; 6] = [
    Family {
        id: "default",
        name: "Default",
        light: Tokens {
            dark: false,
            base: rgb(255, 255, 255),
            surface: rgb(245, 247, 250),
            surface_raised: rgb(255, 255, 255),
            well: rgb(245, 247, 250),
            overlay: rgb(255, 255, 255),
            foreground: rgb(20, 20, 20),
            muted: rgb(100, 116, 139),
            subtle: rgb(148, 163, 184),
            // The star's yellow is light, so what sits on it is dark.
            inverted: rgb(20, 20, 20),
            primary: rgb(255, 226, 0),
            primary_hover: rgb(209, 185, 0),
            primary_subtle: rgb(255, 250, 209),
            primary_text: rgb(146, 108, 0),
            border: rgb(226, 232, 240),
            border_strong: rgb(203, 213, 225),
            ring: rgb(146, 108, 0),
            success: rgb(22, 163, 74),
            warning: rgb(180, 120, 8),
            warning_subtle: rgb(254, 249, 195),
            danger: rgb(220, 38, 38),
            danger_subtle: rgb(254, 226, 226),
        },
        dark: Tokens {
            dark: true,
            base: rgb(23, 23, 23),
            surface: rgb(38, 38, 38),
            surface_raised: rgb(46, 46, 46),
            well: rgb(15, 15, 15),
            overlay: rgb(38, 38, 38),
            foreground: rgb(245, 245, 245),
            muted: rgb(163, 163, 163),
            subtle: rgb(115, 115, 115),
            inverted: rgb(23, 23, 23),
            primary: rgb(255, 226, 0),
            primary_hover: rgb(209, 185, 0),
            primary_subtle: rgb(74, 68, 18),
            primary_text: rgb(255, 226, 0),
            border: rgb(64, 64, 64),
            border_strong: rgb(82, 82, 82),
            ring: rgb(255, 226, 0),
            success: rgb(74, 222, 128),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(113, 63, 18),
            danger: rgb(248, 113, 113),
            danger_subtle: rgb(127, 29, 29),
        },
    },
    Family {
        id: "ocean",
        name: "Ocean",
        light: Tokens {
            dark: false,
            base: rgb(240, 249, 255),
            surface: rgb(224, 242, 254),
            surface_raised: rgb(255, 255, 255),
            well: rgb(224, 242, 254),
            overlay: rgb(255, 255, 255),
            foreground: rgb(12, 74, 110),
            muted: rgb(38, 125, 164),
            subtle: rgb(125, 211, 252),
            inverted: rgb(255, 255, 255),
            primary: rgb(2, 132, 199),
            primary_hover: rgb(3, 105, 161),
            primary_subtle: rgb(186, 230, 253),
            primary_text: rgb(2, 132, 199),
            border: rgb(186, 230, 253),
            border_strong: rgb(125, 211, 252),
            ring: rgb(2, 132, 199),
            success: rgb(5, 150, 105),
            warning: rgb(217, 119, 6),
            warning_subtle: rgb(254, 243, 199),
            danger: rgb(220, 38, 38),
            danger_subtle: rgb(254, 226, 226),
        },
        dark: Tokens {
            dark: true,
            base: rgb(12, 32, 45),
            surface: rgb(17, 47, 65),
            surface_raised: rgb(24, 62, 84),
            well: rgb(8, 21, 29),
            overlay: rgb(17, 47, 65),
            foreground: rgb(224, 242, 254),
            muted: rgb(125, 211, 252),
            subtle: rgb(56, 189, 248),
            inverted: rgb(12, 32, 45),
            primary: rgb(56, 189, 248),
            primary_hover: rgb(14, 165, 233),
            primary_subtle: rgb(8, 47, 73),
            primary_text: rgb(56, 189, 248),
            border: rgb(24, 62, 84),
            border_strong: rgb(40, 90, 120),
            ring: rgb(56, 189, 248),
            success: rgb(74, 222, 128),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(113, 63, 18),
            danger: rgb(248, 113, 113),
            danger_subtle: rgb(127, 29, 29),
        },
    },
    Family {
        id: "forest",
        name: "Forest",
        light: Tokens {
            dark: false,
            base: rgb(240, 253, 244),
            surface: rgb(220, 252, 231),
            surface_raised: rgb(255, 255, 255),
            well: rgb(220, 252, 231),
            overlay: rgb(255, 255, 255),
            foreground: rgb(20, 83, 45),
            muted: rgb(46, 139, 81),
            subtle: rgb(134, 239, 172),
            inverted: rgb(255, 255, 255),
            primary: rgb(22, 163, 74),
            primary_hover: rgb(21, 128, 61),
            primary_subtle: rgb(187, 247, 208),
            primary_text: rgb(22, 163, 74),
            border: rgb(187, 247, 208),
            border_strong: rgb(134, 239, 172),
            ring: rgb(22, 163, 74),
            success: rgb(22, 163, 74),
            warning: rgb(217, 119, 6),
            warning_subtle: rgb(254, 243, 199),
            danger: rgb(220, 38, 38),
            danger_subtle: rgb(254, 226, 226),
        },
        dark: Tokens {
            dark: true,
            base: rgb(10, 26, 15),
            surface: rgb(18, 42, 24),
            surface_raised: rgb(27, 60, 35),
            well: rgb(6, 17, 10),
            overlay: rgb(18, 42, 24),
            foreground: rgb(220, 252, 231),
            muted: rgb(134, 239, 172),
            subtle: rgb(74, 222, 128),
            inverted: rgb(10, 26, 15),
            primary: rgb(74, 222, 128),
            primary_hover: rgb(34, 197, 94),
            primary_subtle: rgb(6, 38, 16),
            primary_text: rgb(74, 222, 128),
            border: rgb(27, 60, 35),
            border_strong: rgb(40, 85, 50),
            ring: rgb(74, 222, 128),
            success: rgb(74, 222, 128),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(113, 63, 18),
            danger: rgb(248, 113, 113),
            danger_subtle: rgb(127, 29, 29),
        },
    },
    Family {
        id: "sunset",
        name: "Sunset",
        light: Tokens {
            dark: false,
            base: rgb(255, 251, 235),
            surface: rgb(254, 243, 199),
            surface_raised: rgb(255, 255, 255),
            well: rgb(254, 243, 199),
            overlay: rgb(255, 255, 255),
            foreground: rgb(120, 53, 15),
            muted: rgb(175, 102, 42),
            subtle: rgb(253, 186, 116),
            inverted: rgb(255, 255, 255),
            primary: rgb(239, 68, 68),
            primary_hover: rgb(220, 38, 38),
            primary_subtle: rgb(254, 202, 202),
            primary_text: rgb(239, 68, 68),
            border: rgb(253, 230, 138),
            border_strong: rgb(252, 211, 77),
            ring: rgb(239, 68, 68),
            success: rgb(22, 163, 74),
            warning: rgb(217, 119, 6),
            warning_subtle: rgb(254, 243, 199),
            danger: rgb(220, 38, 38),
            danger_subtle: rgb(254, 226, 226),
        },
        dark: Tokens {
            dark: true,
            base: rgb(28, 14, 8),
            surface: rgb(45, 22, 12),
            surface_raised: rgb(64, 32, 18),
            well: rgb(18, 9, 5),
            overlay: rgb(45, 22, 12),
            foreground: rgb(254, 243, 199),
            muted: rgb(253, 186, 116),
            subtle: rgb(251, 146, 60),
            inverted: rgb(28, 14, 8),
            primary: rgb(248, 113, 113),
            primary_hover: rgb(239, 68, 68),
            primary_subtle: rgb(69, 20, 20),
            primary_text: rgb(248, 113, 113),
            border: rgb(64, 32, 18),
            border_strong: rgb(90, 45, 25),
            ring: rgb(248, 113, 113),
            success: rgb(74, 222, 128),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(113, 63, 18),
            danger: rgb(248, 113, 113),
            danger_subtle: rgb(127, 29, 29),
        },
    },
    Family {
        id: "midnight",
        name: "Midnight",
        light: Tokens {
            dark: false,
            base: rgb(241, 245, 249),
            surface: rgb(226, 232, 240),
            surface_raised: rgb(255, 255, 255),
            well: rgb(226, 232, 240),
            overlay: rgb(255, 255, 255),
            foreground: rgb(15, 23, 42),
            muted: rgb(71, 85, 105),
            subtle: rgb(100, 116, 139),
            inverted: rgb(255, 255, 255),
            primary: rgb(251, 146, 60),
            primary_hover: rgb(249, 115, 22),
            primary_subtle: rgb(255, 237, 213),
            primary_text: rgb(234, 88, 12),
            border: rgb(203, 213, 225),
            border_strong: rgb(148, 163, 184),
            ring: rgb(251, 146, 60),
            success: rgb(22, 163, 74),
            warning: rgb(234, 179, 8),
            warning_subtle: rgb(254, 249, 195),
            danger: rgb(220, 38, 38),
            danger_subtle: rgb(254, 226, 226),
        },
        dark: Tokens {
            dark: true,
            base: rgb(15, 23, 42),
            surface: rgb(30, 41, 59),
            surface_raised: rgb(51, 65, 85),
            well: rgb(10, 15, 27),
            overlay: rgb(30, 41, 59),
            foreground: rgb(241, 245, 249),
            muted: rgb(148, 163, 184),
            subtle: rgb(100, 116, 139),
            inverted: rgb(15, 23, 42),
            primary: rgb(251, 146, 60),
            primary_hover: rgb(249, 115, 22),
            primary_subtle: rgb(67, 20, 7),
            primary_text: rgb(251, 146, 60),
            border: rgb(51, 65, 85),
            border_strong: rgb(71, 85, 105),
            ring: rgb(251, 146, 60),
            success: rgb(74, 222, 128),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(113, 63, 18),
            danger: rgb(248, 113, 113),
            danger_subtle: rgb(127, 29, 29),
        },
    },
    Family {
        id: "lavender",
        name: "Lavender",
        light: Tokens {
            dark: false,
            base: rgb(250, 245, 255),
            surface: rgb(243, 232, 255),
            surface_raised: rgb(255, 255, 255),
            well: rgb(243, 232, 255),
            overlay: rgb(255, 255, 255),
            foreground: rgb(59, 7, 100),
            muted: rgb(129, 107, 194),
            subtle: rgb(196, 181, 253),
            inverted: rgb(255, 255, 255),
            primary: rgb(124, 58, 237),
            primary_hover: rgb(109, 40, 217),
            primary_subtle: rgb(237, 233, 254),
            primary_text: rgb(124, 58, 237),
            border: rgb(221, 214, 254),
            border_strong: rgb(196, 181, 253),
            ring: rgb(124, 58, 237),
            success: rgb(22, 163, 74),
            warning: rgb(217, 119, 6),
            warning_subtle: rgb(254, 243, 199),
            danger: rgb(220, 38, 38),
            danger_subtle: rgb(254, 226, 226),
        },
        dark: Tokens {
            dark: true,
            base: rgb(18, 10, 35),
            surface: rgb(28, 16, 54),
            surface_raised: rgb(40, 24, 74),
            well: rgb(12, 6, 23),
            overlay: rgb(28, 16, 54),
            foreground: rgb(243, 232, 255),
            muted: rgb(216, 180, 254),
            subtle: rgb(192, 132, 252),
            inverted: rgb(18, 10, 35),
            primary: rgb(167, 139, 250),
            primary_hover: rgb(139, 92, 246),
            primary_subtle: rgb(46, 16, 101),
            primary_text: rgb(167, 139, 250),
            border: rgb(40, 24, 74),
            border_strong: rgb(60, 36, 100),
            ring: rgb(167, 139, 250),
            success: rgb(74, 222, 128),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(113, 63, 18),
            danger: rgb(248, 113, 113),
            danger_subtle: rgb(127, 29, 29),
        },
    },
];

fn default_family() -> &'static Family {
    let [family, ..] = &FAMILIES;
    family
}

impl Tokens {
    /// The palette in effect, as [`apply`] last set it.
    pub fn get(ctx: &egui::Context) -> Arc<Tokens> {
        ctx.data(|data| data.get_temp::<Arc<Tokens>>(egui::Id::NULL)).unwrap_or_else(|| Arc::new(default_family().dark.clone()))
    }

    /// This palette with `accent` for its accent colour, and the colours that follow from one.
    pub fn with_accent(mut self, accent: Color32) -> Tokens {
        let white = Color32::WHITE;
        self.primary = accent;
        self.ring = accent;
        self.primary_hover = crate::motion::mix(accent, Color32::BLACK, 0.18);
        self.primary_subtle = if self.dark { crate::motion::mix(self.base, accent, 0.22) } else { crate::motion::mix(accent, white, 0.82) };
        // A system's accent is chosen for filling a button, not for text on this palette's background.
        self.primary_text = readable(accent, self.base);
        self.inverted = if contrast(white, accent) >= contrast(Color32::BLACK, accent) { white } else { rgb(20, 20, 20) };
        self
    }
}

/// How bright a colour is to the eye, from 0 to 1 (WCAG's relative luminance).
fn luminance(color: Color32) -> f32 {
    let channel = |value: u8| {
        let value = f32::from(value) / 255.0;
        if value <= 0.04045 { value / 12.92 } else { ((value + 0.055) / 1.055).powf(2.4) }
    };
    0.2126 * channel(color.r()) + 0.7152 * channel(color.g()) + 0.0722 * channel(color.b())
}

/// WCAG's contrast ratio between two colours: 1 for the same colour, 21 for black on white.
fn contrast(a: Color32, b: Color32) -> f32 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

/// `color`, moved away from `on` as far as it takes to be read as text on it.
fn readable(color: Color32, on: Color32) -> Color32 {
    let away = if luminance(on) < 0.5 { Color32::WHITE } else { Color32::BLACK };
    (0..=10).map(|step| crate::motion::mix(color, away, step as f32 / 10.0)).find(|&mixed| contrast(mixed, on) >= 4.5).unwrap_or(away)
}

/// Which theme the interface is drawn in: what Preferences chooses and a run remembers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// A [`Family`]'s id.
    pub family: &'static str,
    pub dark: bool,
    /// The system's accent colour in place of the family's, where the system has one.
    pub system_accent: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self { family: default_family().id, dark: true, system_accent: true }
    }
}

impl Theme {
    /// A choice as [`Theme::stored`] wrote it; what it does not say, or says wrongly, is the default.
    pub fn parse(text: &str) -> Self {
        let mut theme = Self::default();
        for word in text.split_whitespace() {
            match word {
                "dark" => theme.dark = true,
                "light" => theme.dark = false,
                "system-accent" => theme.system_accent = true,
                "own-accent" => theme.system_accent = false,
                id => {
                    if let Some(family) = FAMILIES.iter().find(|family| family.id == id) {
                        theme.family = family.id;
                    }
                }
            }
        }
        theme
    }

    pub fn stored(&self) -> String {
        let mode = if self.dark { "dark" } else { "light" };
        format!("{} {mode} {}", self.family, if self.system_accent { "system-accent" } else { "own-accent" })
    }

    pub fn family(&self) -> &'static Family {
        FAMILIES.iter().find(|family| family.id == self.family).unwrap_or_else(default_family)
    }

    /// The palette this choice comes to; `accent` is the system's accent colour, if it has one.
    pub fn tokens(&self, accent: Option<Color32>) -> Tokens {
        let tokens = self.family().variant(self.dark).clone();
        match accent.filter(|_| self.system_accent) {
            Some(accent) => tokens.with_accent(accent),
            None => tokens,
        }
    }
}

/// The interface's typeface, the same in the browser build, which has no system fonts.
const UI_FONT: &[u8] = include_bytes!("../assets/fonts/NotoSans-Regular.ttf");

/// The interface font before egui's own, which keep the symbols and the fixed-width text.
pub fn fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("ui".to_owned(), Arc::new(egui::FontData::from_static(UI_FONT)));
    if let Some(names) = fonts.families.get_mut(&FontFamily::Proportional) {
        names.insert(0, "ui".to_owned());
        // The arrows and triangles set in running text are only in egui's monospace font.
        names.push("Hack".to_owned());
    }
    fonts
}

pub fn body(size: f32) -> FontId {
    FontId::new(size, FontFamily::Proportional)
}

/// Set egui's style from a palette and make it the one [`Tokens::get`] returns.
pub fn apply(ctx: &egui::Context, tokens: &Tokens) {
    let theme = if tokens.dark { egui::Theme::Dark } else { egui::Theme::Light };
    ctx.set_visuals_of(theme, visuals(tokens));
    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 5.0);
        style.spacing.button_padding = crate::controls::PADDING;
        style.spacing.interact_size = egui::vec2(32.0, crate::controls::HEIGHT);
        style.spacing.menu_margin = egui::Margin::same(6);
        style.text_styles.insert(egui::TextStyle::Body, body(13.5));
        style.text_styles.insert(egui::TextStyle::Button, body(13.5));
        style.text_styles.insert(egui::TextStyle::Small, body(11.5));
        style.text_styles.insert(egui::TextStyle::Heading, body(20.0));
    });
    ctx.options_mut(|options| {
        options.fallback_theme = theme;
        options.theme_preference = if tokens.dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light };
    });
    ctx.data_mut(|data| data.insert_temp(egui::Id::NULL, Arc::new(tokens.clone())));
}

fn visuals(t: &Tokens) -> Visuals {
    let mut v = if t.dark { Visuals::dark() } else { Visuals::light() };
    // Small enough that a checkbox still reads as a box, not a radio button.
    let radius = CornerRadius::same(crate::controls::RADIUS);
    v.panel_fill = t.base;
    v.window_fill = t.overlay;
    v.extreme_bg_color = t.well;
    v.faint_bg_color = t.surface;
    v.window_stroke = Stroke::new(1.0, t.border);
    v.window_corner_radius = CornerRadius::same(10);
    v.menu_corner_radius = radius;
    v.selection.bg_fill = t.primary.gamma_multiply(0.35);
    v.selection.stroke = Stroke::new(1.0, t.primary);
    v.hyperlink_color = t.primary_text;
    v.override_text_color = Some(t.foreground);
    v.warn_fg_color = t.warning;
    v.error_fg_color = t.danger;

    v.widgets.noninteractive.bg_fill = t.surface;
    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, t.foreground);
    v.widgets.noninteractive.corner_radius = radius;

    v.widgets.inactive.bg_fill = t.surface;
    v.widgets.inactive.weak_bg_fill = t.surface;
    v.widgets.inactive.bg_stroke = Stroke::new(1.0, t.border);
    v.widgets.inactive.fg_stroke = Stroke::new(1.0, t.foreground);
    v.widgets.inactive.corner_radius = radius;

    v.widgets.hovered.bg_fill = t.surface_raised;
    v.widgets.hovered.weak_bg_fill = t.border;
    v.widgets.hovered.bg_stroke = Stroke::new(1.0, t.border_strong);
    v.widgets.hovered.fg_stroke = Stroke::new(1.0, t.foreground);
    v.widgets.hovered.corner_radius = radius;

    v.widgets.active.bg_fill = t.border_strong;
    v.widgets.active.weak_bg_fill = t.border_strong;
    v.widgets.active.bg_stroke = Stroke::new(1.0, t.ring);
    v.widgets.active.fg_stroke = Stroke::new(1.0, t.foreground);
    v.widgets.active.corner_radius = radius;

    v.widgets.open = v.widgets.hovered;
    // egui grows a hovered or pressed widget by a pixel or two, which moves the text of anything left-aligned: a tab, a
    // list row.
    for state in [&mut v.widgets.noninteractive, &mut v.widgets.inactive, &mut v.widgets.hovered, &mut v.widgets.active, &mut v.widgets.open] {
        state.expansion = 0.0;
    }
    v
}

/// The document tabs and the splits between them, in the palette's colours.
pub fn dock_style(ui: &egui::Ui) -> egui_dock::Style {
    let t = Tokens::get(ui.ctx());
    let mut style = egui_dock::Style::from_egui(ui.style());
    style.main_surface_border_stroke = Stroke::NONE;
    style.separator.color_idle = t.border;
    style.separator.color_hovered = t.border_strong;
    style.separator.color_dragged = t.primary;
    style.tab_bar.bg_fill = t.base;
    style.tab_bar.hline_color = t.border;
    style.tab_bar.corner_radius = CornerRadius::ZERO;
    // Tall and wide enough to hit without aiming; the dock's own 24 points is cramped next to the toolbar's buttons.
    style.tab_bar.height = 32.0;
    style.tab.minimum_width = Some(96.0);
    style.tab.tab_body.bg_fill = t.base;
    style.tab.tab_body.stroke = Stroke::NONE;
    style.tab.tab_body.corner_radius = CornerRadius::ZERO;
    style.tab.hline_below_active_tab_name = true;
    let radius = CornerRadius { nw: crate::controls::RADIUS, ne: crate::controls::RADIUS, sw: 0, se: 0 };
    for (state, fill, text, edge) in [
        (&mut style.tab.active, t.surface, t.foreground, t.border),
        (&mut style.tab.inactive, t.base, t.muted, Color32::TRANSPARENT),
        (&mut style.tab.focused, t.surface, t.primary_text, t.border),
        (&mut style.tab.hovered, t.surface, t.foreground, t.border),
        (&mut style.tab.active_with_kb_focus, t.surface, t.primary_text, t.border),
        (&mut style.tab.inactive_with_kb_focus, t.base, t.foreground, t.border),
        (&mut style.tab.focused_with_kb_focus, t.surface, t.primary_text, t.border),
    ] {
        state.bg_fill = fill;
        state.text_color = text;
        state.outline_color = edge;
        state.corner_radius = radius;
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn applying_a_palette_makes_it_the_one_in_effect() {
        let ctx = egui::Context::default();
        assert!(Tokens::get(&ctx).dark);
        let light = Theme { dark: false, ..Theme::default() }.tokens(None);
        apply(&ctx, &light);
        assert_eq!(*Tokens::get(&ctx), light);
        assert_eq!(ctx.global_style().visuals.panel_fill, light.base);
        let ocean = Theme { family: "ocean", ..Theme::default() }.tokens(None);
        apply(&ctx, &ocean);
        assert_eq!(ctx.global_style().visuals.panel_fill, ocean.base);
        assert_ne!(ocean.base, Theme::default().tokens(None).base);
    }

    #[test]
    fn a_choice_is_stored_and_read_back() {
        for family in &FAMILIES {
            for (dark, system_accent) in [(true, false), (false, true)] {
                let theme = Theme { family: family.id, dark, system_accent };
                assert_eq!(Theme::parse(&theme.stored()), theme);
            }
        }
        // What an earlier version stored, and what nothing stored.
        assert_eq!(Theme::parse("light"), Theme { dark: false, ..Theme::default() });
        assert_eq!(Theme::parse("no-such-theme"), Theme::default());
    }

    #[test]
    fn every_palette_is_in_its_own_mode_and_readable() {
        for family in &FAMILIES {
            assert!(family.dark.dark && !family.light.dark, "{}", family.id);
            for tokens in [&family.light, &family.dark] {
                assert!(contrast(tokens.foreground, tokens.base) >= 7.0, "{} foreground", family.id);
                assert!(contrast(tokens.muted, tokens.base) >= 3.9, "{} muted", family.id);
            }
        }
    }

    #[test]
    fn the_system_accent_is_used_only_when_chosen_and_stays_readable() {
        let navy = Color32::from_rgb(10, 20, 90);
        let plain = Theme { system_accent: false, ..Theme::default() };
        assert_eq!(plain.tokens(Some(navy)), plain.tokens(None));
        let chosen = Theme::default();
        // A system without an accent colour leaves the theme's own.
        assert_eq!(chosen.tokens(None), plain.tokens(None));
        for dark in [true, false] {
            for accent in [navy, Color32::from_rgb(255, 235, 60)] {
                let tokens = Theme { dark, ..chosen }.tokens(Some(accent));
                assert_eq!(tokens.primary, accent);
                assert!(contrast(tokens.primary_text, tokens.base) >= 4.5);
                assert!(contrast(tokens.inverted, tokens.primary) >= 4.5);
            }
        }
    }
}
