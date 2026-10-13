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
        glue: bigstar_formats::GlueBlock {
            flags: vec![1],
            values: vec![bigstar_formats::GlueValue { hash: 2, payload: vec![3, 4], payload_order: 0 }],
        },
        stage_object_id_base: 326,
        registration_hashes: vec![7],
        sprite_hashes: vec![None, Some(7)],
        dirty: true,
        can_undo: true,
        can_redo: false,
        undo_names: vec!["Move".to_owned()],
        redo_names: Vec::new(),
    })
}

#[test]
fn graphics_messages_survive_the_wire() {
    use crate::{ActorDraw, ActorInput, DrawCommand, DrawKind, LevelGraphics, ProjectSettings, TextureData, TilesetGraphics};

    let texture = TextureData { id: "ab12".into(), width: 16, height: 16, rgba: (0..1024u32).map(|i| (i * 7) as u8).collect() };
    let bytes = encode_server_frame(&ServerFrame::Response { id: 1, result: Ok(Response::Textures(vec![texture.clone()])) }).unwrap();
    // Pixels travel as one binary blob, not as an array of small integers.
    assert!(bytes.len() < 1024 + 128, "{} bytes for 1024 bytes of pixels", bytes.len());
    server_round_trip(ServerFrame::Response { id: 1, result: Ok(Response::Textures(vec![texture])) });

    let graphics = LevelGraphics {
        level: 3,
        generation: 9,
        tilesets: [
            TilesetGraphics { atlas: Some("a".into()), blocks: 256, objects: vec![bigstar_formats::ObjectLayout::default()], error: None },
            TilesetGraphics::default(),
            TilesetGraphics { error: Some("no".into()), ..TilesetGraphics::default() },
        ],
        ..LevelGraphics::default()
    };
    server_round_trip(ServerFrame::Response { id: 2, result: Ok(Response::LevelGraphics(Box::new(graphics))) });
    let draw = ActorDraw {
        found: true,
        commands: vec![DrawCommand { kind: DrawKind::Line, texture: None, x: 1.5, y2: -3.0, color: 0xff00_00ff, ..DrawCommand::default() }],
        custom_rendered: true,
        grab: Some(bigstar_level::SpriteBounds { x: -8, y: 4, width: 16, height: 32 }),
        render: None,
        error: None,
    };
    server_round_trip(ServerFrame::Response { id: 3, result: Ok(Response::ActorDraws { generation: 4, draws: vec![draw] }) });
    server_round_trip(ServerFrame::Event(Event::GraphicsChanged { generation: 5 }));
    server_round_trip(ServerFrame::Event(Event::FilesChanged { paths: vec!["proj/a.lua".into()] }));
    server_round_trip(ServerFrame::Event(Event::SettingsChanged(ProjectSettings { active_variant: "fr".into(), ..ProjectSettings::default() })));
    let input = ActorInput { sprite_type: 83, hash: None, x: 1, y: 2, pixel_x: 16.0, pixel_y: 32.0, settings: 0x10, events: [1, 2] };
    client_round_trip(ClientFrame::Request { id: 10, request: Request::RenderActors { inputs: vec![input] } });
    client_round_trip(ClientFrame::Request { id: 11, request: Request::SetActiveVariant { variant: "en".into() } });
    client_round_trip(ClientFrame::Request { id: 12, request: Request::ProjectData });
    for request in
        [Request::LevelBytes { level: 2 }, Request::FindEntities { sprites: true, id: 21 }, Request::Recovery, Request::Recover { accept: false }]
    {
        client_round_trip(ClientFrame::Request { id: 14, request });
    }
    let edit = LevelEdit::ReplaceSerialized { course: vec![1, 2, 3], bgdat: vec![0xff, 0xff], name: "Hex edit level".into() };
    client_round_trip(ClientFrame::Request { id: 15, request: Request::EditLevel { level: 1, base_revision: 3, edit } });
    let bytes = crate::LevelBytes { course: vec![9; 40], bgdat: vec![0xff, 0xff], course_file_id: Some(300), bgdat_file_id: None };
    server_round_trip(ServerFrame::Response { id: 6, result: Ok(Response::LevelBytes(Box::new(bytes))) });
    let found = crate::FoundEntity { area: "A01_1".into(), title: "1-1".into(), index: 3, x: -2, y: 7, settings: Some(5) };
    server_round_trip(ServerFrame::Response { id: 7, result: Ok(Response::Found(vec![found])) });
    let offered = crate::RecoverableLevel { area: "A01_1".into(), title: "1-1".into() };
    server_round_trip(ServerFrame::Response { id: 8, result: Ok(Response::Recovery(vec![offered])) });
    server_round_trip(ServerFrame::Response { id: 9, result: Ok(Response::Recovered(vec!["A01_1".into()])) });
    let glue = bigstar_formats::GlueBlock { flags: vec![9], values: Vec::new() };
    let edit = LevelEdit::SetGlue { glue, name: "Edit level data".into() };
    client_round_trip(ClientFrame::Request { id: 13, request: Request::EditLevel { level: 1, base_revision: 2, edit } });
    let mut overlays = [[0; 10]; 16];
    overlays[3][9] = 77;
    let data = crate::ProjectData { modules: Default::default(), object_bank_overlays: Some(overlays) };
    server_round_trip(ServerFrame::Response { id: 4, result: Ok(Response::ProjectData(Box::new(data))) });
}
