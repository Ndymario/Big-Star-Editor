//! A small labelled tab hung from the top corner of a rectangle, which picks the rectangle up where its area cannot
//! (a big container would be in the way of every click that misses). Where it sits and how it looks are the tab's
//! own: the owner says which corner, what it says and what colour it is, and gets back a box to draw and to hit-test.

use egui::{Align2, Color32, CornerRadius, FontId, Painter, Pos2, Rect, Vec2};

/// The size of a tab's label, in screen points.
pub const TEXT: f32 = 11.0;

/// The width of one letter of the monospace face, as a fraction of its size.
const LETTER: f32 = 0.6;
const PADDING: Vec2 = Vec2::new(10.0, 6.0);
const ROUNDING: u8 = 3;

/// Which top corner of its rectangle a tab hangs from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CornerTab {
    pub anchor: Anchor,
    pub label: String,
}

impl CornerTab {
    pub fn new(anchor: Anchor, label: impl Into<String>) -> Self {
        Self { anchor, label: label.into() }
    }

    /// The tab's box, given the part of its rectangle that is in view (screen points): at the anchored corner of that
    /// part, so it keeps to the edges of the view for as long as any of the rectangle is in it. `None` when none is.
    pub fn place(&self, shown: Rect) -> Option<Rect> {
        if !shown.is_positive() {
            return None;
        }
        let size = Vec2::new(self.label.chars().count() as f32 * TEXT * LETTER, TEXT) + PADDING;
        let left = match self.anchor {
            Anchor::Left => shown.min.x,
            // A rectangle narrower than its tab lets the tab stick out on the right rather than the left.
            Anchor::Right => (shown.max.x - size.x).max(shown.min.x),
        };
        Some(Rect::from_min_size(Pos2::new(left, shown.min.y), size))
    }

    /// Draws the tab at `at` (from `place`) in `fill`, its far-from-the-corner side rounded.
    pub fn paint(&self, painter: &Painter, at: Rect, fill: Color32) {
        let rounding = match self.anchor {
            Anchor::Left => CornerRadius { se: ROUNDING, ..CornerRadius::ZERO },
            Anchor::Right => CornerRadius { sw: ROUNDING, ..CornerRadius::ZERO },
        };
        painter.rect_filled(at, rounding, fill);
        painter.text(at.center(), Align2::CENTER_CENTER, &self.label, FontId::monospace(TEXT), Color32::WHITE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tab_hangs_from_its_corner_of_what_is_in_view() {
        let shown = Rect::from_min_size(Pos2::new(10.0, 20.0), Vec2::new(200.0, 100.0));
        let left = CornerTab::new(Anchor::Left, "View 0").place(shown).unwrap();
        let right = CornerTab::new(Anchor::Right, "Zone 0").place(shown).unwrap();
        assert_eq!(left.left_top(), shown.left_top());
        assert_eq!(right.right_top(), shown.right_top());
        assert!(!left.intersects(right));
    }

    #[test]
    fn a_longer_label_makes_a_wider_tab_and_nothing_in_view_has_no_tab() {
        let shown = Rect::from_min_size(Pos2::ZERO, Vec2::splat(100.0));
        assert!(
            CornerTab::new(Anchor::Left, "View 10").place(shown).unwrap().width()
                > CornerTab::new(Anchor::Left, "View 1").place(shown).unwrap().width()
        );
        assert_eq!(CornerTab::new(Anchor::Left, "View 0").place(Rect::NOTHING), None);
    }

    #[test]
    fn a_right_tab_wider_than_its_rectangle_keeps_the_rectangles_left_edge() {
        let shown = Rect::from_min_size(Pos2::new(5.0, 5.0), Vec2::new(10.0, 40.0));
        assert_eq!(CornerTab::new(Anchor::Right, "Zone 0").place(shown).unwrap().min.x, 5.0);
    }
}
