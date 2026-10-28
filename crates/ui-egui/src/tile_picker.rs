//! The sheet docked beside a tilemap or an object: drag over it to choose a rectangle of tiles (or Map16 blocks),
//! which is what the map is then painted with. As NSMB Editor's tile picker, with its own zoom.

use bigstar_formats::RgbaImage;
use egui::{Color32, Ui};

use crate::paint::{Pattern, Point};
use crate::zoom_canvas::{self, CanvasState, Overlay, Steps};

pub struct PickerState {
    pub canvas: CanvasState,
    /// The cell the selection was started on and the one it was dragged to.
    anchor: Point,
    corner: Point,
    texture: Option<(u64, egui::TextureHandle)>,
}

impl Default for PickerState {
    fn default() -> Self {
        Self { canvas: CanvasState::fitting_width(), anchor: (0, 0), corner: (0, 0), texture: None }
    }
}

impl PickerState {
    /// The selection's left, top, width and height, in cells.
    pub fn selection(&self) -> (usize, usize, usize, usize) {
        let (left, top) = (self.anchor.0.min(self.corner.0).max(0) as usize, self.anchor.1.min(self.corner.1).max(0) as usize);
        let (right, bottom) = (self.anchor.0.max(self.corner.0).max(0) as usize, self.anchor.1.max(self.corner.1).max(0) as usize);
        (left, top, right - left + 1, bottom - top + 1)
    }

    /// Selects the one cell `index` is, `columns` across.
    pub fn select(&mut self, index: usize, columns: usize) {
        let columns = columns.max(1);
        self.anchor = ((index % columns) as i64, (index / columns) as i64);
        self.corner = self.anchor;
    }

    /// The first cell selected.
    pub fn first(&self, columns: usize) -> usize {
        let (left, top, _, _) = self.selection();
        top * columns.max(1) + left
    }

    /// The selected cells' indices, `columns` across; a cell past the last of `count` is `None`.
    pub fn indices(&self, columns: usize, count: usize) -> Pattern<Option<usize>> {
        let (left, top, width, height) = self.selection();
        let mut cells = Vec::with_capacity(width * height);
        for y in top..top + height {
            for x in left..left + width {
                let index = y * columns.max(1) + x;
                cells.push((x < columns && index < count).then_some(index));
            }
        }
        Pattern { width, height, cells }
    }
}

/// What the person did with the picker this frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Picked {
    /// The selection moved.
    pub changed: bool,
    /// A selection was finished: the button came up.
    pub finished: bool,
}

/// Draws the picker over `image`, a sheet of `cell`-pixel cells `columns` across.
#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut Ui,
    name: &str,
    image: &RgbaImage,
    version: u64,
    state: &mut PickerState,
    cell: usize,
    columns: usize,
    max_height: f32,
) -> Picked {
    let (left, top, width, height) = state.selection();
    let selection =
        Overlay::cells((left as i64, top as i64), ((left + width - 1) as i64, (top + height - 1) as i64), cell, Color32::from_rgb(255, 200, 40))
            .filled(Color32::from_rgba_unmultiplied(255, 200, 40, 40));
    let steps = Steps { fine: 0, coarse: cell };
    ui.horizontal(|ui| zoom_canvas::zoom_controls(ui, &mut state.canvas, steps));
    let input = zoom_canvas::show(ui, name, image, version, &mut state.texture, &mut state.canvas, steps, &[selection], max_height);
    let mut picked = Picked { changed: false, finished: input.released().is_some() };
    let Some((x, y)) = input.at else { return picked };
    let rows = image.height / cell.max(1);
    let at = (x.div_euclid(cell as i64).clamp(0, columns.max(1) as i64 - 1), y.div_euclid(cell as i64).clamp(0, rows.max(1) as i64 - 1));
    if input.pressed().is_some() {
        state.anchor = at;
        state.corner = at;
        picked.changed = true;
    } else if input.down().is_some() && state.corner != at {
        state.corner = at;
        picked.changed = true;
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_reads_as_indices_in_either_drag_direction() {
        let mut state = PickerState { anchor: (2, 1), corner: (1, 0), ..PickerState::default() };
        assert_eq!(state.selection(), (1, 0, 2, 2));
        let picked = state.indices(4, 6);
        assert_eq!(picked.cells, [Some(1), Some(2), Some(5), None], "index 6 is past the last");
        state.select(9, 4);
        assert_eq!((state.selection(), state.first(4)), ((1, 2, 1, 1), 9));
    }
}
