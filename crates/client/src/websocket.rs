use bigstar_core::{Error, ErrorCode};
use bigstar_protocol::{ClientFrame, PROTOCOL_VERSION, Request, RequestId, ServerFrame, decode_server_frame, encode_client_frame};
use ewebsock::{WsEvent, WsMessage, WsReceiver, WsSender};

use crate::{Connection, ConnectionState, HostInfo, Incoming, Waker};

/// `ws://host:port` and a token as the address of a host's socket. The token is percent-encoded, so a token typed by
/// a person survives the query string.
pub fn websocket_url(base: &str, token: &str) -> String {
    let base = base.trim_end_matches('/');
    let mut url = format!("{base}/ws?token=");
    for byte in token.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            url.push(char::from(byte));
        } else {
            url.push_str(&format!("%{byte:02X}"));
        }
    }
    url
}

/// A connection to a host over a WebSocket, the same code natively and in the browser.
pub struct WebSocket {
    sender: Option<WsSender>,
    receiver: Option<WsReceiver>,
    token: String,
    state: ConnectionState,
    next_id: RequestId,
    /// Requests sent before the handshake finished.
    queued: Vec<(RequestId, Request)>,
    pending: Vec<Incoming>,
}

impl WebSocket {
    /// `url` is what `websocket_url` makes. Never fails: a connection that cannot be made is a `Closed` state.
    pub fn connect(url: &str, token: &str, waker: Option<Waker>) -> Self {
        let wake = move || {
            if let Some(waker) = &waker {
                waker();
            }
        };
        let (sender, receiver, state) = match ewebsock::connect_with_wakeup(url, ewebsock::Options::default(), wake) {
            Ok((sender, receiver)) => (Some(sender), Some(receiver), ConnectionState::Connecting),
            Err(reason) => (None, None, ConnectionState::Closed(reason)),
        };
        let pending = if matches!(state, ConnectionState::Closed(_)) { vec![Incoming::State(state.clone())] } else { Vec::new() };
        Self { sender, receiver, token: token.to_owned(), state, next_id: 1, queued: Vec::new(), pending }
    }

    fn transmit(&mut self, id: RequestId, request: Request) {
        let frame = ClientFrame::Request { id, request };
        let outcome = encode_client_frame(&frame).map(|bytes| {
            if let Some(sender) = self.sender.as_mut() {
                sender.send(WsMessage::Binary(bytes));
            }
        });
        if let Err(error) = outcome {
            self.pending.push(Incoming::Response { id, result: Err(error) });
        }
    }

    fn set_state(&mut self, state: ConnectionState, out: &mut Vec<Incoming>) {
        self.state = state.clone();
        out.push(Incoming::State(state));
    }

    fn fail(&mut self, reason: String, out: &mut Vec<Incoming>) {
        if matches!(self.state, ConnectionState::Closed(_) | ConnectionState::Refused(_)) {
            return;
        }
        self.set_state(ConnectionState::Closed(reason), out);
        // Whatever was still waiting for the handshake will never be answered.
        for (id, _) in std::mem::take(&mut self.queued) {
            out.push(Incoming::Response { id, result: Err(Error::new(ErrorCode::IoError, "the connection closed")) });
        }
    }

    fn receive(&mut self, message: WsMessage, out: &mut Vec<Incoming>) {
        let WsMessage::Binary(bytes) = message else { return };
        match decode_server_frame(&bytes) {
            Ok(ServerFrame::Welcome { protocol_version, host_version, workspace_name, has_workspace }) => {
                if protocol_version != PROTOCOL_VERSION {
                    let reason = format!("the host speaks protocol {protocol_version}, this frontend {PROTOCOL_VERSION}");
                    self.set_state(ConnectionState::Refused(reason), out);
                    return;
                }
                self.set_state(ConnectionState::Connected(HostInfo { host_version, workspace_name, has_workspace }), out);
                for (id, request) in std::mem::take(&mut self.queued) {
                    self.transmit(id, request);
                }
            }
            Ok(ServerFrame::Refused { reason }) => self.set_state(ConnectionState::Refused(reason), out),
            Ok(ServerFrame::Response { id, result }) => out.push(Incoming::Response { id, result }),
            Ok(ServerFrame::Event(event)) => out.push(Incoming::Event(event)),
            Err(error) => {
                log::warn!("dropping the connection after a bad frame: {error}");
                self.fail(error.message, out);
            }
        }
    }
}

impl Connection for WebSocket {
    fn send(&mut self, request: Request) -> RequestId {
        let id = self.next_id;
        self.next_id += 1;
        match self.state {
            ConnectionState::Connecting => self.queued.push((id, request)),
            ConnectionState::Connected(_) => self.transmit(id, request),
            ConnectionState::Refused(_) | ConnectionState::Closed(_) => {
                self.pending.push(Incoming::Response { id, result: Err(Error::new(ErrorCode::IoError, "not connected to a host")) });
            }
        }
        id
    }

    fn poll(&mut self) -> Vec<Incoming> {
        let mut out = std::mem::take(&mut self.pending);
        while let Some(event) = self.receiver.as_ref().and_then(WsReceiver::try_recv) {
            match event {
                WsEvent::Opened => {
                    let hello = ClientFrame::Hello { protocol_version: PROTOCOL_VERSION, token: self.token.clone() };
                    match encode_client_frame(&hello) {
                        Ok(bytes) => {
                            if let Some(sender) = self.sender.as_mut() {
                                sender.send(WsMessage::Binary(bytes));
                            }
                        }
                        Err(error) => self.fail(error.message, &mut out),
                    }
                }
                WsEvent::Message(message) => self.receive(message, &mut out),
                WsEvent::Error(reason) => self.fail(reason, &mut out),
                WsEvent::Closed => self.fail("the connection closed".to_owned(), &mut out),
            }
        }
        out
    }

    fn state(&self) -> &ConnectionState {
        &self.state
    }
}
