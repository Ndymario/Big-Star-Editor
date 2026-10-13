//! Headless screenshots: renders the frontend offscreen (wgpu), against the built-in demo host, so it works with a locked
//! screen and without a ROM.
//!
//! `cargo run -p bigstar-ui-egui --example ui_shot -- <out-dir>`
//!
//! Writes `connect.png`, `projects.png`, `levels.png` and `level.png` (the demo level, a sprite selected) into the directory.

use bigstar_ui_egui::demo::DemoConnection;
use bigstar_ui_egui::{BigStarApp, Item, Kind, MemoryServices};
use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

fn shot(harness: &mut Harness<'_, BigStarApp>, dir: &std::path::Path, name: &str) -> Result<(), String> {
    harness.run();
    let image = harness.render()?;
    let path = dir.join(name);
    image.save(&path).map_err(|e| e.to_string())?;
    println!("{}", path.display());
    Ok(())
}

fn build(app: BigStarApp) -> Harness<'static, BigStarApp> {
    Harness::builder()
        .with_size(Vec2::new(1400.0, 800.0))
        .with_pixels_per_point(1.5)
        .with_max_steps(16)
        .wgpu()
        .build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app)
}

/// `ui_shot <out-dir> <host> <token>`: connects to a real host instead, opens its first project and writes `remote-levels.png`, `remote.png` (A01_1 fitted) and `remote-100.png` (at 100%).
fn remote(dir: &std::path::Path, host: &str, token: &str) -> Result<(), String> {
    let mut harness = build(BigStarApp::new(Box::new(MemoryServices::default())));
    harness.run();
    let ctx = harness.ctx.clone();
    harness.state_mut().connect_to_host(&ctx, host, token);
    let until = |harness: &mut Harness<'_, BigStarApp>, done: &dyn Fn(&BigStarApp) -> bool| {
        for _ in 0..300 {
            if done(harness.state()) {
                return true;
            }
            harness.step();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    };
    if !until(&mut harness, &|app| app.session().is_some_and(|s| s.connected() && !s.projects.is_empty())) {
        shot(&mut harness, dir, "remote.png")?;
        return Err("no projects arrived".to_owned());
    }
    let name = harness.state().session().and_then(|s| s.projects.first().map(|p| p.name.clone())).unwrap_or_default();
    println!("connected; first project: {name}");
    if let Some(button) = harness.query_all_by_label(&name).next() {
        button.click();
    }
    until(&mut harness, &|app| app.session().is_some_and(|s| !s.worlds.is_empty()));
    shot(&mut harness, dir, "remote-levels.png")?;
    if let Some(session) = harness.state_mut().session_mut() {
        session.open_level("A01_1");
    }
    // The level, then its graphics, draw lists and textures, each a round trip.
    let drawn = |app: &BigStarApp| {
        app.session().and_then(|s| s.level.as_ref().map(|v| (s, v))).is_some_and(|(s, v)| {
            v.art.graphics.is_some() && v.art.requested.is_empty() && v.art.texture_ids().iter().all(|id| s.textures.contains_key(id))
        })
    };
    until(&mut harness, &drawn);
    for _ in 0..10 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    shot(&mut harness, dir, "remote.png")?;
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level.as_mut()) {
        view.zoom_about(1.0, view.canvas.min);
        view.pan = egui::Vec2::new(0.0, 160.0);
        view.selection = vec![Item::new(Kind::Sprite, 0), Item::new(Kind::Object, 3)];
    }
    shot(&mut harness, dir, "remote-100.png")
}

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let dir = std::path::PathBuf::from(args.next().ok_or("usage: ui_shot <out-dir> [<host> <token>]")?);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if let (Some(host), Some(token)) = (args.next(), args.next()) {
        return remote(&dir, &host, &token);
    }

    let mut connect = build(BigStarApp::new(Box::new(MemoryServices::default())));
    shot(&mut connect, &dir, "connect.png")?;

    let app = BigStarApp::with_connection(Box::new(MemoryServices::default()), Box::new(DemoConnection::new()));
    let mut harness = build(app);
    shot(&mut harness, &dir, "projects.png")?;
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    shot(&mut harness, &dir, "levels.png")?;
    harness.get_by_label("Area 1").click();
    harness.run();
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level.as_mut()) {
        view.selection.push(Item::new(Kind::Sprite, 1));
        view.selection.push(Item::new(Kind::Object, 4));
    }
    shot(&mut harness, &dir, "level.png")?;
    // One actor selected: the properties show its own fields, and the picker its category.
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level.as_mut()) {
        view.selection = vec![Item::new(Kind::Sprite, 0)];
    }
    harness.run();
    if let Some(node) = harness.query_all_by_label("Sprites").next() {
        node.click();
    }
    harness.run();
    if let Some(node) = harness.query_by_label("Enemies (2)") {
        node.click();
    }
    shot(&mut harness, &dir, "properties.png")?;
    if let Some(node) = harness.query_by_label("Level Data") {
        node.click();
    }
    shot(&mut harness, &dir, "level-data.png")
}
