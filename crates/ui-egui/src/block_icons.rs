//! The small pictures the canvas puts on a question block or a brick to say what is inside it. Drawn from shapes, so
//! they need no art from the ROM and stay sharp at any zoom.

use std::f32::consts::{PI, TAU};

use bigstar_formats::BlockContents;
use egui::{Color32, Painter, Pos2, Rect, Shape, Stroke, Vec2};

const OUTLINE: Color32 = Color32::from_rgb(28, 22, 18);
const RED: Color32 = Color32::from_rgb(222, 46, 32);
const GREEN: Color32 = Color32::from_rgb(46, 172, 62);
const BLUE: Color32 = Color32::from_rgb(52, 110, 226);
const ORANGE: Color32 = Color32::from_rgb(244, 126, 26);
const GOLD: Color32 = Color32::from_rgb(250, 200, 40);
const DARK_GOLD: Color32 = Color32::from_rgb(178, 118, 16);
const CREAM: Color32 = Color32::from_rgb(250, 230, 186);
const WHITE: Color32 = Color32::from_rgb(250, 250, 250);

/// Where a block's icon goes: its lower-right corner, so the block itself still shows what kind it is.
pub fn icon_rect(block: Rect) -> Rect {
    let side = block.width() * 0.62;
    Rect::from_min_size(block.max - Vec2::splat(side + block.width() * 0.02), Vec2::splat(side))
}

/// Draws `contents` filling `rect`, which is square.
pub fn paint(painter: &Painter, rect: Rect, contents: BlockContents) {
    let line = Stroke::new((rect.width() / 14.0).max(0.6), OUTLINE);
    // Points given in the icon's own unit square.
    let at = |x: f32, y: f32| rect.min + rect.size() * Vec2::new(x, y);
    let scale = rect.width();
    match contents {
        BlockContents::PowerUp | BlockContents::Mushroom => mushroom(painter, rect, RED, line),
        BlockContents::OneUp => mushroom(painter, rect, GREEN, line),
        BlockContents::MiniMushroom => mushroom(painter, Rect::from_center_size(at(0.5, 0.62), rect.size() * 0.68), BLUE, line),
        BlockContents::MegaMushroom => {
            mushroom(painter, rect, ORANGE, line);
            painter.circle_stroke(rect.center(), scale * 0.5, Stroke::new(line.width, GOLD));
        }
        BlockContents::FireFlower => {
            painter.line_segment([at(0.5, 0.55), at(0.5, 0.98)], Stroke::new(scale * 0.14, GREEN));
            painter.add(Shape::ellipse_filled(at(0.5, 0.36), Vec2::new(0.44, 0.32) * scale, ORANGE));
            painter.add(Shape::ellipse_stroke(at(0.5, 0.36), Vec2::new(0.44, 0.32) * scale, line));
            painter.add(Shape::ellipse_filled(at(0.5, 0.36), Vec2::new(0.24, 0.16) * scale, GOLD));
        }
        BlockContents::Star => star(painter, at(0.5, 0.53), scale * 0.5, GOLD, line),
        BlockContents::Coin => coin(painter, at(0.5, 0.5), scale, line),
        BlockContents::Coins => {
            coin(painter, at(0.36, 0.42), scale * 0.8, line);
            coin(painter, at(0.64, 0.58), scale * 0.8, line);
        }
        BlockContents::Vine => {
            painter.line_segment([at(0.5, 0.02), at(0.5, 0.98)], Stroke::new(scale * 0.12, GREEN));
            for (x, y) in [(0.3, 0.32), (0.7, 0.62)] {
                painter.add(Shape::ellipse_filled(at(x, y), Vec2::new(0.22, 0.12) * scale, GREEN));
                painter.add(Shape::ellipse_stroke(at(x, y), Vec2::new(0.22, 0.12) * scale, line));
            }
        }
        BlockContents::Springboard => {
            for y in [0.12, 0.76] {
                let bar = Rect::from_min_max(at(0.1, y), at(0.9, y + 0.14));
                painter.rect(bar, 0.0, RED, line, egui::StrokeKind::Middle);
            }
            let zigzag = vec![at(0.5, 0.26), at(0.25, 0.42), at(0.75, 0.6), at(0.5, 0.76)];
            painter.add(Shape::line(zigzag, Stroke::new(scale * 0.1, Color32::from_gray(150))));
        }
        BlockContents::Boo => {
            painter.circle(rect.center(), scale * 0.45, WHITE, line);
            for x in [0.36, 0.64] {
                painter.add(Shape::ellipse_filled(at(x, 0.42), Vec2::new(0.06, 0.12) * scale, OUTLINE));
            }
        }
        BlockContents::BlueShell => {
            let dome = arc(at(0.5, 0.68), scale * 0.44, scale * 0.5, PI, TAU);
            painter.add(Shape::convex_polygon(dome, BLUE, line));
            let rim = Rect::from_min_max(at(0.04, 0.66), at(0.96, 0.84));
            painter.rect(rim, scale * 0.08, CREAM, line, egui::StrokeKind::Middle);
        }
    }
}

/// Points on an ellipse about `centre`, from angle `from` to `to` (radians, clockwise on screen since y is down).
fn arc(centre: Pos2, rx: f32, ry: f32, from: f32, to: f32) -> Vec<Pos2> {
    const STEPS: usize = 12;
    (0..=STEPS)
        .map(|step| {
            let angle = from + (to - from) * step as f32 / STEPS as f32;
            centre + Vec2::new(angle.cos() * rx, angle.sin() * ry)
        })
        .collect()
}

fn mushroom(painter: &Painter, rect: Rect, cap: Color32, line: Stroke) {
    let at = |x: f32, y: f32| rect.min + rect.size() * Vec2::new(x, y);
    let scale = rect.width();
    let stem = Rect::from_min_max(at(0.28, 0.5), at(0.72, 0.96));
    painter.rect(stem, scale * 0.1, CREAM, line, egui::StrokeKind::Middle);
    let dome = arc(at(0.5, 0.6), scale * 0.48, scale * 0.52, PI, TAU);
    painter.add(Shape::convex_polygon(dome, cap, line));
    painter.circle_filled(at(0.5, 0.3), scale * 0.12, WHITE);
    painter.circle_filled(at(0.2, 0.47), scale * 0.07, WHITE);
    painter.circle_filled(at(0.8, 0.47), scale * 0.07, WHITE);
}

fn coin(painter: &Painter, centre: Pos2, scale: f32, line: Stroke) {
    let radius = Vec2::new(0.3, 0.44) * scale;
    painter.add(Shape::ellipse_filled(centre, radius, GOLD));
    painter.add(Shape::ellipse_stroke(centre, radius, line));
    painter.line_segment([centre - Vec2::new(0.0, 0.24 * scale), centre + Vec2::new(0.0, 0.24 * scale)], Stroke::new(scale * 0.08, DARK_GOLD));
}

/// A five-pointed star: not convex, so drawn as its middle and a triangle for each point.
fn star(painter: &Painter, centre: Pos2, radius: f32, fill: Color32, line: Stroke) {
    let point = |index: f32, r: f32| {
        let angle = -PI / 2.0 + index * TAU / 10.0;
        centre + Vec2::new(angle.cos(), angle.sin()) * r
    };
    let inner: Vec<Pos2> = (0..5).map(|i| point(i as f32 * 2.0 + 1.0, radius * 0.45)).collect();
    let outline: Vec<Pos2> = (0..10).map(|i| point(i as f32, if i % 2 == 0 { radius } else { radius * 0.45 })).collect();
    painter.add(Shape::convex_polygon(inner.clone(), fill, Stroke::NONE));
    for i in 0..5 {
        let tip = point(i as f32 * 2.0, radius);
        let (left, right) = (point(i as f32 * 2.0 - 1.0, radius * 0.45), point(i as f32 * 2.0 + 1.0, radius * 0.45));
        painter.add(Shape::convex_polygon(vec![left, tip, right], fill, Stroke::NONE));
    }
    painter.add(Shape::closed_line(outline, line));
}
