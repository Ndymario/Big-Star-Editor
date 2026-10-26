use std::collections::VecDeque;

use bigstar_client::{Connection, ConnectionState, HostInfo, Incoming};
use bigstar_formats::{
    NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelStageObject, NsmbLevelView, NsmbLevelZone,
    OBJECT_LAYOUT_EMPTY_TILE, ObjectLayout, ObjectLayoutTile, RgbaImage,
};
use bigstar_level::{LevelEntityInsertions, SpriteInsertion};
use bigstar_protocol::{Event, LevelEdit, LevelGraphics, LevelSnapshot, Request, RequestId, Response, ResponseResult, TextureData, TilesetGraphics};
use egui::{Key, Modifiers, PointerButton, Pos2, Rect, Vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::demo::{DemoConnection, DemoTileset};
use crate::level_view::{Backgrounds, Gesture, Handle, Item, Kind, LevelView, handle_centres};
use crate::properties::Tab;
use crate::{BigStarApp, MemoryServices, Session};

fn settle(session: &mut Session) {
    for _ in 0..5 {
        session.poll();
    }
}

fn opened() -> Session {
    let mut session = Session::new(Box::new(DemoConnection::new()));
    settle(&mut session);
    session.open_project("demo");
    settle(&mut session);
    session.open_level("A01_1");
    settle(&mut session);
    session
}

#[test]
fn a_connected_host_lists_its_projects_then_the_levels_of_the_opened_one() {
    let mut session = Session::new(Box::new(DemoConnection::new()));
    assert!(session.connected());
    settle(&mut session);
    assert_eq!(session.projects.len(), 1);
    session.open_project("demo");
    settle(&mut session);
    assert_eq!(session.project.as_ref().map(|p| p.name.as_str()), Some("Demo"));
    assert_eq!(session.worlds.len(), 1);
    assert!(session.level().is_none());
}

#[test]
fn opening_a_level_gives_a_snapshot_and_edits_come_back_with_a_new_revision() {
    let mut session = opened();
    let view = session.level_mut().unwrap();
    assert_eq!(view.snapshot.revision, 1);
    view.selection.push(Item::new(Kind::Sprite, 0));
    let edit = view.move_edit(Vec2::new(16.0, 0.0)).unwrap();
    view.gesture = Some(Gesture::Move { delta: Vec2::new(16.0, 0.0), submitted: true });
    let before = view.snapshot.course.sprites[0].x;
    session.edit(edit);
    settle(&mut session);
    let view = session.level().unwrap();
    assert_eq!(view.snapshot.course.sprites[0].x, before + 1);
    assert_eq!((view.snapshot.revision, view.snapshot.dirty, view.snapshot.can_undo), (2, true, true));
    assert_eq!(view.snapshot.undo_names, ["Move selection"]);
    assert_eq!(view.gesture, None, "the moved things stop being previews once the host has answered");
}

#[test]
fn undo_redo_and_save_follow_the_hosts_answers() {
    let mut session = opened();
    let remove = {
        let view = session.level_mut().unwrap();
        view.selection.push(Item::new(Kind::Object, 0));
        view.remove_edit().unwrap()
    };
    session.edit(remove);
    settle(&mut session);
    assert_eq!(session.level().unwrap().snapshot.objects.len(), 6);
    session.undo();
    settle(&mut session);
    assert_eq!(session.level().unwrap().snapshot.objects.len(), 7);
    session.redo();
    settle(&mut session);
    assert_eq!(session.level().unwrap().snapshot.objects.len(), 6);
    session.save();
    settle(&mut session);
    assert!(!session.level().unwrap().snapshot.dirty);
    assert_eq!(session.log.last().map(|l| l.text.as_str()), Some("Saved 1-1"));
}

#[test]
fn a_refused_edit_is_reported_and_the_level_is_fetched_again() {
    let mut session = opened();
    // An edit against a revision the host no longer has.
    if let Some(view) = session.level_mut() {
        view.snapshot.revision = 99;
    }
    session.edit(LevelEdit::Remove { selection: Default::default(), name: "Delete".to_owned() });
    settle(&mut session);
    assert!(session.notice.as_ref().is_some_and(|n| n.error));
    assert_eq!(session.level().unwrap().snapshot.revision, 1, "the host's revision replaced ours");
}

#[test]
fn a_demo_host_answers_requests_it_does_not_know_with_an_error() {
    let mut connection = DemoConnection::new();
    let id = connection.send(Request::ListProjects);
    let id2 = connection.send(Request::CloseLevel { level: 1 });
    let id3 = connection.send(Request::SetActiveModule { module: "x".to_owned() });
    let answers = connection.poll();
    assert_eq!(answers.len(), 3);
    assert!(matches!(&answers[0], Incoming::Response { id: first, result: Ok(Response::Projects(_)) } if *first == id));
    assert!(matches!(&answers[1], Incoming::Response { id: second, result: Ok(Response::Done) } if *second == id2));
    assert!(matches!(&answers[2], Incoming::Response { id: third, result: Err(_) } if *third == id3));
}

#[test]
fn an_opened_level_fetches_its_graphics_actors_and_textures() {
    let session = opened();
    let view = session.level().unwrap();
    let graphics = view.art.graphics.as_ref().expect("level graphics");
    assert_eq!(graphics.tilesets[0].blocks, 16);
    assert!(view.art.tilesets[0].is_some());
    assert!(!view.art.actors.catalog.actors.is_empty());
    // Every sprite has its draw list, and every texture named is here.
    assert_eq!(view.art.draws.len(), 4);
    assert!(view.art.requested.is_empty());
    for id in view.art.texture_ids() {
        assert!(session.textures.contains_key(&id), "{id}");
    }
    assert!(session.textures.contains_key("demo-walker"));
    // The ground object is laid out of blocks, so the tile array has cells.
    let mut view = view.clone();
    assert!(view.tilemap().cell_at(0, 20).draws());
}

/// What a host was asked, and what the test has it answer.
#[derive(Default)]
struct Scripted {
    sent: Vec<(RequestId, Request)>,
    inbox: VecDeque<Incoming>,
}

/// A host that answers what the test tells it to, when it says so.
struct Shared(std::sync::Arc<std::sync::Mutex<Scripted>>);

impl Connection for Shared {
    fn send(&mut self, request: Request) -> RequestId {
        let mut script = self.0.lock().unwrap();
        let id = script.sent.len() as RequestId + 1;
        script.sent.push((id, request));
        id
    }

    fn poll(&mut self) -> Vec<Incoming> {
        self.0.lock().unwrap().inbox.drain(..).collect()
    }

    fn state(&self) -> &ConnectionState {
        // The state never changes in these tests.
        &CONNECTED
    }
}

static CONNECTED: std::sync::LazyLock<ConnectionState> = std::sync::LazyLock::new(|| {
    ConnectionState::Connected(HostInfo { host_version: "test".to_owned(), workspace_name: "Test".to_owned(), has_workspace: true })
});

fn last_request(script: &std::sync::Arc<std::sync::Mutex<Scripted>>, matches: impl Fn(&Request) -> bool) -> RequestId {
    script.lock().unwrap().sent.iter().rev().find(|(_, r)| matches(r)).map(|(id, _)| *id).expect("request sent")
}

fn answer(script: &std::sync::Arc<std::sync::Mutex<Scripted>>, id: RequestId, result: ResponseResult) {
    script.lock().unwrap().inbox.push_back(Incoming::Response { id, result });
}

#[test]
fn graphics_from_before_a_change_on_disk_are_dropped() {
    let script = std::sync::Arc::new(std::sync::Mutex::new(Scripted::default()));
    let mut session = Session::new(Box::new(Shared(script.clone())));
    let snapshot = demo_snapshot();
    session.open_level("A01_1");
    let open = last_request(&script, |r| matches!(r, Request::OpenLevel { .. }));
    answer(&script, open, Ok(Response::Level(Box::new(snapshot))));
    session.poll();
    let first = last_request(&script, |r| matches!(r, Request::LevelGraphics { .. }));
    assert!(script.lock().unwrap().sent.iter().any(|(_, r)| matches!(r, Request::ActorCatalog)));
    assert!(script.lock().unwrap().sent.iter().any(|(_, r)| matches!(r, Request::RenderActors { inputs } if inputs.len() == 4)));

    // A file changed: generation 2. The answer to the request made before it is stale.
    script.lock().unwrap().inbox.push_back(Incoming::Event(Event::GraphicsChanged { generation: 2 }));
    session.poll();
    let second = last_request(&script, |r| matches!(r, Request::LevelGraphics { .. }));
    assert_ne!(first, second, "the change asks again");
    let graphics = |generation, blocks| LevelGraphics {
        level: 1,
        generation,
        tilesets: [TilesetGraphics { blocks, ..Default::default() }, TilesetGraphics::default(), TilesetGraphics::default()],
        ..Default::default()
    };
    answer(&script, first, Ok(Response::LevelGraphics(Box::new(graphics(1, 5)))));
    session.poll();
    assert!(session.level().unwrap().art.graphics.is_none(), "an answer from generation 1 is dropped");
    answer(&script, second, Ok(Response::LevelGraphics(Box::new(graphics(2, 7)))));
    session.poll();
    assert_eq!(session.level().unwrap().art.graphics.as_ref().map(|g| g.tilesets[0].blocks), Some(7));

    // Draw lists from before the change are asked for again, and a stale answer does not land.
    let draws = last_request(&script, |r| matches!(r, Request::RenderActors { .. }));
    answer(&script, draws, Ok(Response::ActorDraws { generation: 1, draws: vec![Default::default(); 4] }));
    session.poll();
    assert!(session.level().unwrap().art.draws.is_empty());
    let again = last_request(&script, |r| matches!(r, Request::RenderActors { .. }));
    assert_ne!(again, draws);
    // A refused one leaves the sprites as markers rather than waiting forever.
    answer(&script, again, Err(bigstar_core::Error::new(bigstar_core::ErrorCode::Unsupported, "no")));
    session.poll();
    let view = session.level().unwrap();
    assert_eq!(view.art.draws.len(), 4);
    assert!(view.art.draws.values().all(|d| !d.found));
    assert!(session.notice.is_none(), "graphics failures are logged, not put in front of the person");
}

#[test]
fn an_edit_that_changes_what_the_level_draws_from_asks_for_its_graphics_again() {
    let script = std::sync::Arc::new(std::sync::Mutex::new(Scripted::default()));
    let mut session = Session::new(Box::new(Shared(script.clone())));
    let snapshot = demo_snapshot();
    session.open_level("A01_1");
    let open = last_request(&script, |r| matches!(r, Request::OpenLevel { .. }));
    answer(&script, open, Ok(Response::Level(Box::new(snapshot.clone()))));
    session.poll();
    let first = last_request(&script, |r| matches!(r, Request::LevelGraphics { .. }));

    // An edit that leaves the tilesets, layers and palettes alone does not.
    let edited =
        |snapshot: &LevelSnapshot| Ok(Response::Edited { snapshot: Box::new(snapshot.clone()), outcome: bigstar_protocol::EditOutcome::None });
    let mut references = snapshot.tileset_references;
    session.edit_level(snapshot.level, LevelEdit::SetTilesetReferences { references });
    let edit = last_request(&script, |r| matches!(r, Request::EditLevel { .. }));
    answer(&script, edit, edited(&LevelSnapshot { revision: 2, ..snapshot.clone() }));
    session.poll();
    assert_eq!(last_request(&script, |r| matches!(r, Request::LevelGraphics { .. })), first);

    // A new background palette does: the picture on screen was made with the old one.
    references.background_palette_id += 1;
    session.edit_level(snapshot.level, LevelEdit::SetTilesetReferences { references });
    let edit = last_request(&script, |r| matches!(r, Request::EditLevel { .. }));
    answer(&script, edit, edited(&LevelSnapshot { revision: 3, tileset_references: references, ..snapshot }));
    session.poll();
    assert_ne!(last_request(&script, |r| matches!(r, Request::LevelGraphics { .. })), first);
}

fn demo_snapshot() -> LevelSnapshot {
    let mut demo = DemoConnection::new();
    demo.send(Request::OpenLevel { area: "A01_1".to_owned() });
    match demo.poll().pop() {
        Some(Incoming::Response { result: Ok(Response::Level(snapshot)), .. }) => *snapshot,
        other => panic!("{other:?}"),
    }
}

#[test]
fn textures_are_asked_for_once_and_kept() {
    let mut session = opened();
    let count = session.textures.len();
    assert!(count >= 2);
    // Reopening the level asks for nothing it already holds.
    let level = session.active.unwrap();
    session.close_level(level);
    settle(&mut session);
    session.open_level("A01_1");
    settle(&mut session);
    assert_eq!(session.textures.len(), count);
    let texture: &TextureData = session.textures.values().next().unwrap();
    assert_eq!(texture.rgba.len(), (texture.width * texture.height * 4) as usize);
}

// --- the canvas, driven through the whole app ----------------------------------------------------------------------

fn harness_on(connection: DemoConnection, size: Vec2) -> Harness<'static, BigStarApp> {
    let app = BigStarApp::with_connection(Box::new(MemoryServices::default()), Box::new(connection));
    Harness::builder().with_size(size).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app)
}

fn harness() -> Harness<'static, BigStarApp> {
    harness_on(DemoConnection::new(), Vec2::new(1500.0, 800.0))
}

/// The app with the level open on its canvas.
fn canvas_on(connection: DemoConnection, size: Vec2) -> Harness<'static, BigStarApp> {
    let mut harness = harness_on(connection, size);
    harness.run();
    let session = harness.state_mut().session_mut().unwrap();
    session.open_project("demo");
    harness.run();
    harness.state_mut().session_mut().unwrap().open_level("A01_1");
    harness.run();
    harness
}

fn level_with(items: LevelEntityInsertions) -> Harness<'static, BigStarApp> {
    canvas_on(DemoConnection::with_level(items), Vec2::new(1500.0, 800.0))
}

fn view<'h>(harness: &'h Harness<'_, BigStarApp>) -> &'h LevelView {
    harness.state().session().unwrap().level().unwrap()
}

fn view_mut<'a>(harness: &'a mut Harness<'_, BigStarApp>) -> &'a mut LevelView {
    harness.state_mut().session_mut().unwrap().level_mut().unwrap()
}

fn screen(harness: &Harness<'_, BigStarApp>, world: Pos2) -> Pos2 {
    view(harness).to_screen(world)
}

fn button(harness: &Harness<'_, BigStarApp>, pos: Pos2, button: PointerButton, pressed: bool, modifiers: Modifiers) {
    harness.event(egui::Event::PointerButton { pos, button, pressed, modifiers });
}

/// Press at `from`, move to `to` and release there, a frame for each.
fn drag(harness: &mut Harness<'_, BigStarApp>, from: Pos2, to: Pos2, modifiers: Modifiers) {
    harness.hover_at(from);
    harness.run();
    button(harness, from, PointerButton::Primary, true, modifiers);
    harness.run();
    harness.hover_at(to);
    harness.run();
    button(harness, to, PointerButton::Primary, false, modifiers);
    harness.run();
}

fn click(harness: &mut Harness<'_, BigStarApp>, at: Pos2, which: PointerButton, modifiers: Modifiers) {
    harness.hover_at(at);
    harness.run();
    button(harness, at, which, true, modifiers);
    button(harness, at, which, false, modifiers);
    harness.run();
}

fn select(harness: &mut Harness<'_, BigStarApp>, items: &[Item]) {
    view_mut(harness).selection = items.to_vec();
    harness.run();
}

fn object(object_id: u16, x: i16, y: i16, width: u16, height: u16) -> NsmbLevelObject {
    NsmbLevelObject { object_id, tileset_slot: 0, x, y, width, height }
}

fn view_at(x: u16, y: u16) -> NsmbLevelView {
    NsmbLevelView { x, y, width: 256, height: 192, ..Default::default() }
}

#[test]
fn the_screens_lead_from_projects_to_the_preview_to_a_document() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    // Picking an area shows it in the Preview tab; nothing is editable yet.
    harness.get_all_by_label("Area 1").next().unwrap().click();
    harness.run();
    let session = harness.state().session().unwrap();
    assert_eq!(session.active, None);
    let previewed = harness.state().workspace.preview.unwrap();
    assert_eq!(session.levels[&previewed].snapshot.title, "1-1");
    harness.get_by_label("Objects 7   Sprites 4   Entrances 2   Views 1   Zones 1");
    // The editing row is already there, with nothing to act on, so opening a document does not shift the layout.
    assert!(egui_kittest::kittest::NodeT::accesskit_node(&harness.get_by_label("Undo")).is_disabled());
    assert!(egui_kittest::kittest::NodeT::accesskit_node(&harness.get_by_label("Save")).is_disabled());
    // Edit opens it as a document of its own, which is what the panels and menus act on. The first Edit is the menu's.
    harness.get_all_by_label("Edit").nth(1).unwrap().click();
    harness.run();
    let session = harness.state().session().unwrap();
    assert_eq!(session.active, Some(previewed), "the host hands back the same session");
    assert!(harness.state().workspace.is_document(previewed));
    harness.get_by_label("Save");
    harness.get_by_label("Fit");
}

#[test]
fn the_preview_keeps_the_level_before_until_the_next_one_is_drawn() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    harness.get_all_by_label("Area 1").next().unwrap().click();
    harness.run();
    let first = harness.state().workspace.preview.unwrap();
    harness.get_all_by_label("Area 2").next().unwrap().click();
    // Frame by frame: whatever the tab shows has its graphics and its actors' draw lists, never a level half arrived.
    for _ in 0..8 {
        harness.step();
        let app = harness.state();
        let session = app.session().unwrap();
        let shown = &session.levels[&app.workspace.preview.unwrap()];
        assert!(shown.art.graphics.is_some() && shown.art.requested.is_empty(), "{} shown half drawn", shown.snapshot.area);
    }
    let app = harness.state();
    let session = app.session().unwrap();
    let second = app.workspace.preview.unwrap();
    assert_eq!(session.levels[&second].snapshot.area, "A01_2");
    assert!(!session.levels.contains_key(&first), "the level before is closed once it is replaced");
}

#[test]
fn dragging_a_sprite_moves_it_by_one_undoable_edit() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    let (centre, x_before, zoom) = {
        let view = view(&harness);
        (view.to_screen(view.bounds(Item::new(Kind::Sprite, 0)).unwrap().center()), view.snapshot.course.sprites[0].x, view.zoom)
    };
    drag(&mut harness, centre, centre + Vec2::new(64.0 * zoom, 0.0), Modifiers::NONE);
    let view = view(&harness);
    assert_eq!(view.selection, [Item::new(Kind::Sprite, 0)]);
    assert_eq!(view.snapshot.course.sprites[0].x, x_before + 4, "moved by 64 level pixels (four tiles) in one step");
    assert_eq!(view.snapshot.undo_names, ["Move selection"]);
}

#[test]
fn right_click_creates_the_current_kind_and_left_click_never_does() {
    let mut harness = level_with(LevelEntityInsertions::default());
    let at = view(&harness).canvas.center();
    // Left-click selects and marquees. It never creates, whatever kind is chosen.
    click(&mut harness, at, PointerButton::Primary, Modifiers::NONE);
    assert!(view(&harness).snapshot.objects.is_empty());

    // Right-click creates without anything armed: object 0 of tileset slot 0.
    click(&mut harness, at, PointerButton::Secondary, Modifiers::NONE);
    let snapshot = &view(&harness).snapshot;
    assert_eq!(snapshot.objects.len(), 1);
    assert_eq!((snapshot.objects[0].tileset_slot, snapshot.objects[0].object_id), (0, 0));
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 0)]);

    // A different pick changes what is stamped, not whether stamping works.
    view_mut(&mut harness).placement.set_object(1, 12);
    assert_eq!(view(&harness).placement.kind, Kind::Object);
    click(&mut harness, at + Vec2::new(20.0, 20.0), PointerButton::Secondary, Modifiers::NONE);
    let snapshot = &view(&harness).snapshot;
    assert_eq!(snapshot.objects.len(), 2);
    assert_eq!((snapshot.objects[1].tileset_slot, snapshot.objects[1].object_id), (1, 12));

    // The kind alone decides what is created, views included.
    view_mut(&mut harness).placement.kind = Kind::View;
    click(&mut harness, at + Vec2::new(40.0, 40.0), PointerButton::Secondary, Modifiers::NONE);
    assert_eq!(view(&harness).snapshot.course.views.len(), 1);
    assert_eq!(view(&harness).snapshot.undo_names.first().map(String::as_str), Some("Add view"));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(view(&harness).snapshot.course.views.len(), 0);
}

#[test]
fn a_right_drag_stretches_the_object_it_places() {
    let mut harness = level_with(LevelEntityInsertions::default());
    let (from, to) = (view(&harness).to_screen(Pos2::new(36.0, 36.0)), view(&harness).to_screen(Pos2::new(84.0, 52.0)));
    assert!(view(&harness).canvas.contains(from) && view(&harness).canvas.contains(to));
    harness.hover_at(from);
    harness.run();
    button(&harness, from, PointerButton::Secondary, true, Modifiers::NONE);
    harness.run();
    harness.hover_at(to);
    harness.run();
    // Nothing exists until the button comes up, though the canvas already shows it.
    assert!(view(&harness).snapshot.objects.is_empty());
    assert!(view(&harness).placing_rect().is_some());
    button(&harness, to, PointerButton::Secondary, false, Modifiers::NONE);
    harness.run();
    let view = view(&harness);
    assert_eq!(view.snapshot.objects, [object(0, 2, 2, 4, 2)]);
    assert_eq!(view.snapshot.undo_names, ["Add object"]);
    assert_eq!((view.gesture, view.selection.as_slice()), (None, [Item::new(Kind::Object, 0)].as_slice()));
}

#[test]
fn a_custom_placement_carries_its_hash() {
    let mut harness = level_with(LevelEntityInsertions::default());
    view_mut(&mut harness).placement.set_custom_sprite(0xf46a_44b9);
    let at = view(&harness).canvas.center();
    click(&mut harness, at, PointerButton::Secondary, Modifiers::NONE);
    let snapshot = &view(&harness).snapshot;
    assert_eq!(snapshot.course.sprites.len(), 1);
    assert_eq!(snapshot.sprite_hashes, [Some(0xf46a_44b9)]);
    assert_eq!(snapshot.course.sprites[0].id, 326);
}

#[test]
fn an_object_knob_resizes_and_escape_cancels() {
    let mut harness = level_with(LevelEntityInsertions { objects: vec![object(1, 10, 10, 2, 2)], ..Default::default() });
    select(&mut harness, &[Item::new(Kind::Object, 0)]);
    let knob = screen(&harness, Pos2::new(12.0 * 16.0, 12.0 * 16.0));
    assert_eq!(view(&harness).handle_at(knob), Some(Handle::BottomRight));
    drag(&mut harness, knob, knob + Vec2::new(40.0, 40.0), Modifiers::NONE);
    let resized = &view(&harness).snapshot.objects[0];
    assert!(resized.width > 2 && resized.height > 2, "{resized:?}");
    assert_eq!(view(&harness).snapshot.undo_names.first().map(String::as_str), Some("Resize selection"));
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!((view(&harness).snapshot.objects[0].width, view(&harness).snapshot.objects[0].height), (2, 2));

    // Escape mid-drag abandons it: the release changes nothing.
    select(&mut harness, &[Item::new(Kind::Object, 0)]);
    let knob = screen(&harness, Pos2::new(12.0 * 16.0, 12.0 * 16.0));
    harness.hover_at(knob);
    harness.run();
    button(&harness, knob, PointerButton::Primary, true, Modifiers::NONE);
    harness.run();
    harness.hover_at(knob + Vec2::new(40.0, 40.0));
    harness.run();
    assert!(matches!(view(&harness).gesture, Some(Gesture::Resize { .. })));
    harness.key_press(Key::Escape);
    harness.run();
    button(&harness, knob + Vec2::new(40.0, 40.0), PointerButton::Primary, false, Modifiers::NONE);
    harness.run();
    assert_eq!((view(&harness).snapshot.objects[0].width, view(&harness).snapshot.objects[0].height), (2, 2));
    assert!(!view(&harness).snapshot.can_redo || view(&harness).snapshot.undo_names.is_empty());
}

#[test]
fn every_resizable_kind_has_eight_knobs() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 10, 10, 4, 4)],
        zones: vec![NsmbLevelZone { x: 320, y: 320, width: 256, height: 192, ..Default::default() }],
        views: vec![view_at(640, 640)],
        sprites: vec![SpriteInsertion { value: NsmbLevelStageObject { id: 7, x: 4, y: 4, ..Default::default() }, hash: None }],
        ..Default::default()
    });
    // Objects, zones and views all resize; sprites and entrances do not.
    for kind in [Kind::Object, Kind::Zone, Kind::View] {
        select(&mut harness, &[Item::new(kind, 0)]);
        let view = view(&harness);
        let rect = view.screen_rect(view.selected_resizable_rect().unwrap_or_else(|| panic!("{kind:?} has no knobs")));
        // All eight answer, each with its own corner or edge.
        for (handle, centre) in Handle::ALL.into_iter().zip(handle_centres(rect)) {
            assert_eq!(view.handle_at(centre), Some(handle), "{kind:?}");
        }
        assert_eq!(view.handle_at(rect.center()), None);
    }
    select(&mut harness, &[Item::new(Kind::Sprite, 0)]);
    assert_eq!(view(&harness).selected_resizable_rect(), None);
    // Only one selected thing has knobs.
    select(&mut harness, &[Item::new(Kind::Zone, 0), Item::new(Kind::View, 0)]);
    assert_eq!(view(&harness).selected_resizable_rect(), None);
}

#[test]
fn the_top_left_knob_moves_a_views_origin_rather_than_its_extent() {
    let mut harness = level_with(LevelEntityInsertions { views: vec![view_at(320, 320)], ..Default::default() });
    select(&mut harness, &[Item::new(Kind::View, 0)]);
    // The bottom-right knob grows the view away from its origin.
    let knob = view(&harness).screen_rect(view(&harness).selected_resizable_rect().unwrap()).right_bottom();
    drag(&mut harness, knob, knob + Vec2::new(48.0, 48.0), Modifiers::NONE);
    let grown = view(&harness).snapshot.course.views[0].clone();
    assert_eq!((grown.x, grown.y), (320, 320));
    assert!(grown.width > 256 && grown.height > 192);

    // The top-left knob moves the origin instead.
    select(&mut harness, &[Item::new(Kind::View, 0)]);
    let knob = view(&harness).screen_rect(view(&harness).selected_resizable_rect().unwrap()).left_top();
    drag(&mut harness, knob, knob + Vec2::new(32.0, 32.0), Modifiers::NONE);
    let moved = &view(&harness).snapshot.course.views[0];
    assert!(moved.x > 320 && moved.y > 320);
    assert!(moved.width < grown.width && moved.height < grown.height);
    // The far edge held still.
    assert_eq!(u32::from(moved.x) + u32::from(moved.width), u32::from(grown.x) + u32::from(grown.width));
}

#[test]
fn selection_ignores_the_create_kind() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 1, 1, 1, 1)],
        sprites: vec![SpriteInsertion { value: NsmbLevelStageObject { id: 7, x: 10, y: 10, ..Default::default() }, hash: None }],
        ..Default::default()
    });
    select(&mut harness, &[Item::new(Kind::Object, 0), Item::new(Kind::Sprite, 0)]);
    // Deciding what to place next (the canvas toolbar) is not a reason to drop what is held, and nothing has to be
    // switched to before it can be grabbed.
    for kind in [Kind::Sprite, Kind::Object, Kind::Zone, Kind::View] {
        view_mut(&mut harness).placement.kind = kind;
        harness.run();
        let view = view(&harness);
        assert_eq!(view.selection.len(), 2);
        assert_eq!(view.panel.tab, Tab::Entity(kind), "the properties show what a right-click makes");
        assert_eq!(view.hit_test(Pos2::new(20.0, 20.0)).map(|i| i.kind), Some(Kind::Object));
        assert_eq!(view.hit_test(Pos2::new(160.0, 160.0)).map(|i| i.kind), Some(Kind::Sprite));
    }
    // A kind shortcut takes the properties to that kind's first item, as the Qt editor's does.
    harness.key_press(Key::S);
    harness.run();
    assert_eq!(view(&harness).selection, [Item::new(Kind::Sprite, 0)]);
    harness.key_press(Key::C);
    harness.run();
    assert_eq!((view(&harness).panel.tab, view(&harness).selection.len()), (Tab::Course, 0));
}

#[test]
fn containers_yield_the_click_to_what_they_enclose() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 16, 16, 4, 4)],
        zones: vec![NsmbLevelZone { x: 0, y: 0, width: 1024, height: 768, ..Default::default() }],
        views: vec![NsmbLevelView { width: 1024, height: 768, ..Default::default() }],
        ..Default::default()
    });
    // Over the object all three overlap, and the object wins; clear of it the zone beats the larger view.
    assert_eq!(view(&harness).hit_test(Pos2::new(260.0, 260.0)).map(|i| i.kind), Some(Kind::Object));
    assert_eq!(view(&harness).hit_test(Pos2::new(700.0, 600.0)).map(|i| i.kind), Some(Kind::Zone));
    // A view is not taken by its area, even where nothing else covers it...
    view_mut(&mut harness).snapshot.course.zones[0].width = 64;
    view_mut(&mut harness).snapshot.course.zones[0].height = 64;
    assert_eq!(view(&harness).hit_test(Pos2::new(700.0, 600.0)), None);
    // ...only by the tab at its top-left corner, which is over the zone there.
    let area = Rect::from_min_size(Pos2::ZERO, Vec2::new(1024.0, 768.0));
    let tab = view(&harness).view_tab(0, area).unwrap();
    assert_eq!(tab.min, screen(&harness, Pos2::ZERO));
    click(&mut harness, tab.center(), PointerButton::Primary, Modifiers::NONE);
    assert_eq!(view(&harness).selection, [Item::new(Kind::View, 0)]);
    // Scrolled away from that corner, the tab keeps to the canvas's corner for as long as the view is on the canvas.
    view_mut(&mut harness).selection.clear();
    view_mut(&mut harness).pan = Vec2::new(300.0, 200.0);
    harness.run();
    let tab = view(&harness).view_tab(0, area).unwrap();
    assert_eq!(tab.min, view(&harness).canvas.min);
    click(&mut harness, tab.center(), PointerButton::Primary, Modifiers::NONE);
    assert_eq!(view(&harness).selection, [Item::new(Kind::View, 0)]);
    view_mut(&mut harness).pan = Vec2::new(2000.0, 200.0);
    assert_eq!(view(&harness).view_tab(0, area), None, "nothing to hold a tab once the view is off the canvas");
}

#[test]
fn shift_click_adds_and_removes_and_a_marquee_selects_what_it_touches() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 4, 4, 1, 1), object(1, 8, 4, 1, 1), object(1, 30, 30, 1, 1)],
        ..Default::default()
    });
    let centre = |harness: &Harness<'_, BigStarApp>, x: f32, y: f32| screen(harness, Pos2::new(x * 16.0 + 8.0, y * 16.0 + 8.0));
    let first = centre(&harness, 4.0, 4.0);
    click(&mut harness, first, PointerButton::Primary, Modifiers::NONE);
    let second = centre(&harness, 8.0, 4.0);
    click(&mut harness, second, PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 0), Item::new(Kind::Object, 1)]);
    click(&mut harness, first, PointerButton::Primary, Modifiers::SHIFT);
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 1)]);
    // A click on nothing clears, and a drag from nothing is a rubber band.
    let empty = screen(&harness, Pos2::new(2.0 * 16.0, 2.0 * 16.0));
    let across = screen(&harness, Pos2::new(10.0 * 16.0, 6.0 * 16.0));
    drag(&mut harness, empty, across, Modifiers::NONE);
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 0), Item::new(Kind::Object, 1)]);
    assert_eq!(view(&harness).gesture, None);
}

#[test]
fn a_sprite_missing_its_bank_says_so_and_a_button_loads_it() {
    let mut harness = level_with(LevelEntityInsertions {
        sprites: vec![SpriteInsertion { value: NsmbLevelStageObject { id: 20, x: 6, y: 6, ..Default::default() }, hash: None }],
        ..Default::default()
    });
    let mut catalog = (*view(&harness).art.actors.catalog).clone();
    catalog.sprite_banks = vec![[0, 0]; 30];
    catalog.sprite_banks[20] = [2, 5];
    view_mut(&mut harness).art.actors = crate::level_view::ActorIndex::new(std::sync::Arc::new(catalog));
    select(&mut harness, &[Item::new(Kind::Sprite, 0)]);
    assert_eq!(view(&harness).missing_bank(0), Some((2, 5)));
    harness.get_by_label("Load bank 5 in slot 2").click();
    harness.run();
    assert_eq!(view(&harness).snapshot.course.object_banks[2], 5);
    assert_eq!(view(&harness).missing_bank(0), None);
    assert!(harness.query_by_label("Load bank 5 in slot 2").is_none(), "nothing left to warn about");
}

#[test]
fn ctrl_a_selects_everything_and_a_alone_still_pans() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 2, 3, 1, 1), object(2, 30, 30, 1, 1)],
        entrances: vec![NsmbLevelEntrance { x: 32, y: 48, ..Default::default() }],
        ..Default::default()
    });
    let pan = view(&harness).pan;
    harness.key_press_modifiers(Modifiers::COMMAND, Key::A);
    harness.run();
    let selection = view(&harness).selection.clone();
    for item in [Item::new(Kind::Object, 0), Item::new(Kind::Object, 1), Item::new(Kind::Entrance, 0)] {
        assert!(selection.contains(&item), "{item:?} is selected");
    }
    assert_eq!(selection.len(), view(&harness).entity_rects().len());
    assert_eq!(view(&harness).pan, pan, "Ctrl+A does not scroll the way A does");
    // Everything selected moves together, as one step.
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!((view(&harness).snapshot.objects[0].x, view(&harness).snapshot.objects[1].x), (3, 31));
}

#[test]
fn ctrl_drag_clones_and_ctrl_d_and_page_keys_are_one_step_each() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 2, 3, 1, 1), object(2, 5, 6, 1, 1)],
        sprites: vec![SpriteInsertion {
            value: NsmbLevelStageObject { id: 326, x: 7, y: 8, settings: 9, ..Default::default() },
            hash: Some(0x1234_5678),
        }],
        ..Default::default()
    });
    // Ctrl+D copies the selection one tile along, as pasting it would.
    select(&mut harness, &[Item::new(Kind::Object, 0), Item::new(Kind::Sprite, 0)]);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::D);
    harness.run();
    let snapshot = view(&harness).snapshot.clone();
    assert_eq!((snapshot.objects.len(), snapshot.course.sprites.len()), (3, 2));
    assert_eq!((snapshot.objects[2].x, snapshot.objects[2].y), (3, 4));
    assert_eq!(snapshot.sprite_hashes[1], Some(0x1234_5678));
    assert_eq!(snapshot.undo_names[0], "Paste selection");
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 2), Item::new(Kind::Sprite, 1)]);
    harness.key_press_modifiers(Modifiers::COMMAND, Key::Z);
    harness.run();
    assert_eq!(view(&harness).snapshot.objects.len(), 2);

    // Ctrl-drag drags a copy and leaves the original where it was.
    let grab = screen(&harness, Pos2::new(2.0 * 16.0 + 8.0, 3.0 * 16.0 + 8.0));
    let zoom = view(&harness).zoom;
    drag(&mut harness, grab, grab + Vec2::new(64.0 * zoom, 0.0), Modifiers::COMMAND);
    let snapshot = &view(&harness).snapshot;
    assert_eq!(snapshot.objects.len(), 3);
    assert_eq!((snapshot.objects[0].x, snapshot.objects[0].y), (2, 3));
    assert_eq!((snapshot.objects[2].x, snapshot.objects[2].y), (7, 4), "the copy, one tile along, then dragged four");
    assert_eq!(snapshot.undo_names[..2], ["Move selection", "Paste selection"]);

    // PageUp brings the one selected thing forward in draw order, and the selection follows it.
    select(&mut harness, &[Item::new(Kind::Object, 0)]);
    harness.key_press(Key::PageUp);
    harness.run();
    assert_eq!(view(&harness).snapshot.objects[1].object_id, 1);
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 1)]);
    assert_eq!(view(&harness).snapshot.undo_names[0], "Bring forward");
    harness.key_press(Key::PageDown);
    harness.run();
    assert_eq!(view(&harness).selection, [Item::new(Kind::Object, 0)]);
}

#[test]
fn arrows_nudge_by_the_snap_and_delete_removes() {
    let mut harness = level_with(LevelEntityInsertions {
        objects: vec![object(1, 2, 3, 1, 1)],
        entrances: vec![NsmbLevelEntrance { x: 32, y: 48, ..Default::default() }],
        ..Default::default()
    });
    select(&mut harness, &[Item::new(Kind::Object, 0)]);
    harness.key_press(Key::ArrowRight);
    harness.run();
    assert_eq!(view(&harness).snapshot.objects[0].x, 3, "objects step by a tile");
    select(&mut harness, &[Item::new(Kind::Entrance, 0)]);
    harness.get_by_label("8 px").click();
    harness.run();
    harness.key_press(Key::ArrowDown);
    harness.run();
    assert_eq!(view(&harness).snapshot.course.entrances[0].y, 56, "everything else by the snap size");
    harness.key_press(Key::Delete);
    harness.run();
    assert!(view(&harness).snapshot.course.entrances.is_empty());
    assert!(view(&harness).selection.is_empty());
    assert_eq!(view(&harness).snapshot.undo_names[0], "Delete selection");
}

#[test]
fn panning_by_middle_button_space_alt_and_wasd() {
    let mut harness = level_with(LevelEntityInsertions { objects: vec![object(1, 2, 3, 1, 1)], ..Default::default() });
    let at = view(&harness).canvas.center();
    let pan = |harness: &Harness<'_, BigStarApp>| view(harness).pan;
    let zoom = view(&harness).zoom;

    let before = pan(&harness);
    harness.hover_at(at);
    harness.run();
    button(&harness, at, PointerButton::Middle, true, Modifiers::NONE);
    harness.run();
    harness.hover_at(at + Vec2::new(30.0, 0.0));
    harness.run();
    button(&harness, at + Vec2::new(30.0, 0.0), PointerButton::Middle, false, Modifiers::NONE);
    harness.run();
    assert!((pan(&harness).x - (before.x - 30.0 / zoom)).abs() < 0.01, "the level follows the pointer");

    // Alt pans even over an entity.
    let object = screen(&harness, Pos2::new(40.0, 56.0));
    let before = pan(&harness);
    drag(&mut harness, object, object + Vec2::new(0.0, 20.0), Modifiers::ALT);
    assert!((pan(&harness).y - (before.y - 20.0 / zoom)).abs() < 0.01);
    assert_eq!(view(&harness).snapshot.objects[0].y, 3, "nothing moved");

    // Space held pans too.
    let before = pan(&harness);
    harness.key_down(Key::Space);
    harness.run();
    drag(&mut harness, object, object + Vec2::new(-10.0, 0.0), Modifiers::NONE);
    harness.key_up(Key::Space);
    harness.run();
    assert!((pan(&harness).x - (before.x + 10.0 / zoom)).abs() < 0.01);

    // W, A and D scroll 32 points; S chooses sprites.
    let before = pan(&harness);
    harness.key_press(Key::A);
    harness.key_press(Key::W);
    harness.key_press(Key::S);
    harness.run();
    assert!((pan(&harness) - (before - Vec2::new(32.0, 32.0) / zoom)).length() < 0.01);
    assert_eq!(view(&harness).placement.kind, Kind::Sprite);
}

#[test]
fn zoom_presets_pin_the_zoom_and_fit_unpins_it() {
    let mut harness = level_with(LevelEntityInsertions { objects: vec![object(1, 2, 3, 1, 1)], ..Default::default() });
    let fitted = view(&harness).zoom;
    assert!(!view(&harness).zoom_pinned);
    harness.get_by_label("200%").click();
    harness.run();
    assert_eq!((view(&harness).zoom, view(&harness).zoom_pinned), (2.0, true));
    // The wheel zooms about the pointer: what is under it stays put.
    let at = view(&harness).canvas.center() + Vec2::new(40.0, 10.0);
    let under = view(&harness).to_world(at);
    harness.hover_at(at);
    harness.run();
    harness.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Line,
        delta: Vec2::new(0.0, 1.0),
        modifiers: Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    harness.run();
    assert!(view(&harness).zoom > 2.0);
    assert!((view(&harness).to_world(at) - under).length() < 0.01);
    harness.get_by_label("Fit").click();
    harness.run();
    assert_eq!((view(&harness).zoom, view(&harness).zoom_pinned), (fitted, false));
}

#[test]
fn unsigned_entities_snap_back_and_hold_the_same_bound() {
    let mut harness = level_with(LevelEntityInsertions {
        entrances: vec![NsmbLevelEntrance { x: 32, y: 48, ..Default::default() }],
        zones: vec![NsmbLevelZone { x: 64, y: 80, width: 256, height: 192, ..Default::default() }],
        views: vec![NsmbLevelView { x: 96, y: 112, width: 256, height: 192, ..Default::default() }],
        ..Default::default()
    });
    for kind in [Kind::Entrance, Kind::Zone, Kind::View] {
        select(&mut harness, &[Item::new(kind, 0)]);
        assert_eq!(view(&harness).move_edit(Vec2::new(-1024.0, -1024.0)), None, "{kind:?}");
    }
    // A near edge dragged past the origin moves nothing and keeps the size; one that stays in bounds lands.
    select(&mut harness, &[Item::new(Kind::Zone, 0)]);
    assert_eq!(view(&harness).resize_edit(Handle::TopLeft.edges(Vec2::new(-1024.0, -1024.0))), None);
    let edit = view(&harness).resize_edit(Handle::TopLeft.edges(Vec2::new(-64.0, -80.0))).unwrap();
    harness.state_mut().session_mut().unwrap().edit(edit);
    harness.run();
    let zone = &view(&harness).snapshot.course.zones[0];
    assert_eq!((zone.x, zone.y, zone.width, zone.height), (0, 0, 320, 272));
}

#[test]
fn a_signed_path_node_moves_past_the_origin() {
    let mut harness = level_with(LevelEntityInsertions {
        paths: vec![NsmbLevelPath { nodes: vec![NsmbLevelPathNode { x: -20, y: -30, ..Default::default() }], ..Default::default() }],
        ..Default::default()
    });
    select(&mut harness, &[Item::node(Kind::Path, 0, 0)]);
    let edit = view(&harness).move_edit(Vec2::new(-8.0, -8.0)).unwrap();
    harness.state_mut().session_mut().unwrap().edit(edit);
    harness.run();
    let node = &view(&harness).snapshot.course.paths[0].nodes[0];
    assert_eq!((node.x, node.y), (-28, -38));
}

// --- what the canvas draws -----------------------------------------------------------------------------------------

fn rendered(harness: &mut Harness<'_, BigStarApp>) -> image::RgbaImage {
    harness.run();
    harness.render().expect("render")
}

fn pixel(image: &image::RgbaImage, at: Pos2) -> [u8; 4] {
    image.get_pixel(at.x.round() as u32, at.y.round() as u32).0
}

fn lightness([r, g, b, _]: [u8; 4]) -> u32 {
    (u32::from(r.max(g).max(b)) + u32::from(r.min(g).min(b))) / 2
}

fn drawn(connection: DemoConnection, size: Vec2) -> Harness<'static, BigStarApp> {
    let app = BigStarApp::with_connection(Box::new(MemoryServices::default()), Box::new(connection));
    let mut harness = Harness::builder().with_size(size).with_max_steps(16).wgpu().build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    harness.state_mut().session_mut().unwrap().open_level("A01_1");
    harness.run();
    harness
}

#[test]
fn the_canvas_follows_the_theme() {
    let mut harness = drawn(DemoConnection::with_level(LevelEntityInsertions::default()).without_background(), Vec2::new(1200.0, 500.0));
    let inside = view(&harness).canvas.min + Vec2::new(30.0, 30.0);
    harness.state_mut().set_dark(false);
    let light = rendered(&mut harness);
    harness.state_mut().set_dark(true);
    let dark = rendered(&mut harness);
    // The map surface follows the theme, and so does the minimap's; neither may stay light on a dark one. The fine grid
    // makes the surface a wash a few units off its flat colour, which the thresholds leave room for.
    let minimap = view(&harness).canvas.max - Vec2::new(100.0, 25.0);
    for at in [inside, minimap] {
        assert!(lightness(pixel(&light, at)) > 180, "{:?}", pixel(&light, at));
        assert!(lightness(pixel(&dark, at)) < 80, "{:?}", pixel(&dark, at));
    }
}

#[test]
fn the_out_of_bounds_quadrant_is_tinted() {
    let items = LevelEntityInsertions { objects: vec![object(1, 4, 4, 4, 4)], ..Default::default() };
    let mut harness = drawn(DemoConnection::with_level(items).without_background(), Vec2::new(1200.0, 500.0));
    harness.state_mut().set_dark(false);
    let image = rendered(&mut harness);
    // Either side of the origin, away from grid lines and the object, so only the tint separates them.
    let outside = pixel(&image, screen(&harness, Pos2::new(-40.0, -40.0)));
    let inside = pixel(&image, screen(&harness, Pos2::new(40.0, 150.0)));
    assert!(outside[1] < inside[1], "{outside:?} {inside:?}");
    assert!(i32::from(outside[0]) - i32::from(outside[1]) > i32::from(inside[0]) - i32::from(inside[1]));
}

#[test]
fn an_object_in_front_culls_the_one_behind() {
    // A level is one array of blocks, not a stack of objects: a cell keeps whichever object wrote it last, and the
    // eraser every tileset has clears what it is laid over.
    let blank = RgbaImage { width: 16, height: 16, pixels: vec![0; 16 * 16 * 4] };
    let red = RgbaImage { width: 16, height: 16, pixels: [230, 0, 0, 255].repeat(16 * 16) };
    let mut blocks = vec![blank; 7];
    blocks.push(red);
    let layout = |tile| ObjectLayout { rows: vec![vec![ObjectLayoutTile::new(tile, 0)]], ..Default::default() };
    let tileset = DemoTileset { blocks, objects: vec![layout(7), layout(OBJECT_LAYOUT_EMPTY_TILE)], behaviors: Vec::new() };
    let items = LevelEntityInsertions { objects: vec![object(0, 4, 4, 4, 4), object(1, 5, 5, 2, 2)], ..Default::default() };
    let connection = DemoConnection::with_level(items).without_background().with_tileset(0, Some(tileset));
    let mut harness = drawn(connection, Vec2::new(1300.0, 600.0));
    harness.state_mut().set_dark(false);
    view_mut(&mut harness).zoom = 1.0;
    view_mut(&mut harness).zoom_pinned = true;
    view_mut(&mut harness).pan = Vec2::ZERO;
    view_mut(&mut harness).options.show_grid = false;
    let image = rendered(&mut harness);
    let behind = pixel(&image, screen(&harness, Pos2::new(4.0 * 16.0 + 8.0, 4.0 * 16.0 + 8.0)));
    let erased = pixel(&image, screen(&harness, Pos2::new(5.0 * 16.0 + 8.0, 5.0 * 16.0 + 8.0)));
    assert!(i32::from(behind[0]) > i32::from(behind[1]) + 60, "{behind:?}");
    assert!(i32::from(erased[0]) < i32::from(erased[1]) + 30, "{erased:?}");
}

#[test]
fn actors_draw_their_own_art_and_the_rest_are_numbered_markers() {
    let items = LevelEntityInsertions {
        sprites: vec![
            SpriteInsertion { value: NsmbLevelStageObject { id: 21, x: 4, y: 4, ..Default::default() }, hash: None },
            SpriteInsertion { value: NsmbLevelStageObject { id: 55, x: 8, y: 4, ..Default::default() }, hash: None },
        ],
        ..Default::default()
    };
    let mut harness = drawn(DemoConnection::with_level(items).without_background(), Vec2::new(1300.0, 600.0));
    harness.state_mut().set_dark(false);
    view_mut(&mut harness).zoom = 2.0;
    view_mut(&mut harness).zoom_pinned = true;
    view_mut(&mut harness).pan = Vec2::ZERO;
    view_mut(&mut harness).options.show_grid = false;
    let image = rendered(&mut harness);
    // The walker's body is brown; the actor with no renderer is a grey box.
    let body = pixel(&image, screen(&harness, Pos2::new(4.0 * 16.0 + 4.0, 4.0 * 16.0 + 11.0)));
    assert!(i32::from(body[0]) > i32::from(body[2]) + 60, "{body:?}");
    let marker = pixel(&image, screen(&harness, Pos2::new(8.0 * 16.0 + 2.0, 4.0 * 16.0 + 2.0)));
    assert!(i32::from(marker[0]).abs_diff(i32::from(marker[2])) < 20 && marker[0] < 200, "{marker:?}");
}

#[test]
fn a_trackpad_pans_with_two_fingers_and_zooms_with_command() {
    let mut harness = level_with(LevelEntityInsertions { objects: vec![object(1, 2, 3, 1, 1)], ..Default::default() });
    let at = view(&harness).canvas.center();
    let (pan, zoom) = (view(&harness).pan, view(&harness).zoom);
    let scroll = |harness: &Harness<'_, BigStarApp>, delta: Vec2, modifiers: Modifiers| {
        harness.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta, modifiers, phase: egui::TouchPhase::Move });
    };
    harness.hover_at(at);
    harness.run();
    // The level follows the fingers, both ways, and the zoom stays where it was.
    scroll(&harness, Vec2::new(30.0, -20.0), Modifiers::NONE);
    harness.run();
    assert!((view(&harness).pan - (pan - Vec2::new(30.0, -20.0) / zoom)).length() < 0.01);
    assert_eq!((view(&harness).zoom, view(&harness).zoom_pinned), (zoom, false));
    // With Cmd (Ctrl elsewhere) held it zooms about the pointer instead.
    let under = view(&harness).to_world(at);
    scroll(&harness, Vec2::new(0.0, 50.0), Modifiers::COMMAND);
    harness.run();
    assert!(view(&harness).zoom > zoom);
    assert!((view(&harness).to_world(at) - under).length() < 0.01);
}

// --- the side panels -----------------------------------------------------------------------------------------------

/// The demo level's walker (sprite 0), selected.
fn walker_selected() -> Harness<'static, BigStarApp> {
    // Tall enough that the actor's fields sit under the list without scrolling.
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 900.0));
    select(&mut harness, &[Item::new(Kind::Sprite, 0)]);
    harness.run();
    harness
}

#[test]
fn a_selected_actor_shows_its_name_and_its_own_fields() {
    let mut harness = walker_selected();
    assert_eq!(view(&harness).panel.tab, Tab::Entity(Kind::Sprite));
    harness.get_by_label("Walker");
    harness.get_by_label("Enemies · actor type 21");
    // A field that is on or off is a checkbox; clicking it writes only its bit.
    harness.get_by_label("Winged").click();
    harness.run();
    let snapshot = &view(&harness).snapshot;
    assert_eq!(snapshot.course.sprites[0].settings, 1 << 8);
    assert_eq!(snapshot.undo_names[0], "Edit sprite");
}

#[test]
fn the_panel_lists_every_item_of_its_kind_and_a_line_selects_it() {
    let node = |x| NsmbLevelPathNode { x, y: 64, ..Default::default() };
    let path = |id, nodes| NsmbLevelPath { id, nodes, unknown: 0 };
    let mut harness =
        level_with(LevelEntityInsertions { paths: vec![path(7, vec![node(32)]), path(3, vec![node(96), node(128)])], ..Default::default() });
    let first = view(&harness).snapshot.course.paths.len() - 2;
    harness.get_by_label("Paths").click();
    harness.run();
    // Picking a path selects all of it and opens it onto its nodes.
    harness.get_by_label("Path 3 · 2 nodes").click();
    harness.run();
    assert_eq!(view(&harness).selection, [Item::new(Kind::Path, first + 1)]);
    harness.get_by_label("Node 1 (128, 64)").click();
    harness.run();
    assert_eq!(view(&harness).selection, [Item::node(Kind::Path, first + 1, 1)]);
    // A selection made on the canvas lights its line the same way.
    select(&mut harness, &[Item::node(Kind::Path, first, 0)]);
    harness.get_by_label("Node 0 (32, 64)");
    assert!(harness.query_by_label("Node 1 (128, 64)").is_none());
}

#[test]
fn a_number_is_a_spin_box_whose_arrows_step_it_by_one_edit() {
    let mut harness = walker_selected();
    // The position's line comes first with arrows, and is laid out from the right: Y's arrows come before X's.
    let x = view(&harness).snapshot.course.sprites[0].x;
    harness.get_all_by_label("Increase").nth(1).unwrap().click();
    harness.run();
    assert_eq!(view(&harness).snapshot.course.sprites[0].x, x + 1);
    harness.get_all_by_label("Decrease").nth(1).unwrap().click();
    harness.run();
    let snapshot = &view(&harness).snapshot;
    assert_eq!((snapshot.course.sprites[0].x, snapshot.undo_names.len()), (x, 2));
}

#[test]
fn the_properties_panel_holds_its_width_over_an_actors_fields() {
    let mut harness = walker_selected();
    let edge = |harness: &Harness<'_, BigStarApp>| harness.get_by_label("Level Data").rect().left();
    let before = edge(&harness);
    harness.run_steps(8);
    assert_eq!(edge(&harness), before);
}

#[test]
fn the_history_menus_take_several_steps_at_once() {
    let mut harness = walker_selected();
    for x in [1, 2, 3] {
        let edit = view(&harness).move_edit(Vec2::new(16.0 * x as f32, 0.0)).unwrap();
        harness.state_mut().session_mut().unwrap().edit(edit);
        harness.run();
    }
    let x = view(&harness).snapshot.course.sprites[0].x;
    assert_eq!(view(&harness).snapshot.undo_names.len(), 3);
    harness.get_by_label("Undo").click_secondary();
    harness.run();
    // The third entry is the oldest step: picking it undoes all three.
    harness.get_all_by_label("Move selection").nth(2).unwrap().click();
    harness.run();
    let view = view(&harness);
    assert_eq!((view.snapshot.course.sprites[0].x, view.snapshot.redo_names.len()), (x - 6, 3));
}

#[test]
fn copy_and_paste_go_through_the_system_clipboard() {
    let mut harness = walker_selected();
    harness.event(egui::Event::Copy);
    // The copy is in the output of the frame that handled it.
    harness.step();
    let copied = harness
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(text) => Some(text.clone()),
            _ => None,
        })
        .expect("copied");
    assert!(copied.starts_with("BigStar-Entities/1\n"));
    let sprites = view(&harness).snapshot.course.sprites.len();
    harness.event(egui::Event::Paste(copied));
    harness.run();
    let view_now = view(&harness);
    assert_eq!(view_now.snapshot.course.sprites.len(), sprites + 1);
    assert_eq!(view_now.snapshot.undo_names[0], "Paste selection");
    assert_eq!(view_now.selection, [Item::new(Kind::Sprite, sprites)], "what was pasted is selected");
    // Something else on the clipboard is reported, not pasted.
    harness.event(egui::Event::Paste("hello".to_owned()));
    harness.run();
    let session = harness.state().session().unwrap();
    assert_eq!(session.notice.as_ref().map(|n| n.text.as_str()), Some("The clipboard holds nothing copied from a Big Star level"));
    // Cut is a copy and a delete.
    select(&mut harness, &[Item::new(Kind::Sprite, 0)]);
    harness.event(egui::Event::Cut);
    harness.run();
    assert_eq!(view(&harness).snapshot.course.sprites.len(), sprites);
}

#[test]
fn level_data_flags_are_set_from_the_tab() {
    let mut harness = walker_selected();
    harness.get_by_label("Level Data").click();
    harness.run();
    harness.get_by_label("demo.lowGravity");
    let toggle = harness.query_all(egui_kittest::kittest::by().role(egui::accesskit::Role::CheckBox)).next().unwrap();
    toggle.click();
    harness.run();
    let snapshot = &view(&harness).snapshot;
    assert_eq!(snapshot.glue.flags, [bigstar_formats::fnv1a_32("demo.lowGravity")]);
    assert_eq!(snapshot.undo_names[0], "Edit level data");
}

#[test]
fn the_picker_chooses_what_a_right_click_places() {
    let mut harness = walker_selected();
    harness.get_by_label("Palette").click();
    harness.run();
    harness.get_by_label("Object 0:2").click();
    harness.run();
    let placement = &view(&harness).placement;
    assert_eq!((placement.kind, placement.object_slot, placement.object_id), (Kind::Object, 0, 2));
    // The properties follow what a right-click makes.
    assert_eq!(view(&harness).panel.tab, Tab::Entity(Kind::Object));
    harness.get_by_label("Object 0:5 - eraser");

    // The picker's tabs come first; the properties have tabs of the same names.
    harness.get_all_by_label("Sprites").next().unwrap().click();
    harness.run();
    harness.get_by_label("Enemies (2)").click();
    harness.run();
    harness.get_by_label("Walker (21)").click();
    harness.run();
    let placement = &view(&harness).placement;
    assert_eq!((placement.kind, placement.sprite_type, placement.sprite_hash), (Kind::Sprite, 21, None));
    // The open category asked the host for its actors' stills.
    assert!(view(&harness).art.draws.keys().any(|k| k.sprite_type == 21 && k.x == 0 && k.y == 0));

    harness.get_by_label("Custom actors").click();
    harness.run();
    harness.get_by_label("Signpost  [object 400]").click();
    harness.run();
    assert_eq!(view(&harness).placement.sprite_hash, Some(crate::demo::SIGNPOST));

    // The filter narrows every page.
    harness.get_all_by_label("Objects").next().unwrap().click();
    harness.run();
    // The picker's filter is the first text field in the window.
    harness.query_all(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput)).next().unwrap().focus();
    harness.run();
    harness.query_all(egui_kittest::kittest::by().role(egui::accesskit::Role::TextInput)).next().unwrap().type_text("eraser");
    harness.run();
    assert_eq!(harness.state().picker.search, "eraser");
    assert_eq!(harness.query_all_by_label_contains("Object 0:").count(), 1);
}

// --- the workspace and the level tools ------------------------------------------------------------------------------

fn open_document(harness: &mut Harness<'_, BigStarApp>, area: &str) -> bigstar_protocol::LevelId {
    harness.state_mut().session_mut().unwrap().open_level(area);
    harness.run();
    harness.state().session().unwrap().active.expect("the opened document is active")
}

#[test]
fn each_level_is_a_document_and_the_focused_one_is_active() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    let first = harness.state().session().unwrap().active.unwrap();
    let second = open_document(&mut harness, "A01_2");
    assert_ne!(first, second);
    assert_eq!(harness.state().workspace.documents().len(), 2);
    // The properties and the top bar follow the tab in front.
    harness.get_by_label("1-1 Area 2 · 2 objects · 1 sprites · 1 entrances · revision 1");
    // Bringing the first tab forward makes it the active document.
    harness.state_mut().workspace.show_document(first);
    harness.run();
    assert_eq!(harness.state().session().unwrap().active, Some(first));
    // An edit goes to the document it was made in.
    let mut moved = view(&harness).clone();
    moved.selection = vec![Item::new(Kind::Object, 0)];
    harness.state_mut().session_mut().unwrap().edit(moved.move_edit(Vec2::new(16.0, 0.0)).unwrap());
    harness.run();
    let session = harness.state().session().unwrap();
    assert!(session.levels[&first].snapshot.dirty && !session.levels[&second].snapshot.dirty);
}

#[test]
fn closing_a_document_with_unsaved_edits_asks_first() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    let level = harness.state().session().unwrap().active.unwrap();
    select(&mut harness, &[Item::new(Kind::Object, 0)]);
    harness.key_press(Key::Delete);
    harness.run();
    {
        let app = harness.state_mut();
        let (workspace, session) = (&mut app.workspace, app.session.as_mut().unwrap());
        workspace.close_document(session, level);
    }
    harness.run();
    harness.get_by_label("This level has changes that were not saved. Closing it loses them.");
    harness.get_by_label("Stay").click();
    harness.run();
    assert!(harness.state().workspace.is_document(level), "staying keeps it");
    {
        let app = harness.state_mut();
        let (workspace, session) = (&mut app.workspace, app.session.as_mut().unwrap());
        workspace.close_document(session, level);
    }
    harness.run();
    harness.get_by_label("Close without saving").click();
    harness.run();
    assert!(!harness.state().workspace.is_document(level));
    assert!(!harness.state().session().unwrap().levels.contains_key(&level), "the host session is closed");
}

#[test]
fn closing_a_document_without_saving_drops_its_edits_from_the_preview_too() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    harness.get_all_by_label("Area 1").next().unwrap().click();
    harness.run();
    harness.get_all_by_label("Edit").nth(1).unwrap().click();
    harness.run();
    let level = harness.state().session().unwrap().active.unwrap();
    assert_eq!(harness.state().workspace.preview, Some(level), "the preview shares the document's session");
    select(&mut harness, &[Item::new(Kind::Object, 0)]);
    harness.key_press(Key::Delete);
    harness.run();
    {
        let app = harness.state_mut();
        let (workspace, session) = (&mut app.workspace, app.session.as_mut().unwrap());
        workspace.close_document(session, level);
    }
    harness.run();
    harness.get_by_label("Close without saving").click();
    harness.run();
    let app = harness.state();
    let session = app.session().unwrap();
    let shown = &session.levels[&app.workspace.preview.unwrap()];
    assert!(!shown.snapshot.dirty, "the preview shows the level as saved");
}

#[test]
fn the_canvas_switches_are_shared_by_every_level_and_remembered() {
    let services = MemoryServices::default();
    let later = services.sharing_values();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    let first = open_document(&mut harness, "A01_1");
    {
        let options = &mut harness.state_mut().session_mut().unwrap().levels.get_mut(&first).unwrap().options;
        (options.show_grid, options.show_minimap, options.snap_size) = (false, false, 8.0);
        options.backgrounds = Backgrounds::Hidden;
    }
    harness.run();
    // A level opened afterwards has them as they were left, and so does one that was already open.
    let second = open_document(&mut harness, "A01_2");
    let options = |harness: &Harness<'_, BigStarApp>, level| harness.state().session().unwrap().levels[&level].options;
    assert_eq!(options(&harness, second), options(&harness, first));
    assert!(!options(&harness, second).show_grid);
    harness.state_mut().session_mut().unwrap().levels.get_mut(&second).unwrap().options.show_grid = true;
    harness.run();
    assert!(options(&harness, first).show_grid && !options(&harness, first).show_minimap);
    drop(harness);

    // A later run starts with them.
    let app = BigStarApp::with_connection(Box::new(later), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    let level = open_document(&mut harness, "A01_1");
    let options = options(&harness, level);
    assert!((options.show_grid, options.show_minimap, options.snap_size) == (true, false, 8.0), "{options:?}");
    assert_eq!(options.backgrounds, Backgrounds::Hidden);
}

#[test]
fn open_documents_and_their_cameras_are_remembered_per_host_and_project() {
    let services = MemoryServices::default();
    let later = services.sharing_values();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    open_document(&mut harness, "A01_1");
    let second = open_document(&mut harness, "A01_2");
    harness.state_mut().session_mut().unwrap().levels.get_mut(&second).unwrap().zoom_about(2.0, Pos2::new(700.0, 400.0));
    let pan = harness.state().session().unwrap().levels[&second].pan;
    harness.state_mut().workspace.detach(second);
    harness.run();
    drop(harness);

    // A later run, against the same host and project, puts it all back.
    let app = BigStarApp::with_connection(Box::new(later), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    harness.run();
    let app = harness.state();
    let session = app.session().unwrap();
    assert_eq!(app.workspace.documents().len(), 2);
    let restored = session.levels.values().find(|v| v.snapshot.area == "A01_2").unwrap();
    assert_eq!((restored.zoom, restored.pan, restored.zoom_pinned), (2.0, pan, true));
    assert!(app.workspace.detached.contains(&restored.snapshot.level), "it reopens in its own window");
}

#[test]
fn the_hex_editor_replaces_the_level_as_one_step() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.get_by_label("Tools").click();
    harness.run();
    harness.get_by_label("Hex Editor…").click();
    harness.run();
    let objects = view(&harness).snapshot.objects.len();
    {
        let editor = harness.state_mut().tools.hex.as_mut().expect("the hex editor is open");
        // One object fewer: each is ten bytes of the BGDat.
        editor.bgdat.select(0..10);
        editor.bgdat.delete(false);
    }
    harness.run();
    // The editor's Save, not the level's: the window is drawn last.
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    let snapshot = &view(&harness).snapshot;
    assert_eq!((snapshot.objects.len(), snapshot.undo_names[0].as_str()), (objects - 1, "Hex edit level"));
    assert!(harness.state().tools.hex.is_none());
}

#[test]
fn the_hex_editor_types_over_bytes_and_keeps_its_own_undo() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.state_mut().session_mut().unwrap().request_level_bytes(crate::BytesFor::HexEditor);
    harness.run();
    let first = harness.state().tools.hex.as_ref().unwrap().course.bytes[0];
    // Two digits write over the first byte.
    let level_undo = view(&harness).snapshot.undo_names.len();
    {
        let course = &mut harness.state_mut().tools.hex.as_mut().unwrap().course;
        course.type_digit(0xa);
        course.type_digit(0x5);
    }
    harness.run();
    let editor = harness.state().tools.hex.as_ref().unwrap();
    assert_eq!((editor.course.bytes[0], editor.course.caret, editor.course.dirty()), (0xa5, 1, first != 0xa5));
    assert_eq!(view(&harness).snapshot.undo_names.len(), level_undo, "nothing reaches the level until Save");
    // Cancel leaves the level as it was.
    harness.get_all_by_label("Cancel").last().unwrap().click();
    harness.run();
    assert!(harness.state().tools.hex.is_none());
    assert_eq!(view(&harness).snapshot.undo_names.len(), level_undo);
}

#[test]
fn a_level_exports_and_imports_as_nml() {
    let services = MemoryServices::default();
    let shelf = services.shelf.clone();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    open_document(&mut harness, "A01_2");
    harness.state_mut().session_mut().unwrap().request_level_bytes(crate::BytesFor::Export);
    harness.run();
    let (name, bytes) = shelf.lock().unwrap().saved.pop().expect("exported");
    assert_eq!(name, "1-1 Area 2.nml");
    let package = bigstar_level::decode_nml(&bytes).unwrap();
    assert_eq!((package.course_file_id, package.bgdat_file_id), (300, 301));

    // Importing it into 1-1 makes 1-1 hold 1-1 Area 2.
    open_document(&mut harness, "A01_1");
    shelf.lock().unwrap().to_open.push_back(("1-1 Area 2.nml".to_owned(), bytes));
    harness.get_by_label("File").click();
    harness.run();
    harness.get_by_label("Import Level (.nml)…").click();
    harness.run();
    let snapshot = &view(&harness).snapshot;
    assert_eq!((snapshot.area.as_str(), snapshot.objects.len(), snapshot.undo_names[0].as_str()), ("A01_1", 2, "Import level"));
}

#[test]
fn the_finder_opens_the_level_and_selects_what_it_found() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.get_by_label("Edit").click();
    harness.run();
    harness.get_by_label("Find Sprite…").click();
    harness.run();
    harness.state_mut().tools.finder.as_mut().unwrap().id = 21;
    harness.get_by_label("Find").click();
    harness.run();
    let found = harness.state().session().unwrap().found.clone().unwrap();
    assert_eq!(found.entities.iter().map(|e| (e.area.as_str(), e.index)).collect::<Vec<_>>(), [("A01_1", 0), ("A01_1", 3), ("A01_2", 0)]);
    // Picking the one in Area 2 opens it and selects the sprite.
    harness.get_by_label("1-1 Area 2").click();
    harness.run();
    let view = view(&harness);
    assert_eq!((view.snapshot.area.as_str(), view.selection.as_slice()), ("A01_2", &[Item::new(Kind::Sprite, 0)][..]));
}

#[test]
fn the_sprite_event_viewer_lists_every_event_a_sprite_names() {
    let items = LevelEntityInsertions {
        sprites: vec![
            SpriteInsertion { value: NsmbLevelStageObject { id: 21, events: [5, 0], ..Default::default() }, hash: None },
            SpriteInsertion { value: NsmbLevelStageObject { id: 64, events: [2, 5], ..Default::default() }, hash: None },
        ],
        ..Default::default()
    };
    let mut harness = level_with(items);
    harness.state_mut().tools.events = true;
    harness.run();
    // The window and its heading both carry the name.
    assert!(harness.query_all_by_label("Sprite Event Viewer").next().is_some());
    // Two rows name event 5 and one names event 2, which comes first.
    assert_eq!(harness.query_all_by_label("5").count(), 2);
    assert_eq!(harness.query_all_by_label("Coin").count(), 2, "one row for each event the coin names");
    harness.get_all_by_label("1").next().unwrap().click();
    harness.run();
    assert_eq!(view(&harness).selection, [Item::new(Kind::Sprite, 1)]);
}

#[test]
fn autosaves_an_interrupted_session_left_are_offered_back() {
    let mut harness = harness_on(DemoConnection::new().with_recovery(), Vec2::new(1500.0, 800.0));
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    harness.get_by_label("Big Star found 1 autosaved level(s) from a session that ended without saving. Recover them?");
    harness.get_by_label("Recover").click();
    harness.run();
    harness.run();
    // The recovered level opens as a document holding the autosave, unsaved.
    let view = view(&harness);
    assert_eq!((view.snapshot.area.as_str(), view.snapshot.objects.len(), view.snapshot.dirty), ("A01_1", 6, true));
    assert_eq!(view.snapshot.undo_names[0], "Recover autosave");
}

// --- the filesystem dock ------------------------------------------------------------------------------------------------

/// The app with the demo project open and the Filesystem dock showing; the shelf is the test's file dialogs.
fn filesystem() -> (Harness<'static, BigStarApp>, std::sync::Arc<std::sync::Mutex<crate::FileShelf>>) {
    let services = MemoryServices::default();
    let shelf = services.shelf.clone();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    harness.get_by_label("Filesystem").click();
    harness.run();
    (harness, shelf)
}

#[test]
fn the_dock_lists_the_rom_in_id_order_and_opens_archives_on_demand() {
    let (mut harness, _) = filesystem();
    // The root is open; its folders are rows, and open with their arrow (the order is `build_tree`'s, tested there).
    harness.get_by_label("course");
    harness.state_mut().files.expanded.insert("course".to_owned());
    harness.state_mut().files.expanded.insert("dat".to_owned());
    harness.run();
    harness.get_by_label("A01_1.bin");
    // An archive lists its members the first time it opens.
    assert!(harness.query_by_label("a.bin").is_none());
    harness.state_mut().files.expanded.insert("dat/demo.narc".to_owned());
    harness.state_mut().session_mut().unwrap().list_archive("dat/demo.narc");
    harness.run();
    harness.get_by_label("a.bin");
    // Picking a file says what it is and which editor opens it.
    harness.get_by_label("A01_1.bin").click();
    harness.run();
    harness.get_by_label_contains("course/A01_1.bin   ·   File ID: 100");
}

#[test]
fn files_are_added_under_z_new_replaced_and_removed_through_the_module() {
    let (mut harness, shelf) = filesystem();
    shelf.lock().unwrap().to_open.push_back(("hello.bin".to_owned(), vec![1, 2, 3]));
    harness.get_by_label("Add file").click();
    harness.run();
    // The destination is asked for, under z_new/ by default.
    harness.get_by_label("Add").click();
    harness.run();
    harness.run();
    let listing = harness.state().session().unwrap().filesystem.clone().unwrap();
    let added = listing.entries.iter().find(|e| e.path == "z_new/hello.bin").expect("added");
    assert_eq!((added.status, added.from_module, added.id_is_planned), (bigstar_protocol::FileStatus::Pending, true, true));
    assert!(listing.pending);
    harness.get_by_label_contains("Module files have changed since the last build");

    // Replacing a file the ROM holds writes the module's copy over it.
    harness.state_mut().files.expanded.insert("message".to_owned());
    harness.run();
    harness.get_by_label("msg.bmg").click();
    harness.run();
    shelf.lock().unwrap().to_open.push_back(("msg.bmg".to_owned(), b"MESGbmg1new".to_vec()));
    harness.get_by_label("Replace").click();
    harness.run();
    harness.run();
    let listing = harness.state().session().unwrap().filesystem.clone().unwrap();
    assert!(listing.entries.iter().any(|e| e.path == "message/msg.bmg" && e.from_module));

    // A module's file can be removed, after asking.
    harness.get_by_label("msg.bmg").click();
    harness.run();
    harness.get_by_label("Remove").click();
    harness.run();
    harness.get_by_label_contains("Move this module file to the project's trash");
    harness.get_all_by_label("Remove").last().unwrap().click();
    harness.run();
    harness.run();
    let listing = harness.state().session().unwrap().filesystem.clone().unwrap();
    assert!(listing.entries.iter().any(|e| e.path == "message/msg.bmg" && !e.from_module));
}

#[test]
fn a_file_extracts_and_opens_as_a_hex_document_that_saves_to_the_module() {
    let (mut harness, shelf) = filesystem();
    harness.state_mut().files.expanded.insert("message".to_owned());
    harness.run();
    harness.get_by_label("msg.bmg").click();
    harness.run();
    harness.get_by_label("Extract").click();
    harness.run();
    assert_eq!(shelf.lock().unwrap().saved.pop(), Some(("msg.bmg".to_owned(), crate::demo::demo_messages())));

    harness.get_by_label("Hex edit").click();
    harness.run();
    let (key, path) = {
        let files = &harness.state().workspace.files;
        let (key, doc) = files.iter().next().expect("a document");
        (*key, doc.path.clone())
    };
    assert_eq!(path, "message/msg.bmg");
    if let crate::workspace::FileView::Hex(hex) = &mut harness.state_mut().workspace.files.get_mut(&key).unwrap().view {
        hex.select(0..hex.bytes.len());
        hex.paste("AA BB");
    }
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    harness.state_mut().session_mut().unwrap().read_file("message/msg.bmg", crate::session::FileRead::Extract);
    harness.run();
    assert_eq!(shelf.lock().unwrap().saved.pop().map(|(_, bytes)| bytes), Some(vec![0xaa, 0xbb]));
}

#[test]
fn a_level_row_hands_the_person_over_to_its_file() {
    let (mut harness, _) = filesystem();
    harness.get_by_label("Levels").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    // A level of several areas holds them; its areas name their files.
    harness.get_all_by_label("Area 1").next().unwrap().click_secondary();
    harness.run();
    harness.get_by_label("Show Course in Filesystem").click();
    harness.run();
    let files = &harness.state().files;
    assert_eq!(files.selected.as_deref(), Some("course/A01_1.bin"));
    assert!(files.expanded.contains("course"));
    harness.get_by_label("A01_1.bin");
}

#[test]
fn a_tile_sheet_opens_in_the_graphics_editor_pairs_its_palette_and_saves_painted_pixels() {
    let (mut harness, _) = filesystem();
    harness.state_mut().files.expanded.insert("BG_ncg".to_owned());
    harness.run();
    harness.get_by_label("demo_ncg.bin").click();
    harness.run();
    // It is picked up as graphics, and Open opens the graphics editor rather than hex.
    harness.get_by_label_contains("Opens in: Graphics");
    harness.get_by_label("Open").click();
    harness.run();
    let key = *harness.state().workspace.files.keys().next().unwrap();
    assert!(matches!(harness.state().workspace.files[&key].view, crate::workspace::FileView::Graphics(..)));
    harness.get_by_label_contains("Indices are shown as greys until a palette file is chosen.");

    // Pairing the sibling palette (as its combo box does) reads it from the host and colours the sheet.
    {
        let doc = harness.state_mut().workspace.files.get_mut(&key).unwrap();
        doc.waiting.push(("BG_ncg/demo_ncl.bin".to_owned(), true));
    }
    harness.state_mut().session_mut().unwrap().read_file("BG_ncg/demo_ncl.bin", crate::session::FileRead::Companion);
    harness.run();
    harness.run();
    let crate::workspace::FileView::Graphics(doc, _) = &harness.state().workspace.files[&key].view else { panic!() };
    assert_eq!(doc.pairing.palette.as_deref(), Some("BG_ncg/demo_ncl.bin"));
    assert!(!doc.render().info.contains("greys"));

    // Painting index 1 into the first pixel and saving writes the sheet into the module.
    if let crate::workspace::FileView::Graphics(doc, _) = &mut harness.state_mut().workspace.files.get_mut(&key).unwrap().view {
        doc.selected_color = 1;
        doc.paint(0, 0);
    }
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    let listing = harness.state().session().unwrap().filesystem.clone().unwrap();
    assert!(listing.entries.iter().any(|e| e.path == "BG_ncg/demo_ncg.bin" && e.from_module));
}

/// The bytes the demo host holds for `path` now, read back the way Extract reads them.
fn file_bytes(harness: &mut Harness<'static, BigStarApp>, shelf: &std::sync::Arc<std::sync::Mutex<crate::FileShelf>>, path: &str) -> Vec<u8> {
    harness.state_mut().session_mut().unwrap().read_file(path, crate::session::FileRead::Extract);
    harness.run();
    shelf.lock().unwrap().saved.pop().map(|(_, bytes)| bytes).unwrap()
}

#[test]
fn a_message_file_opens_in_the_message_editor_and_saves_its_edits() {
    let (mut harness, shelf) = filesystem();
    harness.state_mut().files.expanded.insert("message".to_owned());
    harness.run();
    harness.get_by_label("msg.bmg").click();
    harness.run();
    harness.get_by_label_contains("Opens in: Messages");
    harness.get_by_label("Open").click();
    harness.run();
    let key = *harness.state().workspace.files.keys().next().unwrap();
    assert!(matches!(harness.state().workspace.files[&key].view, crate::workspace::FileView::Messages { .. }));
    harness.get_by_label_contains("3 message(s) · ");
    // The list previews plain text; the editor holds the escape.
    harness.get_by_label(" Luigi has appeared!");
    harness.get_by_label("Game over").click();
    harness.run();
    harness.get_by_label_contains("9 character(s) · 1 line(s) · 0 escape sequence(s)");

    // Added at the end with the next free ID, then saved into the module.
    harness.get_by_label("Add").click();
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    let saved = bigstar_formats::parse_bmg(&file_bytes(&mut harness, &shelf, "message/msg.bmg")).unwrap();
    assert_eq!(saved.messages.len(), 4);
    assert_eq!(saved.messages[3].id, Some(43));
    assert_eq!(saved.messages[0], bigstar_formats::parse_bmg(&crate::demo::demo_messages()).unwrap().messages[0]);
}

#[test]
fn a_touch_layout_and_the_rom_metadata_open_as_pages_of_the_graphics_editor() {
    let (mut harness, shelf) = filesystem();
    harness.state_mut().files.expanded.insert("uiStudio".to_owned());
    harness.run();
    harness.get_by_label("demo.bnbl").click();
    harness.run();
    harness.get_by_label_contains("Opens in: Graphics");
    harness.get_by_label("Open").click();
    harness.run();
    harness.get_by_label_contains("JNBL touch rectangles.");
    harness.get_by_label("LayoutPreview");
    let key = *harness.state().workspace.files.keys().next().unwrap();
    let crate::workspace::FileView::Resource { doc, .. } = &harness.state().workspace.files[&key].view else { panic!("not a resource page") };
    assert_eq!(doc.rows, [["16", "32", "0", "0", "64", "48"], ["128", "144", "0", "0", "96", "32"]]);
    harness.get_by_label("Add").click();
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    assert_eq!(bigstar_formats::parse_bnbl(&file_bytes(&mut harness, &shelf, "uiStudio/demo.bnbl")).unwrap().entries.len(), 3);

    // File 131 opens as the metadata page from the Tools menu, whatever its name says.
    harness.get_by_label("Tools").click();
    harness.run();
    harness.get_by_label("ROM User Metadata…").click();
    harness.run();
    harness.run();
    harness.get_by_label_contains("Names and object notes for this ROM");
    let doc = harness.state().workspace.files.values().find(|doc| doc.path == "NSMBeUD").unwrap();
    let crate::workspace::FileView::Resource { doc, .. } = &doc.view else { panic!("not a resource page") };
    assert_eq!(doc.sections.len(), 2);
    assert_eq!(doc.sections[1].entries, [("3".to_owned(), "Pipe".to_owned())]);
}

#[test]
fn the_rom_banner_opens_from_the_tools_menu_and_saves_to_the_configured_file() {
    let (mut harness, _) = filesystem();
    harness.get_by_label("Tools").click();
    harness.run();
    harness.get_by_label("ROM Banner…").click();
    harness.run();
    harness.run();
    let (key, doc) = harness.state().workspace.files.iter().next().map(|(k, d)| (*k, d)).unwrap();
    assert_eq!(doc.destination, crate::workspace::Destination::Banner);
    assert_eq!(doc.path, "assets/banner.bin");
    harness.get_by_label("GraphicsCanvas");
    harness.get_by_label("Titles").click();
    harness.run();
    if let crate::workspace::FileView::Resource { doc, .. } = &mut harness.state_mut().workspace.files.get_mut(&key).unwrap().view {
        assert_eq!(doc.titles[1], "Big Star Demo\nNintendo");
        doc.titles[1] = "Edited".to_owned();
        doc.dirty = true;
    }
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    // Opened again, the banner is what was saved.
    harness.state_mut().workspace.files.clear();
    harness.get_by_label("Tools").click();
    harness.run();
    harness.get_by_label("ROM Banner…").click();
    harness.run();
    harness.run();
    let doc = harness.state().workspace.files.values().next().unwrap();
    let crate::workspace::FileView::Resource { doc, .. } = &doc.view else { panic!("not a resource page") };
    assert_eq!(doc.titles[1], "Edited");
    assert!(!doc.dirty);
}

#[test]
fn the_tileset_workspace_opens_from_tools_and_saves_only_what_changed() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    let level = view(&harness).snapshot.level;
    harness.get_by_label("Tools").click();
    harness.run();
    harness.get_by_label("Tilesets and Backgrounds…").click();
    harness.run();
    harness.run();
    assert!(harness.state().workspace.tilesets.contains_key(&level));
    for tab in ["Slots", "Graphics", "Map16", "Objects", "Behaviors", "Background", "Foreground"] {
        harness.get_by_label(tab);
    }
    harness.get_by_label("Map16").click();
    harness.run();
    harness.get_by_label("Show 3D tiles");
    harness.get_by_label("GraphicsCanvas");

    // A Map16 quadrant and the level tileset id change; Save writes the one file and edits the level.
    {
        let (doc, _) = harness.state_mut().workspace.tilesets.get_mut(&level).unwrap();
        doc.map16_entry = 8;
        let mut value = doc.map16_value().unwrap();
        value.horizontal_flip = true;
        doc.set_map16_value(value);
        doc.references.tileset_id = 9;
    }
    harness.run();
    // The workspace's Save, drawn after the level's toolbar.
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    assert!(!harness.state().workspace.tilesets.contains_key(&level), "a saved workspace closes, as the dialog did");
    assert_eq!(view(&harness).snapshot.tileset_references.tileset_id, 9);
    let listing = harness.state().session().unwrap().filesystem.clone().expect("a write fetches the listing again");
    assert!(listing.entries.iter().any(|e| e.path == "BG_pnl/demo_t1_pnl.bin" && e.from_module));
    assert!(listing.entries.iter().all(|e| e.path != "BG_ncg/demo_t1_ncg.bin" || !e.from_module), "untouched files stay the ROM's");
    let log = &harness.state().session().unwrap().log;
    assert!(log.iter().any(|line| line.text == "Saved 1 edited graphics file(s) to the modules."));
}

#[test]
fn a_build_reports_into_the_output_panel_and_its_diagnostics_show_where() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.get_by_label("Run Patcher").click();
    harness.run();
    harness.run();
    let state = &harness.state().session().unwrap().build;
    assert!(!state.running);
    assert_eq!(state.status, "Succeeded with 1 warning(s)");
    assert_eq!(state.diagnostics.len(), 1);
    assert!(harness.state().session().unwrap().notice.as_ref().is_some_and(|n| !n.error && n.text == "NCPatcher completed with 1 warning(s)."));
    // The Output panel opened: the diagnostic's headline, its location, and the build's own lines in the log.
    harness.get_by_label("NCP0204");
    harness.get_by_label("unused variable 'speed'");
    harness.get_by_label("[arm9] compiling source/main.cpp");
    harness.get_by_label("  -> arm9 (patched)");
    harness.get_by_label("source/main.cpp:3:9").click();
    harness.run();
    harness.get_by_label_contains("Line 3, column 9");
    harness.get_by_label("OK").click();
    harness.run();
    // The build applied: the open level was opened again from the new ROM.
    assert_eq!(harness.state().workspace.documents().len(), 1);
}

#[test]
fn the_variant_selector_switches_the_whole_window() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.get_by_label("Variant:");
    // The selector shows the active variant, none yet.
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|node| egui_kittest::kittest::NodeT::accesskit_node(node).value().as_deref() == Some("(none)"))
        .expect("the variant selector")
        .click();
    harness.run();
    harness.get_by_label("fr").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().session().unwrap().settings.as_ref().unwrap().active_variant, "fr");
    assert_eq!(harness.state().workspace.documents().len(), 1, "the open level follows into the variant");
}

/// The frontend on the demo host with Preferences open, and the values it stores.
fn preferences_open(services: MemoryServices) -> (Harness<'static, BigStarApp>, MemoryServices) {
    let stored = services.sharing_values();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    harness.get_by_label("File").click();
    harness.run();
    harness.get_by_label("Preferences…").click();
    harness.run();
    (harness, stored)
}

#[test]
fn a_floating_window_opens_in_the_middle_of_the_screen() {
    let (harness, _) = preferences_open(MemoryServices::default());
    // egui names a window's area after its title, which it holds as an `Option`.
    let window = harness.ctx.memory(|memory| memory.area_rect(egui::Id::new(Some("Preferences")))).unwrap();
    let screen = harness.ctx.content_rect();
    assert!((window.center() - screen.center()).length() < 1.0, "{window:?} in {screen:?}");
}

#[test]
fn a_theme_chosen_in_preferences_shows_at_once_and_is_kept_only_when_saved() {
    use crate::theme::{Theme, Tokens};
    let (mut harness, stored) = preferences_open(MemoryServices::default());
    let default = Theme::default().tokens(None);
    let ocean = Theme { family: "ocean", ..Theme::default() }.tokens(None);
    assert_eq!(*Tokens::get(&harness.ctx), default);
    harness.get_by_label("Ocean").click();
    harness.run();
    assert_eq!(*Tokens::get(&harness.ctx), ocean);
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(*Tokens::get(&harness.ctx), default);
    assert_eq!(crate::Services::load(&stored, crate::THEME_KEY), None);

    harness.get_by_label("File").click();
    harness.run();
    harness.get_by_label("Preferences…").click();
    harness.run();
    harness.get_by_label("Ocean").click();
    harness.get_by_label("Light").click();
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    let chosen = Theme { family: "ocean", dark: false, ..Theme::default() };
    assert_eq!(*Tokens::get(&harness.ctx), chosen.tokens(None));
    assert_eq!(crate::Services::load(&stored, crate::THEME_KEY), Some(chosen.stored()));
}

#[test]
fn the_systems_accent_colour_can_stand_in_for_the_themes() {
    use crate::theme::Tokens;
    let checkbox = "Use the system's accent colour";
    // A system that has no accent colour offers nothing to tick.
    let (harness, _) = preferences_open(MemoryServices::default());
    assert!(egui_kittest::kittest::NodeT::accesskit_node(&harness.get_by_label(checkbox)).is_disabled());

    let purple = [111, 86, 168];
    let (mut harness, stored) = preferences_open(MemoryServices::with_accent(purple));
    // It is what a new installation starts with, until the theme's own is asked for.
    assert_eq!(Tokens::get(&harness.ctx).primary, egui::Color32::from_rgb(111, 86, 168));
    harness.get_by_label(checkbox).click();
    harness.run();
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    assert_ne!(Tokens::get(&harness.ctx).primary, egui::Color32::from_rgb(111, 86, 168));
    assert_eq!(crate::Services::load(&stored, crate::THEME_KEY).as_deref(), Some("default dark own-accent"));
}

#[test]
fn preferences_choose_the_language_here_and_the_projects_autosave_and_backups_on_the_host() {
    let services = MemoryServices::default();
    let stored = services.sharing_values();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(DemoConnection::new()));
    let mut harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    harness.run();
    harness.state_mut().session_mut().unwrap().open_project("demo");
    harness.run();
    harness.get_by_label("File").click();
    harness.run();
    harness.get_by_label("Preferences…").click();
    harness.run();
    harness
        .get_all_by_role(egui::accesskit::Role::ComboBox)
        .find(|node| egui_kittest::kittest::NodeT::accesskit_node(node).value().as_deref() == Some("English"))
        .expect("the language list")
        .click();
    harness.run();
    harness.get_by_label("Nederlands").click();
    harness.run();
    if let Some(preferences) = harness.state_mut().preferences.as_mut() {
        preferences.autosave_minutes = 5;
        preferences.backups_kept = 3;
    }
    harness.get_all_by_label("Save").last().unwrap().click();
    harness.run();
    harness.run();
    // The menus read in Dutch now, and the choice is kept for the next run.
    harness.get_by_label("Gereedschap");
    harness.get_by_label("Beeld");
    harness.get_by_label("Bestandssysteem");
    assert_eq!(crate::Services::load(&stored, crate::LANGUAGE_KEY).as_deref(), Some("nl"));
    let settings = harness.state().session().unwrap().settings.clone().unwrap();
    assert_eq!((settings.autosave_minutes, settings.backups_kept), (5, 3));
    let again = BigStarApp::new(Box::new(stored.sharing_values()));
    assert_eq!(again.language, crate::Language::Dutch);
}

/// The app on `connection`, with the shelf that stands for its file dialogs.
fn harness_with_shelf(connection: DemoConnection) -> (Harness<'static, BigStarApp>, std::sync::Arc<std::sync::Mutex<crate::services::FileShelf>>) {
    let services = MemoryServices::default();
    let shelf = services.shelf.clone();
    let app = BigStarApp::with_connection(Box::new(services), Box::new(connection));
    let harness =
        Harness::builder().with_size(Vec2::new(1500.0, 800.0)).with_max_steps(16).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app);
    (harness, shelf)
}

#[test]
fn a_rom_opened_directly_has_no_builds_variants_or_glue() {
    let (mut harness, shelf) = harness_with_shelf(DemoConnection::without_workspace());
    harness.run();
    // As the desktop's Open ROM leaves it: a ROM on this machine, by the path the file dialog gave, on a host with no
    // workspace.
    harness.state_mut().session_mut().unwrap().open_rom("/roms/rom.nds");
    harness.run();
    harness.run();
    assert!(harness.state().session().unwrap().rom_mode());
    harness.state_mut().session_mut().unwrap().open_level("A01_1");
    harness.run();
    harness.run();

    // The ROM is the whole connection: one line of the File menu closes it, and there is no workspace to go back to.
    harness.get_by_label("File").click();
    harness.run();
    assert_eq!(harness.get_all_by_label("Close ROM").count(), 1);
    assert!(harness.query_by_label("Close project").is_none());
    harness.key_press(egui::Key::Escape);
    harness.run();
    for absent in ["Run Patcher", "Variant:", "Build", "Level Data", "Save to module", "Download ROM", "Close Workspace", "Disconnect"] {
        assert!(harness.query_by_label(absent).is_none(), "{absent} is shown for a ROM edited directly");
    }
    assert!(harness.get_all_by_label("Properties").next().is_some());

    // Save ROM As asks the file dialog where, and the title follows the new file.
    shelf.lock().unwrap().paths.push_back("/roms/copy.nds".to_owned());
    harness.get_by_label("Save ROM As…").click();
    harness.run();
    harness.run();
    assert_eq!(harness.state().session().unwrap().project.as_ref().map(|p| p.name.as_str()), Some("copy.nds"));

    // Closing the ROM ends the connection rather than leaving a page with nothing on it.
    harness.get_by_label("File").click();
    harness.run();
    harness.get_by_label("Close ROM").click();
    harness.run();
    assert!(harness.state().session().is_none());
}

#[test]
fn a_connection_without_a_workspace_uploads_a_rom_and_downloads_it_again() {
    let (mut harness, shelf) = harness_with_shelf(DemoConnection::without_workspace());
    harness.run();
    // No projects to list: the screen offers an upload, and there is nothing to close yet but the connection.
    harness.get_by_label("Disconnect");
    shelf.lock().unwrap().to_open.push_back(("My Game.nds".to_owned(), vec![7; 10]));
    harness.get_by_label("Upload ROM…").click();
    harness.run();
    harness.run();
    let session = harness.state().session().unwrap();
    assert!(session.uploaded() && session.upload.is_none());
    assert_eq!(session.project.as_ref().map(|p| p.name.as_str()), Some("My Game.nds"));
    // An uploaded ROM has no path on the host, but it is a ROM: its levels open.
    assert!(harness.query_by_label("This project has no ROM, so no level can be opened.").is_none());
    harness.state_mut().session_mut().unwrap().open_level("A01_1");
    harness.run();
    harness.run();
    for absent in ["Run Patcher", "Save ROM As…", "Save to module"] {
        assert!(harness.query_by_label(absent).is_none(), "{absent} is shown for an uploaded ROM");
    }

    // A save lives only in the host's memory until a download takes it.
    harness.state_mut().session_mut().unwrap().save_level(1);
    harness.run();
    harness.run();
    assert!(harness.state().session().unwrap().undownloaded);
    harness.get_by_label("Download ROM •").click();
    harness.run();
    harness.run();
    harness.run();
    assert_eq!(shelf.lock().unwrap().saved.pop(), Some(("rom.nds".to_owned(), b"DEMO ROM BYTES".to_vec())));
    assert!(!harness.state().session().unwrap().undownloaded);
    harness.get_by_label("Download ROM");
}

#[test]
fn an_ncpatcher_project_keeps_its_builds_and_variants() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.get_by_label("File").click();
    harness.run();
    for present in ["Run Patcher", "Variant:", "Build", "Level Data", "Close Workspace", "Close project"] {
        harness.get_by_label(present);
    }
    for absent in ["Save ROM As…", "Close ROM", "Disconnect"] {
        assert!(harness.query_by_label(absent).is_none(), "{absent} is shown for an NCPatcher project");
    }
    // A project with a usable clean ROM does not ask for one.
    assert!(harness.query_by_label("Use This ROM").is_none());
}

#[test]
fn a_project_without_a_clean_rom_asks_for_one() {
    let mut harness = canvas_on(DemoConnection::without_clean_rom(), Vec2::new(1500.0, 800.0));
    harness.get_by_label("This project has no clean ROM to build from yet.");
    assert_eq!(harness.state().session().unwrap().project.as_ref().unwrap().rom_path, None);
    harness.get_by_label("clean.nds").click();
    harness.run();
    harness.get_by_label("Use This ROM").click();
    harness.run();
    harness.run();
    let settings = harness.state().session().unwrap().settings.clone().unwrap();
    assert_eq!((settings.clean_rom.as_str(), settings.clean_rom_problem), ("demo/clean.nds", None));
    assert!(harness.query_by_label("Use This ROM").is_none(), "the question closes once it is answered");
    // The project reads the ROM now, rather than going on as if none had been chosen.
    let project = harness.state().session().unwrap().project.clone().unwrap();
    assert_eq!(project.rom_path.as_deref(), Some("demo/clean.nds"));
    assert!(harness.query_by_label("This project has no ROM, so no level can be opened.").is_none());
}

#[test]
fn closing_every_tab_leaves_an_empty_well_that_takes_new_ones() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1500.0, 800.0));
    harness.state_mut().session_mut().unwrap().open_level("A01_2");
    harness.run();
    let documents = harness.state().workspace.documents();
    assert_eq!(documents.len(), 2);
    for level in documents {
        let app = harness.state_mut();
        let (workspace, session) = (&mut app.workspace, app.session.as_mut().unwrap());
        workspace.close_document(session, level);
        harness.run();
    }
    // The Preview tab closes like any other, and with it the last tab: egui_dock empties its main surface then.
    let workspace = &mut harness.state_mut().workspace;
    let preview = workspace.dock.find_tab(&crate::workspace::DocTab::Preview).expect("the Preview tab");
    workspace.dock.remove_tab(preview);
    harness.run();
    harness.run();
    assert!(harness.state().workspace.documents().is_empty());
    // And a level opened afterwards finds somewhere to go.
    harness.state_mut().session_mut().unwrap().open_level("A01_1");
    harness.run();
    harness.run();
    assert_eq!(harness.state().workspace.documents().len(), 1);
}

#[test]
fn the_menu_bar_opens_its_menus_under_the_pointer() {
    let mut harness = canvas_on(DemoConnection::new(), Vec2::new(1200.0, 600.0));
    // Coming onto a title opens its menu, with no press, from the first point egui counts as on it: a pointer that
    // creeps up stays a frame or more in the margin just outside the button.
    let file = harness.get_by_label("File").rect();
    for x in [file.left() - 3.0, file.left() - 2.0, file.left() - 1.0, file.left() + 2.0] {
        harness.event(egui::Event::PointerMoved(egui::pos2(x, file.center().y)));
        harness.step();
    }
    harness.run();
    assert!(harness.query_by_label("Preferences…").is_some());
    // Going down into it keeps it.
    harness.get_by_label("Preferences…").hover();
    harness.run();
    assert!(harness.query_by_label("Preferences…").is_some());
    // Moving along the bar swaps it for the next one.
    harness.get_by_label("Edit").hover();
    harness.run();
    assert!(harness.query_by_label("Preferences…").is_none());
    assert!(harness.query_by_label("Find Sprite…").is_some());
    // A press on the title keeps the menu open rather than closing what the pointer opened.
    harness.get_by_label("Edit").click();
    harness.run();
    assert!(harness.query_by_label("Find Sprite…").is_some());
    // Escape closes it, and it stays closed under a pointer that has not moved.
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.query_by_label("Find Sprite…").is_none());
    // Coming back onto it opens it again, and leaving both the title and the menu closes it.
    harness.event(egui::Event::PointerMoved(egui::pos2(600.0, 400.0)));
    harness.run();
    harness.get_by_label("Edit").hover();
    harness.run();
    assert!(harness.query_by_label("Find Sprite…").is_some());
    harness.event(egui::Event::PointerMoved(egui::pos2(600.0, 400.0)));
    harness.run();
    assert!(harness.query_by_label("Find Sprite…").is_none());
}
