//! Colours and the egui style. One palette in a light and a dark mode; every colour the interface draws its chrome in
//! is a token here, and [`apply`] turns the tokens into the style egui's own widgets take.

use std::sync::Arc;

use egui::{Color32, CornerRadius, FontFamily, FontId, Stroke, Visuals};

use crate::i18n::{self, key, tr};

/// One palette, light or dark.
#[derive(Debug, Clone, PartialEq)]
pub struct Tokens {
    pub dark: bool,
    /// Window background.
    pub base: Color32,
    /// Card, panel and field background.
    pub surface: Color32,
    pub surface_raised: Color32,
    /// What is drawn on: the level canvas.
    pub well: Color32,
    /// Dialog and menu background.
    pub overlay: Color32,
    /// A button or drop-down, and the text on it; a palette may set them apart from the panel it sits on.
    pub control: Color32,
    pub control_hover: Color32,
    pub control_text: Color32,
    /// What is typed into, in the text of a control.
    pub field: Color32,
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

/// The themes, the default one first.
pub static FAMILIES: [Family; 7] = [
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
            control: rgb(245, 247, 250),
            control_hover: rgb(226, 232, 240),
            control_text: rgb(20, 20, 20),
            field: rgb(245, 247, 250),
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
            control: rgb(38, 38, 38),
            control_hover: rgb(64, 64, 64),
            control_text: rgb(245, 245, 245),
            field: rgb(15, 15, 15),
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
            control: rgb(224, 242, 254),
            control_hover: rgb(186, 230, 253),
            control_text: rgb(12, 74, 110),
            field: rgb(224, 242, 254),
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
            control: rgb(17, 47, 65),
            control_hover: rgb(24, 62, 84),
            control_text: rgb(224, 242, 254),
            field: rgb(8, 21, 29),
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
            control: rgb(220, 252, 231),
            control_hover: rgb(187, 247, 208),
            control_text: rgb(20, 83, 45),
            field: rgb(220, 252, 231),
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
            control: rgb(18, 42, 24),
            control_hover: rgb(27, 60, 35),
            control_text: rgb(220, 252, 231),
            field: rgb(6, 17, 10),
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
            control: rgb(254, 243, 199),
            control_hover: rgb(253, 230, 138),
            control_text: rgb(120, 53, 15),
            field: rgb(254, 243, 199),
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
            control: rgb(45, 22, 12),
            control_hover: rgb(64, 32, 18),
            control_text: rgb(254, 243, 199),
            field: rgb(18, 9, 5),
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
            control: rgb(226, 232, 240),
            control_hover: rgb(203, 213, 225),
            control_text: rgb(15, 23, 42),
            field: rgb(226, 232, 240),
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
            control: rgb(30, 41, 59),
            control_hover: rgb(51, 65, 85),
            control_text: rgb(241, 245, 249),
            field: rgb(10, 15, 27),
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
            control: rgb(243, 232, 255),
            control_hover: rgb(221, 214, 254),
            control_text: rgb(59, 7, 100),
            field: rgb(243, 232, 255),
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
            control: rgb(28, 16, 54),
            control_hover: rgb(40, 24, 74),
            control_text: rgb(243, 232, 255),
            field: rgb(12, 6, 23),
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
    // Neither mode at its extreme: for those who find a white window too bright and a black one too dark.
    Family {
        id: "overcast",
        name: "Overcast",
        light: Tokens {
            dark: false,
            base: rgb(179, 179, 179),
            surface: rgb(166, 166, 166),
            surface_raised: rgb(196, 196, 196),
            well: rgb(156, 156, 156),
            overlay: rgb(190, 190, 190),
            control: rgb(59, 59, 59),
            control_hover: rgb(78, 78, 78),
            control_text: rgb(240, 240, 240),
            field: rgb(36, 36, 36),
            foreground: rgb(16, 16, 16),
            muted: rgb(70, 70, 70),
            subtle: rgb(96, 96, 96),
            inverted: rgb(255, 255, 255),
            primary: rgb(70, 112, 180),
            primary_hover: rgb(56, 94, 158),
            primary_subtle: rgb(150, 167, 190),
            primary_text: rgb(24, 66, 132),
            border: rgb(148, 148, 148),
            border_strong: rgb(124, 124, 124),
            ring: rgb(24, 66, 132),
            success: rgb(0, 78, 32),
            warning: rgb(98, 58, 0),
            warning_subtle: rgb(205, 188, 130),
            danger: rgb(135, 10, 10),
            danger_subtle: rgb(210, 165, 160),
        },
        dark: Tokens {
            dark: true,
            base: rgb(62, 62, 62),
            surface: rgb(74, 74, 74),
            surface_raised: rgb(86, 86, 86),
            well: rgb(48, 48, 48),
            overlay: rgb(74, 74, 74),
            control: rgb(74, 74, 74),
            control_hover: rgb(92, 92, 92),
            control_text: rgb(238, 238, 238),
            field: rgb(48, 48, 48),
            foreground: rgb(238, 238, 238),
            muted: rgb(182, 182, 182),
            subtle: rgb(142, 142, 142),
            inverted: rgb(20, 20, 20),
            primary: rgb(110, 165, 235),
            primary_hover: rgb(85, 140, 215),
            primary_subtle: rgb(66, 86, 114),
            primary_text: rgb(140, 188, 245),
            border: rgb(92, 92, 92),
            border_strong: rgb(112, 112, 112),
            ring: rgb(140, 188, 245),
            success: rgb(134, 239, 172),
            warning: rgb(253, 224, 71),
            warning_subtle: rgb(105, 75, 25),
            danger: rgb(252, 165, 165),
            danger_subtle: rgb(122, 45, 45),
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

    /// The palette `t` of the way from this one to `to`, each colour mixed; past halfway it is in `to`'s mode.
    pub fn mix(&self, to: &Tokens, t: f32) -> Tokens {
        let mix = |a, b| crate::motion::mix(a, b, t);
        Tokens {
            dark: if t < 0.5 { self.dark } else { to.dark },
            base: mix(self.base, to.base),
            surface: mix(self.surface, to.surface),
            surface_raised: mix(self.surface_raised, to.surface_raised),
            well: mix(self.well, to.well),
            overlay: mix(self.overlay, to.overlay),
            control: mix(self.control, to.control),
            control_hover: mix(self.control_hover, to.control_hover),
            control_text: mix(self.control_text, to.control_text),
            field: mix(self.field, to.field),
            foreground: mix(self.foreground, to.foreground),
            muted: mix(self.muted, to.muted),
            subtle: mix(self.subtle, to.subtle),
            inverted: mix(self.inverted, to.inverted),
            primary: mix(self.primary, to.primary),
            primary_hover: mix(self.primary_hover, to.primary_hover),
            primary_subtle: mix(self.primary_subtle, to.primary_subtle),
            primary_text: mix(self.primary_text, to.primary_text),
            border: mix(self.border, to.border),
            border_strong: mix(self.border_strong, to.border_strong),
            ring: mix(self.ring, to.ring),
            success: mix(self.success, to.success),
            warning: mix(self.warning, to.warning),
            warning_subtle: mix(self.warning_subtle, to.warning_subtle),
            danger: mix(self.danger, to.danger),
            danger_subtle: mix(self.danger_subtle, to.danger_subtle),
        }
    }

    /// This palette with `accent` for its accent colour, and the colours that follow from one.
    pub fn with_accent(mut self, accent: Color32) -> Tokens {
        let white = Color32::WHITE;
        self.primary = accent;
        self.ring = accent;
        self.primary_hover = crate::motion::mix(accent, Color32::BLACK, 0.18);
        self.primary_subtle = crate::motion::mix(self.base, accent, if self.dark { 0.22 } else { 0.18 });
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
    let away = if contrast(Color32::WHITE, on) >= contrast(Color32::BLACK, on) { Color32::WHITE } else { Color32::BLACK };
    (0..=10).map(|step| crate::motion::mix(color, away, step as f32 / 10.0)).find(|&mixed| contrast(mixed, on) >= 4.5).unwrap_or(away)
}

/// A theme someone made or imported: a palette in each mode, like a [`Family`], kept in the settings.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomTheme {
    /// What a stored choice names it by: `custom-` and a number, so it has no spaces and is no family's.
    pub id: String,
    pub name: String,
    pub light: Tokens,
    pub dark: Tokens,
}

/// A theme of either sort.
#[derive(Debug, Clone, Copy)]
pub struct AnyTheme<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub light: &'a Tokens,
    pub dark: &'a Tokens,
    pub built_in: bool,
}

impl<'a> AnyTheme<'a> {
    /// The name shown to people: a built-in theme's in the interface's language, a custom theme's as it was typed.
    /// `name` stays the English one, which is what is exported and what [`distinct_name`] keeps clear of.
    pub fn display_name(&self) -> &'a str {
        if !self.built_in {
            return self.name;
        }
        match self.id {
            "default" => i18n::text(key!("theme.family.default")),
            "ocean" => i18n::text(key!("theme.family.ocean")),
            "forest" => i18n::text(key!("theme.family.forest")),
            "sunset" => i18n::text(key!("theme.family.sunset")),
            "midnight" => i18n::text(key!("theme.family.midnight")),
            "lavender" => i18n::text(key!("theme.family.lavender")),
            "overcast" => i18n::text(key!("theme.family.overcast")),
            _ => self.name,
        }
    }

    pub fn variant(&self, dark: bool) -> &'a Tokens {
        if dark { self.dark } else { self.light }
    }
}

/// Every theme to choose from: the families, then the custom ones.
pub fn all_themes(custom: &[CustomTheme]) -> impl Iterator<Item = AnyTheme<'_>> {
    let families =
        FAMILIES.iter().map(|family| AnyTheme { id: family.id, name: family.name, light: &family.light, dark: &family.dark, built_in: true });
    families.chain(custom.iter().map(|theme| AnyTheme { id: &theme.id, name: &theme.name, light: &theme.light, dark: &theme.dark, built_in: false }))
}

/// One colour of a palette: what a theme file calls it, what the theme editor calls it, and where it is.
pub struct Token {
    pub key: &'static str,
    /// The language key of what the theme editor calls it; `i18n::text` turns it into words.
    pub label: &'static str,
    pub color: fn(&mut Tokens) -> &mut Color32,
}

const fn token(key: &'static str, label: &'static str, color: fn(&mut Tokens) -> &mut Color32) -> Token {
    Token { key, label, color }
}

/// Every colour of a palette, in the order the theme editor shows them and a theme file lists them. A group is named by a
/// language key, as a token is labelled.
pub const TOKEN_GROUPS: [(&str, &[Token]); 6] = [
    (
        key!("theme.group.backgrounds"),
        &[
            token("base", key!("theme.token.window"), |t| &mut t.base),
            token("surface", key!("theme.token.card_and_panel"), |t| &mut t.surface),
            token("surfaceRaised", key!("theme.token.raised_card"), |t| &mut t.surface_raised),
            token("well", key!("theme.token.canvas"), |t| &mut t.well),
            token("overlay", key!("theme.token.dialog_and_menu"), |t| &mut t.overlay),
        ],
    ),
    (
        key!("theme.group.controls"),
        &[
            token("control", key!("theme.token.button"), |t| &mut t.control),
            token("controlHover", key!("theme.token.button_under_the_pointer"), |t| &mut t.control_hover),
            token("controlText", key!("theme.token.button_text"), |t| &mut t.control_text),
            token("field", key!("theme.token.text_field"), |t| &mut t.field),
        ],
    ),
    (
        key!("theme.group.text"),
        &[
            token("foreground", key!("theme.token.text"), |t| &mut t.foreground),
            token("muted", key!("theme.token.muted_text"), |t| &mut t.muted),
            token("subtle", key!("theme.token.placeholder_text"), |t| &mut t.subtle),
            token("inverted", key!("theme.token.text_on_the_accent"), |t| &mut t.inverted),
        ],
    ),
    (
        key!("theme.group.accent"),
        &[
            token("primary", key!("theme.token.accent"), |t| &mut t.primary),
            token("primaryHover", key!("theme.token.accent_under_the_pointer"), |t| &mut t.primary_hover),
            token("primarySubtle", key!("theme.token.accent_tint"), |t| &mut t.primary_subtle),
            token("primaryText", key!("theme.token.accent_text"), |t| &mut t.primary_text),
            token("ring", key!("theme.token.focus_ring"), |t| &mut t.ring),
        ],
    ),
    (
        key!("theme.group.borders"),
        &[
            token("border", key!("theme.token.border"), |t| &mut t.border),
            token("borderStrong", key!("theme.token.strong_border"), |t| &mut t.border_strong),
        ],
    ),
    (
        key!("theme.group.status"),
        &[
            token("success", key!("theme.token.success"), |t| &mut t.success),
            token("warning", key!("theme.token.warning"), |t| &mut t.warning),
            token("warningSubtle", key!("theme.token.warning_tint"), |t| &mut t.warning_subtle),
            token("danger", key!("theme.token.danger"), |t| &mut t.danger),
            token("dangerSubtle", key!("theme.token.danger_tint"), |t| &mut t.danger_subtle),
        ],
    ),
];

/// A colour as a theme file writes it: `"234 88 12"`.
pub fn channels(color: Color32) -> String {
    format!("{} {} {}", color.r(), color.g(), color.b())
}

fn parse_channels(text: &str) -> Option<Color32> {
    let mut parts = text.split_whitespace().map(|part| part.parse::<u8>().ok());
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some(Some(r)), Some(Some(g)), Some(Some(b)), None) => Some(rgb(r, g, b)),
        _ => None,
    }
}

fn tokens_json(tokens: &Tokens) -> serde_json::Value {
    let mut tokens = tokens.clone();
    let colors = TOKEN_GROUPS.iter().flat_map(|(_, group)| group.iter());
    serde_json::Value::Object(colors.map(|token| (token.key.to_owned(), channels(*(token.color)(&mut tokens)).into())).collect())
}

/// The colours a theme file lists under `key`, over those of `over`, and whether it listed any this editor knows.
fn tokens_from_json(file: &serde_json::Value, key: &str, over: &Tokens) -> Result<(Tokens, bool), String> {
    let mut tokens = over.clone();
    let Some(listed) = file.get(key) else { return Ok((tokens, false)) };
    let listed = listed.as_object().ok_or_else(|| tr!("theme.error.not_a_list", list = key))?;
    let mut any = false;
    for token in TOKEN_GROUPS.iter().flat_map(|(_, group)| group.iter()) {
        if let Some(value) = listed.get(token.key) {
            let color = value.as_str().and_then(parse_channels);
            *(token.color)(&mut tokens) = color.ok_or_else(|| tr!("theme.error.not_numbers", token = token.key, list = key))?;
            any = true;
        }
    }
    Ok((tokens, any))
}

fn theme_json(name: &str, light: &Tokens, dark: &Tokens) -> serde_json::Map<String, serde_json::Value> {
    let mut json = serde_json::Map::new();
    json.insert("name".to_owned(), name.into());
    json.insert("lightTokens".to_owned(), tokens_json(light));
    json.insert("darkTokens".to_owned(), tokens_json(dark));
    json
}

/// A theme as a file to share. The system's accent colour is not part of it: that is this machine's.
pub fn theme_file(theme: AnyTheme<'_>) -> Result<String, String> {
    serde_json::to_string_pretty(&theme_json(theme.name, theme.light, theme.dark)).map_err(|error| error.to_string())
}

/// What a theme is exported as: a file name that holds only what every system allows.
pub fn theme_file_name(name: &str) -> String {
    let words: Vec<_> = name.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).collect();
    format!("bigstar-theme-{}.json", words.join("-").to_lowercase())
}

fn theme_from_json(file: &serde_json::Value) -> Result<CustomTheme, String> {
    if !file.is_object() {
        return Err(tr!("theme.error.holds_no_theme").to_owned());
    }
    let name = match file.get("name") {
        None => "",
        Some(name) => name.as_str().ok_or_else(|| tr!("theme.error.name_not_text").to_owned())?,
    };
    if name.trim().is_empty() {
        return Err(tr!("theme.error.no_name").to_owned());
    }
    // What a file leaves out is the default theme's, in each mode.
    let (light, any_light) = tokens_from_json(file, "lightTokens", &default_family().light)?;
    let (dark, any_dark) = tokens_from_json(file, "darkTokens", &default_family().dark)?;
    if !any_light && !any_dark {
        return Err(tr!("theme.error.no_colours").to_owned());
    }
    Ok(CustomTheme { id: String::new(), name: tidy_name(name), light, dark })
}

/// The theme a file holds, with no id yet. Anything else in the file is ignored, a colour this editor cannot read is not.
pub fn parse_theme_file(text: &str) -> Result<CustomTheme, String> {
    let file = serde_json::from_str(text).map_err(|error| tr!("theme.error.not_a_theme_file", error))?;
    theme_from_json(&file)
}

/// The custom themes as the settings keep them.
pub fn stored_themes(custom: &[CustomTheme]) -> Option<String> {
    let stored: Vec<_> = custom
        .iter()
        .map(|theme| {
            let mut json = serde_json::Map::new();
            json.insert("id".to_owned(), theme.id.clone().into());
            json.extend(theme_json(&theme.name, &theme.light, &theme.dark));
            json
        })
        .collect();
    serde_json::to_string(&stored).ok()
}

/// The custom themes [`stored_themes`] wrote, each on its own, so a damaged one costs only itself.
pub fn parse_stored_themes(text: &str) -> Vec<CustomTheme> {
    let entries: Vec<serde_json::Value> = serde_json::from_str(text).unwrap_or_default();
    let mut themes: Vec<CustomTheme> = Vec::new();
    for entry in &entries {
        let Ok(mut theme) = theme_from_json(entry) else { continue };
        theme.id = entry
            .get("id")
            .and_then(serde_json::Value::as_str)
            .filter(|id| id.starts_with("custom-") && !id.contains(char::is_whitespace) && !themes.iter().any(|theme| theme.id == *id))
            .map_or_else(|| new_theme_id(&themes), str::to_owned);
        themes.push(theme);
    }
    themes
}

/// The lowest id no custom theme has.
pub fn new_theme_id(custom: &[CustomTheme]) -> String {
    (1u32..).map(|number| format!("custom-{number}")).find(|id| custom.iter().all(|theme| theme.id != *id)).unwrap_or_default()
}

fn tidy_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `name`, numbered if a theme other than `except` already goes by it.
pub fn distinct_name(custom: &[CustomTheme], name: &str, except: Option<&str>) -> String {
    let name = tidy_name(name);
    // A built-in theme is also known by its name in the interface's language, which a new name must not copy either.
    let taken =
        |candidate: &str| all_themes(custom).any(|theme| (theme.name == candidate || theme.display_name() == candidate) && Some(theme.id) != except);
    if !taken(&name) {
        return name;
    }
    (2u32..).map(|number| format!("{name} ({number})")).find(|candidate| !taken(candidate)).unwrap_or(name)
}

/// Whether the interface is light, dark, or as the system is, which may change on a schedule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Light,
    Dark,
    System,
}

impl Mode {
    pub const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    pub fn name(self) -> &'static str {
        match self {
            Self::Light => i18n::text(key!("theme.mode.light")),
            Self::Dark => i18n::text(key!("theme.mode.dark")),
            Self::System => i18n::text(key!("theme.mode.system")),
        }
    }

    fn word(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }
}

/// Which theme the interface is drawn in: what Preferences chooses and a run remembers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub mode: Mode,
    /// The ids of the themes drawn in the light mode and in the dark, which may differ. One that names no theme is the
    /// default.
    pub light: String,
    pub dark: String,
    /// The system's accent colour in place of the theme's, where the system has one.
    pub system_accent: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self { mode: Mode::Dark, light: default_family().id.to_owned(), dark: default_family().id.to_owned(), system_accent: true }
    }
}

fn family_id(id: &str) -> Option<&'static str> {
    FAMILIES.iter().find(|family| family.id == id).map(|family| family.id)
}

impl Theme {
    /// A choice as [`Theme::stored`] wrote it; what it does not say is the default. A family named on its own, as an
    /// earlier version stored it, is both modes'.
    pub fn parse(text: &str) -> Self {
        let mut theme = Self::default();
        for word in text.split_whitespace() {
            match word {
                "system-accent" => theme.system_accent = true,
                "own-accent" => theme.system_accent = false,
                word => {
                    if let Some(mode) = Mode::ALL.into_iter().find(|mode| mode.word() == word) {
                        theme.mode = mode;
                    } else if let Some(id) = word.strip_prefix("light:") {
                        theme.light = id.to_owned();
                    } else if let Some(id) = word.strip_prefix("dark:") {
                        theme.dark = id.to_owned();
                    } else if let Some(id) = family_id(word) {
                        (theme.light, theme.dark) = (id.to_owned(), id.to_owned());
                    }
                }
            }
        }
        theme
    }

    pub fn stored(&self) -> String {
        let accent = if self.system_accent { "system-accent" } else { "own-accent" };
        format!("{} light:{} dark:{} {accent}", self.mode.word(), self.light, self.dark)
    }

    /// Whether the interface is dark, given the system's mode where it says it; a system that does not is dark.
    pub fn is_dark(&self, system: Option<egui::Theme>) -> bool {
        match self.mode {
            Mode::Light => false,
            Mode::Dark => true,
            Mode::System => system != Some(egui::Theme::Light),
        }
    }

    /// The theme drawn in the dark or the light mode, among the families and `custom`.
    pub fn family<'a>(&self, custom: &'a [CustomTheme], dark: bool) -> AnyTheme<'a> {
        let id = if dark { &self.dark } else { &self.light };
        let family = default_family();
        let default = AnyTheme { id: family.id, name: family.name, light: &family.light, dark: &family.dark, built_in: true };
        all_themes(custom).find(|theme| theme.id == id).unwrap_or(default)
    }

    /// Chooses the theme drawn in the dark or the light mode.
    pub fn set_family(&mut self, dark: bool, id: &str) {
        id.clone_into(if dark { &mut self.dark } else { &mut self.light });
    }

    /// Stops choosing a theme that is gone, for the default; whether it was chosen.
    pub fn forget(&mut self, id: &str) -> bool {
        let default = default_family().id;
        let chosen = self.light == id || self.dark == id;
        for slot in [&mut self.light, &mut self.dark] {
            if slot == id {
                default.clone_into(slot);
            }
        }
        chosen
    }

    /// `tokens` with the system's accent colour, if it has one and it is wanted.
    pub fn accented(&self, tokens: Tokens, accent: Option<Color32>) -> Tokens {
        match accent.filter(|_| self.system_accent) {
            Some(accent) => tokens.with_accent(accent),
            None => tokens,
        }
    }

    /// The palette of the dark or the light mode; `accent` is the system's accent colour, if it has one.
    pub fn tokens(&self, custom: &[CustomTheme], dark: bool, accent: Option<Color32>) -> Tokens {
        self.accented(self.family(custom, dark).variant(dark).clone(), accent)
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
    // Anything that can be pressed says so under the pointer, as a link does.
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);

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
        let light = Theme::default().tokens(&[], false, None);
        apply(&ctx, &light);
        assert_eq!(*Tokens::get(&ctx), light);
        assert_eq!(ctx.global_style().visuals.panel_fill, light.base);
        let ocean = Theme { dark: "ocean".to_owned(), ..Theme::default() }.tokens(&[], true, None);
        apply(&ctx, &ocean);
        assert_eq!(ctx.global_style().visuals.panel_fill, ocean.base);
        assert_ne!(ocean.base, Theme::default().tokens(&[], true, None).base);
    }

    #[test]
    fn palettes_mix_from_one_to_the_other() {
        let (dark, light) = (Theme::default().tokens(&[], true, None), Theme::default().tokens(&[], false, None));
        assert_eq!(dark.mix(&light, 0.0), dark);
        assert_eq!(dark.mix(&light, 1.0), light);
        let halfway = dark.mix(&light, 0.5);
        assert!(!halfway.dark);
        assert_ne!(halfway.base, dark.base);
        assert_ne!(halfway.base, light.base);
    }

    #[test]
    fn a_choice_is_stored_and_read_back() {
        for (light, dark) in FAMILIES.iter().zip(FAMILIES.iter().rev()) {
            for (mode, system_accent) in [(Mode::Dark, false), (Mode::Light, true), (Mode::System, false)] {
                let theme = Theme { mode, light: light.id.to_owned(), dark: dark.id.to_owned(), system_accent };
                assert_eq!(Theme::parse(&theme.stored()), theme);
            }
        }
        // What an earlier version stored, and what nothing stored.
        assert_eq!(Theme::parse("light"), Theme { mode: Mode::Light, ..Theme::default() });
        assert_eq!(
            Theme::parse("ocean dark own-accent"),
            Theme { light: "ocean".to_owned(), dark: "ocean".to_owned(), system_accent: false, ..Theme::default() }
        );
        assert_eq!(Theme::parse("no-such-theme"), Theme::default());
    }

    #[test]
    fn the_system_mode_follows_the_system_and_each_mode_has_its_own_family() {
        let theme = Theme { mode: Mode::System, light: "sunset".to_owned(), dark: "midnight".to_owned(), ..Theme::default() };
        assert!(!theme.is_dark(Some(egui::Theme::Light)));
        assert!(theme.is_dark(Some(egui::Theme::Dark)));
        assert!(theme.is_dark(None));
        assert_eq!(theme.family(&[], false).id, "sunset");
        assert_eq!(theme.family(&[], true).id, "midnight");
        assert!(!Theme { mode: Mode::Light, ..theme.clone() }.is_dark(Some(egui::Theme::Dark)));
    }

    #[test]
    fn every_palette_is_in_its_own_mode_and_readable() {
        for family in &FAMILIES {
            assert!(family.dark.dark && !family.light.dark, "{}", family.id);
            for tokens in [&family.light, &family.dark] {
                assert!(contrast(tokens.foreground, tokens.base) >= 7.0, "{} foreground", family.id);
                assert!(contrast(tokens.muted, tokens.base) >= 3.9, "{} muted", family.id);
                assert!(contrast(tokens.control_text, tokens.control) >= 4.5, "{} control", family.id);
                assert!(contrast(tokens.control_text, tokens.control_hover) >= 4.5, "{} control under the pointer", family.id);
                assert!(contrast(tokens.control_text, tokens.field) >= 4.5, "{} field", family.id);
            }
        }
    }

    #[test]
    fn overcast_is_between_a_light_and_a_dark_window_and_readable_in_both() {
        let overcast = Theme { light: "overcast".to_owned(), dark: "overcast".to_owned(), system_accent: false, ..Theme::default() };
        let default = Theme { system_accent: false, ..Theme::default() };
        assert!(luminance(overcast.tokens(&[], false, None).base) < luminance(default.tokens(&[], false, None).base) * 0.5);
        assert!(luminance(overcast.tokens(&[], true, None).base) > luminance(default.tokens(&[], true, None).base) * 3.0);
        for dark in [false, true] {
            let t = overcast.tokens(&[], dark, None);
            for background in [t.base, t.surface, t.surface_raised, t.well, t.overlay, t.primary_subtle, t.warning_subtle, t.danger_subtle] {
                assert!(contrast(t.foreground, background) >= 4.5, "dark {dark}: foreground on {background:?}");
            }
            for text in [t.primary_text, t.success, t.warning, t.danger] {
                assert!(contrast(text, t.base) >= 4.5, "dark {dark}: {text:?} on the base");
            }
            assert!(contrast(t.inverted, t.primary) >= 4.5, "dark {dark}: on the primary");
            // A system's accent keeps its soft fill as dim as the window, not a bright pastel on grey.
            let subtle = overcast.tokens(&[], dark, Some(Color32::from_rgb(230, 60, 120))).primary_subtle;
            assert!(contrast(subtle, t.base) < 1.5, "dark {dark}: {subtle:?}");
            // On a mid grey, black is the way to go for text, though the grey is under half as bright as white.
            let accented = overcast.tokens(&[], dark, Some(Color32::from_rgb(111, 86, 168)));
            assert!(contrast(accented.primary_text, accented.base) >= 4.5, "dark {dark}: {:?}", accented.primary_text);
        }
    }

    #[test]
    fn the_system_accent_is_used_only_when_chosen_and_stays_readable() {
        let navy = Color32::from_rgb(10, 20, 90);
        let plain = Theme { system_accent: false, ..Theme::default() };
        assert_eq!(plain.tokens(&[], true, Some(navy)), plain.tokens(&[], true, None));
        let chosen = Theme::default();
        // A system without an accent colour leaves the theme's own.
        assert_eq!(chosen.tokens(&[], true, None), plain.tokens(&[], true, None));
        for dark in [true, false] {
            for accent in [navy, Color32::from_rgb(255, 235, 60)] {
                let tokens = chosen.tokens(&[], dark, Some(accent));
                assert_eq!(tokens.primary, accent);
                assert!(contrast(tokens.primary_text, tokens.base) >= 4.5);
                assert!(contrast(tokens.inverted, tokens.primary) >= 4.5);
            }
        }
    }

    fn custom(id: &str, name: &str) -> CustomTheme {
        let mut theme = parse_theme_file(&format!(r#"{{"name":"{name}","lightTokens":{{"base":"1 2 3"}}}}"#)).unwrap();
        theme.id = id.to_owned();
        theme
    }

    #[test]
    fn every_colour_of_every_family_survives_a_theme_file() {
        assert_eq!(TOKEN_GROUPS.iter().map(|(_, group)| group.len()).sum::<usize>(), 25);
        for theme in all_themes(&[]) {
            let read = parse_theme_file(&theme_file(theme).unwrap()).unwrap();
            assert_eq!(read.name, theme.name);
            assert_eq!(&read.light, theme.light, "{} light", theme.id);
            assert_eq!(&read.dark, theme.dark, "{} dark", theme.id);
        }
    }

    #[test]
    fn a_token_reaches_a_colour_of_its_own() {
        for token in TOKEN_GROUPS.iter().flat_map(|(_, group)| group.iter()) {
            let mut changed = default_family().dark.clone();
            *(token.color)(&mut changed) = rgb(1, 2, 3);
            assert_ne!(changed, default_family().dark, "{}", token.key);
            let others = TOKEN_GROUPS.iter().flat_map(|(_, group)| group.iter()).filter(|other| other.key != token.key);
            for other in others {
                assert_ne!(*(other.color)(&mut changed), rgb(1, 2, 3), "{} and {}", token.key, other.key);
            }
        }
    }

    #[test]
    fn a_theme_file_that_cannot_be_read_says_why() {
        let error = |text: &str| parse_theme_file(text).unwrap_err();
        assert!(error("not json").starts_with("This is not a theme file:"));
        assert!(error("[]").starts_with("This is not a theme file:"));
        assert!(error(r#"{"name":7,"lightTokens":{"base":"1 2 3"}}"#).starts_with("This is not a theme file:"));
        assert!(error(r#"{"name":"x","lightTokens":[]}"#).starts_with("This is not a theme file:"));
        assert_eq!(error(r#"{"lightTokens":{"base":"1 2 3"}}"#), "This theme has no name.");
        assert_eq!(error(r#"{"name":"  ","lightTokens":{"base":"1 2 3"}}"#), "This theme has no name.");
        assert_eq!(error(r#"{"name":"x"}"#), "This theme has no colours.");
        assert_eq!(error(r#"{"name":"x","lightTokens":{},"darkTokens":{"unknown":"1 2 3"}}"#), "This theme has no colours.");
        for bad in [r#""red""#, "7", r##""#ff0000""##, r#""300 0 0""#, r#""1 2""#, r#""1 2 3 4""#] {
            let file = format!(r#"{{"name":"x","darkTokens":{{"primary":{bad}}}}}"#);
            assert_eq!(error(&file), "primary in darkTokens is not three numbers from 0 to 255.", "{bad}");
        }
    }

    #[test]
    fn a_theme_file_may_leave_things_out_and_hold_what_is_not_known_here() {
        let file = r#"{"id":"custom-9","builtIn":false,"name":"  A   name ","createdAt":"x","lightTokens":{"base":"1 2 3","generic1Bg":"not read"}}"#;
        let theme = parse_theme_file(file).unwrap();
        assert_eq!((theme.id.as_str(), theme.name.as_str()), ("", "A name"));
        assert_eq!(theme.light.base, rgb(1, 2, 3));
        assert_eq!(theme.light.surface, default_family().light.surface);
        assert_eq!(theme.dark, default_family().dark);
        assert!(!theme.light.dark && theme.dark.dark);
    }

    #[test]
    fn a_damaged_stored_theme_costs_only_itself() {
        let themes = [custom("custom-1", "One"), custom("custom-4", "Four")];
        let stored = stored_themes(&themes).unwrap();
        assert_eq!(parse_stored_themes(&stored), themes);
        let mut entries: Vec<serde_json::Value> = serde_json::from_str(&stored).unwrap();
        entries.insert(1, serde_json::json!({"id": "custom-2", "lightTokens": {"base": "1 2 3"}}));
        entries.push(serde_json::json!(7));
        // An id that could be mistaken for another's, or break the stored choice, is replaced.
        entries.push(serde_json::json!({"id": "custom-1", "name": "Again", "lightTokens": {"base": "1 2 3"}}));
        entries.push(serde_json::json!({"id": "two words", "name": "Spaced", "lightTokens": {"base": "1 2 3"}}));
        let read = parse_stored_themes(&serde_json::to_string(&entries).unwrap());
        let ids: Vec<_> = read.iter().map(|theme| (theme.id.as_str(), theme.name.as_str())).collect();
        assert_eq!(ids, [("custom-1", "One"), ("custom-4", "Four"), ("custom-2", "Again"), ("custom-3", "Spaced")]);
        assert_eq!(parse_stored_themes("not json"), []);
    }

    #[test]
    fn a_custom_theme_is_chosen_like_a_family_until_it_is_gone() {
        let themes = [custom("custom-1", "Mine")];
        let theme = Theme { light: "custom-1".to_owned(), dark: "ocean".to_owned(), ..Theme::default() };
        assert_eq!(Theme::parse(&theme.stored()), theme);
        assert_eq!(theme.family(&themes, false).name, "Mine");
        assert!(!theme.family(&themes, false).built_in);
        assert_eq!(theme.tokens(&themes, false, None).base, rgb(1, 2, 3));
        // A choice that names no theme is the default's.
        assert_eq!(theme.family(&[], false).id, "default");
        let mut forgetting = theme.clone();
        assert!(!forgetting.forget("custom-2"));
        assert!(forgetting.forget("custom-1"));
        assert_eq!((forgetting.light.as_str(), forgetting.dark.as_str()), ("default", "ocean"));
    }

    #[test]
    fn a_new_theme_gets_an_id_and_a_name_of_its_own() {
        let themes = [custom("custom-1", "Mine"), custom("custom-3", "Mine (2)")];
        assert_eq!(new_theme_id(&themes), "custom-2");
        assert_eq!(distinct_name(&themes, "Other", None), "Other");
        assert_eq!(distinct_name(&themes, "Mine", None), "Mine (3)");
        assert_eq!(distinct_name(&themes, "Ocean", None), "Ocean (2)");
        // Saving a theme under its own name leaves it alone.
        assert_eq!(distinct_name(&themes, "Mine", Some("custom-1")), "Mine");
        assert_eq!(theme_file_name("Mine: a/b (2)"), "bigstar-theme-mine-a-b-2.json");
    }
}
