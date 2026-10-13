//! Movement: the short transitions a control asks for each frame, as values from 0 to 1 or a place being approached.

use egui::{Color32, Context, Id, emath::easing};

/// A colour change under the pointer.
pub const COLOR: f32 = 0.15;
/// An indicator sliding to another tab.
pub const SLOW: f32 = 0.3;

/// From 0 to 1 while `on` holds and back when it does not, over `time` seconds: how far a hover or a tick has got.
///
/// What was last drawn is forgotten once a frame passes without the widget: coming back to a screen, a row that was
/// under the pointer when it was left starts at rest instead of fading out from hovered.
pub fn toward(ctx: &Context, id: Id, on: bool, time: f32) -> f32 {
    let seen = id.with("seen");
    let frame = ctx.cumulative_pass_nr();
    let (last, round) = ctx.data(|data| data.get_temp::<(u64, u32)>(seen)).unwrap_or((frame, 0));
    // A new id is a fresh animation, at rest where `on` says.
    let round = if frame.saturating_sub(last) > 2 { round + 1 } else { round };
    ctx.data_mut(|data| data.insert_temp(seen, (frame, round)));
    ctx.animate_bool_with_time_and_easing(id.with(round), on, time, easing::cubic_in_out)
}

/// [`toward`] for the pointer resting on what `response` belongs to.
pub fn hover(ui: &egui::Ui, response: &egui::Response, time: f32) -> f32 {
    toward(ui.ctx(), response.id.with("hover"), response.hovered(), time)
}

#[derive(Clone, Copy)]
struct Slide {
    from: f32,
    to: f32,
    start: f64,
    /// The pass it was last asked for in.
    pass: u64,
}

/// `target`, approached over `time` seconds whenever it changes (ease-out): where a sliding indicator is. The first
/// value is taken at once.
pub fn slide(ctx: &Context, id: Id, target: f32, time: f32) -> f32 {
    let now = ctx.input(|i| i.time);
    let pass = ctx.cumulative_pass_nr();
    let fresh = Slide { from: target, to: target, start: now - f64::from(time), pass };
    // Not asked for in a while (its screen was left): rest at `target` rather than finish a movement nobody saw.
    let mut slide = ctx.data(|data| data.get_temp::<Slide>(id)).filter(|slide| pass.saturating_sub(slide.pass) <= 2).unwrap_or(fresh);
    slide.pass = pass;
    let at = |slide: &Slide| {
        let t = if time <= 0.0 { 1.0 } else { (((now - slide.start) as f32) / time).clamp(0.0, 1.0) };
        (slide.from + (slide.to - slide.from) * easing::cubic_out(t), t)
    };
    if slide.to != target {
        slide = Slide { from: at(&slide).0, to: target, start: now, pass };
    }
    let (value, t) = at(&slide);
    ctx.data_mut(|data| data.insert_temp(id, slide));
    if t < 1.0 {
        ctx.request_repaint();
    }
    value
}

/// The colour `t` of the way from `a` to `b`.
pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    if t <= 0.0 {
        a
    } else if t >= 1.0 {
        b
    } else {
        a.lerp_to_gamma(b, t)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_mix_from_one_to_the_other() {
        let (a, b) = (Color32::from_rgb(0, 0, 0), Color32::from_rgb(200, 100, 50));
        assert_eq!(mix(a, b, 0.0), a);
        assert_eq!(mix(a, b, 1.0), b);
        assert_eq!(mix(a, b, 0.5), Color32::from_rgb(100, 50, 25));
    }

    #[test]
    fn a_slide_starts_where_it_is_and_ends_at_its_target() {
        let ctx = Context::default();
        let id = Id::new("slide");
        let frame = |time: f64, target: f32| {
            let mut value = 0.0;
            let input = egui::RawInput { time: Some(time), ..Default::default() };
            let mut output = ctx.run_ui(input, |ui| {
                value = slide(ui.ctx(), id, target, 0.3);
            });
            output.textures_delta.clear();
            value
        };
        // Nothing to come from: the first target is where it is.
        assert_eq!(frame(0.0, 10.0), 10.0);
        assert_eq!(frame(1.0, 50.0), 10.0);
        let midway = frame(1.15, 50.0);
        assert!(midway > 10.0 && midway < 50.0, "{midway}");
        assert_eq!(frame(1.4, 50.0), 50.0);
    }
}
