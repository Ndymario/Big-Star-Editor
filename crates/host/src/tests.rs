use bigstar_core::{ErrorCode, Fx32};
use bigstar_formats::{NSMB_LEVEL_BLOCK_COUNT, NsmbLevelObject};
use bigstar_level::{EditableLevel, IndexedLevelValue, LevelEntityInsertions, LevelEntitySelection, LevelEntityUpdates};
use bigstar_project::test_support::{RomFile, TempDir, make_narc, make_rom, write_bytes};
use bigstar_protocol::{DirectoryEntryKind, EditOutcome, Event, FileIcon, FileStatus, LevelEdit, LevelSnapshot, Request, Response, Service};

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
fn level_data_is_in_the_snapshot_and_edited_as_one_step() {
    let (_fixture, mut session) = Fixture::with_project("host-glue");
    let opened = open_level(&mut session, "A01_1");
    assert_eq!((opened.glue.clone(), opened.stage_object_id_base, opened.registration_hashes.clone()), (Default::default(), 326, vec![]));
    let glue = bigstar_formats::GlueBlock { flags: vec![0x1234_5678], values: vec![] };
    let request =
        Request::EditLevel { level: opened.level, base_revision: 1, edit: LevelEdit::SetGlue { glue: glue.clone(), name: "Edit level data".into() } };
    let (changed, _) = edited(session.handle(request).unwrap());
    assert_eq!(changed.glue, glue);
    assert_eq!(changed.undo_names, ["Edit level data"]);

    // A module sprite registers itself, and the snapshot says so.
    let sprite = bigstar_level::SpriteInsertion { value: Default::default(), hash: Some(0xabcd_0001) };
    let items = LevelEntityInsertions { sprites: vec![sprite], ..LevelEntityInsertions::default() };
    let request = Request::EditLevel { level: opened.level, base_revision: 2, edit: LevelEdit::Add { items, name: "Add custom sprite".into() } };
    let (added, _) = edited(session.handle(request).unwrap());
    assert_eq!(added.registration_hashes, [0xabcd_0001]);
    assert_eq!(added.course.sprites[0].id, 326);
}

#[test]
fn project_data_names_the_module_catalog() {
    let (_fixture, mut session) = Fixture::with_project("host-project-data");
    let Response::ProjectData(data) = session.handle(Request::ProjectData).unwrap() else { panic!() };
    assert_eq!(data.modules.stage_object_id_base, 326);
    // The synthetic ROM's ARM9 holds no bank table.
    assert_eq!(data.object_bank_overlays, None);
}

/// Against a real ROM, named by `BIGSTAR_NSMB_ROM`: the bank table is found in its ARM9. Skipped without one.
#[test]
fn a_real_rom_names_the_overlay_of_every_object_bank() {
    let Some(rom) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let fixture = Fixture::new("host-real-banks");
    std::fs::create_dir_all(fixture.root.path().join("roms")).unwrap();
    std::fs::copy(rom, fixture.root.path().join("roms/game.nds")).unwrap();
    let mut session = fixture.session();
    session.handle(Request::ImportRom { rom_path: "roms/game.nds".into(), project_path: "proj".into(), name: None }).unwrap();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let Response::ProjectData(data) = session.handle(Request::ProjectData).unwrap() else { panic!() };
    let table = data.object_bank_overlays.expect("the clean ROM's bank table");
    assert_eq!((table[0][0], table[15][9]), (22, 21));
    assert_eq!(table[10], [u32::MAX; 10], "a slot with no banks");
}

#[test]
fn a_real_level_serves_its_tilesets_and_backgrounds_as_files() {
    let Some(rom) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let fixture = Fixture::new("host-real-tilesets");
    std::fs::create_dir_all(fixture.root.path().join("roms")).unwrap();
    std::fs::copy(rom, fixture.root.path().join("roms/game.nds")).unwrap();
    let mut session = fixture.session();
    session.handle(Request::ImportRom { rom_path: "roms/game.nds".into(), project_path: "proj".into(), name: None }).unwrap();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let snapshot = open_level(&mut session, "A01_1");
    let Response::TilesetResources(resources) = session.handle(Request::TilesetResources { level: snapshot.level }).unwrap() else { panic!() };
    assert_eq!(resources.tilesets.len(), 3);
    // Slot 0 is jyotyu, shared by every level; the level's own is in slot 1.
    assert_eq!(resources.tilesets[0].graphics.rom_path, "BG_ncg/d_2d_A_J_jyotyu_ncg.bin");
    assert!(resources.tilesets.iter().all(|slot| !slot.map16.bytes.is_empty()));
    assert!(!resources.background.screen.bytes.is_empty());
    assert_eq!(resources.references, snapshot.tileset_references);
    assert_eq!(resources.tilesets[1].map16.rom_path, "BG_pnl/d_2d_PA_I_M_yakata.bin", "1-1's tileset id 1");
}

#[test]
fn a_level_round_trips_through_its_bytes_as_one_step() {
    let (_fixture, mut session) = Fixture::with_project("host-bytes");
    let opened = open_level(&mut session, "A01_1");
    let Response::LevelBytes(bytes) = session.handle(Request::LevelBytes { level: opened.level }).unwrap() else { panic!() };
    assert!(bytes.course_file_id.is_some() && bytes.bgdat_file_id.is_some(), "plain ROM files have ids");
    // One object fewer: the BGDat is ten bytes an object, then the terminator.
    let mut bgdat = bytes.bgdat.clone();
    bgdat.drain(..10);
    let edit = LevelEdit::ReplaceSerialized { course: bytes.course.clone(), bgdat, name: "Hex edit level".into() };
    let (replaced, _) = edited(session.handle(Request::EditLevel { level: opened.level, base_revision: 1, edit }).unwrap());
    assert_eq!((replaced.objects.len(), replaced.undo_names[0].as_str()), (opened.objects.len() - 1, "Hex edit level"));
    // Garbage is refused and changes nothing.
    let edit = LevelEdit::ReplaceSerialized { course: vec![1, 2, 3], bgdat: vec![], name: "x".into() };
    assert!(session.handle(Request::EditLevel { level: opened.level, base_revision: 2, edit }).is_err());
}

#[test]
fn finders_search_every_level_including_unsaved_edits() {
    let (_fixture, mut session) = Fixture::with_project("host-find");
    let Response::Found(found) = session.handle(Request::FindEntities { sprites: false, id: 2 }).unwrap() else { panic!() };
    let areas: Vec<&str> = found.iter().map(|f| f.area.as_str()).collect();
    assert!(areas.contains(&"A01_1") && areas.contains(&"J01_1@Dat_Field"), "{areas:?}");
    // An open session counts as it stands.
    let opened = open_level(&mut session, "A01_1");
    let remove = LevelEntitySelection { objects: (0..opened.objects.len()).collect(), ..LevelEntitySelection::default() };
    session
        .handle(Request::EditLevel { level: opened.level, base_revision: 1, edit: LevelEdit::Remove { selection: remove, name: "x".into() } })
        .unwrap();
    let Response::Found(found) = session.handle(Request::FindEntities { sprites: false, id: 2 }).unwrap() else { panic!() };
    assert!(found.iter().all(|f| f.area != "A01_1"));
    let Response::Found(none) = session.handle(Request::FindEntities { sprites: true, id: 9999 }).unwrap() else { panic!() };
    assert!(none.is_empty());
}

#[test]
fn unsaved_levels_are_autosaved_and_offered_to_the_next_session() {
    let (fixture, mut session) = Fixture::with_project("host-recovery");
    let opened = open_level(&mut session, "A01_1");
    move_first_object(&mut session, &opened, 9);
    session.shared().write_recovery_snapshots();
    let autosaves = fixture.root.path().join("proj/.big-star-autosave");
    assert_eq!(std::fs::read_dir(&autosaves).unwrap().count(), 1);
    // This session's own autosave is not offered back to it.
    let Response::Recovery(offered) = session.handle(Request::Recovery).unwrap() else { panic!() };
    assert!(offered.is_empty());

    // The next host finds it, and accepting it makes a dirty session holding the edit.
    drop(session);
    let mut next = fixture.session();
    next.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let Response::Recovery(offered) = next.handle(Request::Recovery).unwrap() else { panic!() };
    assert_eq!(offered, [bigstar_protocol::RecoverableLevel { area: "A01_1".into(), title: "1-1 Area 1".into() }]);
    let Response::Recovered(areas) = next.handle(Request::Recover { accept: true }).unwrap() else { panic!() };
    assert_eq!(areas, ["A01_1"]);
    let recovered = open_level(&mut next, "A01_1");
    assert_eq!((recovered.objects[0].x, recovered.dirty, recovered.undo_names[0].as_str()), (9, true, "Recover autosave"));

    // Saved, there is nothing left to recover; declined, the autosaves go.
    next.handle(Request::SaveLevel { level: recovered.level }).unwrap();
    next.shared().write_recovery_snapshots();
    assert_eq!(std::fs::read_dir(&autosaves).unwrap().count(), 0);
    std::fs::write(autosaves.join(super::host::recovery_name_for_tests("A01_1")), b"stale").unwrap();
    let Response::Recovered(none) = next.handle(Request::Recover { accept: false }).unwrap() else { panic!() };
    assert!(none.is_empty());
    assert_eq!(std::fs::read_dir(&autosaves).unwrap().count(), 0);
}

/// A project whose last build left a module graph naming module `mine` with a NitroFS tree.
fn with_module(label: &str) -> (Fixture, HostSession) {
    let (fixture, session) = Fixture::with_project(label);
    let project = fixture.root.path().join("proj");
    let modules = format!(
        r#"{{"schema":"ncpatcher.modules/1","dir":"{dir}/modules","modules":[{{"key":"mine","id":"Mine","name":"My module","enabled":true,
            "dir":"{dir}/modules/mine","nitrofs":{{"dir":"{dir}/modules/mine/nitrofs"}},"components":[]}}]}}"#,
        dir = project.display()
    );
    write_bytes(&project.join("build/generated/modules.json"), modules.as_bytes());
    std::fs::create_dir_all(project.join("modules/mine/nitrofs")).unwrap();
    // Reopened, so the host reads the graph.
    let mut session = session;
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    (fixture, session)
}

fn listing(session: &mut HostSession) -> bigstar_protocol::FilesystemListing {
    match session.handle(Request::Filesystem).unwrap() {
        Response::Filesystem(listing) => *listing,
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_filesystem_lists_the_rom_with_ids_icons_and_archive_members_on_demand() {
    let (_fixture, mut session) = Fixture::with_project("host-fs");
    let listing = listing(&mut session);
    let find = |path: &str| listing.entries.iter().find(|e| e.path == path).cloned().unwrap_or_else(|| panic!("{path}"));
    let course = find("course/A01_1.bin");
    assert_eq!((course.status, course.icon, course.id.is_some(), course.from_module), (FileStatus::Vanilla, FileIcon::Level, true, false));
    assert_eq!(find("dat/Dat_Field.narc").icon, FileIcon::Archive);
    assert!(!listing.pending);

    let Response::Files(members) = session.handle(Request::ListArchive { path: "dat/Dat_Field.narc".into() }).unwrap() else { panic!() };
    assert_eq!(
        members.iter().map(|m| (m.path.as_str(), m.member_id)).collect::<Vec<_>>(),
        [("dat/Dat_Field.narc!J01_1.bin", Some(0)), ("dat/Dat_Field.narc!J01_1_bgdat.bin", Some(1))]
    );
    let Response::FileBytes { bytes, .. } = session.handle(Request::ReadFile { path: "dat/Dat_Field.narc!J01_1_bgdat.bin".into() }).unwrap() else {
        panic!()
    };
    assert_eq!(bytes, level_files().1);

    // The level list names the files the way the dock does.
    let Response::Levels(worlds) = session.handle(Request::ListLevels).unwrap() else { panic!() };
    let areas: Vec<_> = worlds.iter().flat_map(|w| &w.levels).flat_map(|l| &l.areas).filter(|a| a.available).collect();
    assert!(areas.iter().any(|a| a.course_path == "course/A01_1.bin" && a.bgdat_path == "course/A01_1_bgdat.bin"));
    assert!(areas.iter().any(|a| a.course_path == "dat/Dat_Field.narc!J01_1.bin"));
}

#[test]
fn writes_go_to_the_module_and_new_files_under_z_new() {
    let (fixture, mut session) = with_module("host-fs-write");
    let project = fixture.root.path().join("proj");
    let listing = listing(&mut session);
    assert_eq!(listing.modules, [bigstar_protocol::ModuleChoice { key: "mine".into(), name: "My module".into() }]);
    assert_eq!(listing.active_module, "mine");

    session.handle(Request::WriteFile { path: "course/A01_1.bin".into(), bytes: vec![1, 2, 3] }).unwrap();
    assert_eq!(std::fs::read(project.join("modules/mine/nitrofs/course/A01_1.bin")).unwrap(), [1, 2, 3]);
    let Response::FileTargets(holding) = session.handle(Request::FileTargets { path: "course/A01_1.bin".into() }).unwrap() else { panic!() };
    assert_eq!(holding, [""], "the base layer holds it");
    // Reads go through NCPatcher's plan; with no NCPatcher to ask there is none, and the ROM's copy is what reads, as
    // in the Qt editor.
    let Response::FileBytes { bytes, .. } = session.handle(Request::ReadFile { path: "course/A01_1.bin".into() }).unwrap() else { panic!() };
    assert_eq!(bytes, level_files().0);

    let error = session.handle(Request::WriteFile { path: "elsewhere/new.bin".into(), bytes: vec![1] }).unwrap_err();
    assert_eq!(error.message, "New NitroFS files must be under z_new/.");
    session.handle(Request::WriteFile { path: "z_new/new.bin".into(), bytes: vec![9] }).unwrap();
    assert!(project.join("modules/mine/nitrofs/z_new/new.bin").is_file());
}

#[test]
fn several_files_are_written_at_once() {
    let (fixture, mut session) = with_module("host-fs-write-many");
    let project = fixture.root.path().join("proj");
    listing(&mut session);
    session.drain_events();
    let files = vec![
        bigstar_protocol::FileWrite { path: "course/A01_1.bin".into(), bytes: vec![1] },
        bigstar_protocol::FileWrite { path: "z_new/b.bin".into(), bytes: vec![2] },
    ];
    assert_eq!(session.handle(Request::WriteFiles { files }).unwrap(), Response::Done);
    assert_eq!(std::fs::read(project.join("modules/mine/nitrofs/course/A01_1.bin")).unwrap(), [1]);
    assert_eq!(std::fs::read(project.join("modules/mine/nitrofs/z_new/b.bin")).unwrap(), [2]);
    // The write announces the change itself; the project watcher may announce the new files again, later.
    assert!(session.drain_events().iter().any(|event| matches!(event, Event::GraphicsChanged { .. })));

    // The first file that cannot be written stops the rest, and says why.
    let files = vec![
        bigstar_protocol::FileWrite { path: "elsewhere/new.bin".into(), bytes: vec![1] },
        bigstar_protocol::FileWrite { path: "z_new/c.bin".into(), bytes: vec![3] },
    ];
    assert_eq!(session.handle(Request::WriteFiles { files }).unwrap_err().message, "New NitroFS files must be under z_new/.");
    assert!(!project.join("modules/mine/nitrofs/z_new/c.bin").exists());
}

#[test]
fn a_removed_module_file_goes_to_the_projects_trash() {
    let (fixture, mut session) = with_module("host-fs-remove");
    let project = fixture.root.path().join("proj");
    // A module file the view knows about (it reads the module trees through the plan, which needs NCPatcher; without
    // one the ROM's own file cannot be removed).
    let error = session.handle(Request::RemoveFile { path: "course/A01_1.bin".into() }).unwrap_err();
    assert_eq!(error.message, "only a file a module supplies can be removed");
    let error = session.handle(Request::RemoveFile { path: "dat/Dat_Field.narc!J01_1.bin".into() }).unwrap_err();
    assert!(error.message.contains("there is no file") || error.message.contains("archive"), "{}", error.message);
    assert!(!project.join(super::host::TRASH_DIR).exists());
}

/// A host whose NCPatcher is a script printing `config` as `config dump`'s answer.
#[cfg(unix)]
fn with_config(label: &str, config: &str) -> (Fixture, HostSession) {
    use std::os::unix::fs::PermissionsExt;
    let (fixture, _) = Fixture::with_project(label);
    let answer = fixture.root.path().join("config.json");
    write_bytes(&answer, config.as_bytes());
    let script = fixture.root.path().join("ncpatcher.sh");
    let text = format!("#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = config ] && exec cat '{}'; done\nexit 3\n", answer.display());
    write_bytes(&script, text.as_bytes());
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let options = crate::HostOptions { ncpatcher: script, ..crate::HostOptions::default() };
    let mut session = Host::with_options(fixture.root.path(), options).unwrap().session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    (fixture, session)
}

#[test]
fn the_banner_needs_a_configured_file_to_go_to() {
    // Without NCPatcher's configuration there is nowhere for an edit to go, so the banner is not offered.
    let (_fixture, mut session) = Fixture::with_project("host-banner-none");
    let error = session.handle(Request::Banner).unwrap_err();
    assert!(error.message.contains("nowhere for an edited banner to go"), "{}", error.message);
}

#[cfg(unix)]
#[test]
fn the_banner_is_read_from_the_rom_until_the_configured_file_exists() {
    let config = r#"{"schema":"ncpatcher.config/1","version":2,"rom-banner":"","variants":{}}"#;
    let (_fixture, mut session) = with_config("host-banner-unconfigured", config);
    let error = session.handle(Request::Banner).unwrap_err();
    assert!(error.message.contains("configures no rom.banner"), "{}", error.message);

    let config = r#"{"schema":"ncpatcher.config/1","version":2,"rom-banner":"assets/banner.bin","variants":{}}"#;
    let (fixture, mut session) = with_config("host-banner", config);
    let Response::Banner { bytes, target } = session.handle(Request::Banner).unwrap() else { panic!("not a banner") };
    assert_eq!(target, "assets/banner.bin");
    assert_eq!(bytes.get(..3), Some(&[1, 0, 0xAB][..]), "the ROM's own banner");

    let mut edited = bytes.clone();
    edited[0x20] = 0x55;
    assert_eq!(session.handle(Request::WriteBanner { bytes: edited.clone() }).unwrap(), Response::Done);
    assert_eq!(std::fs::read(fixture.root.path().join("proj/assets/banner.bin")).unwrap(), edited);
    let Response::Banner { bytes, .. } = session.handle(Request::Banner).unwrap() else { panic!("not a banner") };
    assert_eq!(bytes, edited, "the configured file once it exists");
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

mod graphics {
    use std::time::{Duration, Instant};

    use bigstar_protocol::{ActorInput, DrawKind, Event, Request, Response, Service};

    use super::{Fixture, open_level};
    use crate::HostSession;

    fn write(path: &std::path::Path, text: impl AsRef<[u8]>) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// A 3x2 PNG the actor's `init` loads beside its script.
    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        let image = image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]));
        image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        bytes
    }

    const ACTOR: &str = r##"
local still
return {
    hash = 0x1234,
    name = "Project Actor",
    category = "Tests",
    fields = { { key = "size", label = "Size", bitOffset = 0, bitWidth = 4 } },
    init = function(ctx) still = ctx.loadImage("images/still.png") end,
    grabBounds = function(actor) return { x = actor.pixelX, y = actor.pixelY, width = 16 + actor.fields.size, height = 16 } end,
    render = function(actor, renderer)
        renderer:draw(still, { x = actor.pixelX, y = actor.pixelY })
        renderer:fillRect({ x = actor.pixelX, y = actor.pixelY, width = 4, height = 4, color = "#ff000080" })
        return true
    end,
}"##;

    fn wait_for(session: &mut HostSession, wanted: impl Fn(&Event) -> bool) -> Vec<Event> {
        let deadline = Instant::now() + Duration::from_secs(15);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            seen.extend(session.drain_events());
            if seen.iter().any(&wanted) {
                return seen;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("the event never came; saw {seen:?}");
    }

    #[test]
    fn actors_render_through_the_host_and_their_stills_arrive_by_hash() {
        let (fixture, mut session) = Fixture::with_project("host-actors");
        let project = fixture.root.path().join("proj");
        write(&project.join("bigstar/actors/custom.lua"), ACTOR);
        write(&project.join("bigstar/actors/images/still.png"), png());

        let Response::ActorCatalog(catalog) = session.handle(Request::ActorCatalog).unwrap() else { panic!("not a catalog") };
        // Every shipped actor, and the project's own in front of them.
        assert_eq!(catalog.actors.len(), 306);
        let custom = catalog.actors.iter().find(|actor| actor.definition.hash == Some(0x1234)).unwrap();
        assert!(custom.has_renderer && custom.init_error.is_none(), "{:?}", custom.init_error);
        assert_eq!(custom.definition.category, "Tests");
        // The synthetic ROM has no class table, so vanilla sprites have nothing to map to.
        assert!(catalog.sprite_object_ids.is_empty());

        let input = ActorInput { sprite_type: 0, hash: Some(0x1234), x: 2, y: 3, pixel_x: 32.0, pixel_y: 48.0, settings: 5, events: [0, 0] };
        let missing = ActorInput { hash: None, ..input.clone() };
        let Response::ActorDraws { generation, draws } = session.handle(Request::RenderActors { inputs: vec![input, missing] }).unwrap() else {
            panic!("not draws")
        };
        assert!(generation >= 1);
        assert_eq!(draws.len(), 2);
        let draw = &draws[0];
        assert!(draw.found && draw.custom_rendered && draw.error.is_none(), "{:?}", draw.error);
        assert_eq!(draw.grab.unwrap().width, 21, "the declared field reaches the bounds function");
        assert_eq!(draw.commands.len(), 2);
        assert_eq!(draw.commands[1].kind, DrawKind::FillRect);
        assert_eq!(draw.commands[1].color, 0xff00_0080);
        assert!(!draws[1].found, "a vanilla sprite with no class table has no actor");

        // The still travels by content hash, once.
        let texture = draw.commands[0].texture.clone().unwrap();
        let Response::Textures(textures) = session.handle(Request::GetTextures { ids: vec![texture.clone(), "nonsense".into()] }).unwrap() else {
            panic!("not textures")
        };
        assert_eq!(textures.len(), 1);
        assert_eq!((textures[0].width, textures[0].height), (3, 2));
        assert_eq!(&textures[0].rgba[..4], &[10, 20, 30, 255]);
        assert_eq!(textures[0].id, texture);
    }

    #[test]
    fn level_graphics_report_what_could_not_be_resolved() {
        let (_fixture, mut session) = Fixture::with_project("host-level-graphics");
        let snapshot = open_level(&mut session, "A01_1");
        assert_eq!(snapshot.sprite_hashes.len(), snapshot.course.sprites.len());
        let Response::LevelGraphics(graphics) = session.handle(Request::LevelGraphics { level: snapshot.level }).unwrap() else {
            panic!("not graphics")
        };
        // The synthetic ROM is not NSMB: the overlay holds no resource tables, which is an answer, not a failure.
        assert_eq!(graphics.level, snapshot.level);
        assert!(graphics.error.is_some());
        assert!(session.handle(Request::LevelGraphics { level: 999 }).is_err());
    }

    #[test]
    fn the_variant_persists_and_closes_level_sessions() {
        let (fixture, mut session) = Fixture::with_project("host-variant");
        let Response::Settings(settings) = session.handle(Request::ProjectSettings).unwrap() else { panic!("not settings") };
        assert!(settings.active_variant.is_empty());
        assert_eq!(settings.generated_path, "build/generated");
        assert!(settings.patcher_error.is_some(), "this project has no ncpatcher.yaml for NCPatcher to read");

        let snapshot = open_level(&mut session, "A01_1");
        session.drain_events();
        let Response::Settings(switched) = session.handle(Request::SetActiveVariant { variant: "fr".into() }).unwrap() else {
            panic!("not settings")
        };
        assert_eq!(switched.active_variant, "fr");
        let events = session.drain_events();
        assert!(events.iter().any(|event| matches!(event, Event::SettingsChanged(settings) if settings.active_variant == "fr")));
        assert!(events.iter().any(|event| matches!(event, Event::GraphicsChanged { .. })));
        // The old session read through the old variant's layers, so it is gone.
        assert!(session.handle(Request::CloseLevel { level: snapshot.level }).is_err());
        session.handle(Request::SetActiveModule { module: "coop".into() }).unwrap();

        // Another host on the same workspace finds the choice where it was left.
        let mut again = fixture.session();
        again.handle(Request::OpenProject { path: "proj".into() }).unwrap();
        let Response::Settings(restored) = again.handle(Request::ProjectSettings).unwrap() else { panic!("not settings") };
        assert_eq!((restored.active_variant.as_str(), restored.active_module.as_str()), ("fr", "coop"));
    }

    #[test]
    fn a_changed_script_makes_the_graphics_stale() {
        let (fixture, mut session) = Fixture::with_project("host-watch");
        let project = fixture.root.path().join("proj");
        let Response::ActorCatalog(before) = session.handle(Request::ActorCatalog).unwrap() else { panic!("not a catalog") };
        // Let the watcher settle so the import's own writes are not mistaken for this change.
        std::thread::sleep(Duration::from_millis(600));
        session.drain_events();

        write(&project.join("bigstar/actors/late.lua"), r#"return { hash = 0x99, name = "Late" }"#);
        let events = wait_for(&mut session, |event| matches!(event, Event::GraphicsChanged { .. }));
        assert!(events.iter().any(|event| matches!(event, Event::FilesChanged { paths } if paths.iter().any(|p| p.ends_with("late.lua")))));
        let generation =
            events.iter().find_map(|event| if let Event::GraphicsChanged { generation } = event { Some(*generation) } else { None }).unwrap();
        assert!(generation > before.generation);

        // The next answer is rebuilt from disk and carries the new generation.
        let Response::ActorCatalog(after) = session.handle(Request::ActorCatalog).unwrap() else { panic!("not a catalog") };
        assert!(after.generation >= generation);
        assert_eq!(after.actors.len(), before.actors.len() + 1);
    }
}

/// Events until one answers `done`, or none for five seconds.
fn events_until(session: &mut HostSession, done: impl Fn(&Event) -> bool) -> Vec<Event> {
    let start = std::time::Instant::now();
    let mut seen = Vec::new();
    while start.elapsed() < std::time::Duration::from_secs(5) {
        let events = session.drain_events();
        let finished = events.iter().any(&done);
        seen.extend(events);
        if finished {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    seen
}

#[test]
fn a_build_needs_a_yaml_config_and_a_legacy_one_is_offered_for_migration() {
    let (fixture, mut session) = Fixture::with_project("host-build-config");
    assert_eq!(session.handle(Request::BuildConfig).unwrap(), Response::BuildConfig(bigstar_protocol::BuildConfigKind::None));
    let error = session.handle(Request::Build { verbose: false }).unwrap_err();
    assert_eq!(error.message, "This project has no ncpatcher.yaml, so there is nothing for NCPatcher to build.");
    write_bytes(&fixture.root.path().join("proj/ncpatcher.json"), b"{}");
    assert_eq!(
        session.handle(Request::BuildConfig).unwrap(),
        Response::BuildConfig(bigstar_protocol::BuildConfigKind::Legacy("ncpatcher.json".into()))
    );
    assert!(session.handle(Request::Build { verbose: false }).unwrap_err().message.contains("legacy ncpatcher.json"));
    // Cancelling with nothing running is not an error.
    assert_eq!(session.handle(Request::CancelBuild).unwrap(), Response::Done);
}

#[cfg(unix)]
#[test]
fn a_build_streams_what_it_does_and_its_rom_becomes_the_projects() {
    use std::os::unix::fs::PermissionsExt;
    let (fixture, _) = Fixture::with_project("host-build");
    let project = fixture.root.path().join("proj");
    write_bytes(&project.join("ncpatcher.yaml"), b"version: 2\n");
    // Builds by copying the project's ROM to `--out`, after one progress line, one diagnostic and one log line.
    let script = fixture.root.path().join("ncpatcher.sh");
    let text = format!(
        r#"#!/bin/sh
out=""; prev=""; build=""
for a in "$@"; do [ "$prev" = --out ] && out="$a"; [ "$a" = build ] && build=1; prev="$a"; done
[ -z "$build" ] && exit 3
echo '{{"type":"progress","phase":"compile","current":1,"total":2,"item":"main.cpp"}}'
echo '{{"type":"diagnostic","level":"warning","code":"NCP0100","message":"unused variable","location":{{"file":"src/main.cpp","line":3,"col":5}}}}'
echo linking >&2
cp {rom} "$out"
"#,
        rom = bigstar_project::INPUT_ROM_FILE_NAME
    );
    write_bytes(&script, text.as_bytes());
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let options = crate::HostOptions { ncpatcher: script, ..crate::HostOptions::default() };
    let mut session = Host::with_options(fixture.root.path(), options).unwrap().session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let snapshot = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &snapshot, 7);
    assert!(moved.dirty);
    session.drain_events();

    assert_eq!(session.handle(Request::Build { verbose: false }).unwrap(), Response::Done);
    let events = events_until(&mut session, |event| matches!(event, Event::Build(bigstar_protocol::BuildEvent::Finished(_))));
    let build: Vec<&bigstar_protocol::BuildEvent> =
        events.iter().filter_map(|event| if let Event::Build(build) = event { Some(build) } else { None }).collect();
    assert!(matches!(build.first(), Some(bigstar_protocol::BuildEvent::Started { .. })));
    assert!(build.iter().any(|e| matches!(e, bigstar_protocol::BuildEvent::Progress { phase, total: 2, .. } if phase == "compile")));
    assert!(build.iter().any(|e| matches!(e, bigstar_protocol::BuildEvent::Diagnostic(d)
        if d.code == "NCP0100" && d.file == "src/main.cpp" && d.line == 3 && d.level == bigstar_protocol::DiagnosticLevel::Warning)));
    assert!(build.iter().any(|e| matches!(e, bigstar_protocol::BuildEvent::Log(line) if line == "linking")));
    let Some(bigstar_protocol::BuildEvent::Finished(outcome)) = build.last() else { panic!("{build:?}") };
    assert!(outcome.ok && outcome.applied, "{outcome:?}");
    // The edit was saved first, the built ROM is the project's now, and every level session closed to read it.
    assert!(project.join(bigstar_project::BUILT_ROM_FILE_NAME).is_file());
    assert!(events.iter().any(|event| matches!(event, Event::SettingsChanged(_))));
    assert!(session.handle(Request::CloseLevel { level: snapshot.level }).is_err());
    let reopened = open_level(&mut session, "A01_1");
    assert!(!reopened.dirty);
    assert_eq!(reopened.objects[0].x, moved.objects[0].x, "the saved edit");
}

/// A project with module `mine` whose NCPatcher (a script) answers `config dump`, `modules dump` and `files plan`
/// with the documents given, the plan built from `planned` (path, id, module source) entries.
#[cfg(unix)]
fn with_plan(label: &str, planned: &[(&str, i64, &str)]) -> (Fixture, HostSession) {
    use std::os::unix::fs::PermissionsExt;
    let (fixture, _) = Fixture::with_project(label);
    let project = fixture.root.path().join("proj");
    std::fs::create_dir_all(project.join("modules/mine/nitrofs")).unwrap();
    let modules = format!(
        r#"{{"schema":"ncpatcher.modules/1","dir":"{dir}/modules","modules":[{{"key":"mine","id":"Mine","name":"My module","enabled":true,
            "dir":"{dir}/modules/mine","nitrofs":{{"dir":"{dir}/modules/mine/nitrofs"}},"components":[]}}]}}"#,
        dir = project.display()
    );
    let files: Vec<String> = planned
        .iter()
        .map(|(path, id, source)| {
            format!(r#"{{"id":{id},"path":"{path}","size":3,"action":"modified","source":"{source}","module":"mine","component":"Vanilla"}}"#)
        })
        .collect();
    let plan = format!(r#"{{"schema":"ncpatcher.files/1","planned":true,"count":{},"files":[{}]}}"#, files.len(), files.join(","));
    let config = r#"{"schema":"ncpatcher.config/1","version":2,"variants":{}}"#;
    let answers = fixture.root.path().join("answers");
    for (name, text) in [("config", config.to_owned()), ("modules", modules), ("plan", plan)] {
        write_bytes(&answers.join(format!("{name}.json")), text.as_bytes());
    }
    let script = fixture.root.path().join("ncpatcher.sh");
    let text = format!(
        "#!/bin/sh\nfor a in \"$@\"; do case \"$a\" in config|modules|plan) exec cat '{}/'\"$a\".json;; esac; done\nexit 3\n",
        answers.display()
    );
    write_bytes(&script, text.as_bytes());
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let options = crate::HostOptions { ncpatcher: script, ..crate::HostOptions::default() };
    let mut session = Host::with_options(fixture.root.path(), options).unwrap().session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    (fixture, session)
}

#[cfg(unix)]
#[test]
fn module_changes_travel_as_a_patch_that_checks_file_ids() {
    let (_fixture, mut session) = with_plan("host-patch-none", &[]);
    assert_eq!(session.handle(Request::ExportPatch).unwrap_err().message, "There are no module changes to distribute.");

    let (fixture, mut session) = with_plan("host-patch", &[("course/A01_1.bin", 0, "modules/mine/nitrofs/course/A01_1.bin")]);
    let target = fixture.root.path().join("proj/modules/mine/nitrofs/course/A01_1.bin");
    write_bytes(&target, &[1, 2, 3]);
    let id = listing(&mut session).entries.iter().find(|e| e.path == "course/A01_1.bin").and_then(|e| e.id).unwrap();
    let Response::Patch { name, bytes } = session.handle(Request::ExportPatch).unwrap() else { panic!() };
    assert!(name.ends_with(".nsmbpatch"));
    let mut entries = bigstar_level::decode_nsmbpatch(&bytes).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!((entries[0].path.as_str(), entries[0].contents.as_slice()), ("course/A01_1.bin", &[1, 2, 3][..]));
    assert_eq!(i64::from(entries[0].expected_file_id), id);

    // A patch made against another ROM says so before anything is written.
    entries[0].expected_file_id += 1;
    entries[0].contents = vec![9];
    let other = bigstar_level::encode_nsmbpatch(&entries).unwrap();
    assert_eq!(session.handle(Request::ImportPatch { bytes: other.clone(), accept_id_mismatch: false }).unwrap(), Response::PatchIdsDiffer);
    assert_eq!(std::fs::read(&target).unwrap(), [1, 2, 3]);
    assert_eq!(session.handle(Request::ImportPatch { bytes: other, accept_id_mismatch: true }).unwrap(), Response::Done);
    assert_eq!(std::fs::read(&target).unwrap(), [9]);
}

#[cfg(unix)]
#[test]
fn xdelta_runs_on_paths_under_the_workspace() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new("host-xdelta");
    let root = fixture.root.path();
    let script = root.join("xdelta.sh");
    let text = format!("#!/bin/sh\necho \"$@\" > '{}/args'\n[ \"$1\" = -d ] && {{ echo 'bad patch' >&2; exit 1; }}\nexit 0\n", root.display());
    write_bytes(&script, text.as_bytes());
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let options = crate::HostOptions { xdelta: script, ..crate::HostOptions::default() };
    let mut session = Host::with_options(root, options).unwrap().session();
    let request = |create| Request::Xdelta { create, source: "a.nds".into(), target: "b.nds".into(), output: "out/p.xdelta".into() };
    assert_eq!(session.handle(request(true)).unwrap(), Response::Done);
    let args = std::fs::read_to_string(root.join("args")).unwrap();
    let real = std::fs::canonicalize(root).unwrap();
    assert_eq!(args.trim(), format!("-e -f -s {0}/a.nds {0}/b.nds {0}/out/p.xdelta", real.display()));
    assert_eq!(session.handle(request(false)).unwrap_err().message.trim(), "bad patch");
    let outside = Request::Xdelta { create: true, source: "../a.nds".into(), target: "b".into(), output: "c".into() };
    assert!(session.handle(outside).is_err(), "nothing outside the workspace");
}
