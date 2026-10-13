//! Nitro LZ10/LZ11, Yaz0 and the BLZ overlay codec. The compressors must produce the same bytes as the C++ ones.

use bigstar_core::{Error, ErrorCode, Result};

const OUTPUT_LIMIT: usize = 256 * 1024 * 1024;

fn err(code: ErrorCode, message: &str) -> Error {
    Error::new(code, message)
}

fn at(data: &[u8], index: usize) -> u8 {
    data.get(index).copied().unwrap_or(0)
}

fn put(data: &mut [u8], index: usize, value: u8) {
    if let Some(slot) = data.get_mut(index) {
        *slot = value;
    }
}

#[derive(Clone, Copy, Default)]
struct LzMatch {
    length: usize,
    displacement: usize,
}

struct ForwardMatcher<'a> {
    data: &'a [u8],
    head: Vec<isize>,
    previous: Vec<isize>,
}

impl<'a> ForwardMatcher<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, head: vec![-1; 65536], previous: vec![-1; data.len()] }
    }

    fn key(&self, position: usize) -> usize {
        (usize::from(at(self.data, position)) * 251 * 251 + usize::from(at(self.data, position + 1)) * 251 + usize::from(at(self.data, position + 2)))
            & 0xFFFF
    }

    fn insert(&mut self, position: usize) {
        if position + 2 >= self.data.len() {
            return;
        }
        let bucket = self.key(position);
        let head = self.head.get(bucket).copied().unwrap_or(-1);
        if let Some(slot) = self.previous.get_mut(position) {
            *slot = head;
        }
        if let Some(slot) = self.head.get_mut(bucket) {
            *slot = position as isize;
        }
    }

    fn find(&self, position: usize, maximum: usize) -> LzMatch {
        if position + 2 >= self.data.len() {
            return LzMatch::default();
        }
        let lower = position.saturating_sub(4096) as isize;
        let mut best = LzMatch::default();
        let mut candidate = self.head.get(self.key(position)).copied().unwrap_or(-1);
        while candidate >= lower {
            let start = candidate as usize;
            let limit = maximum.min(self.data.len() - position);
            let mut length = 0;
            while length < limit && at(self.data, start + length) == at(self.data, position + length) {
                length += 1;
            }
            if length > best.length {
                best = LzMatch { length, displacement: position - start };
            }
            if best.length == limit {
                break;
            }
            candidate = self.previous.get(start).copied().unwrap_or(-1);
        }
        if best.length < 3 { LzMatch::default() } else { best }
    }
}

fn compress_nitro_lz(data: &[u8], lz11: bool) -> Result<Vec<u8>> {
    if (!lz11 && data.len() > 0xFFFFFF) || data.len() > u32::MAX as usize {
        return Err(err(ErrorCode::InvalidArgument, "input is too large for the selected Nitro LZ header"));
    }
    let mut output = Vec::with_capacity(data.len() + data.len() / 8 + 8);
    output.push(if lz11 { 0x11 } else { 0x10 });
    if lz11 && (data.is_empty() || data.len() > 0xFFFFFF) {
        output.extend_from_slice(&[0, 0, 0]);
        output.extend_from_slice(&(data.len() as u32).to_le_bytes());
    } else {
        output.extend_from_slice(&(data.len() as u32).to_le_bytes()[..3]);
    }

    let mut matcher = ForwardMatcher::new(data);
    let mut position = 0usize;
    while position < data.len() {
        let flag_offset = output.len();
        output.push(0);
        let mut bit = 7i32;
        while bit >= 0 && position < data.len() {
            let found = matcher.find(position, if lz11 { 0x10110 } else { 18 });
            let consumed = if found.length != 0 { found.length } else { 1 };
            if found.length == 0 {
                output.push(at(data, position));
            } else {
                if let Some(flags) = output.get_mut(flag_offset) {
                    *flags |= 1 << bit;
                }
                let displacement = found.displacement - 1;
                let length = found.length;
                if !lz11 {
                    output.push((((length - 3) << 4) | (displacement >> 8)) as u8);
                    output.push(displacement as u8);
                } else if length <= 16 {
                    output.push((((length - 1) << 4) | (displacement >> 8)) as u8);
                    output.push(displacement as u8);
                } else if length <= 272 {
                    let l = length - 0x11;
                    output.push((l >> 4) as u8);
                    output.push(((l << 4) | (displacement >> 8)) as u8);
                    output.push(displacement as u8);
                } else {
                    let l = length - 0x111;
                    output.push((0x10 | (l >> 12)) as u8);
                    output.push((l >> 4) as u8);
                    output.push(((l << 4) | (displacement >> 8)) as u8);
                    output.push(displacement as u8);
                }
            }
            for i in 0..consumed {
                matcher.insert(position + i);
            }
            position += consumed;
            bit -= 1;
        }
    }
    Ok(output)
}

pub fn compress_nitro_lz10(data: &[u8]) -> Result<Vec<u8>> {
    compress_nitro_lz(data, false)
}

pub fn compress_nitro_lz11(data: &[u8]) -> Result<Vec<u8>> {
    compress_nitro_lz(data, true)
}

pub fn decompress_nitro_lz(data: &[u8]) -> Result<Vec<u8>> {
    let kind = at(data, 0);
    if data.len() < 4 || (kind != 0x10 && kind != 0x11) {
        return Err(err(ErrorCode::ParseError, "Nitro LZ stream has an unsupported header"));
    }
    let mut input = 4usize;
    let mut output_size = usize::from(at(data, 1)) | (usize::from(at(data, 2)) << 8) | (usize::from(at(data, 3)) << 16);
    if output_size == 0 && kind == 0x11 {
        if data.len() < 8 {
            return Err(err(ErrorCode::OutOfBounds, "extended Nitro LZ header is truncated"));
        }
        output_size = (4..8).rev().fold(0usize, |acc, i| (acc << 8) | usize::from(at(data, i)));
        input = 8;
    }
    if output_size > OUTPUT_LIMIT {
        return Err(err(ErrorCode::InvalidArgument, "Nitro LZ output exceeds the safety limit"));
    }
    let mut output: Vec<u8> = Vec::with_capacity(output_size);
    while output.len() < output_size {
        if input >= data.len() {
            return Err(err(ErrorCode::OutOfBounds, "Nitro LZ flag byte is truncated"));
        }
        let flags = at(data, input);
        input += 1;
        let mut bit = 7i32;
        while bit >= 0 && output.len() < output_size {
            let current = bit;
            bit -= 1;
            if flags & (1 << current) == 0 {
                if input >= data.len() {
                    return Err(err(ErrorCode::OutOfBounds, "Nitro LZ literal is truncated"));
                }
                output.push(at(data, input));
                input += 1;
                continue;
            }
            if input + 1 >= data.len() {
                return Err(err(ErrorCode::OutOfBounds, "Nitro LZ back-reference is truncated"));
            }
            let (length, displacement);
            if kind == 0x10 {
                let first = usize::from(at(data, input));
                let second = usize::from(at(data, input + 1));
                input += 2;
                length = (first >> 4) + 3;
                displacement = (((first & 0x0F) << 8) | second) + 1;
            } else {
                let first = usize::from(at(data, input));
                input += 1;
                let high = first >> 4;
                if high == 0 {
                    if input + 1 >= data.len() {
                        return Err(err(ErrorCode::OutOfBounds, "Nitro LZ11 token is truncated"));
                    }
                    let second = usize::from(at(data, input));
                    let third = usize::from(at(data, input + 1));
                    input += 2;
                    length = (((first & 0x0F) << 4) | (second >> 4)) + 0x11;
                    displacement = (((second & 0x0F) << 8) | third) + 1;
                } else if high == 1 {
                    if input + 2 >= data.len() {
                        return Err(err(ErrorCode::OutOfBounds, "Nitro LZ11 long token is truncated"));
                    }
                    let second = usize::from(at(data, input));
                    let third = usize::from(at(data, input + 1));
                    let fourth = usize::from(at(data, input + 2));
                    input += 3;
                    length = (((first & 0x0F) << 12) | (second << 4) | (third >> 4)) + 0x111;
                    displacement = (((third & 0x0F) << 8) | fourth) + 1;
                } else {
                    let second = usize::from(at(data, input));
                    input += 1;
                    length = high + 1;
                    displacement = (((first & 0x0F) << 8) | second) + 1;
                }
            }
            if displacement == 0 || displacement > output.len() {
                return Err(err(ErrorCode::ParseError, "Nitro LZ back-reference precedes the output buffer"));
            }
            let mut i = 0;
            while i < length && output.len() < output_size {
                output.push(at(&output, output.len() - displacement));
                i += 1;
            }
        }
    }
    Ok(output)
}

pub fn decompress_yaz0(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 16 || !data.starts_with(b"Yaz0") {
        return Err(err(ErrorCode::ParseError, "Yaz0 header is missing or truncated"));
    }
    let output_size = (4..8).fold(0usize, |acc, i| (acc << 8) | usize::from(at(data, i)));
    if output_size > OUTPUT_LIMIT {
        return Err(err(ErrorCode::InvalidArgument, "Yaz0 output exceeds the safety limit"));
    }
    let mut output: Vec<u8> = Vec::with_capacity(output_size);
    let mut input = 16usize;
    while output.len() < output_size {
        if input >= data.len() {
            return Err(err(ErrorCode::OutOfBounds, "Yaz0 flag byte is truncated"));
        }
        let flags = at(data, input);
        input += 1;
        let mut bit = 7i32;
        while bit >= 0 && output.len() < output_size {
            let current = bit;
            bit -= 1;
            if flags & (1 << current) != 0 {
                if input >= data.len() {
                    return Err(err(ErrorCode::OutOfBounds, "Yaz0 literal is truncated"));
                }
                output.push(at(data, input));
                input += 1;
                continue;
            }
            if input + 1 >= data.len() {
                return Err(err(ErrorCode::OutOfBounds, "Yaz0 back-reference is truncated"));
            }
            let first = usize::from(at(data, input));
            let second = usize::from(at(data, input + 1));
            input += 2;
            let displacement = (((first & 0x0F) << 8) | second) + 1;
            let mut length = first >> 4;
            if length == 0 {
                if input >= data.len() {
                    return Err(err(ErrorCode::OutOfBounds, "Yaz0 long token is truncated"));
                }
                length = usize::from(at(data, input)) + 0x12;
                input += 1;
            } else {
                length += 2;
            }
            if displacement > output.len() {
                return Err(err(ErrorCode::ParseError, "Yaz0 back-reference precedes the output buffer"));
            }
            let mut i = 0;
            while i < length && output.len() < output_size {
                output.push(at(&output, output.len() - displacement));
                i += 1;
            }
        }
    }
    Ok(output)
}

pub fn compress_yaz0(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() > u32::MAX as usize {
        return Err(err(ErrorCode::InvalidArgument, "input is too large for Yaz0"));
    }
    let mut output = b"Yaz0".to_vec();
    output.extend_from_slice(&(data.len() as u32).to_be_bytes());
    output.extend_from_slice(&[0; 8]);
    let mut matcher = ForwardMatcher::new(data);
    let mut position = 0usize;
    while position < data.len() {
        let flag_offset = output.len();
        output.push(0);
        let mut bit = 7i32;
        while bit >= 0 && position < data.len() {
            let found = matcher.find(position, 0x111);
            let consumed = if found.length != 0 { found.length } else { 1 };
            if found.length == 0 {
                if let Some(flags) = output.get_mut(flag_offset) {
                    *flags |= 1 << bit;
                }
                output.push(at(data, position));
            } else {
                let displacement = found.displacement - 1;
                if found.length >= 0x12 {
                    output.push((displacement >> 8) as u8);
                    output.push(displacement as u8);
                    output.push((found.length - 0x12) as u8);
                } else {
                    output.push((((found.length - 2) << 4) | (displacement >> 8)) as u8);
                    output.push(displacement as u8);
                }
            }
            for i in 0..consumed {
                matcher.insert(position + i);
            }
            position += consumed;
            bit -= 1;
        }
    }
    while !output.len().is_multiple_of(4) {
        output.push(0);
    }
    Ok(output)
}

pub fn decompress_blz_overlay(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 8 {
        return Err(err(ErrorCode::OutOfBounds, "BLZ overlay footer is truncated"));
    }
    let footer = data.len() - 8;
    let word = |o: usize| (0..4).rev().fold(0u32, |acc, i| (acc << 8) | u32::from(at(data, o + i)));
    let packed = word(footer);
    let extra = word(footer + 4);
    if extra == 0 {
        return Ok(data.to_vec());
    }
    let header_size = (packed >> 24) as usize;
    let compressed_size = (packed & 0x00FF_FFFF) as usize;
    if header_size < 8 || compressed_size < header_size || compressed_size > data.len() || extra as usize > OUTPUT_LIMIT {
        return Ok(data.to_vec());
    }
    let mut output = data.to_vec();
    output.resize(data.len() + extra as usize, 0);
    let mut source = data.len() - header_size;
    let source_limit = data.len() - compressed_size;
    let mut destination = output.len();
    while source > source_limit {
        source -= 1;
        let flags = at(&output, source);
        let mut bit = 7i32;
        while bit >= 0 && source > source_limit {
            let current = bit;
            bit -= 1;
            if flags & (1 << current) == 0 {
                if source == 0 || destination == 0 {
                    return Err(err(ErrorCode::ParseError, "BLZ literal is invalid"));
                }
                source -= 1;
                destination -= 1;
                let byte = at(&output, source);
                put(&mut output, destination, byte);
                continue;
            }
            if source < 2 {
                return Err(err(ErrorCode::OutOfBounds, "BLZ token is truncated"));
            }
            source -= 1;
            let high = at(&output, source);
            source -= 1;
            let low = at(&output, source);
            let displacement = ((usize::from(high) << 8 | usize::from(low)) & 0x0FFF) + 2;
            let length = usize::from(high >> 4) + 3;
            for _ in 0..length {
                if destination == 0 || destination + displacement >= output.len() {
                    return Err(err(ErrorCode::ParseError, "BLZ back-reference is invalid"));
                }
                let byte = at(&output, destination + displacement);
                put(&mut output, destination - 1, byte);
                destination -= 1;
            }
        }
    }
    Ok(output)
}

fn backward_match(data: &[u8], remaining: usize, match_position: &mut usize) -> usize {
    let maximum = remaining.min(18);
    let available = (data.len() - remaining).min(4098);
    let mut best = 0;
    for i in 0..available {
        if at(data, remaining + i) != at(data, remaining - 1) {
            continue;
        }
        let limit = maximum.min(i + 1);
        let mut length = 0;
        while length < limit && at(data, remaining - 1 - length) == at(data, remaining + i - length) {
            length += 1;
        }
        if length > best {
            best = length;
            *match_position = i;
        }
    }
    best
}

/// Returns an empty vector when the data cannot be made smaller, as the C++ does.
pub fn compress_blz_overlay(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 16 || data.len() > 0xFF_FFFF {
        return Ok(Vec::new());
    }
    let mut work = vec![0u8; data.len()];
    let mut source = data.len();
    let mut target = data.len();
    while source > 0 {
        if target == 0 {
            return Ok(Vec::new());
        }
        target -= 1;
        let flag_offset = target;
        let mut flags = 0u8;
        for _ in 0..8 {
            flags <<= 1;
            if source == 0 {
                continue;
            }
            let mut match_position = 0usize;
            let length = backward_match(data, source, &mut match_position);
            if length <= 2 {
                if target == 0 {
                    return Ok(Vec::new());
                }
                target -= 1;
                source -= 1;
                put(&mut work, target, at(data, source));
            } else {
                if target < 2 {
                    return Ok(Vec::new());
                }
                source -= length;
                let packed = ((match_position.wrapping_sub(2) & 0x0FFF) as u16) | (((length - 3) as u16) << 12);
                target -= 1;
                put(&mut work, target, (packed >> 8) as u8);
                target -= 1;
                put(&mut work, target, packed as u8);
                flags |= 1;
            }
        }
        put(&mut work, flag_offset, flags);
    }
    let body_size = data.len() - target;
    let mut total = body_size + 8;
    let padding = (4 - total % 4) % 4;
    total += padding;
    if total >= data.len() {
        return Ok(Vec::new());
    }
    let mut output = vec![0u8; total];
    if let (Some(dst), Some(src)) = (output.get_mut(..data.len() - target), work.get(target..)) {
        dst.copy_from_slice(src);
    }
    let offset_in = (total as u32) | ((8 + padding as u32) << 24);
    if let Some(dst) = output.get_mut(total - 8..total - 4) {
        dst.copy_from_slice(&offset_in.to_le_bytes());
    }
    if let Some(dst) = output.get_mut(total - 4..) {
        dst.copy_from_slice(&((data.len() - total) as u32).to_le_bytes());
    }
    Ok(output)
}
