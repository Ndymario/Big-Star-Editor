//! Big Star Editor `client` layer: how a frontend talks to a host. One `Connection` trait, two transports:
//! `InProcess` (native only) hands requests to a service on a worker thread, `WebSocket` reaches a remote host
//! natively and from the browser alike. Wasm-safe: nothing here touches a file.

mod connection;
mod websocket;

#[cfg(not(target_arch = "wasm32"))]
mod in_process;

pub use connection::{Connection, ConnectionState, HostInfo, Incoming, Waker};
#[cfg(not(target_arch = "wasm32"))]
pub use in_process::InProcess;
pub use websocket::{WebSocket, websocket_url};

#[cfg(test)]
mod tests;
