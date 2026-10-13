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
    // A level whose backgrounds are animated never stops asking to be painted again, so settling is not required.
    harness.run_ok();
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
        app.session().and_then(|s| s.level().map(|v| (s, v))).is_some_and(|(s, v)| {
            v.art.graphics.is_some() && v.art.requested.is_empty() && v.art.texture_ids().iter().all(|id| s.textures.contains_key(id))
        })
    };
    until(&mut harness, &drawn);
    for _ in 0..10 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    shot(&mut harness, dir, "remote.png")?;
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level_mut()) {
        view.zoom_about(1.0, view.canvas.min);
        view.pan = egui::Vec2::new(0.0, 160.0);
        view.selection = vec![Item::new(Kind::Sprite, 0), Item::new(Kind::Object, 3)];
    }
    shot(&mut harness, dir, "remote-100.png")?;
    // The DS screen: where the first entrance opens the camera, and another under the pointer further along.
    let mut pointer = egui::Pos2::ZERO;
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level_mut()) {
        view.options.show_ds_screen = true;
        view.selection = vec![Item::new(Kind::Entrance, 0)];
        pointer = view.canvas.min + egui::Vec2::new(620.0, 150.0);
    }
    harness.event(egui::Event::PointerMoved(pointer));
    shot(&mut harness, dir, "remote-ds.png")?;
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level_mut()) {
        view.options.show_ds_screen = false;
        view.selection.clear();
    }
    // The Filesystem dock, with an archive opened.
    if let Some(tab) = harness.query_by_label("Filesystem") {
        tab.click();
    }
    until(&mut harness, &|app| app.session().is_some_and(|s| s.filesystem.is_some()));
    for _ in 0..5 {
        harness.step();
    }
    shot(&mut harness, dir, "remote-files.png")?;
    // A real tile sheet in the graphics editor, paired with its palette from the next folder.
    let sheet = "BG_ncg/d_2d_A_J_jyotyu_ncg.bin";
    if let Some(session) = harness.state_mut().session_mut() {
        session.read_file(sheet, bigstar_ui_egui::FileRead::Open);
    }
    until(&mut harness, &|app| app.has_document(sheet));
    harness.state_mut().pair_for_screenshot(sheet, "BG_ncl/d_2d_A_J_jyotyu_ncl.bin");
    for _ in 0..20 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    shot(&mut harness, dir, "remote-graphics.png")?;
    // It is an 8bpp sheet; read as one, sixteen tiles across.
    harness.state_mut().choose_for_screenshot(sheet, 1, 16);
    for _ in 0..3 {
        harness.step();
    }
    shot(&mut harness, dir, "remote-graphics-8bpp.png")?;
    // The message editor, a touch layout, a JNCD's texture and an LZ-wrapped ENPG from the ROM.
    for (path, name) in [
        ("script/course.bmg", "remote-messages.png"),
        ("uiStudio/UI_O_menu_select_d.bnbl", "remote-bnbl.png"),
        ("uiStudio/UI_O_menu_select_d.bncl", "remote-bncl.png"),
        ("uiStudio/UI_O_1P_course_select_d.nce.bncd", "remote-bncd.png"),
        ("demo/staffroll_cover.enpg", "remote-enpg.png"),
    ] {
        if let Some(session) = harness.state_mut().session_mut() {
            session.read_file(path, bigstar_ui_egui::FileRead::Open);
        }
        until(&mut harness, &|app| app.has_document(path));
        if name == "remote-bncd.png"
            && let Some(tab) = harness.query_all_by_label("Texture").last()
        {
            tab.click();
        }
        for _ in 0..3 {
            harness.step();
        }
        shot(&mut harness, dir, name)?;
    }
    // A01_1's tilesets and backgrounds.
    harness.state_mut().open_tilesets_for_screenshot("A01_1");
    for _ in 0..30 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    tileset_tabs(&mut harness, dir, "remote-tilesets")?;
    // Run Patcher on a project without NCPatcher's configuration says why there is nothing to build.
    if let Some(button) = harness.query_by_label("Run Patcher") {
        button.click();
    }
    for _ in 0..20 {
        harness.step();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    shot(&mut harness, dir, "remote-build.png")
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

    let app = BigStarApp::with_connection(Box::new(MemoryServices::with_accent([111, 86, 168])), Box::new(DemoConnection::new()));
    let mut harness = build(app);
    shot(&mut harness, &dir, "projects.png")?;
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    // Picking an area previews it; Edit opens it as a document.
    if let Some(area) = harness.query_all_by_label("Area 1").next() {
        area.click();
    }
    shot(&mut harness, &dir, "levels.png")?;
    if let Some(session) = harness.state_mut().session_mut() {
        session.open_level("A01_2");
    }
    harness.run();
    if let Some(edit) = harness.query_all_by_label("Edit").next() {
        edit.click();
    }
    harness.run();
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level_mut()) {
        view.selection.push(Item::new(Kind::Sprite, 1));
        view.selection.push(Item::new(Kind::Object, 4));
    }
    shot(&mut harness, &dir, "level.png")?;
    // One actor selected: the properties show its own fields, and the picker its category.
    if let Some(view) = harness.state_mut().session_mut().and_then(|s| s.level_mut()) {
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
    shot(&mut harness, &dir, "level-data.png")?;
    // The Filesystem dock, and the same window in the light palette.
    if let Some(tab) = harness.query_by_label("Filesystem") {
        tab.click();
    }
    harness.run();
    shot(&mut harness, &dir, "files.png")?;
    harness.state_mut().set_dark(false);
    shot(&mut harness, &dir, "files-light.png")?;
    harness.state_mut().set_dark(true);
    // Preferences, with its themes, and the window in one of them with a system accent.
    for label in ["View", "Preferences…"] {
        if let Some(node) = harness.query_by_label(label) {
            node.click();
        }
        harness.run();
    }
    shot(&mut harness, &dir, "preferences.png")?;
    // The system's accent is on from the start; here the theme's own is asked for.
    for label in ["Midnight", "Use the system's accent colour"] {
        if let Some(node) = harness.query_by_label(label) {
            node.click();
        }
        harness.run();
    }
    shot(&mut harness, &dir, "preferences-midnight.png")?;
    for label in ["Lavender", "Light"] {
        if let Some(node) = harness.query_by_label(label) {
            node.click();
        }
        harness.run();
    }
    shot(&mut harness, &dir, "preferences-lavender-light.png")?;
    if let Some(node) = harness.query_by_label("Cancel") {
        node.click();
    }
    harness.run();
    // The message editor, the uiStudio and metadata pages and the banner, each a document.
    for (path, name) in [("message/msg.bmg", "messages.png"), ("uiStudio/demo.bnbl", "resource-bnbl.png"), ("NSMBeUD", "metadata.png")] {
        if let Some(session) = harness.state_mut().session_mut() {
            session.read_file(path, bigstar_ui_egui::FileRead::Open);
        }
        harness.run();
        harness.run();
        shot(&mut harness, &dir, name)?;
    }
    if let Some(session) = harness.state_mut().session_mut() {
        session.request_banner();
    }
    harness.run();
    shot(&mut harness, &dir, "banner.png")?;
    if let Some(tab) = harness.query_by_label("Titles") {
        tab.click();
    }
    shot(&mut harness, &dir, "banner-titles.png")?;
    // The tileset workspace of the open level, a tab at a time.
    harness.state_mut().open_tilesets_for_screenshot("A01_2");
    harness.run();
    tileset_tabs(&mut harness, &dir, "tilesets")?;
    // A build of the demo project, reported into the Output panel.
    if let Some(button) = harness.query_by_label("Run Patcher") {
        button.click();
    }
    harness.run();
    shot(&mut harness, &dir, "build-output.png")?;
    if let Some(menu) = harness.query_by_label("View") {
        menu.click();
    }
    harness.run();
    if let Some(item) = harness.query_by_label("Preferences…") {
        item.click();
    }
    shot(&mut harness, &dir, "preferences.png")
}

/// Each tab of the open tileset workspace but Slots: graphics, Map16, objects, behaviours and both backgrounds.
fn tileset_tabs(harness: &mut Harness<'_, BigStarApp>, dir: &std::path::Path, prefix: &str) -> Result<(), String> {
    for (index, name) in [(1, "graphics"), (2, "map16"), (3, "objects"), (4, "behaviors"), (5, "background"), (6, "foreground")] {
        harness.state_mut().tileset_tab_for_screenshot(index);
        for _ in 0..3 {
            harness.step();
        }
        shot(harness, dir, &format!("{prefix}-{name}.png"))?;
    }
    Ok(())
}
