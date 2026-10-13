//! Big Star Editor `protocol` layer: every message between the host and a frontend, and how they travel.
//! Pure: no filesystem, no sockets, safe for the browser.
//!
//! A connection opens with a `Hello` carrying the protocol version and the access token, answered by a `Welcome` or a
//! `Refused`. After that the frontend sends `Request`s with ids of its own choosing, the host answers each with one
//! `Response` carrying the same id, and sends `Event`s whenever something happens that nobody asked about.

mod graphics;
mod message;
mod service;
mod wire;

pub use graphics::{
    ATLAS_COLUMNS, ActorCatalog, ActorDraw, ActorInfo, ActorInput, DrawCommand, DrawKind, ImageGraphics, LevelGraphics, ProjectSettings, TextureData,
    TextureId, TilesetGraphics,
};
pub use message::{
    AreaSummary, ClientFrame, DirectoryEntry, DirectoryEntryKind, DirectoryListing, EditOutcome, Event, FileEntry, FileIcon, FileStatus,
    FilesystemListing, FoundEntity, LevelBytes, LevelEdit, LevelId, LevelSnapshot, LevelSummary, LevelWorld, LogLevel, ModuleChoice,
    ObjectBankOverlays, PROTOCOL_VERSION, ProjectData, ProjectInfo, ProjectSummary, RecoverableLevel, Request, RequestId, Response, ResponseResult,
    ServerFrame,
};
pub use service::Service;
pub use wire::{decode_client_frame, decode_server_frame, encode_client_frame, encode_server_frame};

#[cfg(test)]
mod tests;
