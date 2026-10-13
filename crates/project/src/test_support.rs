//! A hand-built `.nds`: small enough to reason about, complete enough to read. Give it ROM paths and bytes and it lays
//! out a header, a name table, a FAT, an overlay table and a banner around them. Shared by every test that needs a ROM
//! to read, because they all have to be told the same story about one.
//!
//! This is test scaffolding, so it asserts on its own inputs instead of returning errors.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use bigstar_formats::{NarcArchive, write_narc};
use bigstar_nds::{DEFAULT_HEADER_REGION_SIZE, OVERLAY_TABLE_ENTRY_SIZE, ROOT_DIRECTORY_ID};

pub struct RomFile {
    pub path: String,
    pub data: Vec<u8>,
}

impl RomFile {
    pub fn new(path: &str, data: &[u8]) -> Self {
        Self { path: path.to_owned(), data: data.to_vec() }
    }
}

pub const ARM9_OFFSET: u32 = 0x4000;
pub const ARM9_SIZE: u32 = 0x20;
pub const ARM7_OFFSET: u32 = 0x4100;
pub const ARM7_SIZE: u32 = 0x20;
pub const BANNER_OFFSET: u32 = 0x4200;
pub const BANNER_SIZE: usize = 0x0A00;
pub const FNT_OFFSET: u32 = 0x4C00;

/// A scratch directory under the system temp dir that removes itself when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!("big-star-{label}-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("create scratch directory");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub fn put16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

pub fn put32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn len32(value: usize) -> u32 {
    u32::try_from(value).expect("fits in u32")
}

fn len16(value: usize) -> u16 {
    u16::try_from(value).expect("fits in u16")
}

/// A directory in the name table being built. Ids follow creation order, and files are numbered directory by directory
/// in that same order, which is what makes a path's id predictable enough to assert on.
#[derive(Default)]
struct Directory {
    name: String,
    parent: usize,
    files: Vec<String>,
    children: Vec<usize>,
    first_file_id: u16,
}

pub struct NameTable {
    pub bytes: Vec<u8>,
    pub ids: BTreeMap<String, u16>,
}

pub fn make_name_table(paths: &[String]) -> NameTable {
    let mut directories = vec![Directory::default()];
    let mut by_path: BTreeMap<String, usize> = BTreeMap::from([(String::new(), 0)]);

    for path in paths {
        let mut current = 0;
        let mut prefix = String::new();
        let parts: Vec<&str> = path.split('/').collect();
        for part in &parts[..parts.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('/');
            }
            prefix.push_str(part);
            if let Some(found) = by_path.get(&prefix) {
                current = *found;
            } else {
                directories.push(Directory { name: (*part).to_owned(), parent: current, ..Directory::default() });
                let created = directories.len() - 1;
                directories[current].children.push(created);
                current = created;
                by_path.insert(prefix.clone(), current);
            }
        }
        directories[current].files.push(path.clone());
    }

    let mut ids = BTreeMap::new();
    let mut next_id: u16 = 0;
    let mut sub_tables: Vec<Vec<u8>> = vec![Vec::new(); directories.len()];
    for i in 0..directories.len() {
        directories[i].first_file_id = next_id;
        for path in &directories[i].files {
            let name = path.rsplit('/').next().unwrap_or(path);
            sub_tables[i].push(u8::try_from(name.len()).expect("short name"));
            sub_tables[i].extend_from_slice(name.as_bytes());
            ids.insert(path.clone(), next_id);
            next_id += 1;
        }
        for &child in &directories[i].children {
            let name = &directories[child].name;
            sub_tables[i].push(u8::try_from(name.len()).expect("short name") | 0x80);
            sub_tables[i].extend_from_slice(name.as_bytes());
            sub_tables[i].extend_from_slice(&(ROOT_DIRECTORY_ID + len16(child)).to_le_bytes());
        }
        sub_tables[i].push(0);
    }

    let mut bytes = vec![0u8; directories.len() * 8];
    let mut offset = bytes.len();
    for (i, directory) in directories.iter().enumerate() {
        put32(&mut bytes, i * 8, len32(offset));
        put16(&mut bytes, i * 8 + 4, directory.first_file_id);
        put16(&mut bytes, i * 8 + 6, if i == 0 { len16(directories.len()) } else { ROOT_DIRECTORY_ID + len16(directory.parent) });
        offset += sub_tables[i].len();
    }
    for sub_table in &sub_tables {
        bytes.extend_from_slice(sub_table);
    }
    NameTable { bytes, ids }
}

/// Members named at the archive root, then members inside a directory: a NARC name table is an FNT, so a member
/// address nests exactly like a ROM path.
pub fn make_narc(members: &[(&str, &[u8])]) -> Vec<u8> {
    let paths: Vec<String> = members.iter().map(|(path, _)| (*path).to_owned()).collect();
    let table = make_name_table(&paths);
    let mut archive = NarcArchive { files: vec![Vec::new(); members.len()], ..NarcArchive::default() };
    for (path, data) in members {
        archive.files[usize::from(table.ids[*path])] = data.to_vec();
    }
    archive.name_table = table.bytes;
    write_narc(&archive).expect("write synthetic narc")
}

fn align_to(value: u32, alignment: u32) -> u32 {
    value.div_ceil(alignment) * alignment
}

/// The ROM. `overlay` becomes ARM9 overlay 0, whose file id follows the NitroFS files, which is where a real ROM puts
/// its overlays too.
pub fn make_rom(files: &[RomFile], overlay: &[u8]) -> Vec<u8> {
    let paths: Vec<String> = files.iter().map(|file| file.path.clone()).collect();
    let table = make_name_table(&paths);

    let fat_offset = align_to(FNT_OFFSET + len32(table.bytes.len()), 4);
    let entries = files.len() + usize::from(!overlay.is_empty());
    let ovt_offset = fat_offset + len32(entries) * 8;
    let entry_size = len32(OVERLAY_TABLE_ENTRY_SIZE);
    let data_offset = align_to(ovt_offset + entry_size, 0x200);

    let mut rom = vec![0u8; data_offset as usize];
    let mut spans = vec![(0u32, 0u32); entries];
    let mut place = |rom: &mut Vec<u8>, id: usize, data: &[u8]| {
        let start = len32(rom.len());
        rom.extend_from_slice(data);
        rom.resize((rom.len() + 3) & !3, 0xFF);
        spans[id] = (start, start + len32(data.len()));
    };
    for file in files {
        place(&mut rom, usize::from(table.ids[&file.path]), &file.data);
    }
    if !overlay.is_empty() {
        place(&mut rom, files.len(), overlay);
    }

    rom[..11].copy_from_slice(b"READER TEST");
    rom[0x00C..0x010].copy_from_slice(b"A2DE");
    put32(&mut rom, 0x020, ARM9_OFFSET);
    put32(&mut rom, 0x02C, ARM9_SIZE);
    put32(&mut rom, 0x030, ARM7_OFFSET);
    put32(&mut rom, 0x03C, ARM7_SIZE);
    put32(&mut rom, 0x040, FNT_OFFSET);
    put32(&mut rom, 0x044, len32(table.bytes.len()));
    put32(&mut rom, 0x048, fat_offset);
    put32(&mut rom, 0x04C, len32(entries) * 8);
    put32(&mut rom, 0x050, ovt_offset);
    put32(&mut rom, 0x054, if overlay.is_empty() { 0 } else { entry_size });
    put32(&mut rom, 0x068, BANNER_OFFSET);
    let total = len32(rom.len());
    put32(&mut rom, 0x080, total);
    put32(&mut rom, 0x084, len32(DEFAULT_HEADER_REGION_SIZE));

    for i in 0..ARM9_SIZE {
        rom[(ARM9_OFFSET + i) as usize] = 0x90u8.wrapping_add(i as u8);
    }
    for i in 0..ARM7_SIZE {
        rom[(ARM7_OFFSET + i) as usize] = 0x70u8.wrapping_add(i as u8);
    }
    put16(&mut rom, BANNER_OFFSET as usize, 0x0001);
    rom[BANNER_OFFSET as usize + 2] = 0xAB;
    rom[FNT_OFFSET as usize..FNT_OFFSET as usize + table.bytes.len()].copy_from_slice(&table.bytes);
    for (i, (start, end)) in spans.iter().enumerate() {
        put32(&mut rom, fat_offset as usize + i * 8, *start);
        put32(&mut rom, fat_offset as usize + i * 8 + 4, *end);
    }
    if !overlay.is_empty() {
        put32(&mut rom, ovt_offset as usize, 0);
        put32(&mut rom, ovt_offset as usize + 0x18, len32(files.len()));
    }
    rom
}

pub fn write_bytes(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent directory");
    }
    std::fs::write(path, bytes).expect("write file");
}
