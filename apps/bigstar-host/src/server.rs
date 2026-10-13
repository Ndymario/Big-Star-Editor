//! The network face of the host: the browser frontend at `GET /` (when the host was built with its bundle) and
//! `GET /ws`. A connection is one `HostSession` behind the same
//! `InProcess` worker the desktop app uses, so what runs here is exactly what runs there, with a socket in front.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::get;
use bigstar_client::{Connection, HostInfo, InProcess, Incoming, Waker};
use bigstar_host::Host;
use bigstar_protocol::{ClientFrame, PROTOCOL_VERSION, ServerFrame, decode_client_frame, encode_server_frame};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::Notify;

/// Largest message a client may send. Level edits are small; this only bounds what a stranger can make us buffer.
const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
/// How long a socket may stay silent before it has said who it is.
const HELLO_TIMEOUT: Duration = Duration::from_secs(10);

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
struct AppState {
    host: Host,
    token: Arc<String>,
}

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

/// A fresh 128-bit token, hex encoded.
pub fn generate_token() -> Result<String, String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("could not read random bytes: {e}"))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Equal when the same, in time that does not depend on where they first differ.
pub fn tokens_match(given: &str, expected: &str) -> bool {
    let (given, expected) = (given.as_bytes(), expected.as_bytes());
    let mut difference = given.len() ^ expected.len();
    for index in 0..given.len().max(expected.len()) {
        difference |= usize::from(given.get(index).copied().unwrap_or(0) ^ expected.get(index).copied().unwrap_or(0));
    }
    difference == 0
}

mod bundle {
    include!(concat!(env!("OUT_DIR"), "/web_bundle.rs"));
}

/// Whether this host carries the browser frontend.
pub fn has_web_bundle() -> bool {
    bundle::WEB_FILES.iter().any(|(name, _)| *name == "index.html")
}

pub fn router(host: Host, token: String) -> Router {
    Router::new().route("/", get(index)).route("/ws", get(upgrade)).route("/{file}", get(asset)).with_state(AppState { host, token: Arc::new(token) })
}

fn content_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("png") => "image/png",
        Some("json") => "application/json",
        _ => "application/octet-stream",
    }
}

/// One file of the bundle. The page and its scripts carry no secret: the socket asks for the token.
async fn asset(Path(file): Path<String>) -> Response {
    match bundle::WEB_FILES.iter().find(|(name, _)| *name == file) {
        Some((name, bytes)) => ([(header::CONTENT_TYPE, content_type(name))], *bytes).into_response(),
        None => (StatusCode::NOT_FOUND, "not found").into_response(),
    }
}

/// Serves until the future is dropped.
pub async fn serve(listener: TcpListener, host: Host, token: String) -> std::io::Result<()> {
    axum::serve(listener, router(host, token)).await
}

/// Binds, then serves; the bound address is reported first because the port may have been chosen by the system.
pub async fn bind(address: SocketAddr) -> std::io::Result<(TcpListener, SocketAddr)> {
    let listener = TcpListener::bind(address).await?;
    let bound = listener.local_addr()?;
    Ok((listener, bound))
}

async fn index() -> Response {
    match bundle::WEB_FILES.iter().find(|(name, _)| *name == "index.html") {
        Some((_, bytes)) => Html(*bytes).into_response(),
        None => Html(
            "<!doctype html><title>Big Star host</title><p>Big Star host is running. Connect a Big Star frontend to <code>/ws</code>.</p>\
             <p>This host was built without the browser frontend: run <code>trunk build --release</code> in <code>apps/bigstar-web</code> and build the host again.</p>",
        )
        .into_response(),
    }
}

async fn upgrade(State(state): State<AppState>, Query(query): Query<WsQuery>, upgrade: WebSocketUpgrade) -> Response {
    // Checked before the upgrade, so a stranger gets no socket at all.
    if !query.token.as_deref().is_some_and(|given| tokens_match(given, &state.token)) {
        return (StatusCode::UNAUTHORIZED, "wrong token").into_response();
    }
    upgrade.max_message_size(MAX_MESSAGE_BYTES).on_upgrade(move |socket| connection(socket, state))
}

async fn send_frame(socket: &mut WebSocket, frame: &ServerFrame) -> bool {
    match encode_server_frame(frame) {
        Ok(bytes) => socket.send(Message::Binary(bytes.into())).await.is_ok(),
        Err(_) => false,
    }
}

async fn read_hello(socket: &mut WebSocket) -> Option<(u32, String)> {
    loop {
        match tokio::time::timeout(HELLO_TIMEOUT, socket.recv()).await.ok()?? {
            Ok(Message::Binary(bytes)) => {
                return match decode_client_frame(&bytes) {
                    Ok(ClientFrame::Hello { protocol_version, token }) => Some((protocol_version, token)),
                    _ => None,
                };
            }
            Ok(Message::Ping(_) | Message::Pong(_)) => {}
            _ => return None,
        }
    }
}

async fn connection(mut socket: WebSocket, state: AppState) {
    let Some((version, token)) = read_hello(&mut socket).await else {
        send_frame(&mut socket, &ServerFrame::Refused { reason: "the first message must be a Hello".to_owned() }).await;
        return;
    };
    if !tokens_match(&token, &state.token) {
        send_frame(&mut socket, &ServerFrame::Refused { reason: "wrong token".to_owned() }).await;
        return;
    }
    if version != PROTOCOL_VERSION {
        let reason = format!("the host speaks protocol {PROTOCOL_VERSION}, the frontend {version}");
        send_frame(&mut socket, &ServerFrame::Refused { reason }).await;
        return;
    }
    let welcome =
        ServerFrame::Welcome { protocol_version: PROTOCOL_VERSION, host_version: VERSION.to_owned(), workspace_name: state.host.workspace_name() };
    if !send_frame(&mut socket, &welcome).await {
        return;
    }

    let notify = Arc::new(Notify::new());
    let waker: Waker = {
        let notify = Arc::clone(&notify);
        Arc::new(move || notify.notify_one())
    };
    let info = HostInfo { host_version: VERSION.to_owned(), workspace_name: state.host.workspace_name() };
    let mut worker = InProcess::spawn(state.host.session(), info, Some(waker));

    // The worker numbers requests itself; the client's own ids are kept beside them until the answers return.
    let mut client_ids: HashMap<u64, u64> = HashMap::new();
    loop {
        tokio::select! {
            received = socket.recv() => {
                let Some(Ok(message)) = received else { break };
                match message {
                    Message::Binary(bytes) => match decode_client_frame(&bytes) {
                        Ok(ClientFrame::Request { id, request }) => {
                            client_ids.insert(worker.send(request), id);
                        }
                        // A second Hello, or a frame that is not one, is a client that has lost the thread.
                        _ => break,
                    },
                    Message::Close(_) => break,
                    _ => {}
                }
            }
            () = notify.notified() => {}
        }
        for incoming in worker.poll() {
            let frame = match incoming {
                Incoming::Response { id, result } => match client_ids.remove(&id) {
                    Some(client_id) => ServerFrame::Response { id: client_id, result },
                    None => continue,
                },
                Incoming::Event(event) => ServerFrame::Event(event),
                Incoming::State(_) => continue,
            };
            if !send_frame(&mut socket, &frame).await {
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use bigstar_client::{Connection, ConnectionState, Incoming, WebSocket, websocket_url};
    use bigstar_core::ErrorCode;
    use bigstar_host::Host;
    use bigstar_project::test_support::{RomFile, TempDir, make_rom, write_bytes};
    use bigstar_protocol::{Event, Request, Response};

    use super::{bind, generate_token, serve, tokens_match};

    /// A server on a free local port, running on a thread of its own for as long as the test process lives.
    fn start(root: &std::path::Path, token: &str) -> String {
        let host = Host::new(root).unwrap();
        let token = token.to_owned();
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let runtime = tokio::runtime::Runtime::new().unwrap();
            runtime.block_on(async {
                let (listener, address) = bind("127.0.0.1:0".parse().unwrap()).await.unwrap();
                sender.send(address).unwrap();
                let _ = serve(listener, host, token).await;
            });
        });
        format!("ws://{}", receiver.recv_timeout(Duration::from_secs(5)).unwrap())
    }

    fn wait_for<C: Connection>(connection: &mut C, mut done: impl FnMut(&Incoming) -> bool) -> Vec<Incoming> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut seen = Vec::new();
        while Instant::now() < deadline {
            for incoming in connection.poll() {
                let finished = done(&incoming);
                seen.push(incoming);
                if finished {
                    return seen;
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("timed out; saw {seen:?}");
    }

    fn answer<C: Connection>(connection: &mut C, request: Request) -> bigstar_core::Result<Response> {
        let id = connection.send(request);
        let seen = wait_for(connection, |m| matches!(m, Incoming::Response { id: got, .. } if *got == id));
        match seen.into_iter().last() {
            Some(Incoming::Response { result, .. }) => result,
            other => panic!("{other:?}"),
        }
    }

    /// The status and body of `GET path` from a running host, over a plain socket.
    fn get(base: &str, path: &str) -> (String, Vec<u8>) {
        use std::io::{Read, Write};
        let address = base.trim_start_matches("ws://");
        let mut stream = std::net::TcpStream::connect(address).unwrap();
        write!(stream, "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n").unwrap();
        let mut response = Vec::new();
        stream.read_to_end(&mut response).unwrap();
        let split = response.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
        let head = String::from_utf8_lossy(&response[..split]).into_owned();
        (head.lines().next().unwrap_or_default().to_owned(), response[split + 4..].to_vec())
    }

    #[test]
    fn the_page_is_the_browser_frontend_when_the_host_carries_it() {
        let root = TempDir::new("host-page");
        let base = start(root.path(), "secret");
        let (status, body) = get(&base, "/");
        assert!(status.contains("200"), "{status}");
        let page = String::from_utf8_lossy(&body);
        if super::has_web_bundle() {
            // Every file the page names is served, and nothing else is.
            assert!(page.contains("bigstar_canvas"));
            for (name, bytes) in super::bundle::WEB_FILES {
                let (status, body) = get(&base, &format!("/{name}"));
                assert!(status.contains("200"), "{name}: {status}");
                assert_eq!(body.len(), bytes.len());
            }
        } else {
            assert!(page.contains("trunk build --release"));
        }
        assert!(get(&base, "/nothing.js").0.contains("404"));
    }

    #[test]
    fn tokens_compare_exactly() {
        assert!(tokens_match("abc", "abc"));
        assert!(!tokens_match("abc", "abd"));
        assert!(!tokens_match("abc", "abcd"));
        assert!(!tokens_match("", "abc"));
        assert!(tokens_match("", ""));
        let (a, b) = (generate_token().unwrap(), generate_token().unwrap());
        assert_eq!((a.len(), b.len()), (32, 32));
        assert_ne!(a, b);
    }

    #[test]
    fn a_frontend_works_through_the_socket() {
        let root = TempDir::new("host-socket");
        write_bytes(&root.path().join("roms/game.nds"), &make_rom(&[RomFile::new("a.bin", b"x")], &[0; 32]));
        let base = start(root.path(), "secret");

        let mut connection = WebSocket::connect(&websocket_url(&base, "secret"), "secret", None);
        // Sent while still connecting: it waits for the handshake.
        let first = connection.send(Request::BrowseDirectory { path: String::new() });
        let seen = wait_for(&mut connection, |m| matches!(m, Incoming::Response { id, .. } if *id == first));
        assert!(seen.iter().any(|m| matches!(m, Incoming::State(ConnectionState::Connected(info)) if !info.workspace_name.is_empty())));
        let Some(Incoming::Response { result: Ok(Response::Directory(listing)), .. }) = seen.last() else { panic!("{seen:?}") };
        assert_eq!(listing.entries.len(), 1);

        let imported = answer(&mut connection, Request::ImportRom { rom_path: "roms/game.nds".into(), project_path: "p".into(), name: None });
        assert!(matches!(imported, Ok(Response::Project(_))));
        assert_eq!(
            answer(&mut connection, Request::ImportRom { rom_path: "../x".into(), project_path: "q".into(), name: None }).unwrap_err().code,
            ErrorCode::InvalidArgument
        );
        assert!(root.path().join("p/rom.nds").is_file());

        // Events reach a second connection.
        let mut other = WebSocket::connect(&websocket_url(&base, "secret"), "secret", None);
        answer(&mut other, Request::OpenProject { path: "p".into() }).unwrap();
        wait_for(&mut connection, |m| matches!(m, Incoming::Event(Event::Log { message, .. }) if message.contains("Opened project")));
    }

    #[test]
    fn the_wrong_token_gets_nothing() {
        let root = TempDir::new("host-token");
        let base = start(root.path(), "secret");

        // Refused before the upgrade: the socket never opens.
        let mut wrong_url = WebSocket::connect(&websocket_url(&base, "guess"), "guess", None);
        let id = wrong_url.send(Request::ListProjects);
        let seen = wait_for(&mut wrong_url, |m| matches!(m, Incoming::Response { id: got, .. } if *got == id));
        assert!(matches!(wrong_url.state(), ConnectionState::Closed(_)), "{:?}", wrong_url.state());
        assert!(matches!(seen.last(), Some(Incoming::Response { result: Err(_), .. })));

        // A right query and a wrong Hello is refused too.
        let mut wrong_hello = WebSocket::connect(&websocket_url(&base, "secret"), "guess", None);
        wait_for(&mut wrong_hello, |m| matches!(m, Incoming::State(ConnectionState::Refused(_))));
        assert!(matches!(wrong_hello.state(), ConnectionState::Refused(reason) if reason == "wrong token"));
        let id = wrong_hello.send(Request::ListProjects);
        let seen = wait_for(&mut wrong_hello, |m| matches!(m, Incoming::Response { id: got, .. } if *got == id));
        assert!(matches!(seen.last(), Some(Incoming::Response { result: Err(_), .. })));
    }
}
