//! The whole stack in one process: the frontend, an in-process host, a project imported through the file browser, a
//! level opened, an object dragged, saved into a module.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::time::{Duration, Instant};

use bigstar_client::{Connection, HostInfo, InProcess, Waker};
use bigstar_host::Host;
use bigstar_level::EditableLevel;
use bigstar_project::test_support::{RomFile, TempDir, make_rom, write_bytes};
use bigstar_ui_egui::{BigStarApp, EMBEDDED_ROOT_KEY, Item, Kind, Services};
use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};

struct EmbeddedServices {
    root: String,
}

impl Services for EmbeddedServices {
    fn load(&self, key: &str) -> Option<String> {
        (key == EMBEDDED_ROOT_KEY).then(|| self.root.clone())
    }

    fn store(&self, _key: &str, _value: &str) {}

    fn can_embed(&self) -> bool {
        true
    }

    fn connect_embedded(&self, root: &str, waker: Waker) -> Result<Box<dyn Connection>, String> {
        let host = Host::new(std::path::Path::new(root)).map_err(|e| e.message)?;
        let info = HostInfo { host_version: "test".to_owned(), workspace_name: host.workspace_name() };
        Ok(Box::new(InProcess::spawn(host.session(), info, Some(waker))))
    }
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn append_block(course: &mut Vec<u8>, index: usize, block: &[u8]) {
    let (offset, size) = (course.len() as u32, block.len() as u32);
    put_u32(course, index * 8, offset);
    put_u32(course, index * 8 + 4, size);
    course.extend_from_slice(block);
}

fn level_files() -> (Vec<u8>, Vec<u8>) {
    let mut course = vec![0u8; bigstar_formats::NSMB_LEVEL_BLOCK_COUNT * 8];
    for i in 0..bigstar_formats::NSMB_LEVEL_BLOCK_COUNT {
        let size = course.len() as u32;
        put_u32(&mut course, i * 8, size);
    }
    append_block(&mut course, 0, &[0; 0x20]);
    append_block(&mut course, 2, &[0; 20]);
    let mut level = EditableLevel::load(&course, &[], None).unwrap();
    level.add_object(bigstar_formats::NsmbLevelObject { object_id: 1, tileset_slot: 0, x: 0, y: 0, width: 4, height: 2 }).unwrap();
    level.add_object(bigstar_formats::NsmbLevelObject { object_id: 2, tileset_slot: 0, x: 10, y: 10, width: 2, height: 2 }).unwrap();
    (level.serialize_course().unwrap(), level.serialize_bgdat().unwrap())
}

/// Steps the app until `done` holds, giving the host's thread time to answer.
fn until(harness: &mut Harness<'_, BigStarApp>, what: &str, done: impl Fn(&BigStarApp) -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done(harness.state()) {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    harness.run();
}

#[test]
fn import_open_drag_and_save_through_the_embedded_host() {
    let workspace = TempDir::new("embedded-app");
    let (course, bgdat) = level_files();
    let rom = make_rom(&[RomFile::new("course/A01_1.bin", &course), RomFile::new("course/A01_1_bgdat.bin", &bgdat)], &[0; 32]);
    write_bytes(&workspace.path().join("roms/game.nds"), &rom);

    let services = EmbeddedServices { root: workspace.path().to_string_lossy().into_owned() };
    let app = BigStarApp::new(Box::new(services));
    let mut harness = Harness::builder().with_size(Vec2::new(1000.0, 640.0)).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();

    harness.get_by_label("Open").click();
    until(&mut harness, "the connection", |app| app.session().is_some_and(|s| s.connected()));
    until(&mut harness, "the project list", |app| app.session().is_some_and(|s| s.projects.is_empty()));

    harness.get_by_label("Import ROM…").click();
    until(&mut harness, "the file browser", |app| app.session().and_then(|s| s.browser.as_ref()).is_some_and(|b| b.listing.is_some()));
    harness.get_by_label("roms").click();
    until(&mut harness, "the roms folder", |app| {
        app.session().and_then(|s| s.browser.as_ref()).and_then(|b| b.listing.as_ref()).is_some_and(|l| l.path == "roms")
    });
    harness.get_by_label("game.nds").click();
    harness.run();
    harness.get_by_label("Import").click();
    until(&mut harness, "the imported project to open", |app| app.session().is_some_and(|s| s.project.is_some() && !s.worlds.is_empty()));
    // A module to save into, as a build of the project would have recorded it; the host's watcher notices it.
    let project = workspace.path().join("roms/game");
    let modules = format!(
        r#"{{"schema":"ncpatcher.modules/1","dir":"{dir}/modules","modules":[{{"key":"mine","id":"Mine","name":"My module","enabled":true,
            "dir":"{dir}/modules/mine","nitrofs":{{"dir":"{dir}/modules/mine/nitrofs"}},"components":[]}}]}}"#,
        dir = project.display()
    );
    write_bytes(&project.join("build/generated/modules.json"), modules.as_bytes());
    std::fs::create_dir_all(project.join("modules/mine/nitrofs")).unwrap();
    let settle = Instant::now();
    while settle.elapsed() < Duration::from_millis(800) {
        harness.step();
        std::thread::sleep(Duration::from_millis(10));
    }

    harness.get_by_label("World 1").click();
    harness.run();
    // Several levels have an "Area 1"; only 1-1's is in the ROM, so only its Edit button is enabled.
    harness.query_all_by_label("Edit").find(|button| !button.accesskit_node().is_disabled()).unwrap().click();
    until(&mut harness, "the level", |app| app.session().is_some_and(|s| s.level().is_some()));

    let (grab, drop, x_before) = {
        let view = harness.state().session().unwrap().level().unwrap();
        let to_screen = |p: egui::Pos2| view.canvas.min + (p.to_vec2() - view.pan) * view.zoom;
        let center = view.bounds(Item::new(Kind::Object, 0)).unwrap().center();
        (to_screen(center), to_screen(center + Vec2::new(64.0, 0.0)), view.snapshot.objects[0].x)
    };
    harness.hover_at(grab);
    harness.run();
    harness.drag_at(grab);
    harness.run();
    harness.hover_at(drop);
    harness.run();
    harness.drop_at(drop);
    until(&mut harness, "the move to be answered", |app| app.session().and_then(|s| s.level()).is_some_and(|v| v.snapshot.dirty));
    let view = harness.state().session().unwrap().level().unwrap();
    assert_eq!(view.snapshot.objects[0].x, x_before + 4, "64 pixels is four tiles");
    assert_eq!(view.snapshot.undo_names, ["Move selection"]);

    harness.get_by_label("Save").click();
    until(&mut harness, "the save", |app| app.session().and_then(|s| s.level()).is_some_and(|v| !v.snapshot.dirty));

    // Into the module, for NCPatcher to build into the ROM.
    let saved = std::fs::read(project.join("modules/mine/nitrofs/course/A01_1.bin")).unwrap();
    let saved_bgdat = std::fs::read(project.join("modules/mine/nitrofs/course/A01_1_bgdat.bin")).unwrap();
    let reloaded = EditableLevel::load(&saved, &saved_bgdat, None).unwrap();
    assert_eq!(reloaded.objects()[0].x, 4);
}
