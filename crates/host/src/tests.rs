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
