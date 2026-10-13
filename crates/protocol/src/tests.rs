use bigstar_core::{Error, ErrorCode};
use bigstar_formats::NsmbLevelObject;
use bigstar_level::{IndexedLevelValue, LevelEntityUpdates};

use crate::{
    ClientFrame, EditOutcome, Event, LevelEdit, LogLevel, PROTOCOL_VERSION, Request, Response, ServerFrame, decode_client_frame, decode_server_frame,
    encode_client_frame, encode_server_frame,
};

fn client_round_trip(frame: ClientFrame) {
    let bytes = encode_client_frame(&frame).expect("encodes");
    assert_eq!(decode_client_frame(&bytes).expect("decodes"), frame);
}

fn server_round_trip(frame: ServerFrame) {
    let bytes = encode_server_frame(&frame).expect("encodes");
    assert_eq!(decode_server_frame(&bytes).expect("decodes"), frame);
}

#[test]
fn client_frames_survive_the_wire() {
    client_round_trip(ClientFrame::Hello { protocol_version: PROTOCOL_VERSION, token: "abc".to_owned() });
    client_round_trip(ClientFrame::Request { id: 7, request: Request::ListProjects });
    client_round_trip(ClientFrame::Request { id: 8, request: Request::OpenLevel { area: "A01_1".to_owned() } });
    let updates = LevelEntityUpdates {
        objects: vec![IndexedLevelValue { index: 2, value: NsmbLevelObject { object_id: 5, x: -3, y: 9, ..NsmbLevelObject::default() } }],
        ..LevelEntityUpdates::default()
    };
    client_round_trip(ClientFrame::Request {
        id: 9,
        request: Request::EditLevel { level: 1, base_revision: 4, edit: LevelEdit::Update { updates, name: "Move".to_owned() } },
    });
}

#[test]
fn server_frames_survive_the_wire() {
    server_round_trip(ServerFrame::Welcome { protocol_version: PROTOCOL_VERSION, host_version: "0.1.0".to_owned(), workspace_name: "w".to_owned() });
    server_round_trip(ServerFrame::Refused { reason: "wrong token".to_owned() });
    server_round_trip(ServerFrame::Response { id: 3, result: Ok(Response::Done) });
    server_round_trip(ServerFrame::Response { id: 4, result: Err(Error::new(ErrorCode::NotFound, "no such level")) });
    server_round_trip(ServerFrame::Response { id: 5, result: Ok(Response::Edited { snapshot: snapshot(), outcome: EditOutcome::Moved(2) }) });
    server_round_trip(ServerFrame::Event(Event::Log { level: LogLevel::Warning, message: "careful".to_owned() }));
    server_round_trip(ServerFrame::Event(Event::LevelChanged { level: 1, revision: 2, dirty: true }));
}

#[test]
fn garbage_is_a_parse_error_not_a_panic() {
    let error = decode_client_frame(&[0xC1, 0x00, 0xFF]).expect_err("not a frame");
    assert_eq!(error.code, ErrorCode::ParseError);
    assert_eq!(decode_server_frame(&[]).expect_err("empty").code, ErrorCode::ParseError);
}

fn snapshot() -> Box<crate::LevelSnapshot> {
    Box::new(crate::LevelSnapshot {
        level: 1,
        area: "A01_1".to_owned(),
        title: "1-1".to_owned(),
        revision: 3,
        course: bigstar_formats::NsmbLevelCourse::default(),
        objects: vec![NsmbLevelObject::default()],
        tileset_references: bigstar_level::LevelTilesetReferences::default(),
        glue_error: None,
        dirty: true,
        can_undo: true,
        can_redo: false,
        undo_names: vec!["Move".to_owned()],
        redo_names: Vec::new(),
    })
}
