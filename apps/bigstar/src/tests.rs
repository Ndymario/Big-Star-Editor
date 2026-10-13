//! The desktop app's frontend against a real host: what only shows with two frontends on one workspace.

use std::time::{Duration, Instant};

use bigstar_client::{HostInfo, InProcess};
use bigstar_formats::{NSMB_LEVEL_BLOCK_COUNT, NsmbLevelObject};
use bigstar_host::Host;
use bigstar_level::{EditableLevel, IndexedLevelValue, LevelEntityUpdates};
use bigstar_project::test_support::{RomFile, TempDir, make_rom, write_bytes};
use bigstar_protocol::{LevelEdit, Request, Service};
use bigstar_ui_egui::Session;

/// A course with two objects and its BGDat.
fn level_files() -> (Vec<u8>, Vec<u8>) {
    let mut course = vec![0u8; NSMB_LEVEL_BLOCK_COUNT * 8];
    let put = |data: &mut Vec<u8>, at: usize, value: u32| data[at..at + 4].copy_from_slice(&value.to_le_bytes());
    for i in 0..NSMB_LEVEL_BLOCK_COUNT {
        let size = course.len() as u32;
        put(&mut course, i * 8, size);
    }
    for (index, block) in [(0usize, vec![0u8; 0x20]), (2, vec![0; 20])] {
        let (offset, size) = (course.len() as u32, block.len() as u32);
        put(&mut course, index * 8, offset);
        put(&mut course, index * 8 + 4, size);
        course.extend_from_slice(&block);
    }
    let mut level = EditableLevel::load(&course, &[], None).unwrap();
    for (id, x) in [(1, 0), (2, 10)] {
        level.add_object(NsmbLevelObject { object_id: id, tileset_slot: 0, x, y: 0, width: 2, height: 2 }).unwrap();
    }
    (level.serialize_course().unwrap(), level.serialize_bgdat().unwrap())
}

fn frontend(host: &Host) -> Session {
    let info = HostInfo { host_version: "test".to_owned(), workspace_name: host.workspace_name() };
    Session::new(Box::new(InProcess::spawn(host.session(), info, None)))
}

/// Polls every frontend until `done`, or panics after five seconds.
fn until(sessions: &mut [&mut Session], done: impl Fn(&[&mut Session]) -> bool) {
    let start = Instant::now();
    while !done(sessions) {
        assert!(
            start.elapsed() < Duration::from_secs(5),
            "timed out: {:?}",
            sessions
                .iter()
                .map(|s| (
                    s.connected(),
                    s.project.is_some(),
                    s.level().map(|v| v.snapshot.revision),
                    s.notice.clone(),
                    s.log.iter().map(|l| l.text.clone()).collect::<Vec<_>>(),
                    s.busy()
                ))
                .collect::<Vec<_>>()
        );
        for session in sessions.iter_mut() {
            session.poll();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn an_edit_in_one_frontend_appears_in_another() {
    let root = TempDir::new("two-frontends");
    let (course, bgdat) = level_files();
    let rom = make_rom(&[RomFile::new("course/A01_1.bin", &course), RomFile::new("course/A01_1_bgdat.bin", &bgdat)], &[0; 32]);
    write_bytes(&root.path().join("roms/game.nds"), &rom);
    let host = Host::new(root.path()).unwrap();
    host.session().handle(Request::ImportRom { rom_path: "roms/game.nds".into(), project_path: "proj".into(), name: None }).unwrap();

    let (mut one, mut two) = (frontend(&host), frontend(&host));
    for session in [&mut one, &mut two] {
        until(&mut [&mut *session], |s| s[0].connected());
        session.open_project("proj");
        until(&mut [&mut *session], |s| s[0].project.is_some());
        session.open_level("A01_1");
        until(&mut [&mut *session], |s| s[0].level().is_some_and(|view| !view.snapshot.objects.is_empty()));
    }

    // The first moves an object; the second, which did nothing, shows it moved and unsaved.
    let snapshot = one.level().unwrap().snapshot.clone();
    let moved = NsmbLevelObject { x: 42, ..snapshot.objects[0].clone() };
    let updates = LevelEntityUpdates { objects: vec![IndexedLevelValue { index: 0, value: moved }], ..LevelEntityUpdates::default() };
    one.edit(LevelEdit::Update { updates, name: "Move".into() });
    until(&mut [&mut one, &mut two], |s| s[1].level().is_some_and(|view| view.snapshot.objects[0].x == 42));
    let seen = &two.level().unwrap().snapshot;
    assert!(seen.dirty);
    assert_eq!(seen.revision, one.level().unwrap().snapshot.revision);

    // And an undo there comes back here.
    two.step_history(true, 1);
    until(&mut [&mut one, &mut two], |s| s[0].level().is_some_and(|view| view.snapshot.objects[0].x == 0 && !view.snapshot.dirty));
}
