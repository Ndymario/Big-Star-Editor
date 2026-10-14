use bigstar_core::{Error, ErrorCode, LittleEndianReader, Result};
use serde::{Deserialize, Serialize};

pub const ROOT_DIRECTORY_ID: u16 = 0xF000;
pub const DS_CARTRIDGE_HEADER_SIZE: usize = 0x200;
pub const DEFAULT_HEADER_REGION_SIZE: usize = 0x4000;
pub const OVERLAY_TABLE_ENTRY_SIZE: usize = 0x20;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomRegion {
    pub rom_offset: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomArmBinary {
    pub rom_offset: u32,
    pub entry_address: u32,
    pub ram_address: u32,
    pub size: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RomHeader {
    pub game_title: String,
    pub game_code: String,
    pub maker_code: String,
    pub unit_code: u8,
    pub encryption_seed_select: u8,
    pub device_capacity: u8,
    pub region: u8,
    pub rom_version: u8,
    pub autostart: u8,
    pub arm9: RomArmBinary,
    pub arm7: RomArmBinary,
    pub fnt: RomRegion,
    pub fat: RomRegion,
    pub arm9_overlay_table: RomRegion,
    pub arm7_overlay_table: RomRegion,
    pub normal_card_control: u32,
    pub key1_card_control: u32,
    pub banner_rom_offset: u32,
    pub secure_area_checksum: u16,
    pub secure_area_delay: u16,
    pub arm9_autoload_hook_address: u32,
    pub arm7_autoload_hook_address: u32,
    pub total_used_rom_size: u32,
    pub header_size: u32,
    pub nintendo_logo_checksum: u16,
    pub header_checksum: u16,
    pub arm9_rom_offset: u32,
    pub arm7_rom_offset: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FatEntry {
    pub start_address: u32,
    pub end_address: u32,
}

impl FatEntry {
    pub fn used(&self) -> bool {
        self.start_address != 0 || self.end_address != 0
    }

    pub fn size(&self) -> u32 {
        if !self.used() || self.end_address < self.start_address { 0 } else { self.end_address - self.start_address }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FntDirectoryEntry {
    pub sub_table_offset: u32,
    pub first_file_id: u16,
    pub parent_directory_id: u16,
    pub directory_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NitroFileEntry {
    pub id: u16,
    /// `/`-separated, relative to the ROM's file root.
    pub path: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OverlayTableEntry {
    pub overlay_id: u32,
    pub ram_address: u32,
    pub ram_size: u32,
    pub bss_size: u32,
    pub static_initializer_start_address: u32,
    pub static_initializer_end_address: u32,
    pub file_id: u32,
    pub reserved: u32,
}

pub(crate) mod header_offset {
    pub const GAME_TITLE: usize = 0x000;
    pub const GAME_CODE: usize = 0x00C;
    pub const MAKER_CODE: usize = 0x010;
    pub const UNIT_CODE: usize = 0x012;
    pub const ENCRYPTION_SEED_SELECT: usize = 0x013;
    pub const DEVICE_CAPACITY: usize = 0x014;
    pub const REGION: usize = 0x01D;
    pub const ROM_VERSION: usize = 0x01E;
    pub const AUTOSTART: usize = 0x01F;
    pub const ARM9_ROM_OFFSET: usize = 0x020;
    pub const ARM9_ENTRY_ADDRESS: usize = 0x024;
    pub const ARM9_RAM_ADDRESS: usize = 0x028;
    pub const ARM9_SIZE: usize = 0x02C;
    pub const ARM7_ROM_OFFSET: usize = 0x030;
    pub const ARM7_ENTRY_ADDRESS: usize = 0x034;
    pub const ARM7_RAM_ADDRESS: usize = 0x038;
    pub const ARM7_SIZE: usize = 0x03C;
    pub const FNT_OFFSET: usize = 0x040;
    pub const FNT_SIZE: usize = 0x044;
    pub const FAT_OFFSET: usize = 0x048;
    pub const FAT_SIZE: usize = 0x04C;
    pub const ARM9_OVERLAY_OFFSET: usize = 0x050;
    pub const ARM9_OVERLAY_SIZE: usize = 0x054;
    pub const ARM7_OVERLAY_OFFSET: usize = 0x058;
    pub const ARM7_OVERLAY_SIZE: usize = 0x05C;
    pub const NORMAL_CARD_CONTROL: usize = 0x060;
    pub const KEY1_CARD_CONTROL: usize = 0x064;
    pub const BANNER_OFFSET: usize = 0x068;
    pub const SECURE_AREA_CHECKSUM: usize = 0x06C;
    pub const SECURE_AREA_DELAY: usize = 0x06E;
    pub const ARM9_AUTOLOAD_HOOK_ADDRESS: usize = 0x070;
    pub const ARM7_AUTOLOAD_HOOK_ADDRESS: usize = 0x074;
    pub const TOTAL_USED_ROM_SIZE: usize = 0x080;
    pub const HEADER_SIZE: usize = 0x084;
    pub const NINTENDO_LOGO_CHECKSUM: usize = 0x15C;
    pub const HEADER_CHECKSUM: usize = 0x15E;
}

fn parse_error(message: &str) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

fn ascii_field(data: &[u8], offset: usize, length: usize) -> Result<String> {
    let bytes = offset
        .checked_add(length)
        .and_then(|end| data.get(offset..end))
        .ok_or_else(|| Error::new(ErrorCode::OutOfBounds, "header field is out of bounds"))?;
    let text = String::from_utf8_lossy(bytes);
    Ok(text.trim_end_matches(['\0', ' ']).to_owned())
}

/// How long the banner at `header.banner_rom_offset` is, from its own version word. A banner declares no length,
/// so a reader that only has the header and the FAT needs both this and [`banner_region_limit`]: the version says
/// what the format implies, and the limit says how much room there actually is.
pub fn banner_size_from_version(banner_data: &[u8]) -> usize {
    match LittleEndianReader::new(banner_data).u16(0) {
        Ok(0x0001 | 0x0002) => 0x0A00,
        Ok(0x0003) => 0x0C00,
        Ok(0x0103) => 0x2400,
        _ => 0,
    }
}

/// Where the region after the banner starts -- the banner's own header does not say how long it is, so the only
/// bound the ROM gives is whatever comes next.
pub fn banner_region_limit(header: &RomHeader, fat: &[FatEntry], rom_size: usize) -> usize {
    let mut next = rom_size;
    let mut consider = |candidate: u32| {
        if candidate > header.banner_rom_offset
            && let Ok(candidate) = usize::try_from(candidate)
            && candidate <= rom_size
        {
            next = next.min(candidate);
        }
    };
    consider(header.arm9.rom_offset);
    consider(header.arm7.rom_offset);
    consider(header.fnt.rom_offset);
    consider(header.fat.rom_offset);
    consider(header.arm9_overlay_table.rom_offset);
    consider(header.arm7_overlay_table.rom_offset);
    consider(header.total_used_rom_size);
    for entry in fat {
        consider(entry.start_address);
    }
    next
}

/// How many bytes the banner at `header.banner_rom_offset` occupies: the length its version implies when that fits
/// before the next region, else all the room there is. `version` is the banner's first two bytes.
pub fn banner_size(header: &RomHeader, fat: &[FatEntry], rom_size: usize, version: &[u8]) -> usize {
    let Ok(offset) = usize::try_from(header.banner_rom_offset) else { return 0 };
    if offset == 0 || offset >= rom_size {
        return 0;
    }
    let available = banner_region_limit(header, fat, rom_size).saturating_sub(offset);
    let implied = banner_size_from_version(version);
    if implied != 0 && implied <= available { implied } else { available }
}

pub fn crc16(data: &[u8], initial: u16) -> u16 {
    let mut crc = initial;
    for &byte in data {
        crc ^= u16::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xA001 } else { crc >> 1 };
        }
    }
    crc
}

fn invalid_rom_name(name: &[u8]) -> bool {
    name.is_empty() || name.contains(&b'/') || name.contains(&b'\\')
}

fn parse_fnt_directory_table(data: &[u8]) -> Result<Vec<FntDirectoryEntry>> {
    if data.len() < 8 {
        return Err(parse_error("FNT directory table is missing the root entry"));
    }
    let reader = LittleEndianReader::new(data);
    let directory_count = usize::from(reader.u16(6)?);
    if directory_count == 0 || directory_count > 4096 {
        return Err(parse_error("FNT directory count is invalid"));
    }
    if directory_count * 8 > data.len() {
        return Err(parse_error("FNT directory table extends past the FNT data"));
    }

    let mut directories = Vec::with_capacity(directory_count);
    for i in 0..directory_count {
        let offset = i * 8;
        let sub_table_offset = reader.u32(offset)?;
        let first_file_id = reader.u16(offset + 4)?;
        let third = reader.u16(offset + 6)?;
        let mut entry = FntDirectoryEntry { sub_table_offset, first_file_id, ..FntDirectoryEntry::default() };
        if i == 0 {
            entry.directory_count = third;
            entry.parent_directory_id = ROOT_DIRECTORY_ID;
        } else {
            entry.parent_directory_id = third;
        }
        directories.push(entry);
    }
    Ok(directories)
}

pub fn parse_rom_header(data: &[u8]) -> Result<RomHeader> {
    if data.len() < DS_CARTRIDGE_HEADER_SIZE {
        return Err(parse_error("NDS header needs at least 0x200 bytes"));
    }
    use header_offset as at;
    let reader = LittleEndianReader::new(data);
    let arm = |rom_offset, entry, ram, size| -> Result<RomArmBinary> {
        Ok(RomArmBinary {
            rom_offset: reader.u32(rom_offset)?,
            entry_address: reader.u32(entry)?,
            ram_address: reader.u32(ram)?,
            size: reader.u32(size)?,
        })
    };
    let region = |offset, size| -> Result<RomRegion> { Ok(RomRegion { rom_offset: reader.u32(offset)?, size: reader.u32(size)? }) };

    let arm9 = arm(at::ARM9_ROM_OFFSET, at::ARM9_ENTRY_ADDRESS, at::ARM9_RAM_ADDRESS, at::ARM9_SIZE)?;
    let arm7 = arm(at::ARM7_ROM_OFFSET, at::ARM7_ENTRY_ADDRESS, at::ARM7_RAM_ADDRESS, at::ARM7_SIZE)?;
    Ok(RomHeader {
        game_title: ascii_field(data, at::GAME_TITLE, 12)?,
        game_code: ascii_field(data, at::GAME_CODE, 4)?,
        maker_code: ascii_field(data, at::MAKER_CODE, 2)?,
        unit_code: reader.u8(at::UNIT_CODE)?,
        encryption_seed_select: reader.u8(at::ENCRYPTION_SEED_SELECT)?,
        device_capacity: reader.u8(at::DEVICE_CAPACITY)?,
        region: reader.u8(at::REGION)?,
        rom_version: reader.u8(at::ROM_VERSION)?,
        autostart: reader.u8(at::AUTOSTART)?,
        arm9,
        arm7,
        fnt: region(at::FNT_OFFSET, at::FNT_SIZE)?,
        fat: region(at::FAT_OFFSET, at::FAT_SIZE)?,
        arm9_overlay_table: region(at::ARM9_OVERLAY_OFFSET, at::ARM9_OVERLAY_SIZE)?,
        arm7_overlay_table: region(at::ARM7_OVERLAY_OFFSET, at::ARM7_OVERLAY_SIZE)?,
        normal_card_control: reader.u32(at::NORMAL_CARD_CONTROL)?,
        key1_card_control: reader.u32(at::KEY1_CARD_CONTROL)?,
        banner_rom_offset: reader.u32(at::BANNER_OFFSET)?,
        secure_area_checksum: reader.u16(at::SECURE_AREA_CHECKSUM)?,
        secure_area_delay: reader.u16(at::SECURE_AREA_DELAY)?,
        arm9_autoload_hook_address: reader.u32(at::ARM9_AUTOLOAD_HOOK_ADDRESS)?,
        arm7_autoload_hook_address: reader.u32(at::ARM7_AUTOLOAD_HOOK_ADDRESS)?,
        total_used_rom_size: reader.u32(at::TOTAL_USED_ROM_SIZE)?,
        header_size: reader.u32(at::HEADER_SIZE)?,
        nintendo_logo_checksum: reader.u16(at::NINTENDO_LOGO_CHECKSUM)?,
        header_checksum: reader.u16(at::HEADER_CHECKSUM)?,
        arm9_rom_offset: arm9.rom_offset,
        arm7_rom_offset: arm7.rom_offset,
    })
}

pub fn parse_fat(data: &[u8]) -> Result<Vec<FatEntry>> {
    if !data.len().is_multiple_of(8) {
        return Err(parse_error("FAT size must be a multiple of 8 bytes"));
    }
    let reader = LittleEndianReader::new(data);
    (0..data.len() / 8)
        .map(|i| {
            let start = reader.u32(i * 8)?;
            let end = reader.u32(i * 8 + 4)?;
            if end < start {
                return Err(parse_error("FAT entry end address is before its start address"));
            }
            Ok(FatEntry { start_address: start, end_address: end })
        })
        .collect()
}

struct FntWalk<'a> {
    data: &'a [u8],
    directories: Vec<FntDirectoryEntry>,
    visiting: Vec<bool>,
    visited: Vec<bool>,
    files: Vec<NitroFileEntry>,
}

impl FntWalk<'_> {
    fn directory(&mut self, index: usize, base_path: &str) -> Result<()> {
        let directory = *self.directories.get(index).ok_or_else(|| parse_error("FNT sub-directory ID is out of range"))?;
        if self.visiting.get(index).copied().unwrap_or(false) {
            return Err(parse_error("FNT directory table contains a cycle"));
        }
        if self.visited.get(index).copied().unwrap_or(false) {
            return Ok(());
        }
        if let Some(flag) = self.visiting.get_mut(index) {
            *flag = true;
        }

        let data = self.data;
        let mut offset = usize::try_from(directory.sub_table_offset).unwrap_or(usize::MAX);
        if offset >= data.len() {
            return Err(parse_error("FNT sub-table offset is out of bounds"));
        }
        let mut next_file_id = directory.first_file_id;
        loop {
            let type_length = *data.get(offset).ok_or_else(|| parse_error("FNT sub-table is missing its terminator"))?;
            offset += 1;
            if type_length == 0 {
                break;
            }
            let is_directory = type_length & 0x80 != 0;
            let length = usize::from(type_length & 0x7F);
            let name = data.get(offset..offset + length).filter(|_| length != 0).ok_or_else(|| parse_error("FNT entry name length is invalid"))?;
            offset += length;
            if invalid_rom_name(name) {
                return Err(parse_error("FNT entry contains an invalid path name"));
            }
            let name = String::from_utf8_lossy(name);
            let path = if base_path.is_empty() { name.into_owned() } else { format!("{base_path}/{name}") };

            if is_directory {
                let id = LittleEndianReader::new(data).u16(offset).map_err(|_| parse_error("FNT directory entry is missing its directory ID"))?;
                offset += 2;
                if id <= ROOT_DIRECTORY_ID {
                    return Err(parse_error("FNT sub-directory ID is invalid"));
                }
                self.directory(usize::from(id - ROOT_DIRECTORY_ID), &path)?;
            } else {
                if next_file_id > 0xEFFF {
                    return Err(parse_error("FNT file ID is outside the valid file range"));
                }
                self.files.push(NitroFileEntry { id: next_file_id, path });
                next_file_id += 1;
            }
        }

        if let Some(flag) = self.visiting.get_mut(index) {
            *flag = false;
        }
        if let Some(flag) = self.visited.get_mut(index) {
            *flag = true;
        }
        Ok(())
    }
}

pub fn parse_fnt(data: &[u8]) -> Result<Vec<NitroFileEntry>> {
    let directories = parse_fnt_directory_table(data)?;
    let count = directories.len();
    let mut walk = FntWalk { data, directories, visiting: vec![false; count], visited: vec![false; count], files: Vec::new() };
    walk.directory(0, "")?;
    let mut files = walk.files;
    files.sort_by_key(|f| f.id);
    Ok(files)
}

pub fn parse_overlay_table(data: &[u8]) -> Result<Vec<OverlayTableEntry>> {
    if !data.len().is_multiple_of(OVERLAY_TABLE_ENTRY_SIZE) {
        return Err(parse_error("overlay table size must be a multiple of 0x20 bytes"));
    }
    let reader = LittleEndianReader::new(data);
    (0..data.len() / OVERLAY_TABLE_ENTRY_SIZE)
        .map(|i| {
            let o = i * OVERLAY_TABLE_ENTRY_SIZE;
            Ok(OverlayTableEntry {
                overlay_id: reader.u32(o)?,
                ram_address: reader.u32(o + 0x04)?,
                ram_size: reader.u32(o + 0x08)?,
                bss_size: reader.u32(o + 0x0C)?,
                static_initializer_start_address: reader.u32(o + 0x10)?,
                static_initializer_end_address: reader.u32(o + 0x14)?,
                file_id: reader.u32(o + 0x18)?,
                reserved: reader.u32(o + 0x1C)?,
            })
        })
        .collect()
}
