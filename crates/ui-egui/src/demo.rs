//! A host that lives inside the frontend, with one made-up project and one made-up level. It exists so screenshots and
//! tests have something to show without a ROM; it understands just enough of the protocol to move and delete things.

use bigstar_client::{Connection, ConnectionState, HostInfo, Incoming};
use bigstar_core::{Error, ErrorCode};
use bigstar_formats::{
    NsmbLevelCourse, NsmbLevelEntrance, NsmbLevelObject, NsmbLevelPath, NsmbLevelPathNode, NsmbLevelStageObject, NsmbLevelView, NsmbLevelZone,
};
use bigstar_protocol::{
    AreaSummary, EditOutcome, Event, LevelEdit, LevelSnapshot, LevelSummary, LevelWorld, LogLevel, ProjectInfo, ProjectSummary, Request, RequestId,
    Response, ResponseResult,
};

const AREA: &str = "A01_1";

#[derive(Clone)]
struct State {
    course: NsmbLevelCourse,
    objects: Vec<NsmbLevelObject>,
}

pub struct DemoConnection {
    state: ConnectionState,
    next_id: RequestId,
    inbox: Vec<Incoming>,
    /// Events the request being answered raised; they follow its response.
    events: Vec<Incoming>,
    history: Vec<(String, State)>,
    position: usize,
    clean: usize,
    revision: u64,
}

fn object(object_id: u16, x: i16, y: i16, width: u16, height: u16) -> NsmbLevelObject {
    NsmbLevelObject { object_id, tileset_slot: 0, x, y, width, height }
}

fn sample_state() -> State {
    let course = NsmbLevelCourse {
        sprites: [(0x15, 26, 18), (0x37, 35, 17), (0x40, 44, 18), (0x15, 55, 16)]
            .into_iter()
            .map(|(id, x, y)| NsmbLevelStageObject { id, x, y, ..Default::default() })
            .collect(),
        entrances: vec![
            NsmbLevelEntrance { id: 0, x: 48, y: 288, ..Default::default() },
            NsmbLevelEntrance { id: 1, x: 1180, y: 224, ..Default::default() },
        ],
        views: vec![NsmbLevelView { id: 0, x: 0, y: 0, width: 1280, height: 384, ..Default::default() }],
        zones: vec![NsmbLevelZone { id: 0, x: 0, y: 32, width: 1280, height: 352, ..Default::default() }],
        paths: vec![NsmbLevelPath {
            id: 0,
            nodes: [(600, 160), (680, 160), (680, 220)].into_iter().map(|(x, y)| NsmbLevelPathNode { x, y, ..Default::default() }).collect(),
            unknown: 0,
        }],
        ..Default::default()
    };
    let objects = vec![
        object(0, 0, 20, 40, 4),
        object(0, 44, 20, 36, 4),
        object(1, 10, 15, 3, 1),
        object(1, 22, 13, 5, 1),
        object(2, 30, 16, 2, 4),
        object(3, 52, 14, 4, 6),
    ];
    State { course, objects }
}

impl DemoConnection {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Connected(HostInfo { host_version: "demo".to_owned(), workspace_name: "Demo workspace".to_owned() }),
            next_id: 1,
            inbox: Vec::new(),
            events: Vec::new(),
            history: vec![("Open".to_owned(), sample_state())],
            position: 0,
            clean: 0,
            revision: 1,
        }
    }

    fn current(&self) -> State {
        self.history.get(self.position).map_or_else(sample_state, |(_, state)| state.clone())
    }

    fn snapshot(&self) -> LevelSnapshot {
        let state = self.current();
        let names =
            |range: &mut dyn Iterator<Item = usize>| range.filter_map(|i| self.history.get(i).map(|(name, _)| name.clone())).collect::<Vec<_>>();
        LevelSnapshot {
            level: 1,
            area: AREA.to_owned(),
            title: "1-1".to_owned(),
            revision: self.revision,
            course: state.course,
            objects: state.objects,
            tileset_references: Default::default(),
            glue_error: None,
            dirty: self.position != self.clean,
            can_undo: self.position > 0,
            can_redo: self.position + 1 < self.history.len(),
            undo_names: names(&mut (1..=self.position).rev()),
            redo_names: names(&mut (self.position + 1..self.history.len())),
        }
    }

    fn edit(&mut self, edit: LevelEdit) -> Result<(), Error> {
        let mut next = self.current();
        let name = match edit {
            LevelEdit::Update { updates, name } => {
                for u in updates.objects {
                    *next.objects.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                for u in updates.sprites {
                    *next.course.sprites.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                for u in updates.entrances {
                    *next.course.entrances.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                for u in updates.views {
                    *next.course.views.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                for u in updates.zones {
                    *next.course.zones.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                for u in updates.paths {
                    *next.course.paths.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                for u in updates.progress_paths {
                    *next.course.progress_paths.get_mut(u.index).ok_or_else(missing)? = u.value;
                }
                name
            }
            LevelEdit::Remove { selection, name } => {
                fn drop_all<T>(list: &mut Vec<T>, mut indices: Vec<usize>) {
                    indices.sort_unstable_by(|a, b| b.cmp(a));
                    for index in indices {
                        if index < list.len() {
                            list.remove(index);
                        }
                    }
                }
                drop_all(&mut next.objects, selection.objects);
                drop_all(&mut next.course.sprites, selection.sprites);
                drop_all(&mut next.course.entrances, selection.entrances);
                drop_all(&mut next.course.views, selection.views);
                drop_all(&mut next.course.zones, selection.zones);
                drop_all(&mut next.course.paths, selection.paths);
                drop_all(&mut next.course.progress_paths, selection.progress_paths);
                name
            }
            _ => return Err(Error::new(ErrorCode::Unsupported, "the demo host only moves and deletes")),
        };
        self.history.truncate(self.position + 1);
        self.history.push((name, next));
        self.position += 1;
        self.revision += 1;
        Ok(())
    }

    fn answer(&mut self, request: Request) -> ResponseResult {
        match request {
            Request::ListProjects => Ok(Response::Projects(vec![ProjectSummary { path: "demo".to_owned(), name: "Demo".to_owned() }])),
            Request::OpenProject { .. } => Ok(Response::Project(ProjectInfo {
                path: "demo".to_owned(),
                name: "Demo".to_owned(),
                rom_path: Some("rom.nds".to_owned()),
                game_title: Some("NEW SUPER MARIO BROS".to_owned()),
                game_code: Some("A2DE".to_owned()),
                file_count: 1204,
            })),
            Request::CloseProject => Ok(Response::Done),
            Request::ListLevels => {
                let area =
                    |token: &str, display: &str, available| AreaSummary { token: token.to_owned(), display_name: display.to_owned(), available };
                let level = |name: &str, areas| LevelSummary { display_name: name.to_owned(), areas };
                Ok(Response::Levels(vec![LevelWorld {
                    name: "World 1".to_owned(),
                    levels: vec![
                        level("1-1", vec![area(AREA, "Area 1", true), area("A01_2", "Area 2", true)]),
                        level("1-2", vec![area("A02_1", "Area 1", false)]),
                    ],
                }]))
            }
            Request::OpenLevel { .. } => Ok(Response::Level(Box::new(self.snapshot()))),
            Request::EditLevel { base_revision, edit, .. } => {
                if base_revision != self.revision {
                    return Err(Error::new(ErrorCode::InvalidArgument, "the level changed since it was last seen"));
                }
                self.edit(edit)?;
                Ok(Response::Edited { snapshot: Box::new(self.snapshot()), outcome: EditOutcome::None })
            }
            Request::UndoLevel { .. } if self.position > 0 => {
                self.position -= 1;
                self.revision += 1;
                Ok(Response::Edited { snapshot: Box::new(self.snapshot()), outcome: EditOutcome::None })
            }
            Request::RedoLevel { .. } if self.position + 1 < self.history.len() => {
                self.position += 1;
                self.revision += 1;
                Ok(Response::Edited { snapshot: Box::new(self.snapshot()), outcome: EditOutcome::None })
            }
            Request::UndoLevel { .. } | Request::RedoLevel { .. } => Err(Error::new(ErrorCode::InvalidArgument, "nothing to undo or redo")),
            Request::SaveLevel { .. } => {
                self.clean = self.position;
                self.revision += 1;
                self.events.push(Incoming::Event(Event::Log { level: LogLevel::Info, message: "Saved 1-1".to_owned() }));
                Ok(Response::Edited { snapshot: Box::new(self.snapshot()), outcome: EditOutcome::None })
            }
            Request::CloseLevel { .. } => Ok(Response::Done),
            _ => Err(Error::new(ErrorCode::Unsupported, "the demo host does not do that")),
        }
    }
}

fn missing() -> Error {
    Error::new(ErrorCode::InvalidArgument, "there is no such entity")
}

impl Default for DemoConnection {
    fn default() -> Self {
        Self::new()
    }
}

impl Connection for DemoConnection {
    fn send(&mut self, request: Request) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        let result = self.answer(request);
        self.inbox.push(Incoming::Response { id, result });
        self.inbox.append(&mut self.events);
        id
    }

    fn poll(&mut self) -> Vec<Incoming> {
        std::mem::take(&mut self.inbox)
    }

    fn state(&self) -> &ConnectionState {
        &self.state
    }
}
