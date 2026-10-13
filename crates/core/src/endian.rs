use crate::{Error, ErrorCode, Result};

fn read_array<const N: usize>(data: &[u8], offset: usize, what: &str) -> Result<[u8; N]> {
    offset
        .checked_add(N)
        .and_then(|end| data.get(offset..end))
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, format!("{what} read is out of bounds")))
}

fn write_array<const N: usize>(data: &mut [u8], offset: usize, value: [u8; N], what: &str) -> Result<()> {
    let target = offset
        .checked_add(N)
        .and_then(|end| data.get_mut(offset..end))
        .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, format!("{what} write is out of bounds")))?;
    target.copy_from_slice(&value);
    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct LittleEndianReader<'a> {
    data: &'a [u8],
}

impl<'a> LittleEndianReader<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data }
    }

    pub fn u8(&self, offset: usize) -> Result<u8> {
        Ok(u8::from_le_bytes(read_array(self.data, offset, "u8")?))
    }

    pub fn u16(&self, offset: usize) -> Result<u16> {
        Ok(u16::from_le_bytes(read_array(self.data, offset, "u16")?))
    }

    pub fn u32(&self, offset: usize) -> Result<u32> {
        Ok(u32::from_le_bytes(read_array(self.data, offset, "u32")?))
    }
}

#[derive(Debug)]
pub struct LittleEndianWriter<'a> {
    data: &'a mut [u8],
}

impl<'a> LittleEndianWriter<'a> {
    pub fn new(data: &'a mut [u8]) -> Self {
        Self { data }
    }

    pub fn u16(&mut self, offset: usize, value: u16) -> Result<()> {
        write_array(self.data, offset, value.to_le_bytes(), "u16")
    }

    pub fn u32(&mut self, offset: usize, value: u32) -> Result<()> {
        write_array(self.data, offset, value.to_le_bytes(), "u32")
    }
}
