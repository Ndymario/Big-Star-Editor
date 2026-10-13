use bigstar_core::{Error, ErrorCode, Result};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::{ClientFrame, ServerFrame};

fn encode<T: Serialize>(frame: &T) -> Result<Vec<u8>> {
    rmp_serde::to_vec_named(frame).map_err(|e| Error::new(ErrorCode::InvalidArgument, format!("could not encode a message: {e}")))
}

fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T> {
    rmp_serde::from_slice(bytes).map_err(|e| Error::new(ErrorCode::ParseError, format!("could not decode a message: {e}")))
}

/// MessagePack with named fields, one frame per binary WebSocket message.
pub fn encode_client_frame(frame: &ClientFrame) -> Result<Vec<u8>> {
    encode(frame)
}

pub fn decode_client_frame(bytes: &[u8]) -> Result<ClientFrame> {
    decode(bytes)
}

pub fn encode_server_frame(frame: &ServerFrame) -> Result<Vec<u8>> {
    encode(frame)
}

pub fn decode_server_frame(bytes: &[u8]) -> Result<ServerFrame> {
    decode(bytes)
}
