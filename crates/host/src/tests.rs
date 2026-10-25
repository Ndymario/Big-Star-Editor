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

/// An NCPatcher project at `root/name`: its `ncpatcher.yaml`, and `rom` as the clean ROM its settings name.
fn make_project(root: &std::path::Path, name: &str, rom: &[u8]) {
    let project = root.join(name);
    write_bytes(&project.join("ncpatcher.yaml"), b"version: 2\n");
    write_bytes(&project.join("rom.nds"), rom);
    write_bytes(&project.join(".big-star-workspace.json"), br#"{"clean_rom": "rom.nds"}"#);
}

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new(label: &str) -> Self {
        Self { root: TempDir::new(label) }
    }

    /// `proj/`, built from a clean ROM that holds 1-1 and the Field download-play area.
    fn with_project(label: &str) -> (Self, HostSession) {
        let fixture = Self::new(label);
        let (course, bgdat) = level_files();
        let narc = make_narc(&[("J01_1.bin", &course), ("J01_1_bgdat.bin", &bgdat)]);
        let rom = make_rom(
            &[RomFile::new("course/A01_1.bin", &course), RomFile::new("course/A01_1_bgdat.bin", &bgdat), RomFile::new("dat/Dat_Field.narc", &narc)],
            &[0; 32],
        );
        write_bytes(&fixture.root.path().join("roms/game.nds"), &rom);
        make_project(fixture.root.path(), "proj", &rom);
        let mut session = Host::new(fixture.root.path()).unwrap().session();
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
    for request in
        [Request::BrowseDirectory { path: "..".into() }, Request::OpenProject { path: "/tmp".into() }, Request::OpenProject { path: "../p".into() }]
    {
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
fn ncpatcher_projects_are_listed_and_opened() {
    let (fixture, mut session) = Fixture::with_project("host-projects");
    // A legacy configuration is a project too, to be offered migration; a manifest of Big Star's own is not.
    write_bytes(&fixture.root.path().join("old/one/ncpatcher.json"), b"{}");
    write_bytes(&fixture.root.path().join("retired/big-star-project.yaml"), b"format: big-star-editor-project\nversion: 3\nname: \"Old\"\n");

    let Response::Projects(projects) = session.handle(Request::ListProjects).unwrap() else { panic!() };
    let listed: Vec<(&str, &str)> = projects.iter().map(|p| (p.path.as_str(), p.name.as_str())).collect();
    assert_eq!(listed, [("old/one", "one"), ("proj", "proj")]);
    assert_eq!(session.handle(Request::OpenProject { path: "retired".into() }).unwrap_err().code, ErrorCode::NotFound);

    let Response::Project(info) = session.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!((info.name.as_str(), info.rom_path.as_deref()), ("proj", Some("rom.nds")));
    assert_eq!(info.file_count, 3);
    assert_eq!(info.mode, bigstar_protocol::ProjectMode::NcPatcher);
    assert!(info.game_code.is_some());
    let Response::Settings(settings) = session.handle(Request::ProjectSettings).unwrap() else { panic!() };
    assert_eq!((settings.clean_rom.as_str(), settings.clean_rom_problem.as_deref()), ("rom.nds", None));

    // A project with no clean ROM opens, has no levels, and says what it lacks.
    session.handle(Request::OpenProject { path: "old/one".into() }).unwrap();
    let Response::Settings(settings) = session.handle(Request::ProjectSettings).unwrap() else { panic!() };
    assert!(settings.clean_rom_problem.unwrap().contains("no clean ROM"));
    let Response::Levels(worlds) = session.handle(Request::ListLevels).unwrap() else { panic!() };
    assert!(worlds.iter().flat_map(|w| &w.levels).flat_map(|l| &l.areas).all(|a| !a.available));
    assert_eq!(session.handle(Request::OpenLevel { area: "A01_1".into() }).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn the_clean_rom_is_asked_for_and_remembered() {
    let fixture = Fixture::new("host-clean-rom");
    let rom = make_rom(&[RomFile::new("course/A01_1.bin", b"x")], &[0; 32]);
    write_bytes(&fixture.root.path().join("roms/game.nds"), &rom);
    write_bytes(&fixture.root.path().join("roms/junk.nds"), b"not a ROM");
    write_bytes(&fixture.root.path().join("proj/ncpatcher.yaml"), b"version: 2\n");
    // Named, but not there: opening says so rather than refusing the project.
    write_bytes(&fixture.root.path().join("proj/.big-star-workspace.json"), br#"{"clean_rom": "gone.nds"}"#);
    let mut session = fixture.session();
    let Response::Project(info) = session.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!(info.rom_path, None);
    let Response::Settings(settings) = session.handle(Request::ProjectSettings).unwrap() else { panic!() };
    assert!(settings.clean_rom_problem.unwrap().contains("gone.nds"));
    assert!(session.handle(Request::Build { verbose: false }).unwrap_err().message.contains("Choose one before building"));

    assert_eq!(session.handle(Request::SetCleanRom { path: "roms/junk.nds".into() }).unwrap_err().code, ErrorCode::InvalidArgument);
    assert_eq!(session.handle(Request::SetCleanRom { path: "../game.nds".into() }).unwrap_err().code, ErrorCode::InvalidArgument);
    let Response::Settings(settings) = session.handle(Request::SetCleanRom { path: "roms/game.nds".into() }).unwrap() else { panic!() };
    assert_eq!(settings.clean_rom_problem, None);
    // Outside the project it is kept as it is; inside, relative to the project.
    let stored = fixture.root.path().join("roms/game.nds").to_string_lossy().replace('\\', "/");
    assert_eq!(settings.clean_rom, stored);
    let events = session.drain_events();
    assert!(events.iter().any(|event| matches!(event, Event::SettingsChanged(s) if s.clean_rom == stored)));
    // Every frontend hears that the project reads a ROM now, not only the one that chose it.
    assert!(events.iter().any(|event| matches!(event, Event::ProjectChanged(info)
        if info.rom_path.as_deref() == Some(stored.as_str()) && info.file_count == 1)));
    // Before a first build it is what the editor reads.
    let Response::Project(info) = session.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!((info.rom_path.as_deref(), info.file_count), (Some(stored.as_str()), 1));

    // Remembered for the next session, which finds nothing wrong with it.
    let mut again = fixture.session();
    let Response::Project(info) = again.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!(info.game_code.as_deref(), Some("A2DE"));
    assert_eq!(crate::stored_clean_rom(&fixture.root.path().join("proj")), Some(fixture.root.path().join("roms/game.nds")));
}

#[test]
fn only_the_desktop_host_takes_a_clean_rom_outside_the_workspace() {
    let outside = TempDir::new("host-clean-rom-outside");
    let rom = outside.path().join("game.nds");
    write_bytes(&rom, &make_rom(&[RomFile::new("course/A01_1.bin", b"x")], &[0; 32]));
    let rom = rom.to_string_lossy().into_owned();
    let fixture = Fixture::new("host-clean-rom-desktop");
    write_bytes(&fixture.root.path().join("proj/ncpatcher.yaml"), b"version: 2\n");

    let mut served = fixture.session();
    served.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    assert_eq!(served.handle(Request::SetCleanRom { path: rom.clone() }).unwrap_err().code, ErrorCode::InvalidArgument);
    drop(served);

    let mut desktop = Host::for_desktop(fixture.root.path()).unwrap().session();
    desktop.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let Response::Settings(settings) = desktop.handle(Request::SetCleanRom { path: rom.clone() }).unwrap() else { panic!() };
    assert_eq!((settings.clean_rom.as_str(), settings.clean_rom_problem), (rom.replace('\\', "/").as_str(), None));
    // Even a served host builds from it once the settings name it.
    let mut again = fixture.session();
    let Response::Project(info) = again.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!(info.file_count, 1);
}

/// A project whose settings name no clean ROM takes the one ncpatcher.yaml names, and keeps it.
#[cfg(unix)]
#[test]
fn the_clean_rom_ncpatcher_yaml_names_is_taken() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new("host-clean-rom-yaml");
    let project = fixture.root.path().join("proj");
    write_bytes(&project.join("ncpatcher.yaml"), b"version: 2\nrom:\n  file: rom.nds\n");
    write_bytes(&project.join("rom.nds"), &make_rom(&[RomFile::new("course/A01_1.bin", b"x")], &[0; 32]));
    let answer = fixture.root.path().join("config.json");
    let config = format!(r#"{{"schema":"ncpatcher.config/1","version":2,"rom-file":"{}","variants":{{}}}}"#, project.join("rom.nds").display());
    write_bytes(&answer, config.as_bytes());
    let script = fixture.root.path().join("ncpatcher.sh");
    let text = format!("#!/bin/sh\nfor a in \"$@\"; do [ \"$a\" = config ] && exec cat '{}'; done\nexit 3\n", answer.display());
    write_bytes(&script, text.as_bytes());
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let options = crate::HostOptions { ncpatcher: script, ..crate::HostOptions::default() };
    let mut session = Host::with_options(fixture.root.path(), options).unwrap().session();

    let Response::Project(info) = session.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!((info.rom_path.as_deref(), info.file_count), (Some("rom.nds"), 1));
    let Response::Settings(settings) = session.handle(Request::ProjectSettings).unwrap() else { panic!() };
    assert_eq!((settings.clean_rom.as_str(), settings.clean_rom_problem), ("rom.nds", None));
    assert_eq!(crate::stored_clean_rom(&project), Some(project.join("rom.nds")));
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
    let (fixture, mut session) = with_module("host-edit");
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
    // Into the module, never the ROM: the ROM's copy is what it was.
    let module = fixture.root.path().join("proj/modules/mine/nitrofs");
    assert!(module.join("course/A01_1.bin").is_file());
    assert!(module.join("course/A01_1_bgdat.bin").is_file());
    let Response::FileBytes { bytes, .. } = session.handle(Request::ReadFile { path: "course/A01_1.bin".into() }).unwrap() else { panic!() };
    assert_eq!(bytes, level_files().0, "without NCPatcher's plan, reads are the ROM's");
}

#[test]
fn a_level_is_not_saved_without_a_module_to_save_into() {
    let (_fixture, mut session) = Fixture::with_project("host-edit-no-module");
    let opened = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &opened, 40);
    let error = session.handle(Request::SaveLevel { level: moved.level }).unwrap_err();
    assert_eq!(error.message, "choose an active module in the Filesystem dock before saving");
    assert!(open_level(&mut session, "A01_1").dirty, "the edit is still there to save");
}

/// A level saved into the module reads back, in a fresh host, through the plan NCPatcher makes of it.
#[cfg(unix)]
#[test]
fn a_saved_level_reads_back_through_the_build_plan() {
    let sources = [
        ("course/A01_1.bin", 0, "modules/mine/nitrofs/course/A01_1.bin"),
        ("course/A01_1_bgdat.bin", 1, "modules/mine/nitrofs/course/A01_1_bgdat.bin"),
    ];
    let (fixture, mut session) = with_plan("host-edit-plan", &sources);
    let opened = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &opened, 40);
    session.handle(Request::SaveLevel { level: moved.level }).unwrap();
    let options = crate::HostOptions { ncpatcher: fixture.root.path().join("ncpatcher.sh"), ..crate::HostOptions::default() };
    let mut other = Host::with_options(fixture.root.path(), options).unwrap().session();
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
    make_project(fixture.root.path(), "proj", &std::fs::read(rom).unwrap());
    let mut session = fixture.session();
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
    make_project(fixture.root.path(), "proj", &std::fs::read(rom).unwrap());
    let mut session = fixture.session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let snapshot = open_level(&mut session, "A01_1");
    let Response::TilesetResources(resources) = session.handle(Request::TilesetResources { level: snapshot.level }).unwrap() else { panic!() };
    assert_eq!(resources.tilesets.len(), 3);
    // Slot 0 is jyotyu, shared by every level; the level's own is in slot 1.
    assert_eq!(resources.tilesets[0].graphics.rom_path, "BG_ncg/d_2d_A_J_jyotyu_ncg.bin");
    assert!(resources.tilesets.iter().all(|slot| !slot.map16.bytes.is_empty()));
    assert!(!resources.background.screen.bytes.is_empty());
    assert_eq!(resources.references, snapshot.tileset_references);
    // 1-1 is grassland: its header names tileset 8, which the game's table makes the nohara set.
    assert_eq!(resources.tilesets[1].map16.rom_path, "BG_pnl/d_2d_PA_I_M_nohara.bin", "1-1's own tileset, in slot 1");
    assert_eq!(resources.tilesets[2].map16.rom_path, "BG_pnl/d_2d_PA_I_S_nohara.bin");
    assert_eq!(resources.tilesets[0].palette.rom_path, "BG_ncl/d_2d_A_J_jyotyu_ncl.bin", "grassland's blocks are the usual colours");
    let grassland_colours = resources.tilesets[1].palette.rom_path.clone();

    // A ghost house's graphics and background with the tileset block still naming grassland, as a level whose header
    // alone was changed has: the shared blocks turn ghost-house grey, the tileset keeps grassland's colours.
    let mut references = snapshot.tileset_references;
    (references.tileset_id, references.background_graphics_id, references.background_screen_id, references.background_palette_id) = (23, 23, 23, 23);
    let edit = LevelEdit::SetTilesetReferences { references };
    edited(session.handle(Request::EditLevel { level: snapshot.level, base_revision: snapshot.revision, edit }).unwrap());
    let Response::TilesetResources(resources) = session.handle(Request::TilesetResources { level: snapshot.level }).unwrap() else { panic!() };
    assert_eq!(resources.tilesets[0].palette.rom_path, "BG_ncl/d_2d_A_J_jyotyu_W_ncl.bin");
    assert_eq!(resources.tilesets[1].palette.rom_path, grassland_colours);
    assert_ne!(resources.tilesets[1].map16.rom_path, "BG_pnl/d_2d_PA_I_M_nohara.bin");
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
    let (fixture, mut session) = with_module("host-recovery");
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
    let (fixture, mut session) = Fixture::with_project(label);
    let project = fixture.root.path().join("proj");
    let modules = format!(
        r#"{{"schema":"ncpatcher.modules/1","dir":"{dir}/modules","modules":[{{"key":"mine","id":"Mine","name":"My module","enabled":true,
            "dir":"{dir}/modules/mine","nitrofs":{{"dir":"{dir}/modules/mine/nitrofs"}},"components":[]}}]}}"#,
        dir = project.display()
    );
    write_bytes(&project.join("build/generated/modules.json"), modules.as_bytes());
    std::fs::create_dir_all(project.join("modules/mine/nitrofs")).unwrap();
    // Opening a project asks NCPatcher about it, so it opens again to read the graph written here.
    session.handle(Request::CloseProject).unwrap();
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
    let (fixture, mut session) = with_module("host-narc");
    let opened = open_level(&mut session, "J01_1@Dat_Field");
    assert_eq!(opened.objects.len(), 2);
    let moved = move_first_object(&mut session, &opened, 12);
    session.handle(Request::SaveLevel { level: moved.level }).unwrap();
    // A member is saved beside its archive, for the next build to sweep back into the container.
    assert!(fixture.root.path().join("proj/modules/mine/nitrofs/dat/Dat_Field_narc/J01_1.bin").is_file());
    assert_eq!(session.handle(Request::OpenLevel { area: "J01_1".into() }).unwrap_err().code, ErrorCode::NotFound);
    assert_eq!(session.handle(Request::OpenLevel { area: "nope".into() }).unwrap_err().code, ErrorCode::NotFound);
}

#[test]
fn a_project_with_unsaved_levels_stays_open() {
    let (fixture, mut session) = Fixture::with_project("host-dirty");
    make_project(fixture.root.path(), "elsewhere", &std::fs::read(fixture.root.path().join("roms/game.nds")).unwrap());
    let opened = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &opened, 3);
    assert_eq!(session.handle(Request::CloseProject).unwrap_err().code, ErrorCode::InvalidArgument);
    assert_eq!(session.handle(Request::OpenProject { path: "elsewhere".into() }).unwrap_err().code, ErrorCode::InvalidArgument);
    // Opening the open project joins it, as another frontend does: the unsaved edit is still there.
    assert!(matches!(session.handle(Request::OpenProject { path: "proj".into() }).unwrap(), Response::Project(_)));
    assert!(open_level(&mut session, "A01_1").dirty);
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

    /// An image already held frees its duplicate, and the allocator hands that address to the next image.
    #[test]
    fn a_texture_is_never_named_by_a_freed_images_address() {
        use std::sync::Arc;

        use bigstar_formats::RgbaImage;

        let image = |shade: u8| RgbaImage { width: 1, height: 1, pixels: vec![shade; 4] };
        let mut store = crate::graphics::TextureStore::default();
        let first = store.add_owned(image(1));
        for shade in 2..=64 {
            assert_eq!(store.add_owned(image(1)), first);
            assert_eq!(store.add(Arc::new(image(1))), first);
            let id = store.add_owned(image(shade));
            assert_ne!(id, first);
            assert_eq!(store.get(&id).unwrap().rgba, vec![shade; 4]);
            let id = store.add(Arc::new(image(shade)));
            assert_eq!(store.get(&id).unwrap().rgba, vec![shade; 4]);
        }
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
    fn a_declared_actor_without_a_script_is_still_in_the_catalog() {
        let (fixture, _) = Fixture::with_project("host-declared-actors");
        let project = fixture.root.path().join("proj");
        write(
            &project.join("build/generated/stageobjects.json"),
            r#"{"schema":"nsmbtool.stageobjects/1","stageObjectIdBase":326,"stageObjects":[
                {"name":"coop.CoopFlagActor.Default","hash":"0xf46a44b9","module":"coop","component":"CustomWorldUnlock","object":"CoopFlagActor","variant":"Default","objectId":387,"info":"CoopFlagActor::ObjectInfo_Default","header":"coop_flag.hpp"}
            ]}"#,
        );
        // The catalog is read when the project opens.
        let mut session = fixture.session();
        session.handle(Request::OpenProject { path: "proj".into() }).unwrap();

        let Response::ActorCatalog(catalog) = session.handle(Request::ActorCatalog).unwrap() else { panic!("not a catalog") };
        let declared = catalog.actors.iter().find(|actor| actor.definition.hash == Some(0xf46a_44b9)).expect("the declared actor is listed");
        assert_eq!(declared.definition.name, "coop.CoopFlagActor.Default");
        assert_eq!(declared.definition.object_id, 387);
        assert!(!declared.has_renderer, "nothing draws it, so it keeps the stock marker");
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
        // The directory exists before the script does: a watch reaches a new directory only after it appears, so a file
        // written into one at once may be reported only as its directory.
        std::fs::create_dir_all(project.join("bigstar/actors")).unwrap();
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
    assert_eq!(session.handle(Request::BuildConfig).unwrap(), Response::BuildConfig(bigstar_protocol::BuildConfigKind::Yaml));
    std::fs::remove_file(fixture.root.path().join("proj/ncpatcher.yaml")).unwrap();
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
    let (fixture, _) = with_module("host-build");
    let project = fixture.root.path().join("proj");
    // Says its builds go to `build/rom.nds`, and builds by copying the clean ROM it is handed to `--out`, after one
    // progress line, one diagnostic and one log line.
    let script = fixture.root.path().join("ncpatcher.sh");
    let text = format!(
        r#"#!/bin/sh
out=""; rom=""; prev=""; build=""; config=""
for a in "$@"; do
  [ "$prev" = --out ] && out="$a"; [ "$prev" = --rom ] && rom="$a"
  [ "$a" = build ] && build=1; [ "$a" = config ] && config=1; prev="$a"
done
if [ -n "$config" ]; then
  echo '{{"schema":"ncpatcher.config/1","version":2,"rom-file":"'"$rom"'","rom-output":"{output}","variants":{{}}}}'
  exit 0
fi
[ -z "$build" ] && exit 3
echo '{{"type":"progress","phase":"compile","current":1,"total":2,"item":"main.cpp"}}'
echo '{{"type":"diagnostic","level":"warning","code":"NCP0100","message":"unused variable","location":{{"file":"src/main.cpp","line":3,"col":5}}}}'
echo linking >&2
cp "$rom" "$out"
"#,
        output = project.join("build/rom.nds").display()
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
    // The edit was saved first, the built ROM is where ncpatcher.yaml sends it and is the project's now, and every
    // level session closed to read it.
    assert!(project.join("build/rom.nds").is_file());
    let Response::Project(info) = session.handle(Request::OpenProject { path: "proj".into() }).unwrap() else { panic!() };
    assert_eq!(info.rom_path.as_deref(), Some("build/rom.nds"));
    assert!(events.iter().any(|event| matches!(event, Event::SettingsChanged(_))));
    assert!(session.handle(Request::CloseLevel { level: snapshot.level }).is_err());
    // The edit went into the module before the build; this stand-in does not build it in, NCPatcher does.
    assert!(project.join("modules/mine/nitrofs/course/A01_1.bin").is_file());
    assert!(!open_level(&mut session, "A01_1").dirty);
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

#[test]
fn preferences_persist_and_saves_keep_that_many_backups() {
    let (fixture, mut session) = with_module("host-preferences");
    let project = fixture.root.path().join("proj");
    let Response::Settings(settings) = session.handle(Request::ProjectSettings).unwrap() else { panic!() };
    assert_eq!((settings.autosave_minutes, settings.backups_kept), (1, 10), "the defaults");
    let Response::Settings(settings) = session.handle(Request::SetPreferences { autosave_minutes: 90, backups_kept: 2 }).unwrap() else { panic!() };
    assert_eq!((settings.autosave_minutes, settings.backups_kept), (60, 2), "clamped");

    // Each save after the first backs up what it overwrites; only the newest two backups of the kind stay.
    let mut snapshot = open_level(&mut session, "A01_1");
    for x in 1..=4 {
        snapshot = move_first_object(&mut session, &snapshot, x);
        let Response::Edited { snapshot: saved, .. } = session.handle(Request::SaveLevel { level: snapshot.level }).unwrap() else { panic!() };
        snapshot = *saved;
    }
    let backups: Vec<String> = std::fs::read_dir(project.join(".big-star-backups"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(backups.len(), 2, "{backups:?}");
    assert!(backups.iter().all(|name| name.starts_with("levels-")));

    // Another host on the project finds the preferences where they were left.
    let mut again = fixture.session();
    again.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let Response::Settings(restored) = again.handle(Request::ProjectSettings).unwrap() else { panic!() };
    assert_eq!((restored.autosave_minutes, restored.backups_kept), (60, 2));
}

/// Background and foreground screens name tiles and palette rows from where the game loads each layer; read from
/// zero, every lookup missed and both layers came out blank. A level without a foreground names it 0xFFFF.
#[test]
fn real_backgrounds_and_foregrounds_draw() {
    let Some(rom) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let fixture = Fixture::new("host-real-backgrounds");
    make_project(fixture.root.path(), "proj", &std::fs::read(rom).unwrap());
    let mut session = fixture.session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let drawn = |session: &mut HostSession, area: &str| {
        let level = open_level(session, area).level;
        let Response::LevelGraphics(graphics) = session.handle(Request::LevelGraphics { level }).unwrap() else { panic!() };
        let ids: Vec<String> = [&graphics.background, &graphics.foreground].iter().filter_map(|layer| layer.texture.clone()).collect();
        let Response::Textures(textures) = session.handle(Request::GetTextures { ids }).unwrap() else { panic!() };
        let opaque =
            |texture: &bigstar_protocol::TextureData| texture.rgba.chunks(4).filter(|pixel| pixel[3] != 0).count() * 100 / (texture.rgba.len() / 4);
        (graphics, textures.iter().map(opaque).collect::<Vec<_>>())
    };
    let (graphics, opaque) = drawn(&mut session, "A01_1");
    assert!(graphics.background.error.is_none() && graphics.foreground.error.is_none(), "{graphics:?}");
    // The shared tileset's question blocks say what they hold, so the canvas can show it.
    let held: std::collections::HashSet<u8> = graphics.tilesets[0]
        .behaviors
        .iter()
        .map(|&b| bigstar_formats::decode_tile_behavior(b))
        .filter(|parts| parts.flags & 4 != 0)
        .map(|parts| parts.parameter)
        .collect();
    // A mushroom or flower, a star and a coin.
    for parameter in [0, 1, 7] {
        assert!(held.contains(&parameter), "{parameter} in {held:?}");
    }
    assert_eq!(opaque.len(), 2);
    assert_eq!(opaque[0], 100, "the sky fills the background");
    assert!((10..100).contains(&opaque[1]), "the hills stand over a see-through foreground: {}%", opaque[1]);
    // 1-1's bonus room names no foreground: nothing is drawn for it, and nothing is wrong.
    let (graphics, _) = drawn(&mut session, "A01_3");
    assert!(graphics.background.texture.is_some());
    assert_eq!((graphics.foreground.texture.as_ref(), graphics.foreground.error.as_ref()), (None, None));
}

/// The shipped background scripts against a real ROM: every layer has a name, and the ones the game animates come
/// back as the frames it plays, each a picture the host can hand over.
#[test]
fn real_backgrounds_are_named_and_animated() {
    use bigstar_level::BackgroundLayer;
    let Some(rom) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let fixture = Fixture::new("host-real-background-scripts");
    make_project(fixture.root.path(), "proj", &std::fs::read(rom).unwrap());
    let mut session = fixture.session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let Response::Levels(worlds) = session.handle(Request::ListLevels).unwrap() else { panic!() };
    let areas: Vec<bigstar_protocol::AreaSummary> = worlds.into_iter().flat_map(|world| world.levels).flat_map(|level| level.areas).collect();
    let (mut animated, mut seen) = (std::collections::BTreeMap::new(), 0);
    for area in areas.iter().filter(|area| area.available) {
        let Ok(Response::Level(snapshot)) = session.handle(Request::OpenLevel { area: area.token.clone() }) else { continue };
        let Response::LevelGraphics(graphics) = session.handle(Request::LevelGraphics { level: snapshot.level }).unwrap() else { panic!() };
        seen += 1;
        assert_eq!(graphics.backgrounds.definitions.len(), 152);
        let references = snapshot.tileset_references;
        for (layer, image, screen) in [
            (BackgroundLayer::Background, &graphics.background, references.background_screen_id),
            (BackgroundLayer::Foreground, &graphics.foreground, references.foreground_screen_id),
        ] {
            assert!(image.error.is_none(), "{} {layer:?}: {:?}", area.course_path, image.error);
            if image.texture.is_none() {
                continue;
            }
            assert!(
                graphics.backgrounds.name(layer, screen).is_some_and(|name| !name.starts_with("Empty")),
                "{} {layer:?} {screen}",
                area.course_path
            );
            if image.frames.is_empty() {
                continue;
            }
            // The first step of every animation the game ships is the picture the files already hold.
            assert_eq!(image.frames.first().map(|frame| &frame.texture), image.texture.as_ref(), "{} {layer:?} {screen}", area.course_path);
            let ids: Vec<String> = image.frames.iter().map(|frame| frame.texture.clone()).collect();
            let Response::Textures(textures) = session.handle(Request::GetTextures { ids: ids.clone() }).unwrap() else { panic!() };
            assert!(textures.iter().all(|texture| (texture.width, texture.height) == (512, 512)));
            let distinct: std::collections::BTreeSet<&String> = ids.iter().collect();
            let ticks: u32 = image.frames.iter().map(|frame| frame.ticks).sum();
            animated.entry((layer, screen)).or_insert((distinct.len(), image.frames.len(), ticks));
        }
        session.handle(Request::CloseLevel { level: snapshot.level }).unwrap();
    }
    assert!(seen > 150, "{seen}");
    println!("{animated:#?}");
    // A castle animates eight pictures of its foreground, five frames each. The volcano plays 40 steps over 448
    // frames, but its six source frames hold only two different pictures.
    assert_eq!(animated.get(&(BackgroundLayer::Foreground, 1)), Some(&(8, 8, 40)));
    assert_eq!(animated.get(&(BackgroundLayer::Foreground, 15)), Some(&(2, 40, 448)));
    assert_eq!(animated.get(&(BackgroundLayer::Background, 7)), Some(&(8, 8, 128)));
}

/// A ROM holding 1-1 and the Field download-play area.
fn small_rom() -> Vec<u8> {
    let (course, bgdat) = level_files();
    let narc = make_narc(&[("J01_1.bin", &course), ("J01_1_bgdat.bin", &bgdat)]);
    make_rom(
        &[RomFile::new("course/A01_1.bin", &course), RomFile::new("course/A01_1_bgdat.bin", &bgdat), RomFile::new("dat/Dat_Field.narc", &narc)],
        &[0; 32],
    )
}

impl Fixture {
    /// The full path of `relative` under the fixture, as a file dialog would give it.
    fn full(&self, relative: &str) -> String {
        self.root.path().join(relative).to_string_lossy().into_owned()
    }

    /// A session on the desktop app's host for ROMs on this machine, which has no workspace.
    fn rom_session(&self) -> HostSession {
        Host::for_local_roms(crate::HostOptions::default()).session()
    }
}

/// `roms/game.nds`, the small ROM, opened to edit directly by its full path.
fn with_rom(label: &str) -> (Fixture, HostSession, Vec<u8>) {
    let fixture = Fixture::new(label);
    let rom = small_rom();
    write_bytes(&fixture.root.path().join("roms/game.nds"), &rom);
    let mut session = fixture.rom_session();
    match session.handle(Request::OpenRom { path: fixture.full("roms/game.nds") }).unwrap() {
        Response::Project(info) => {
            assert_eq!(info.mode, bigstar_protocol::ProjectMode::Rom);
            assert_eq!((info.path, info.name.as_str()), (fixture.full("roms/game.nds"), "game.nds"));
        }
        other => panic!("unexpected {other:?}"),
    }
    (fixture, session, rom)
}

fn rom_file(fixture: &Fixture, path: &str, inside: &str) -> Vec<u8> {
    rom_member(std::fs::read(fixture.root.path().join(path)).unwrap(), inside)
}

fn rom_member(rom: Vec<u8>, inside: &str) -> Vec<u8> {
    let mut reader = bigstar_project::RomReader::from_bytes("rom.nds", rom.into()).unwrap();
    match inside.split_once('!') {
        Some((container, member)) => reader.read_member(container, member).unwrap(),
        None => reader.read(inside).unwrap(),
    }
}

#[test]
fn a_rom_edited_directly_saves_levels_into_itself() {
    let (fixture, mut session, original) = with_rom("host-rom-level");
    let snapshot = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &snapshot, 42);
    // Nothing touches the file until a save.
    assert_eq!(std::fs::read(fixture.root.path().join("roms/game.nds")).unwrap(), original);

    let (saved, _) = edited(session.handle(Request::SaveLevel { level: moved.level }).unwrap());
    assert!(!saved.dirty);
    let course = rom_file(&fixture, "roms/game.nds", "course/A01_1.bin");
    let bgdat = rom_file(&fixture, "roms/game.nds", "course/A01_1_bgdat.bin");
    assert_eq!(EditableLevel::load(&course, &bgdat, None).unwrap().objects()[0].x, 42);
    // The first save kept the ROM it overwrote, untouched.
    let backups: Vec<_> = std::fs::read_dir(fixture.root.path().join("roms/.big-star-backups")).unwrap().flatten().collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(std::fs::read(backups[0].path().join("game.nds")).unwrap(), original);

    // A second save rebuilds from the original again, so the file does not grow with every save.
    let again = move_first_object(&mut session, &saved, 7);
    let (saved, _) = edited(session.handle(Request::SaveLevel { level: again.level }).unwrap());
    let size = std::fs::metadata(fixture.root.path().join("roms/game.nds")).unwrap().len();
    let third = move_first_object(&mut session, &saved, 9);
    session.handle(Request::SaveLevel { level: third.level }).unwrap();
    assert_eq!(std::fs::metadata(fixture.root.path().join("roms/game.nds")).unwrap().len(), size);
    assert_eq!(std::fs::read_dir(fixture.root.path().join("roms/.big-star-backups")).unwrap().count(), 1);

    // Reopened from disk, the level is what was saved.
    let mut fresh = fixture.rom_session();
    fresh.handle(Request::OpenRom { path: fixture.full("roms/game.nds") }).unwrap();
    assert_eq!(open_level(&mut fresh, "A01_1").objects[0].x, 9);
}

#[test]
fn a_rom_edited_directly_replaces_files_and_members_but_adds_none() {
    let (fixture, mut session, _) = with_rom("host-rom-files");
    session.handle(Request::WriteFile { path: "dat/Dat_Field.narc!J01_1_bgdat.bin".into(), bytes: b"MEMBER".to_vec() }).unwrap();
    assert_eq!(rom_file(&fixture, "roms/game.nds", "dat/Dat_Field.narc!J01_1_bgdat.bin"), b"MEMBER");
    // The edit is what the host reads back, and what the ROM's other member still is.
    match session.handle(Request::ReadFile { path: "dat/Dat_Field.narc!J01_1_bgdat.bin".into() }).unwrap() {
        Response::FileBytes { bytes, .. } => assert_eq!(bytes, b"MEMBER"),
        other => panic!("unexpected {other:?}"),
    }
    let (course, _) = level_files();
    assert_eq!(rom_file(&fixture, "roms/game.nds", "dat/Dat_Field.narc!J01_1.bin"), course);

    let refused = session.handle(Request::WriteFile { path: "z_new/fresh.bin".into(), bytes: vec![1] }).unwrap_err();
    assert!(refused.message.contains("files can be replaced but not added"), "{}", refused.message);
    // A refused write leaves nothing behind to fail the next one.
    session.handle(Request::WriteFile { path: "course/A01_1.bin".into(), bytes: course.clone() }).unwrap();
}

#[test]
fn a_rom_edited_directly_refuses_what_needs_an_ncpatcher_project() {
    let (_fixture, mut session, _) = with_rom("host-rom-refusals");
    for request in [
        Request::Build { verbose: false },
        Request::SetActiveVariant { variant: "fr".into() },
        Request::SetActiveModule { module: "coop".into() },
        Request::RemoveFile { path: "course/A01_1.bin".into() },
    ] {
        let error = session.handle(request.clone()).unwrap_err();
        assert!(error.message.contains("editing a ROM directly"), "{request:?}: {}", error.message);
    }
    match session.handle(Request::ProjectSettings).unwrap() {
        Response::Settings(settings) => assert!(settings.variants.is_empty() && settings.modules.is_empty() && settings.patcher_error.is_none()),
        other => panic!("unexpected {other:?}"),
    }
    assert!(matches!(session.handle(Request::FileTargets { path: "course/A01_1.bin".into() }).unwrap(), Response::FileTargets(t) if t.is_empty()));
    assert!(matches!(session.handle(Request::Filesystem).unwrap(), Response::Filesystem(listing) if !listing.entries.is_empty()));
    // There is no workspace behind a ROM on this machine.
    for request in [Request::ListProjects, Request::BrowseDirectory { path: String::new() }, Request::OpenProject { path: "proj".into() }] {
        let error = session.handle(request.clone()).unwrap_err();
        assert!(error.message.contains("no workspace"), "{request:?}: {}", error.message);
    }
}

#[test]
fn a_rom_saved_as_another_takes_later_saves_with_it() {
    let (fixture, mut session, original) = with_rom("host-rom-save-as");
    let snapshot = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &snapshot, 5);
    session.handle(Request::SaveLevel { level: moved.level }).unwrap();
    match session.handle(Request::SaveRomAs { path: fixture.full("roms/copy.nds") }).unwrap() {
        Response::Project(info) => assert_eq!((info.path, info.name.as_str()), (fixture.full("roms/copy.nds"), "copy.nds")),
        other => panic!("unexpected {other:?}"),
    }
    let current = open_level(&mut session, "A01_1");
    let again = move_first_object(&mut session, &current, 11);
    session.handle(Request::SaveLevel { level: again.level }).unwrap();
    let x = |path: &str| {
        let course = rom_file(&fixture, path, "course/A01_1.bin");
        let bgdat = rom_file(&fixture, path, "course/A01_1_bgdat.bin");
        EditableLevel::load(&course, &bgdat, None).unwrap().objects()[0].x
    };
    assert_eq!((x("roms/game.nds"), x("roms/copy.nds")), (5, 11));
    assert_ne!(std::fs::read(fixture.root.path().join("roms/game.nds")).unwrap(), original);
}

#[test]
fn roms_are_opened_by_path_only_by_the_desktop_host() {
    let fixture = Fixture::new("host-rom-paths");
    write_bytes(&fixture.root.path().join("roms/game.nds"), &small_rom());
    // A workspace host, served or not, never opens a ROM by path: not inside its root, and not outside it.
    let mut session = fixture.session();
    for path in [fixture.full("roms/game.nds"), "roms/game.nds".to_owned()] {
        let error = session.handle(Request::OpenRom { path }).unwrap_err();
        assert!(error.message.contains("does not open ROMs by path"), "{}", error.message);
    }
    // The desktop host does, by the full path a file dialog gives.
    let mut local = fixture.rom_session();
    assert!(local.handle(Request::OpenRom { path: "roms/game.nds".into() }).unwrap_err().message.contains("full path"));
    write_bytes(&fixture.root.path().join("other.nds"), b"not a ROM at all");
    assert!(local.handle(Request::OpenRom { path: fixture.full("other.nds") }).is_err());
    let mut renamed = small_rom();
    renamed[0x0C..0x10].copy_from_slice(b"AMKE");
    write_bytes(&fixture.root.path().join("kart.nds"), &renamed);
    let error = local.handle(Request::OpenRom { path: fixture.full("kart.nds") }).unwrap_err();
    assert!(error.message.contains("not New Super Mario Bros."), "{}", error.message);
}

/// Sends `rom` to the host in pieces of `piece` bytes, and gives back what the last one answered.
fn upload(session: &mut HostSession, name: &str, rom: &[u8], piece: usize) -> bigstar_core::Result<Response> {
    let mut last = Err(bigstar_core::Error::new(ErrorCode::InvalidArgument, "nothing sent"));
    for (index, bytes) in rom.chunks(piece).enumerate() {
        let request = Request::UploadRom { name: name.into(), offset: (index * piece) as u64, total: rom.len() as u64, bytes: bytes.to_vec() };
        last = session.handle(request);
        if last.is_err() {
            break;
        }
    }
    last
}

/// The uploaded ROM with its edits, fetched a piece at a time as a frontend would.
fn download(session: &mut HostSession) -> (String, Vec<u8>) {
    let mut bytes = Vec::new();
    loop {
        match session.handle(Request::DownloadRom { offset: bytes.len() as u64 }).unwrap() {
            Response::RomChunk { name, total, offset, bytes: piece } => {
                assert_eq!(offset, bytes.len() as u64);
                bytes.extend_from_slice(&piece);
                if bytes.len() as u64 == total {
                    return (name, bytes);
                }
                assert!(!piece.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

fn entries(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        found.push(entry.path());
        if entry.path().is_dir() {
            found.extend(entries(&entry.path()));
        }
    }
    found.sort();
    found
}

#[test]
fn an_uploaded_rom_is_edited_in_memory_and_leaves_as_a_download() {
    let fixture = Fixture::new("host-upload");
    let rom = small_rom();
    let mut session = fixture.session();
    let before = entries(fixture.root.path());

    match upload(&mut session, "../My Game.nds", &rom, 1000).unwrap() {
        Response::Project(info) => {
            assert_eq!(info.mode, bigstar_protocol::ProjectMode::Uploaded);
            // A name is only ever a name: what an upload calls itself never names a place.
            assert_eq!((info.name.as_str(), info.rom_path), ("My Game.nds", None));
        }
        other => panic!("unexpected {other:?}"),
    }
    let snapshot = open_level(&mut session, "A01_1");
    let moved = move_first_object(&mut session, &snapshot, 33);
    let (saved, _) = edited(session.handle(Request::SaveLevel { level: moved.level }).unwrap());
    assert!(!saved.dirty);
    session.handle(Request::WriteFile { path: "dat/Dat_Field.narc!J01_1_bgdat.bin".into(), bytes: b"MEMBER".to_vec() }).unwrap();
    assert!(matches!(session.handle(Request::Recovery).unwrap(), Response::Recovery(levels) if levels.is_empty()));
    session.shared().write_recovery_snapshots();

    let (name, downloaded) = download(&mut session);
    assert_eq!(name, "My Game.nds");
    let course = rom_member(downloaded.clone(), "course/A01_1.bin");
    let bgdat = rom_member(downloaded.clone(), "course/A01_1_bgdat.bin");
    assert_eq!(EditableLevel::load(&course, &bgdat, None).unwrap().objects()[0].x, 33);
    assert_eq!(rom_member(downloaded, "dat/Dat_Field.narc!J01_1_bgdat.bin"), b"MEMBER");
    // Not a byte of it went to disk: no save, no backup, no autosave.
    assert_eq!(entries(fixture.root.path()), before);
    // A path is nothing to an uploaded ROM.
    assert!(session.handle(Request::SaveRomAs { path: fixture.full("out.nds") }).is_err());
    assert!(!fixture.root.path().join("out.nds").exists());

    // Closing it puts it away, and the workspace is there again.
    session.handle(Request::CloseProject).unwrap();
    assert!(session.handle(Request::ListLevels).is_err());
    assert!(matches!(session.handle(Request::ListProjects).unwrap(), Response::Projects(_)));
}

#[test]
fn uploaded_roms_are_each_their_own_connections() {
    let fixture = Fixture::new("host-upload-private");
    let host = Host::new(fixture.root.path()).unwrap();
    let (mut first, mut second) = (host.session(), host.guest_session());
    upload(&mut first, "a.nds", &small_rom(), 4096).unwrap();
    assert!(second.handle(Request::ListLevels).is_err(), "the second connection has no ROM yet");
    upload(&mut second, "b.nds", &small_rom(), 4096).unwrap();
    first.drain_events();
    second.drain_events();

    let mine = open_level(&mut first, "A01_1");
    let theirs = open_level(&mut second, "A01_1");
    assert_eq!(mine.level, theirs.level, "each has its own numbering");
    let moved = move_first_object(&mut first, &mine, 70);
    first.handle(Request::SaveLevel { level: moved.level }).unwrap();
    assert!(first.drain_events().iter().any(|event| matches!(event, Event::LevelChanged { .. })));
    assert!(second.drain_events().is_empty(), "nothing of the first connection's reaches the second");
    assert_eq!(open_level(&mut second, "A01_1").objects[0].x, theirs.objects[0].x);
    assert_eq!(download(&mut second).0, "b.nds");
}

#[test]
fn a_guest_reaches_nothing_but_what_it_uploads() {
    let fixture = Fixture::new("host-guest");
    write_bytes(&fixture.root.path().join("roms/game.nds"), &small_rom());
    let host = Host::new(fixture.root.path()).unwrap();
    let mut member = host.session();
    let mut guest = host.guest_session();
    let workspace_requests = [
        Request::ListProjects,
        Request::BrowseDirectory { path: String::new() },
        Request::OpenProject { path: "p".into() },
        Request::SetCleanRom { path: "roms/game.nds".into() },
    ];
    for request in workspace_requests.iter().cloned().chain([Request::ListLevels, Request::CloseProject]) {
        assert!(guest.handle(request.clone()).unwrap_err().message.contains("only edit a ROM it uploads"), "{request:?}");
    }
    upload(&mut guest, "game.nds", &small_rom(), 50_000).unwrap();
    for request in workspace_requests {
        assert!(guest.handle(request.clone()).is_err(), "{request:?}");
    }
    // And the workspace's own events are not the guest's to hear.
    make_project(fixture.root.path(), "p", &small_rom());
    member.handle(Request::OpenProject { path: "p".into() }).unwrap();
    assert!(!guest.drain_events().iter().any(|event| matches!(event, Event::Log { message, .. } if message.contains("Opened project"))));
}

#[test]
fn uploads_are_bounded() {
    let rom = small_rom();
    let options = crate::HostOptions { max_rom_bytes: rom.len(), max_uploaded_roms: 1, ..crate::HostOptions::default() };
    let host = Host::for_uploads(options);
    assert!(!host.has_workspace());
    let mut first = host.guest_session();

    let mut too_big = rom.clone();
    too_big.push(0);
    assert!(upload(&mut first, "big.nds", &too_big, 4096).unwrap_err().message.contains("up to"));
    // Pieces out of order are refused, and the upload with them.
    let piece = |offset: usize, len: usize| Request::UploadRom {
        name: "g.nds".into(),
        offset: offset as u64,
        total: rom.len() as u64,
        bytes: rom[offset..offset + len].to_vec(),
    };
    assert!(matches!(first.handle(piece(0, 100)).unwrap(), Response::Uploaded { received: 100 }));
    assert!(first.handle(piece(200, 100)).is_err());
    assert!(first.handle(piece(100, 100)).unwrap_err().message.contains("starts at offset 0"));
    let mut kart = rom.clone();
    kart[0x0C..0x10].copy_from_slice(b"AMKE");
    assert_eq!(upload(&mut first, "kart.nds", &kart, 4096).unwrap_err().code, ErrorCode::Unsupported);

    // The host holds as many as it was told to, and a connection that ends gives its place back.
    upload(&mut first, "g.nds", &rom, 4096).unwrap();
    let mut second = host.guest_session();
    assert!(upload(&mut second, "g.nds", &rom, 4096).unwrap_err().message.contains("as many uploaded ROMs"));
    drop(first);
    upload(&mut second, "g.nds", &rom, 4096).unwrap();
    // A host with no workspace has nothing else to offer even a full session.
    let mut member = host.session();
    assert!(member.handle(Request::ListProjects).unwrap_err().message.contains("no workspace"));
}

/// A real ROM, named by `BIGSTAR_NSMB_ROM`, edited directly: its levels draw from the ROM in memory with the shipped
/// scripts -- tilesets, backgrounds and actors alike -- and a saved edit is in the file when it is opened again.
/// Skipped without one.
#[test]
fn a_real_rom_edited_directly_draws_and_saves() {
    let Some(rom) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let fixture = Fixture::new("host-real-rom-mode");
    std::fs::create_dir_all(fixture.root.path().join("roms")).unwrap();
    std::fs::copy(&rom, fixture.root.path().join("roms/game.nds")).unwrap();
    let original = std::fs::read(&rom).unwrap();
    let mut session = fixture.rom_session();
    session.handle(Request::OpenRom { path: fixture.full("roms/game.nds") }).unwrap();

    let snapshot = open_level(&mut session, "A01_1");
    let Response::LevelGraphics(graphics) = session.handle(Request::LevelGraphics { level: snapshot.level }).unwrap() else { panic!() };
    assert!(graphics.error.is_none(), "{:?}", graphics.error);
    assert!(graphics.tilesets.iter().any(|tileset| tileset.error.is_none() && !tileset.objects.is_empty()));
    assert!(graphics.background.texture.is_some());
    let inputs: Vec<bigstar_protocol::ActorInput> = snapshot
        .course
        .sprites
        .iter()
        .map(|sprite| bigstar_protocol::ActorInput {
            sprite_type: sprite.id,
            x: i32::from(sprite.x),
            y: i32::from(sprite.y),
            pixel_x: f64::from(sprite.x) * 16.0,
            pixel_y: f64::from(sprite.y) * 16.0,
            settings: sprite.settings,
            ..Default::default()
        })
        .collect();
    let Response::ActorDraws { draws, .. } = session.handle(Request::RenderActors { inputs }).unwrap() else { panic!() };
    assert!(draws.iter().any(|draw| draw.found && draw.error.is_none() && !draw.commands.is_empty()), "no actor drew itself");

    let moved = move_first_object(&mut session, &snapshot, snapshot.objects[0].x + 3);
    session.handle(Request::SaveLevel { level: moved.level }).unwrap();
    let saved = std::fs::read(fixture.root.path().join("roms/game.nds")).unwrap();
    assert_ne!(saved, original);
    let mut fresh = fixture.rom_session();
    fresh.handle(Request::OpenRom { path: fixture.full("roms/game.nds") }).unwrap();
    assert_eq!(open_level(&mut fresh, "A01_1").objects[0].x, snapshot.objects[0].x + 3);
    // The other levels are untouched.
    assert_eq!(open_level(&mut fresh, "A01_2").objects, open_level(&mut session, "A01_2").objects);
}

/// A real ROM, named by `BIGSTAR_NSMB_ROM`, uploaded and downloaded in pieces of the size frontends send: untouched, it
/// comes back byte for byte. Skipped without one.
#[test]
fn a_real_rom_uploads_and_downloads_in_pieces() {
    let Some(path) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let rom = std::fs::read(path).unwrap();
    assert!(rom.len() > bigstar_protocol::ROM_CHUNK_BYTES, "the ROM fits one piece, so this proves nothing");
    let mut session = Host::for_uploads(crate::HostOptions::default()).guest_session();
    assert!(matches!(upload(&mut session, "real.nds", &rom, bigstar_protocol::ROM_CHUNK_BYTES).unwrap(), Response::Project(_)));
    assert!(!open_level(&mut session, "A01_1").objects.is_empty());
    assert!(download(&mut session).1 == rom);
}

/// The table of which object bank each sprite needs, against a real ROM: the levels Nintendo shipped load what their
/// sprites need, so a right table finds nothing missing in them. The unused ones, left over from development, do not.
#[test]
fn real_levels_load_the_banks_their_sprites_need() {
    let Some(rom) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let fixture = Fixture::new("host-real-sprite-banks");
    make_project(fixture.root.path(), "proj", &std::fs::read(rom).unwrap());
    let mut session = fixture.session();
    session.handle(Request::OpenProject { path: "proj".into() }).unwrap();
    let Response::ActorCatalog(catalog) = session.handle(Request::ActorCatalog).unwrap() else { panic!("not a catalog") };
    assert_eq!(catalog.sprite_banks.len(), 326);
    assert!(catalog.sprite_banks.iter().any(|&[_, bank]| bank != 0), "some sprites need a bank");
    let Response::Levels(worlds) = session.handle(Request::ListLevels).unwrap() else { panic!() };
    let areas: Vec<bigstar_protocol::AreaSummary> = worlds.into_iter().flat_map(|world| world.levels).flat_map(|level| level.areas).collect();
    let (mut sprites, mut missing) = (0, Vec::new());
    for area in areas.iter().filter(|area| area.available) {
        let Ok(Response::Level(snapshot)) = session.handle(Request::OpenLevel { area: area.token.clone() }) else { continue };
        for sprite in &snapshot.course.sprites {
            sprites += 1;
            let [slot, bank] = catalog.sprite_banks[usize::from(sprite.id)];
            if bank != 0 && snapshot.course.object_banks[usize::from(slot)] != bank {
                missing.push((area.course_path.clone(), sprite.id));
            }
        }
    }
    assert!(sprites > 1000, "{sprites}");
    let shipped: Vec<_> = missing.iter().filter(|(course, _)| !course.starts_with("course/J")).collect();
    assert!(shipped.is_empty(), "{shipped:?}");
    assert!(missing.iter().any(|(course, _)| course == "course/J06_1.bin"), "the unused grassland level is missing some");
}
