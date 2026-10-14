//! Replacing files in a whole `.nds` held in memory. Port of the in-place half of NCPatcher's `NdsRom::commit`
//! (`source/rom/nds_rom.cpp`), so a ROM Big Star writes is laid out the way NCPatcher would have laid it out.
//!
//! Only what editing a ROM directly needs: replacing NitroFS files and the banner. The ARM binaries, the overlay
//! tables, the FNT and the FAT's length never change here, so nothing ever has to move and NCPatcher's full
//! `rebuildLayout` has no counterpart. A file goes back into its own extent when it still fits before whatever comes
//! next, and is appended after the used ROM when it does not; the extent it left becomes dead space, as it does in
//! NCPatcher, whose `rom pack` reclaims it.

use std::collections::BTreeMap;

use bigstar_core::{Error, ErrorCode, LittleEndianWriter, Result};

use crate::rom::header_offset as at;
use crate::{FatEntry, RomHeader, banner_size, crc16, parse_fat, parse_rom_header};

/// The header checksum covers everything before itself.
const HEADER_CHECKSUM_COVERAGE: usize = at::HEADER_CHECKSUM;
/// GBATEK: chip size is 128 KiB shifted left by the stored value; 15 is past anything a u32 offset reaches.
const MAX_DEVICE_CAPACITY: u8 = 15;

fn invalid(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::InvalidArgument, message)
}

fn out_of_bounds(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::OutOfBounds, message)
}

fn align4(value: u32) -> Result<u32> {
    value.checked_add(3).map(|v| v & !3).ok_or_else(|| out_of_bounds("the ROM would outgrow a 32-bit offset"))
}

fn device_capacity_bytes(shift: u8) -> u32 {
    if shift >= MAX_DEVICE_CAPACITY { u32::MAX } else { 0x20000u32 << shift }
}

pub struct NdsRomWriter {
    bytes: Vec<u8>,
    header: RomHeader,
    fat: Vec<FatEntry>,
    pending_files: BTreeMap<u16, Vec<u8>>,
    pending_banner: Option<Vec<u8>>,
}

impl NdsRomWriter {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let header = parse_rom_header(&bytes)?;
        let fat_region = header.fat;
        let start = usize::try_from(fat_region.rom_offset).map_err(|_| out_of_bounds("FAT offset"))?;
        let end = start.checked_add(usize::try_from(fat_region.size).map_err(|_| out_of_bounds("FAT size"))?);
        let fat_bytes = end.and_then(|end| bytes.get(start..end)).ok_or_else(|| out_of_bounds("the FAT is past the end of the ROM"))?;
        let fat = parse_fat(fat_bytes)?;
        Ok(Self { bytes, header, fat, pending_files: BTreeMap::new(), pending_banner: None })
    }

    pub fn header(&self) -> &RomHeader {
        &self.header
    }

    pub fn file_count(&self) -> usize {
        self.fat.len()
    }

    /// The banner region as the ROM holds it now, its length decided as `RomReader::read_banner` decides it.
    pub fn banner_size(&self) -> usize {
        let offset = self.header.banner_rom_offset as usize;
        let version = self.bytes.get(offset..offset.saturating_add(2)).unwrap_or_default();
        banner_size(&self.header, &self.fat, self.bytes.len(), version)
    }

    pub fn set_file(&mut self, file_id: u16, data: Vec<u8>) -> Result<()> {
        if usize::from(file_id) >= self.fat.len() {
            return Err(invalid(format!("cannot write file id {file_id}: this ROM has {} file(s)", self.fat.len())));
        }
        if u32::try_from(data.len()).is_err() {
            return Err(out_of_bounds("a file cannot be larger than 4 GiB"));
        }
        self.pending_files.insert(file_id, data);
        Ok(())
    }

    /// A banner's length is fixed by the version it declares, and growing the region would mean laying the whole
    /// container out again, so a replacement has to be the same size.
    pub fn set_banner(&mut self, data: Vec<u8>) -> Result<()> {
        let size = self.banner_size();
        if size == 0 {
            return Err(invalid("this ROM has no icon/title banner to replace"));
        }
        if data.len() != size {
            return Err(invalid(format!("cannot replace the banner with {} bytes: the ROM's is {size}", data.len())));
        }
        self.pending_banner = Some(data);
        Ok(())
    }

    pub fn dirty(&self) -> bool {
        !self.pending_files.is_empty() || self.pending_banner.is_some()
    }

    /// The ROM with every staged write applied. With nothing staged, the bytes it was given, untouched.
    pub fn commit(mut self) -> Result<Vec<u8>> {
        if !self.dirty() {
            return Ok(self.bytes);
        }
        let starts = self.occupied_starts();

        if let Some(banner) = self.pending_banner.take() {
            self.write_region(self.header.banner_rom_offset, &banner)?;
        }

        let mut used = self.header.total_used_rom_size;
        let pending = std::mem::take(&mut self.pending_files);
        for (file_id, data) in &pending {
            let size = u32::try_from(data.len()).map_err(|_| out_of_bounds("a file cannot be larger than 4 GiB"))?;
            let entry = self.fat.get(usize::from(*file_id)).copied().ok_or_else(|| invalid(format!("no file with id {file_id}")))?;
            let placed = if entry.size() != 0 && size <= self.room_after(entry.start_address, &starts) {
                self.write_region(entry.start_address, data)?;
                FatEntry { start_address: entry.start_address, end_address: entry.start_address + size }
            } else {
                let offset = align4(used)?;
                let end = offset.checked_add(size).ok_or_else(|| out_of_bounds("the ROM would outgrow a 32-bit offset"))?;
                self.write_region(offset, data)?;
                used = end;
                FatEntry { start_address: offset, end_address: end }
            };
            if let Some(slot) = self.fat.get_mut(usize::from(*file_id)) {
                *slot = placed;
            }
        }

        if !pending.is_empty() {
            let mut fat_bytes = vec![0u8; self.fat.len() * 8];
            let mut writer = LittleEndianWriter::new(&mut fat_bytes);
            for (i, entry) in self.fat.iter().enumerate() {
                writer.u32(i * 8, entry.start_address)?;
                writer.u32(i * 8 + 4, entry.end_address)?;
            }
            self.write_region(self.header.fat.rom_offset, &fat_bytes)?;
        }

        let total = self.header.total_used_rom_size.max(used);
        let mut shift = self.header.device_capacity;
        while shift < MAX_DEVICE_CAPACITY && device_capacity_bytes(shift) < total {
            shift += 1;
        }
        LittleEndianWriter::new(&mut self.bytes).u32(at::TOTAL_USED_ROM_SIZE, total)?;
        self.write_region(at::DEVICE_CAPACITY as u32, &[shift])?;
        let covered = self.bytes.get(..HEADER_CHECKSUM_COVERAGE).ok_or_else(|| out_of_bounds("the ROM header is truncated"))?;
        let checksum = crc16(covered, 0xFFFF);
        LittleEndianWriter::new(&mut self.bytes).u16(at::HEADER_CHECKSUM, checksum)?;
        Ok(self.bytes)
    }

    /// Every offset the ROM has something at, sorted: how much room a region has is the distance to the next one.
    fn occupied_starts(&self) -> Vec<u32> {
        let header = &self.header;
        let mut starts = Vec::with_capacity(self.fat.len() + 8);
        for arm in [header.arm9, header.arm7] {
            if arm.size != 0 {
                starts.push(arm.rom_offset);
            }
        }
        for region in [header.arm9_overlay_table, header.arm7_overlay_table, header.fnt, header.fat] {
            if region.size != 0 {
                starts.push(region.rom_offset);
            }
        }
        if header.banner_rom_offset != 0 {
            starts.push(header.banner_rom_offset);
        }
        starts.extend(self.fat.iter().filter(|entry| entry.size() != 0).map(|entry| entry.start_address));
        starts.sort_unstable();
        starts.dedup();
        starts
    }

    fn room_after(&self, start: u32, starts: &[u32]) -> u32 {
        let next = starts.partition_point(|&s| s <= start);
        let limit = starts.get(next).copied().unwrap_or(self.header.total_used_rom_size);
        limit.saturating_sub(start)
    }

    fn write_region(&mut self, offset: u32, data: &[u8]) -> Result<()> {
        let start = usize::try_from(offset).map_err(|_| out_of_bounds("region offset"))?;
        let end = start.checked_add(data.len()).ok_or_else(|| out_of_bounds("region end"))?;
        if end > self.bytes.len() {
            self.bytes.resize(end, 0xFF);
        }
        self.bytes.get_mut(start..end).ok_or_else(|| out_of_bounds("region end"))?.copy_from_slice(data);
        Ok(())
    }
}
