//! Big Star Editor `protocol` layer: every message between the host and a frontend, and how they travel.
//! Pure: no filesystem, no sockets, safe for the browser.
//!
//! A connection opens with a `Hello` carrying the protocol version and the access token, answered by a `Welcome` or a
//! `Refused`. After that the frontend sends `Request`s with ids of its own choosing, the host answers each with one
//! `Response` carrying the same id, and sends `Event`s whenever something happens that nobody asked about.

mod message;
mod service;
mod wire;

pub use message::{
    AreaSummary, ClientFrame, DirectoryEntry, DirectoryEntryKind, DirectoryListing, EditOutcome, Event, LevelEdit, LevelId, LevelSnapshot,
    LevelSummary, LevelWorld, LogLevel, PROTOCOL_VERSION, ProjectInfo, ProjectSummary, Request, RequestId, Response, ResponseResult, ServerFrame,
};
pub use service::Service;
pub use wire::{decode_client_frame, decode_server_frame, encode_client_frame, encode_server_frame};

#[cfg(test)]
mod tests;
