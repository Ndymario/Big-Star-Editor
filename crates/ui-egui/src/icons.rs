//! The interface's icons: Heroicons, and a few of Feather's, kept as the path data of their SVGs and drawn here at the
//! size asked for, in the colour of the text beside them. Nothing comes from a font, so the browser build shows the
//! same icons as the desktop one.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use egui::{Color32, Rect, Response, Sense, TextureHandle, Ui, Vec2};
use vello_cpu::{
    color::palette::css::WHITE,
    kurbo::{Affine, BezPath, Cap, Join, Stroke},
    peniko::Fill,
};

/// The size of an icon beside a line of text.
pub const SIZE: f32 = 16.0;
/// Beside small text.
pub const SIZE_SM: f32 = 14.0;

/// One shape of an icon, as the path data of the SVG it comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Part {
    /// A line of the icon's stroke width, with round ends and corners.
    Stroke(&'static str),
    Fill(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Icon {
    name: &'static str,
    /// The side of the square the paths are drawn in.
    view: f32,
    parts: &'static [Part],
    stroke: f32,
}

/// The drawn icons, by name, stroke width and size in pixels.
type Cache = Arc<Mutex<HashMap<(&'static str, u32, u16), TextureHandle>>>;

impl Icon {
    const fn new(name: &'static str, view: f32, parts: &'static [Part]) -> Self {
        Icon { name, view, parts, stroke: 2.0 }
    }

    /// With thinner lines, for an icon shown large.
    pub const fn thin(mut self) -> Self {
        self.stroke = 1.5;
        self
    }

    /// With thicker lines, for a tick shown small.
    pub const fn bold(mut self) -> Self {
        self.stroke = 3.0;
        self
    }

    /// Draw the icon in `rect`, in `color`.
    pub fn paint(self, painter: &egui::Painter, rect: Rect, color: Color32) {
        let side = rect.width().min(rect.height());
        let rect = Rect::from_center_size(rect.center(), Vec2::splat(side));
        let texture = self.texture(painter.ctx(), side);
        let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
        painter.image(texture.id(), rect, uv, color);
    }

    /// The icon as an image `size` points wide, drawn white: tint it, or put
    /// it in a button that does.
    pub fn image(self, ctx: &egui::Context, size: f32) -> egui::Image<'static> {
        let texture = self.texture(ctx, size);
        egui::Image::new(egui::load::SizedTexture::new(texture.id(), Vec2::splat(size)))
    }

    /// The icon as a widget of its own.
    pub fn show(self, ui: &mut Ui, size: f32, color: Color32) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        if ui.is_rect_visible(rect) {
            self.paint(ui.painter(), rect, color);
        }
        response
    }

    fn texture(self, ctx: &egui::Context, size: f32) -> TextureHandle {
        let pixels = (size * ctx.pixels_per_point()).round().clamp(1.0, 1024.0) as u16;
        let key = (self.name, self.stroke.to_bits(), pixels);
        let id = egui::Id::new("bigstar-icons");
        let cache: Cache = ctx.data_mut(|data| data.get_temp_mut_or_default::<Cache>(id).clone());
        let mut cache = cache.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        cache
            .entry(key)
            .or_insert_with(|| ctx.load_texture(format!("icon-{}-{pixels}", self.name), self.draw(pixels), egui::TextureOptions::LINEAR))
            .clone()
    }

    /// The icon in white on nothing, `pixels` wide.
    fn draw(self, pixels: u16) -> egui::ColorImage {
        let mut ctx = vello_cpu::RenderContext::new(pixels, pixels);
        ctx.set_transform(Affine::scale(f64::from(pixels) / f64::from(self.view)));
        ctx.set_stroke(Stroke::new(f64::from(self.stroke)).with_caps(Cap::Round).with_join(Join::Round));
        for part in self.parts {
            let (data, rule, alpha) = match *part {
                Part::Stroke(data) => (data, None, 1.0),
                Part::Fill(data) => (data, Some(Fill::NonZero), 1.0),
            };
            let Ok(path) = BezPath::from_svg(data) else {
                log::error!("the icon {} has path data that cannot be read", self.name);
                continue;
            };
            ctx.set_paint(WHITE.with_alpha(alpha));
            match rule {
                Some(rule) => {
                    ctx.set_fill_rule(rule);
                    ctx.fill_path(&path);
                }
                None => ctx.stroke_path(&path),
            }
        }
        let mut drawn = vello_cpu::Pixmap::new(pixels, pixels);
        ctx.render(&mut drawn, &mut vello_cpu::Resources::new());
        let side = usize::from(pixels);
        egui::ColorImage::from_rgba_premultiplied([side, side], drawn.data_as_u8_slice())
    }
}

pub const ARCHIVE: Icon =
    Icon::new("archive", 24.0, &[Part::Stroke("M5 8h14M5 8a2 2 0 110-4h14a2 2 0 110 4M5 8v10a2 2 0 002 2h10a2 2 0 002-2V8m-9 4h4")]);

pub const ARROW_UP: Icon = Icon::new("arrow_up", 24.0, &[Part::Stroke("M5 10l7-7m0 0l7 7m-7-7v18")]);

pub const CHECK: Icon = Icon::new("check", 24.0, &[Part::Stroke("M5 13l4 4L19 7")]);

pub const CHEVRON_DOWN: Icon = Icon::new("chevron_down", 24.0, &[Part::Stroke("M19 9l-7 7-7-7")]);

pub const CHEVRON_RIGHT: Icon = Icon::new("chevron_right", 24.0, &[Part::Stroke("M9 5l7 7-7 7")]);

pub const CLOSE: Icon = Icon::new("close", 24.0, &[Part::Stroke("M6 18L18 6M6 6l12 12")]);

pub const DOCUMENT: Icon = Icon::new(
    "document",
    24.0,
    &[Part::Stroke("M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z")],
);

pub const FILE: Icon = Icon::new(
    "file",
    24.0,
    &[Part::Stroke("M7 21h10a2 2 0 002-2V9.414a1 1 0 00-.293-.707l-5.414-5.414A1 1 0 0012.586 3H7a2 2 0 00-2 2v14a2 2 0 002 2z")],
);

pub const FOLDER: Icon = Icon::new("folder", 24.0, &[Part::Stroke("M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z")]);

pub const IMAGE: Icon = Icon::new(
    "image",
    24.0,
    &[Part::Stroke(
        "M4 16l4.586-4.586a2 2 0 012.828 0L16 16m-2-2l1.586-1.586a2 2 0 012.828 0L20 14m-6-6h.01M6 20h12a2 2 0 002-2V6a2 2 0 00-2-2H6a2 2 0 00-2 2v12a2 2 0 002 2z",
    )],
);

pub const INFO: Icon = Icon::new(
    "info",
    24.0,
    &[Part::Stroke("M2 12a10 10 0 1 0 20 0a10 10 0 1 0 -20 0Z"), Part::Stroke("M12 16L12 12"), Part::Stroke("M12 8L12.01 8")],
);

pub const PLAY: Icon =
    Icon::new("play", 20.0, &[Part::Fill("M6.3 2.841A1.5 1.5 0 004 4.11V15.89a1.5 1.5 0 002.3 1.269l9.344-5.89a1.5 1.5 0 000-2.538L6.3 2.84z")]);

pub const PLUS: Icon = Icon::new("plus", 24.0, &[Part::Stroke("M12 4v16m8-8H4")]);

pub const REDO: Icon = Icon::new("redo", 24.0, &[Part::Stroke("M21 10H11a8 8 0 00-8 8v2M21 10l-6 6m6-6l-6-6")]);

pub const REFRESH: Icon = Icon::new(
    "refresh",
    24.0,
    &[Part::Stroke("M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15")],
);

pub const SAVE: Icon =
    Icon::new("save", 24.0, &[Part::Stroke("M8 7H5a2 2 0 00-2 2v9a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-3m-1 4l-3 3m0 0l-3-3m3 3V4")]);

pub const SEARCH: Icon = Icon::new("search", 24.0, &[Part::Stroke("M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z")]);

pub const UNDO: Icon = Icon::new("undo", 24.0, &[Part::Stroke("M3 10h10a8 8 0 018 8v2M3 10l6 6m-6-6l6-6")]);

// The graphics editors' tools.

pub const ZOOM_IN: Icon = Icon::new("zoom_in", 24.0, &[Part::Stroke("M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0zM10 7v6M7 10h6")]);

pub const ZOOM_OUT: Icon = Icon::new("zoom_out", 24.0, &[Part::Stroke("M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0zM7 10h6")]);

pub const PENCIL: Icon =
    Icon::new("pencil", 24.0, &[Part::Stroke("M15.232 5.232l3.536 3.536m-2.036-5.036a2.5 2.5 0 113.536 3.536L6.5 21.036H3v-3.572L16.732 3.732z")]);

pub const ERASER: Icon = Icon::new("eraser", 24.0, &[Part::Stroke("M8 20h12M8 20l-4-4L14 6l6 6-8 8M9 11l6 6")]);

pub const EYEDROPPER: Icon = Icon::new("eyedropper", 24.0, &[Part::Stroke("M14 4l6 6M12 6l6 6M15 9L6 18l-3 1 1-3 9-9")]);

pub const BUCKET: Icon = Icon::new("bucket", 24.0, &[Part::Stroke("M4 12l7-7 7 7-7 7zM4 12h14M20 15l1.5 3a1.5 1.5 0 01-3 0z")]);

pub const LINE: Icon = Icon::new("line", 24.0, &[Part::Stroke("M5 19L19 5")]);

pub const RECTANGLE: Icon = Icon::new("rectangle", 24.0, &[Part::Stroke("M5 5h14v14H5z")]);

pub const SELECT: Icon = Icon::new("select", 24.0, &[Part::Stroke("M4 7V4h3M10 4h4M17 4h3v3M20 10v4M20 17v3h-3M14 20h-4M7 20H4v-3M4 14v-4")]);

pub const FLIP_HORIZONTAL: Icon = Icon::new("flip_horizontal", 24.0, &[Part::Stroke("M12 3v18M9 7l-5 5 5 5zM15 7l5 5-5 5z")]);

pub const FLIP_VERTICAL: Icon = Icon::new("flip_vertical", 24.0, &[Part::Stroke("M3 12h18M7 9l5-5 5 5zM7 15l5 5 5-5z")]);

pub const STAMP: Icon = Icon::new("stamp", 24.0, &[Part::Stroke("M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z")]);

#[cfg(test)]
const ALL: &[Icon] = &[
    ARCHIVE,
    DOCUMENT,
    IMAGE,
    ARROW_UP,
    CHECK,
    CHEVRON_DOWN,
    CHEVRON_RIGHT,
    CLOSE,
    FILE,
    FOLDER,
    INFO,
    PLAY,
    PLUS,
    REDO,
    REFRESH,
    SAVE,
    SEARCH,
    UNDO,
    ZOOM_IN,
    ZOOM_OUT,
    PENCIL,
    ERASER,
    EYEDROPPER,
    BUCKET,
    LINE,
    RECTANGLE,
    SELECT,
    FLIP_HORIZONTAL,
    FLIP_VERTICAL,
    STAMP,
];

#[cfg(test)]
mod tests {
    use super::{ALL, Part};
    use vello_cpu::kurbo::BezPath;

    #[test]
    fn every_mark_set_in_text_is_in_the_fonts_every_build_has() {
        let ctx = egui::Context::default();
        ctx.set_fonts(crate::theme::fonts());
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        let font = egui::FontId::proportional(14.0);
        let missing: Vec<&str> = ["…", "·", "•", "×", "—"].into_iter().filter(|mark| !ctx.fonts_mut(|fonts| fonts.has_glyphs(&font, mark))).collect();
        assert!(missing.is_empty(), "not in the fonts every build has: {missing:?}");
    }

    #[test]
    fn every_icon_has_a_name_of_its_own_and_draws_something() {
        let mut names = std::collections::HashSet::new();
        for icon in ALL {
            assert!(names.insert(icon.name), "two icons are named {}", icon.name);
            for part in icon.parts {
                let (Part::Stroke(data) | Part::Fill(data)) = part;
                assert!(BezPath::from_svg(data).is_ok(), "{} has path data that cannot be read", icon.name);
            }
            let image = icon.draw(32);
            assert!(image.pixels.iter().any(|pixel| pixel.a() > 0), "{} draws nothing", icon.name);
        }
    }
}
