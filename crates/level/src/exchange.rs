//! Interchange files: NSMB Editor's exported level (`.nml`) and level recovery snapshots.
//! Port of `libs/nsmb-project/src/level_exchange.cpp`; pure, so either side of the connection can read them.

use bigstar_core::{Error, ErrorCode, Result};
use serde::{Deserialize, Serialize};

const MAX_BLOB_SIZE: usize = 128 * 1024 * 1024;
const NML_HEADER: &str = "NSMBe5 Exported Level";
const OLD_NML_HEADER: &str = "NSMBe4 Exported Level";
const RECOVERY_HEADER: [u8; 16] = *b"BIGSTAR-RECOVER\x01";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelPackage {
    pub course_file_id: u16,
    pub bgdat_file_id: u16,
    pub course: Vec<u8>,
    pub bgdat: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LevelRecoverySnapshot {
    pub course_path: String,
    pub bgdat_path: String,
    pub package: LevelPackage,
}

fn put_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    put_u32(out, value.len() as u32);
    out.extend_from_slice(value.as_bytes());
}

/// .NET's `BinaryWriter.Write(string)`: a 7-bit varint length, then the bytes.
fn put_dotnet_string(out: &mut Vec<u8>, value: &str) {
    let mut size = value.len() as u32;
    loop {
        let mut byte = (size & 0x7f) as u8;
        size >>= 7;
        if size != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if size == 0 {
            break;
        }
    }
    out.extend_from_slice(value.as_bytes());
}

struct Reader<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn take(&mut self, size: usize) -> Result<&'a [u8]> {
        let end = self.offset.checked_add(size).filter(|&end| end <= self.data.len());
        let slice =
            end.and_then(|end| self.data.get(self.offset..end)).ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "exchange file is truncated"))?;
        self.offset += size;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16> {
        let bytes = self.take(2)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap_or_default()))
    }

    fn u32(&mut self) -> Result<u32> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap_or_default()))
    }

    fn text(&mut self, size: usize) -> Result<String> {
        if size > MAX_BLOB_SIZE {
            return Err(Error::new(ErrorCode::ParseError, "exchange string is too large"));
        }
        Ok(String::from_utf8_lossy(self.take(size)?).into_owned())
    }

    fn string(&mut self) -> Result<String> {
        let size = self.u32()? as usize;
        self.text(size)
    }

    fn dotnet_string(&mut self) -> Result<String> {
        let mut size = 0u32;
        let mut shift = 0;
        while shift < 35 {
            let byte = self.take(1)?.first().copied().unwrap_or(0);
            size |= u32::from(byte & 0x7f).checked_shl(shift).unwrap_or(0);
            if byte & 0x80 == 0 {
                return self.text(size as usize);
            }
            shift += 7;
        }
        Err(Error::new(ErrorCode::ParseError, "exchange string length is invalid"))
    }

    fn done(&self) -> bool {
        self.offset == self.data.len()
    }
}

fn validate_package(package: &LevelPackage) -> Result<()> {
    if package.course.is_empty() {
        return Err(Error::new(ErrorCode::InvalidArgument, "level package has no course data"));
    }
    if package.course.len() > MAX_BLOB_SIZE || package.bgdat.len() > MAX_BLOB_SIZE {
        return Err(Error::new(ErrorCode::InvalidArgument, "level package is too large"));
    }
    Ok(())
}

/// Relative, and with no `.` or `..` component. Both separators count, so a path is judged the same on every
/// platform; a drive letter makes it absolute.
fn safe_patch_path(text: &str) -> bool {
    let bytes = text.as_bytes();
    let absolute = text.starts_with(['/', '\\']) || (bytes.len() >= 2 && bytes.get(1) == Some(&b':'));
    !text.is_empty() && !absolute && text.split(['/', '\\']).all(|part| part != "." && part != "..")
}

fn read_package_body(reader: &mut Reader<'_>) -> Result<LevelPackage> {
    let course_file_id = reader.u16()?;
    let bgdat_file_id = reader.u16()?;
    let course_size = reader.u32()? as usize;
    if course_size > MAX_BLOB_SIZE {
        return Err(Error::new(ErrorCode::ParseError, "course data is too large"));
    }
    let course = reader.take(course_size)?.to_vec();
    let bgdat_size = reader.u32()? as usize;
    if bgdat_size > MAX_BLOB_SIZE {
        return Err(Error::new(ErrorCode::ParseError, "BGDat data is too large"));
    }
    let bgdat = reader.take(bgdat_size)?.to_vec();
    let package = LevelPackage { course_file_id, bgdat_file_id, course, bgdat };
    validate_package(&package)?;
    Ok(package)
}

fn write_package_body(out: &mut Vec<u8>, package: &LevelPackage) {
    put_u16(out, package.course_file_id);
    put_u16(out, package.bgdat_file_id);
    put_u32(out, package.course.len() as u32);
    out.extend_from_slice(&package.course);
    put_u32(out, package.bgdat.len() as u32);
    out.extend_from_slice(&package.bgdat);
}

pub fn encode_nml(package: &LevelPackage) -> Result<Vec<u8>> {
    validate_package(package)?;
    let mut result = Vec::with_capacity(package.course.len() + package.bgdat.len() + 64);
    put_dotnet_string(&mut result, NML_HEADER);
    put_u16(&mut result, 1);
    write_package_body(&mut result, package);
    Ok(result)
}

pub fn decode_nml(bytes: &[u8]) -> Result<LevelPackage> {
    let mut reader = Reader::new(bytes);
    let header = reader.dotnet_string()?;
    if header != NML_HEADER && header != OLD_NML_HEADER {
        return Err(Error::new(ErrorCode::Unsupported, "file is not an NSMB exported level"));
    }
    if reader.u16()? > 1 {
        return Err(Error::new(ErrorCode::Unsupported, "exported level version is newer than Big Star supports"));
    }
    let result = read_package_body(&mut reader)?;
    if !reader.done() {
        return Err(Error::new(ErrorCode::ParseError, "exported level has trailing data"));
    }
    Ok(result)
}

pub fn encode_level_recovery(snapshot: &LevelRecoverySnapshot) -> Result<Vec<u8>> {
    if !safe_patch_path(&snapshot.course_path) || !safe_patch_path(&snapshot.bgdat_path) {
        return Err(Error::new(ErrorCode::InvalidArgument, "recovery snapshot has unsafe level paths"));
    }
    validate_package(&snapshot.package)?;
    let mut result = RECOVERY_HEADER.to_vec();
    put_string(&mut result, &snapshot.course_path);
    put_string(&mut result, &snapshot.bgdat_path);
    write_package_body(&mut result, &snapshot.package);
    Ok(result)
}

pub fn decode_level_recovery(bytes: &[u8]) -> Result<LevelRecoverySnapshot> {
    let mut reader = Reader::new(bytes);
    if reader.take(RECOVERY_HEADER.len())? != RECOVERY_HEADER {
        return Err(Error::new(ErrorCode::Unsupported, "file is not a Big Star recovery snapshot"));
    }
    let course_path = reader.string()?;
    let bgdat_path = reader.string()?;
    if !safe_patch_path(&course_path) || !safe_patch_path(&bgdat_path) {
        return Err(Error::new(ErrorCode::ParseError, "recovery snapshot has unsafe level paths"));
    }
    let package = read_package_body(&mut reader)?;
    if !reader.done() {
        return Err(Error::new(ErrorCode::ParseError, "recovery snapshot has trailing data"));
    }
    Ok(LevelRecoverySnapshot { course_path, bgdat_path, package })
}
