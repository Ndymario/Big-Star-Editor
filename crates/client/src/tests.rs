use std::time::{Duration, Instant};

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_protocol::{Event, LogLevel, Request, Response, Service};

use crate::{Connection, ConnectionState, HostInfo, InProcess, Incoming, websocket_url};

struct Echo {
    events: Vec<Event>,
}

impl Service for Echo {
    fn handle(&mut self, request: Request) -> Result<Response> {
        match request {
            Request::ListProjects => {
                self.events.push(Event::Log { level: LogLevel::Info, message: "listed".to_owned() });
                Ok(Response::Done)
            }
            _ => Err(Error::new(ErrorCode::Unsupported, "echo only lists")),
        }
    }

    fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }
}

fn info() -> HostInfo {
    HostInfo { host_version: "test".to_owned(), workspace_name: "w".to_owned(), has_workspace: true }
}

fn collect(connection: &mut impl Connection, wanted: usize) -> Vec<Incoming> {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut all = Vec::new();
    while all.len() < wanted && Instant::now() < deadline {
        all.extend(connection.poll());
        std::thread::sleep(Duration::from_millis(2));
    }
    all
}

#[test]
fn in_process_answers_requests_and_forwards_events() {
    let mut connection = InProcess::spawn(Echo { events: Vec::new() }, info(), None);
    assert_eq!(connection.state(), &ConnectionState::Connected(info()));
    let first = connection.send(Request::ListProjects);
    let second = connection.send(Request::CloseProject);
    assert_ne!(first, second);

    let got = collect(&mut connection, 3);
    assert!(got.contains(&Incoming::Response { id: first, result: Ok(Response::Done) }), "{got:?}");
    assert!(got.iter().any(|m| matches!(m, Incoming::Response { id, result: Err(e) } if *id == second && e.code == ErrorCode::Unsupported)));
    assert!(got.contains(&Incoming::Event(Event::Log { level: LogLevel::Info, message: "listed".to_owned() })));
}

#[test]
fn in_process_wakes_the_frontend() {
    let woken = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = std::sync::Arc::clone(&woken);
    let waker: crate::Waker = std::sync::Arc::new(move || {
        counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    let mut connection = InProcess::spawn(Echo { events: Vec::new() }, info(), Some(waker));
    connection.send(Request::ListProjects);
    collect(&mut connection, 2);
    assert!(woken.load(std::sync::atomic::Ordering::SeqCst) >= 1);
}

#[test]
fn dropping_an_in_process_connection_stops_its_worker() {
    let connection = InProcess::spawn(Echo { events: Vec::new() }, info(), None);
    drop(connection);
}

#[test]
fn the_websocket_url_carries_an_escaped_token() {
    assert_eq!(websocket_url("ws://127.0.0.1:7878/", "abc-123"), "ws://127.0.0.1:7878/ws?token=abc-123");
    assert_eq!(websocket_url("wss://h", "a b&c"), "wss://h/ws?token=a%20b%26c");
}
