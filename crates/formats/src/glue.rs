use std::collections::HashSet;

use bigstar_core::{Error, ErrorCode, LittleEndianReader, LittleEndianWriter, Result};
use serde::{Deserialize, Serialize};

pub const NSMB_HEADER_SIZE: usize = 0x20;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlueValue {
    pub hash: u32,
    pub payload: Vec<u8>,
    pub payload_order: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlueBlock {
    pub flags: Vec<u32>,
    /// Entry-table order, not payload-offset order.
    pub values: Vec<GlueValue>,
}

impl GlueBlock {
    pub fn is_empty(&self) -> bool {
        self.flags.is_empty() && self.values.is_empty()
    }
}

pub const fn fnv1a_32(text: &str) -> u32 {
    let mut rest = text.as_bytes();
    let mut hash: u32 = 2166136261;
    while let [byte, tail @ ..] = rest {
        hash ^= *byte as u32;
        hash = hash.wrapping_mul(16777619);
        rest = tail;
    }
    hash
}

fn parse_error(message: &str) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

fn invalid(message: &str) -> Error {
    Error::new(ErrorCode::InvalidArgument, message)
}

pub fn parse_glue_block(data: &[u8]) -> Result<GlueBlock> {
    if data.is_empty() {
        return Ok(GlueBlock::default());
    }
    if !data.len().is_multiple_of(4) {
        return Err(parse_error("glue block length is not a multiple of four"));
    }
    if data.len() < 12 {
        return Err(parse_error("glue block is shorter than its flag count and terminator"));
    }
    let r = LittleEndianReader::new(data);
    let flag_count = u64::from(r.u32(0)?);
    let entries_begin64 = 4 + flag_count * 4;
    if entries_begin64 > data.len() as u64 || entries_begin64 + 8 > data.len() as u64 {
        return Err(parse_error("glue flag count does not fit the block"));
    }
    let entries_begin = entries_begin64 as usize;

    let mut block = GlueBlock::default();
    for offset in (4..entries_begin).step_by(4) {
        block.flags.push(r.u32(offset)?);
    }

    struct Entry {
        hash: u32,
        offset: u32,
        table_index: usize,
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut hashes = HashSet::new();
    let mut cursor = entries_begin;
    loop {
        if cursor > data.len() || data.len() - cursor < 8 {
            return Err(parse_error("glue value table has no terminator"));
        }
        let hash = r.u32(cursor)?;
        let offset = r.u32(cursor + 4)?;
        cursor += 8;
        if hash == 0 {
            break;
        }
        if !hashes.insert(hash) {
            return Err(parse_error("glue value table contains a duplicate hash"));
        }
        entries.push(Entry { hash, offset, table_index: entries.len() });
    }
    let payload_begin = cursor;

    let mut by_offset: Vec<&Entry> = entries.iter().collect();
    by_offset.sort_by_key(|e| e.offset);
    for (i, entry) in by_offset.iter().enumerate() {
        let start = entry.offset as usize;
        if start < payload_begin || start >= data.len() {
            return Err(parse_error("glue value offset is outside the payload area"));
        }
        if i != 0 && by_offset.get(i - 1).is_some_and(|p| p.offset == entry.offset) {
            return Err(parse_error("two glue values share one offset"));
        }
    }

    block.values = vec![GlueValue::default(); entries.len()];
    for (order, entry) in by_offset.iter().enumerate() {
        let start = entry.offset as usize;
        let end = by_offset.get(order + 1).map_or(data.len(), |next| next.offset as usize);
        let payload = data.get(start..end).ok_or_else(|| parse_error("glue value offset is outside the payload area"))?;
        if let Some(value) = block.values.get_mut(entry.table_index) {
            *value = GlueValue { hash: entry.hash, payload: payload.to_vec(), payload_order: order };
        }
    }
    Ok(block)
}

pub fn write_glue_block(block: &GlueBlock) -> Result<Vec<u8>> {
    if block.is_empty() {
        return Ok(Vec::new());
    }
    if block.flags.len() > u32::MAX as usize || block.values.len() > (u32::MAX as usize - 12) / 8 {
        return Err(invalid("glue block has too many entries"));
    }
    if block.flags.contains(&0) {
        return Err(invalid("a glue flag has the reserved zero hash"));
    }
    let mut hashes = HashSet::new();
    for value in &block.values {
        if value.hash == 0 {
            return Err(invalid("a glue value has the reserved zero hash"));
        }
        if value.payload.is_empty() {
            return Err(invalid("a glue value has no payload"));
        }
        if !hashes.insert(value.hash) {
            return Err(invalid("glue values contain a duplicate hash"));
        }
    }

    let payload_begin = 4 + block.flags.len() * 4 + (block.values.len() + 1) * 8;
    let mut order: Vec<usize> = (0..block.values.len()).collect();
    order.sort_by_key(|&i| block.values.get(i).map_or(0, |v| v.payload_order));

    let mut offsets = vec![0u32; block.values.len()];
    let mut total = payload_begin;
    for &index in &order {
        if total > u32::MAX as usize {
            return Err(invalid("glue block is too large"));
        }
        if let Some(slot) = offsets.get_mut(index) {
            *slot = total as u32;
        }
        let len = block.values.get(index).map_or(0, |v| v.payload.len());
        let padded = len.checked_add(3).map(|n| n & !3usize).ok_or_else(|| invalid("glue block is too large"))?;
        if total > u32::MAX as usize - padded {
            return Err(invalid("glue block is too large"));
        }
        total += padded;
    }

    let mut data = vec![0u8; total];
    {
        let mut w = LittleEndianWriter::new(&mut data);
        w.u32(0, block.flags.len() as u32)?;
        for (i, flag) in block.flags.iter().enumerate() {
            w.u32(4 + i * 4, *flag)?;
        }
        let mut cursor = 4 + block.flags.len() * 4;
        for (value, offset) in block.values.iter().zip(&offsets) {
            w.u32(cursor, value.hash)?;
            w.u32(cursor + 4, *offset)?;
            cursor += 8;
        }
    }
    for &index in &order {
        let (Some(value), Some(&offset)) = (block.values.get(index), offsets.get(index)) else { continue };
        let start = offset as usize;
        data.get_mut(start..start + value.payload.len()).ok_or_else(|| invalid("glue block is too large"))?.copy_from_slice(&value.payload);
    }
    Ok(data)
}
