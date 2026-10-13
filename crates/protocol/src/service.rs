use bigstar_core::Result;

use crate::{Event, Request, Response};

/// What answers a connection's requests. The host implements it once per connection; the transports only move
/// messages to and from it, so they never learn what a request means.
pub trait Service: Send + 'static {
    fn handle(&mut self, request: Request) -> Result<Response>;

    /// Events produced since the last call, including those caused by other connections.
    fn drain_events(&mut self) -> Vec<Event>;
}
