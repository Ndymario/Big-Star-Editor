use std::sync::Arc;

use bigstar_protocol::{Event, Request, RequestId, ResponseResult};

/// Called from a transport's thread whenever something is waiting in `poll`, so a UI can repaint.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub host_version: String,
    pub workspace_name: String,
    /// Whether the connection reaches a workspace of NCPatcher projects. Without one it edits only ROMs: one it uploads,
    /// or, on the desktop, one picked on this machine.
    pub has_workspace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Connecting,
    Connected(HostInfo),
    /// The host answered the handshake with a refusal: the wrong token or an incompatible protocol version.
    Refused(String),
    Closed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Incoming {
    State(ConnectionState),
    Response { id: RequestId, result: ResponseResult },
    Event(Event),
}

/// A frontend's end of a connection. Nothing blocks: `send` queues, `poll` returns whatever has arrived.
pub trait Connection {
    /// Queues a request and returns the id its `Response` will carry. A request sent before the handshake finishes
    /// waits for it; one sent on a connection that is gone is answered at once with an error.
    fn send(&mut self, request: Request) -> RequestId;

    fn poll(&mut self) -> Vec<Incoming>;

    fn state(&self) -> &ConnectionState;
}
