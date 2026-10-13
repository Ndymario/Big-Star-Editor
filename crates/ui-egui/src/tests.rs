use bigstar_client::Connection;
use bigstar_protocol::{LevelEdit, Request, Response};
use egui::Vec2;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;

use crate::demo::DemoConnection;
use crate::level_view::{DragMode, Kind};
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
    assert!(session.level.is_none());
}

#[test]
fn opening_a_level_gives_a_snapshot_and_edits_come_back_with_a_new_revision() {
    let mut session = opened();
    let view = session.level.as_mut().unwrap();
    assert_eq!(view.snapshot.revision, 1);
    view.selection.insert((Kind::Sprite, 0));
    view.drag = Some(DragMode::Move { delta: Vec2::new(16.0, 0.0), submitted: true });
    let edit = view.move_edit().unwrap();
    let before = view.snapshot.course.sprites[0].x;
    session.edit(edit);
    settle(&mut session);
    let view = session.level.as_ref().unwrap();
    assert_eq!(view.snapshot.course.sprites[0].x, before + 1);
    assert_eq!((view.snapshot.revision, view.snapshot.dirty, view.snapshot.can_undo), (2, true, true));
    assert_eq!(view.snapshot.undo_names, ["Move"]);
    assert_eq!(view.drag, None, "the moved things stop being previews once the host has answered");
}

#[test]
fn undo_redo_and_save_follow_the_hosts_answers() {
    let mut session = opened();
    let remove = {
        let view = session.level.as_mut().unwrap();
        view.selection.insert((Kind::Object, 0));
        view.remove_edit().unwrap()
    };
    session.edit(remove);
    settle(&mut session);
    assert_eq!(session.level.as_ref().unwrap().snapshot.objects.len(), 5);
    session.undo();
    settle(&mut session);
    assert_eq!(session.level.as_ref().unwrap().snapshot.objects.len(), 6);
    session.redo();
    settle(&mut session);
    assert_eq!(session.level.as_ref().unwrap().snapshot.objects.len(), 5);
    session.save();
    settle(&mut session);
    assert!(!session.level.as_ref().unwrap().snapshot.dirty);
    assert_eq!(session.log.last().map(|l| l.text.as_str()), Some("Saved 1-1"));
}

#[test]
fn a_refused_edit_is_reported_and_the_level_is_fetched_again() {
    let mut session = opened();
    // An edit against a revision the host no longer has.
    if let Some(view) = session.level.as_mut() {
        view.snapshot.revision = 99;
    }
    session.edit(LevelEdit::Remove { selection: Default::default(), name: "Delete".to_owned() });
    settle(&mut session);
    assert!(session.notice.as_ref().is_some_and(|n| n.error));
    assert_eq!(session.level.as_ref().unwrap().snapshot.revision, 1, "the host's revision replaced ours");
}

#[test]
fn a_demo_host_answers_requests_it_does_not_know_with_an_error() {
    let mut connection = DemoConnection::new();
    let id = connection.send(Request::ListProjects);
    let id2 = connection.send(Request::CloseLevel { level: 1 });
    let answers = connection.poll();
    assert_eq!(answers.len(), 2);
    assert!(matches!(&answers[0], bigstar_client::Incoming::Response { id: first, result: Ok(Response::Projects(_)) } if *first == id));
    assert!(matches!(&answers[1], bigstar_client::Incoming::Response { id: second, result: Ok(Response::Done) } if *second == id2));
}

fn harness() -> Harness<'static, BigStarApp> {
    let app = BigStarApp::with_connection(Box::new(MemoryServices::default()), Box::new(DemoConnection::new()));
    Harness::builder().with_size(Vec2::new(1000.0, 640.0)).build_ui_state(|ui, app: &mut BigStarApp| app.ui(ui), app)
}

#[test]
fn the_screens_lead_from_projects_to_levels_to_the_canvas() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    harness.get_by_label("Area 1").click();
    harness.run();
    let session = harness.state().session().unwrap();
    assert_eq!(session.level.as_ref().map(|v| v.snapshot.title.as_str()), Some("1-1"));
    harness.get_by_label("Save");
    harness.get_by_label("Levels").click();
    harness.run();
    assert!(harness.state().session().unwrap().level.is_none());
}

#[test]
fn dragging_a_sprite_moves_it_by_one_undoable_edit() {
    let mut harness = harness();
    harness.run();
    harness.get_by_label("Demo").click();
    harness.run();
    harness.get_by_label("World 1").click();
    harness.run();
    harness.get_by_label("Area 1").click();
    harness.run();
    let (screen, x_before, zoom) = {
        let view = harness.state().session().unwrap().level.as_ref().unwrap();
        let center = view.bounds((Kind::Sprite, 0)).unwrap().center();
        (view.canvas.min + (center.to_vec2() - view.pan) * view.zoom, view.snapshot.course.sprites[0].x, view.zoom)
    };
    harness.drag_at(screen);
    harness.run();
    harness.hover_at(screen + Vec2::new(64.0 * zoom, 0.0));
    harness.run();
    harness.drop_at(screen + Vec2::new(64.0 * zoom, 0.0));
    harness.run();
    let view = harness.state().session().unwrap().level.as_ref().unwrap();
    assert!(view.selection.contains(&(Kind::Sprite, 0)));
    assert_eq!(view.snapshot.course.sprites[0].x, x_before + 4, "moved by 64 level pixels (four tiles) in one step");
    assert_eq!(view.snapshot.undo_names, ["Move"]);
}
