use bigstar_core::{ErrorCode, Fx32};
use bigstar_formats::{NSMB_LEVEL_BLOCK_COUNT, NsmbLevelObject};
use bigstar_level::{EditableLevel, IndexedLevelValue, LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates};
use bigstar_project::test_support::{RomFile, TempDir, make_narc, make_rom, write_bytes};
use bigstar_protocol::{DirectoryEntryKind, EditOutcome, Event, LevelEdit, LevelSnapshot, Request, Response, Service};

use crate::{Host, HostSession, Workspace};

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn append_block(course: &mut Vec<u8>, index: usize, block: &[u8]) {
    let (offset, size) = (course.len() as u32, block.len() as u32);
    put_u32(course, index * 8, offset);
    put_u32(course, index * 8 + 4, size);
    course.extend_from_slice(block);
}

fn object(id: u16, x: i16, y: i16) -> NsmbLevelObject {
    NsmbLevelObject { object_id: id, tileset_slot: 0, x, y, width: 2, height: 2 }
}

/// A level with two objects, as the pair of files the ROM holds.
fn level_files() -> (Vec<u8>, Vec<u8>) {
    let mut course = vec![0u8; NSMB_LEVEL_BLOCK_COUNT * 8];
    for i in 0..NSMB_LEVEL_BLOCK_COUNT {
        let size = course.len() as u32;
        put_u32(&mut course, i * 8, size);
    }
    append_block(&mut course, 0, &[0; 0x20]);
    append_block(&mut course, 2, &[0; 20]);
    let mut level = EditableLevel::load(&course, &[], None).unwrap();
    level.add_object(object(1, 0, 0)).unwrap();
    level.add_object(object(2, 10, 10)).unwrap();
    let _ = Fx32::default();
    (level.serialize_course().unwrap(), level.serialize_bgdat().unwrap())
}

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new(label: &str) -> Self {
        Self { root: TempDir::new(label) }
    }

    /// `proj/` imported from a ROM that holds 1-1 and the Field download-play area.
    fn with_project(label: &str) -> (Self, HostSession) {
        let fixture = Self::new(label);
        let (course, bgdat) = level_files();
        let narc = make_narc(&[("J01_1.bin", &course), ("J01_1_bgdat.bin", &bgdat)]);
        let rom = make_rom(
            &[RomFile::new("course/A01_1.bin", &course), RomFile::new("course/A01_1_bgdat.bin", &bgdat), RomFile::new("dat/Dat_Field.narc", &narc)],
            &[0; 32],
        );
        write_bytes(&fixture.root.path().join("roms/game.nds"), &rom);
        let mut session = Host::new(fixture.root.path()).unwrap().session();
        session.handle(Request::ImportRom { rom_path: "roms/game.nds".into(), project_path: "proj".into(), name: None }).unwrap();
        session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
        (fixture, session)
    }

    fn session(&self) -> HostSession {
        Host::new(self.root.path()).unwrap().session()
    }
}

fn open_level(session: &mut HostSession, area: &str) -> LevelSnapshot {
    match session.handle(Request::OpenLevel { area: area.into() }).unwrap() {
        Response::Level(snapshot) => *snapshot,
        other => panic!("unexpected {other:?}"),
    }
}

fn edited(response: Response) -> (LevelSnapshot, EditOutcome) {
    match response {
        Response::Edited { snapshot, outcome } => (*snapshot, outcome),
        other => panic!("unexpected {other:?}"),
    }
}

fn move_first_object(session: &mut HostSession, snapshot: &LevelSnapshot, x: i16) -> LevelSnapshot {
    let moved = NsmbLevelObject { x, ..snapshot.objects[0].clone() };
    let updates = LevelEntityUpdates { objects: vec![IndexedLevelValue { index: 0, value: moved }], ..LevelEntityUpdates::default() };
    let request =
        Request::EditLevel { level: snapshot.level, base_revision: snapshot.revision, edit: LevelEdit::Update { updates, name: "Move".into() } };
    edited(session.handle(request).unwrap()).0
}

#[test]
fn workspace_refuses_paths_that_leave_the_root() {
    let fixture = Fixture::new("host-confine");
    let workspace = Workspace::new(fixture.root.path()).unwrap();
    for bad in ["..", "a/../b", "../x", "/etc/passwd", "a\\b", "C:/windows", "c:x", "a\0b"] {
        assert_eq!(workspace.resolve(bad).unwrap_err().code, ErrorCode::InvalidArgument, "{bad:?} should be refused");
    }
    assert_eq!(workspace.resolve("").unwrap(), workspace.root());
    assert_eq!(workspace.resolve("./a//b/").unwrap(), workspace.root().join("a/b"));
    assert_eq!(workspace.relative(&workspace.root().join("a/b")).as_deref(), Some("a/b"));
}

#[cfg(unix)]
#[test]
fn workspace_refuses_symlinks_that_lead_out() {
    let fixture = Fixture::new("host-symlink");
    let outside = TempDir::new("host-outside");
    write_bytes(&outside.path().join("secret.txt"), b"secret");
    std::os::unix::fs::symlink(outside.path(), fixture.root.path().join("link")).unwrap();
    std::os::unix::fs::symlink(outside.path().join("missing"), fixture.root.path().join("dangling")).unwrap();
    std::fs::create_dir(fixture.root.path().join("inside")).unwrap();
    std::os::unix::fs::symlink(fixture.root.path().join("inside"), fixture.root.path().join("friendly")).unwrap();

    let workspace = Workspace::new(fixture.root.path()).unwrap();
    for bad in ["link", "link/secret.txt", "link/new/file", "dangling", "dangling/x"] {
        assert!(workspace.resolve(bad).is_err(), "{bad} should be refused");
    }
    // A link that stays inside is fine, and resolves to where it really points.
    assert_eq!(workspace.resolve("friendly/x").unwrap(), workspace.root().join("inside/x"));

    let mut session = Host::new(fixture.root.path()).unwrap().session();
    let Response::Directory(listing) = session.handle(Request::BrowseDirectory { path: String::new() }).unwrap() else { panic!() };
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"inside") && names.contains(&"friendly"));
    assert!(!names.contains(&"link") && !names.contains(&"dangling"), "{names:?}");
}

#[test]
fn requests_cannot_reach_outside_the_root() {
    let fixture = Fixture::new("host-escape");
    let mut session = fixture.session();
    for request in [
        Request::BrowseDirectory { path: "..".into() },
        Request::OpenProject { path: "/tmp".into() },
        Request::CreateProject { path: "../elsewhere".into(), name: "x".into() },
        Request::ImportRom { rom_path: "../game.nds".into(), project_path: "p".into(), name: None },
        Request::ImportRom { rom_path: "game.nds".into(), project_path: "../p".into(), name: None },
    ] {
        assert_eq!(session.handle(request.clone()).unwrap_err().code, ErrorCode::InvalidArgument, "{request:?}");
    }
}

#[test]
fn browsing_lists_directories_first() {
    let fixture = Fixture::new("host-browse");
    write_bytes(&fixture.root.path().join("a.nds"), b"x");
    write_bytes(&fixture.root.path().join("sub/inner.nds"), b"xyz");
    write_bytes(&fixture.root.path().join(".hidden"), b"x");
    let mut session = fixture.session();
    let Response::Directory(listing) = session.handle(Request::BrowseDirectory { path: String::new() }).unwrap() else { panic!() };
    assert_eq!(listing.path, "");
    let rows: Vec<(&str, DirectoryEntryKind, u64)> = listing.entries.iter().map(|e| (e.name.as_str(), e.kind, e.size)).collect();
    assert_eq!(rows, [("sub", DirectoryEntryKind::Directory, 0), ("a.nds", DirectoryEntryKind::File, 1)]);
    let Response::Directory(inner) = session.handle(Request::BrowseDirectory { path: "sub".into() }).unwrap() else { panic!() };
    assert_eq!(inner.path, "sub");
    assert_eq!(inner.entries.len(), 1);
}

#[test]
fn projects_are_created_imported_listed_and_opened() {
    let (fixture, mut session) = Fixture::with_project("host-projects");
    let Response::Project(created) = session.handle(Request::CreateProject { path: "empty/one".into(), name: " Empty ".into() }).unwrap() else {
        panic!()
    };
    assert_eq!((created.path.as_str(), created.name.as_str(), created.rom_path.as_deref()), ("empty/one", "Empty", None));
    assert_eq!(
        session.handle(Request::CreateProject { path: "empty/one".into(), name: "again".into() }).unwrap_err().code,
        ErrorCode::InvalidArgument
    );
    assert_eq!(session.handle(Request::CreateProject { path: "x".into(), name: "  ".into() }).unwrap_err().code, ErrorCode::InvalidArgument);

    let Response::Projects(projects) = session.handle(Request::ListProjects).unwrap() else { panic!() };
    let listed: Vec<(&str, &str)> = projects.iter().map(|p| (p.path.as_str(), p.name.as_str())).collect();
    assert_eq!(listed, [("empty/one", "Empty"), ("proj", "proj")]);

    let Response::Project(info) = session.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!(info.rom_path.as_deref(), Some("rom.nds"));
    assert_eq!(info.file_count, 3);
    assert!(info.game_code.is_some());
    assert!(fixture.root.path().join("proj/rom.nds").is_file());
    assert_eq!(
        session.handle(Request::ImportRom { rom_path: "roms/game.nds".into(), project_path: "proj".into(), name: None }).unwrap_err().code,
        ErrorCode::InvalidArgument
    );

    // A project with no ROM opens and has no levels.
    session.handle(Request::OpenProject { path: "empty/one".into() }).unwrap();
    let Response::Levels(worlds) = session.handle(Request::ListLevels).unwrap() else { panic!() };
    assert!(worlds.iter().flat_map(|w| &w.levels).flat_map(|l| &l.areas).all(|a| !a.available));
    assert_eq!(session.handle(Request::OpenLevel { area: "A01_1".into() }).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn level_requests_need_an_open_project() {
    let fixture = Fixture::new("host-noproject");
    let mut session = fixture.session();
    for request in [Request::ListLevels, Request::OpenLevel { area: "A01_1".into() }, Request::SaveLevel { level: 1 }] {
        assert_eq!(session.handle(request).unwrap_err().code, ErrorCode::InvalidArgument);
    }
}

#[test]
fn the_level_list_marks_the_areas_the_rom_holds() {
    let (_fixture, mut session) = Fixture::with_project("host-levels");
    let Response::Levels(worlds) = session.handle(Request::ListLevels).unwrap() else { panic!() };
    assert_eq!(worlds[0].name, "World 1");
    assert_eq!(worlds[0].levels[0].display_name, "1-1");
    let available: Vec<(&str, &str)> = worlds
        .iter()
        .flat_map(|w| w.levels.iter().map(move |l| (w.name.as_str(), l)))
        .flat_map(|(world, l)| l.areas.iter().filter(|a| a.available).map(move |a| (world, a.token.as_str())))
        .collect();
    // The Field area is held by the archive, while the multicard `J01_1` is a plain file the ROM lacks.
    assert_eq!(available, [("World 1", "A01_1"), ("Mario vs. Luigi > Download Play", "J01_1@Dat_Field")]);
}

#[test]
fn edits_undo_redo_and_save() {
    let (fixture, mut session) = Fixture::with_project("host-edit");
    let opened = open_level(&mut session, "A01_1");
    assert_eq!((opened.revision, opened.dirty, opened.title.as_str()), (1, false, "1-1 Area 1"));
    assert_eq!(opened.objects.len(), 2);
    assert_eq!(open_level(&mut session, "A01_1").level, opened.level, "reopening rejoins the session");

    let moved = move_first_object(&mut session, &opened, 40);
    assert_eq!((moved.revision, moved.dirty, moved.can_undo, moved.objects[0].x), (2, true, true, 40));
    assert_eq!(moved.undo_names, ["Move"]);

    let (undone, _) = edited(session.handle(Request::UndoLevel { level: opened.level }).unwrap());
    assert_eq!((undone.revision, undone.dirty, undone.objects[0].x, undone.can_redo), (3, false, 0, true));
    let (redone, _) = edited(session.handle(Request::RedoLevel { level: opened.level }).unwrap());
    assert_eq!((redone.revision, redone.objects[0].x), (4, 40));
    assert_eq!(session.handle(Request::RedoLevel { level: opened.level }).unwrap_err().code, ErrorCode::InvalidArgument);

    let (saved, _) = edited(session.handle(Request::SaveLevel { level: opened.level }).unwrap());
    assert_eq!((saved.revision, saved.dirty), (5, false));
    assert!(fixture.root.path().join("proj/edits/course/A01_1.bin").is_file());
    assert!(fixture.root.path().join("proj/edits/course/A01_1_bgdat.bin").is_file());

    // A fresh host reads the saved edit before the ROM.
    let mut other = fixture.session();
    other.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let reopened = open_level(&mut other, "A01_1");
    assert_eq!((reopened.objects[0].x, reopened.dirty, reopened.revision), (40, false, 1));
}

#[test]
fn stale_and_unknown_edits_are_refused() {
    let (_fixture, mut session) = Fixture::with_project("host-stale");
    let opened = open_level(&mut session, "A01_1");
    move_first_object(&mut session, &opened, 8);

    let stale = Request::EditLevel {
        level: opened.level,
        base_revision: opened.revision,
        edit: LevelEdit::Remove { selection: LevelEntitySelection::default(), name: "x".into() },
    };
    assert_eq!(session.handle(stale).unwrap_err().code, ErrorCode::InvalidArgument);
    let unknown = Request::EditLevel { level: 99, base_revision: 1, edit: LevelEdit::SetObjectBank { slot: 0, value: 1 } };
    assert_eq!(session.handle(unknown).unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(session.handle(Request::UndoLevel { level: 99 }).unwrap_err().code, ErrorCode::NotFound);

    // An edit the level itself rejects leaves it untouched.
    let bad =
        LevelEntityUpdates { objects: vec![IndexedLevelValue { index: 50, value: NsmbLevelObject::default() }], ..LevelEntityUpdates::default() };
    let request = Request::EditLevel { level: opened.level, base_revision: 2, edit: LevelEdit::Update { updates: bad, name: "x".into() } };
    assert_eq!(session.handle(request).unwrap_err().code, ErrorCode::OutOfBounds);
    let again = open_level(&mut session, "A01_1");
    assert_eq!((again.revision, again.objects.len()), (2, 2));
}

#[test]
fn add_and_remove_report_what_changed() {
    let (_fixture, mut session) = Fixture::with_project("host-add");
    let opened = open_level(&mut session, "A01_1");
    let items = LevelEntityInsertions { objects: vec![object(7, 4, 4)], ..LevelEntityInsertions::default() };
    let request = Request::EditLevel { level: opened.level, base_revision: 1, edit: LevelEdit::Add { items, name: "Add object".into() } };
    let (added, outcome) = edited(session.handle(request).unwrap());
    assert_eq!(added.objects.len(), 3);
    let EditOutcome::Added(selection) = outcome else { panic!("{outcome:?}") };
    assert_eq!(selection.objects, [2]);

    let remove = LevelEntitySelection { objects: vec![0, 2], ..LevelEntitySelection::default() };
    let request = Request::EditLevel { level: opened.level, base_revision: 2, edit: LevelEdit::Remove { selection: remove, name: "Delete".into() } };
    let (removed, _) = edited(session.handle(request).unwrap());
    assert_eq!(removed.objects.iter().map(|o| o.object_id).collect::<Vec<_>>(), [2]);
}

#[test]
fn download_play_areas_are_read_from_their_archive() {
    let (fixture, mut session) = Fixture::with_project("host-narc");
    let opened = open_level(&mut session, "J01_1@Dat_Field");
    assert_eq!(opened.objects.len(), 2);
    let moved = move_first_object(&mut session, &opened, 12);
    session.handle(Request::SaveLevel { level: moved.level }).unwrap();
    assert!(fixture.root.path().join("proj/edits/dat/Dat_Field_narc/J01_1.bin").is_file());
    assert_eq!(session.handle(Request::OpenLevel { area: "J01_1".into() }).unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(session.handle(Request::OpenLevel { area: "nope".into() }).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn a_project_with_unsaved_levels_stays_open() {
    let (_fixture, mut session) = Fixture::with_project("host-dirty");
    let opened = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &opened, 3);
    assert_eq!(session.handle(Request::CloseProject).unwrap_err().code, ErrorCode::InvalidArgument);
    assert_eq!(session.handle(Request::OpenProject { path: "proj".into() }).unwrap_err().code, ErrorCode::InvalidArgument);
    session.handle(Request::CloseLevel { level: moved.level }).unwrap();
    assert_eq!(session.handle(Request::CloseLevel { level: moved.level }).unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(session.handle(Request::CloseProject).unwrap(), Response::Done);
    assert_eq!(session.handle(Request::ListLevels).unwrap_err().code, ErrorCode::InvalidArgument);
}

#[test]
fn every_connection_hears_about_changes() {
    let (_fixture, mut first) = Fixture::with_project("host-events");
    let mut second = Host { shared: first_shared(&first) }.session();
    first.drain_events();
    let opened = open_level(&mut first, "A01_1");
    let moved = move_first_object(&mut first, &opened, 5);

    let heard = second.drain_events();
    assert!(heard.contains(&Event::LevelChanged { level: moved.level, revision: 2, dirty: true }), "{heard:?}");
    assert!(first.drain_events().contains(&Event::LevelChanged { level: moved.level, revision: 2, dirty: true }));
    assert!(second.drain_events().is_empty(), "events are delivered once");
}

fn first_shared(session: &HostSession) -> std::sync::Arc<crate::host::Shared> {
    session.shared()
}
