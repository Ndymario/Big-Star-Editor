use crate::{Error, ErrorCode, Result};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ByteBuffer {
    bytes: Vec<u8>,
}

impl ByteBuffer {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn into_vec(self) -> Vec<u8> {
        self.bytes
    }

    pub fn at(&self, offset: usize) -> Result<u8> {
        self.bytes.get(offset).copied().ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "byte offset is outside the buffer"))
    }

    pub fn slice(&self, offset: usize, length: usize) -> Result<&[u8]> {
        offset
            .checked_add(length)
            .and_then(|end| self.bytes.get(offset..end))
            .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "slice extends past the end of the buffer"))
    }

    pub fn write(&mut self, offset: usize, data: &[u8]) -> Result<()> {
        let target = offset
            .checked_add(data.len())
            .and_then(|end| self.bytes.get_mut(offset..end))
            .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "write extends past the end of the buffer"))?;
        target.copy_from_slice(data);
        Ok(())
    }

    pub fn append(&mut self, data: &[u8]) {
        self.bytes.extend_from_slice(data);
    }
}

impl From<Vec<u8>> for ByteBuffer {
    fn from(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}
