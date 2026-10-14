use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::{NarcArchive, decompress_blz_overlay, parse_narc, unwrap_narc};
use bigstar_nds::{
    DS_CARTRIDGE_HEADER_SIZE, FatEntry, NitroFileEntry, OverlayTableEntry, RomHeader, banner_size, parse_fat, parse_fnt, parse_overlay_table,
    parse_rom_header,
};
use serde::{Deserialize, Serialize};

/// Short on purpose. An editor works within one level's resources at a time, and keeping more decompressed overlays or
/// parsed archives than that spends memory answering questions nobody asked.
const CACHE_CAPACITY: usize = 4;

/// One member of a Nitro archive, as the container itself describes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArchiveMember {
    /// The file id the archive's own name table gives it, which is also its position in the list: a NARC carries the
    /// ROM's File Name Table structure and numbers its members the same way. The table it belongs to is the
    /// archive's, not the ROM's, which is why this is never read without the container.
    pub id: u16,
    /// As the archive's own name table spells it.
    pub path: String,
    pub size: u32,
}

/// Most-recently-used first, so the entry a repeat question wants is at the front and the one to drop is at the back.
struct Cache<K, V>(Vec<(K, V)>);

impl<K: PartialEq, V> Cache<K, V> {
    fn promote(&mut self, key: &K) -> Option<&V> {
        let index = self.0.iter().position(|(candidate, _)| candidate == key)?;
        self.0.get_mut(..=index)?.rotate_right(1);
        self.0.first().map(|(_, value)| value)
    }

    fn remember(&mut self, key: K, value: V) -> Option<&V> {
        self.0.insert(0, (key, value));
        self.0.truncate(CACHE_CAPACITY);
        self.0.first().map(|(_, value)| value)
    }
}

/// A built `.nds`, read where it lies.
///
/// Nothing is unpacked. A NitroFS file is a window into the ROM read on demand, an overlay is decompressed when it is
/// asked for, and an archive is parsed only when somebody looks inside it. The two expensive answers -- a decompressed
/// overlay and a parsed archive -- are kept in a small cache, because opening a level asks for the same overlay every
/// single time.
///
/// Reads are not `&self`: the stream position and the caches both move. One reader belongs to one thread.
/// Where a reader's bytes come from: a `.nds` read in place, or a whole ROM somebody handed over -- an upload, or a ROM
/// being edited directly, which never touches the disk until it is saved.
enum RomSource {
    File(File),
    Memory(Arc<[u8]>),
}

pub struct RomReader {
    /// The file, or the name a ROM in memory was given; only ever shown.
    path: PathBuf,
    source: RomSource,
    size: u64,
    header: RomHeader,
    fat: Vec<FatEntry>,
    files: Vec<NitroFileEntry>,
    overlays: Vec<OverlayTableEntry>,
    overlay_cache: Cache<u32, Vec<u8>>,
    archive_cache: Cache<String, NarcArchive>,
}

fn not_found(rom_path: &str) -> Error {
    Error::new(ErrorCode::NotFound, format!("the ROM does not hold {rom_path}"))
}

impl RomReader {
    pub fn open(rom: impl Into<PathBuf>) -> Result<Self> {
        let path: PathBuf = rom.into();
        let size =
            std::fs::metadata(&path).map_err(|_| Error::new(ErrorCode::NotFound, format!("could not measure the ROM {}", path.display())))?.len();
        let stream = File::open(&path).map_err(|_| Error::new(ErrorCode::IoError, format!("could not open the ROM {}", path.display())))?;
        Self::from_source(path, RomSource::File(stream), size)
    }

    /// A ROM held whole in memory. `name` is only for messages.
    pub fn from_bytes(name: impl Into<PathBuf>, bytes: Arc<[u8]>) -> Result<Self> {
        let size = bytes.len() as u64;
        Self::from_source(name.into(), RomSource::Memory(bytes), size)
    }

    fn from_source(path: PathBuf, source: RomSource, size: u64) -> Result<Self> {
        let mut reader = Self {
            path,
            source,
            size,
            header: RomHeader::default(),
            fat: Vec::new(),
            files: Vec::new(),
            overlays: Vec::new(),
            overlay_cache: Cache(Vec::new()),
            archive_cache: Cache(Vec::new()),
        };
        let header_size = u32::try_from(DS_CARTRIDGE_HEADER_SIZE).map_err(|_| Error::new(ErrorCode::OutOfBounds, "header size"))?;
        reader.header = parse_rom_header(&reader.read_region(0, header_size, "ROM header")?)?;

        // The three tables are small and every question needs them, so they are the one thing read up front rather
        // than on demand.
        let header = reader.header.clone();
        reader.fat = parse_fat(&reader.read_region(header.fat.rom_offset, header.fat.size, "FAT")?)?;
        reader.files = parse_fnt(&reader.read_region(header.fnt.rom_offset, header.fnt.size, "FNT")?)?;
        let table = header.arm9_overlay_table;
        reader.overlays = parse_overlay_table(&reader.read_region(table.rom_offset, table.size, "ARM9 overlay table")?)?;
        Ok(reader)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn header(&self) -> &RomHeader {
        &self.header
    }

    /// Every NitroFS file the ROM holds, sorted by id, as `parse_fnt` reports it.
    pub fn files(&self) -> &[NitroFileEntry] {
        &self.files
    }

    fn read_region(&mut self, offset: u32, size: u32, what: &str) -> Result<Vec<u8>> {
        let (offset, length) = (u64::from(offset), u64::from(size));
        if offset > self.size || length > self.size - offset {
            return Err(Error::new(ErrorCode::OutOfBounds, format!("{what} extends past the end of {}", self.path.display())));
        }
        let mut data = vec![0u8; usize::try_from(length).map_err(|_| Error::new(ErrorCode::OutOfBounds, what))?];
        if data.is_empty() {
            return Ok(data);
        }
        let failed = || Error::new(ErrorCode::IoError, format!("could not read {what} from {}", self.path.display()));
        match &mut self.source {
            RomSource::File(stream) => {
                stream.seek(SeekFrom::Start(offset)).map_err(|_| failed())?;
                stream.read_exact(&mut data).map_err(|_| failed())?;
            }
            RomSource::Memory(bytes) => {
                let start = usize::try_from(offset).map_err(|_| failed())?;
                let end = start + data.len();
                data.copy_from_slice(bytes.get(start..end).ok_or_else(failed)?);
            }
        }
        Ok(data)
    }

    /// The id a ROM path has, or nothing when the ROM does not hold that path.
    pub fn file_id(&self, rom_path: &str) -> Option<u16> {
        self.files.iter().find(|file| file.path == rom_path).map(|file| file.id)
    }

    /// How big a file is, straight out of the FAT, without reading it.
    pub fn file_size(&self, file_id: u16) -> Option<u32> {
        let entry = self.fat.get(usize::from(file_id))?;
        Some(if entry.used() { entry.size() } else { 0 })
    }

    /// Whether the ROM holds an archive at this path -- decided by what the bytes decode to, never by the extension,
    /// so a compressed container named `.carc` and a plain one named `.narc` both answer true and `dwc/utility.bin`,
    /// which merely starts with 0x10, does not.
    pub fn is_archive(&mut self, rom_path: &str) -> bool {
        self.archive(rom_path).is_ok()
    }

    pub fn read_id(&mut self, file_id: u16) -> Result<Vec<u8>> {
        let entry =
            *self.fat.get(usize::from(file_id)).ok_or_else(|| Error::new(ErrorCode::NotFound, format!("no FAT entry for file id {file_id}")))?;
        if !entry.used() {
            return Ok(Vec::new());
        }
        self.read_region(entry.start_address, entry.size(), "NitroFS file")
    }

    pub fn read(&mut self, rom_path: &str) -> Result<Vec<u8>> {
        let id = self.file_id(rom_path).ok_or_else(|| not_found(rom_path))?;
        self.read_id(id)
    }

    /// Just the head of a file. Sniffing a format needs a few dozen bytes, and a dock that classified two thousand
    /// entries by reading them whole would spend a game's worth of I/O to draw its icons.
    pub fn read_prefix(&mut self, rom_path: &str, maximum: usize) -> Result<Vec<u8>> {
        let id = self.file_id(rom_path).ok_or_else(|| not_found(rom_path))?;
        let entry = *self.fat.get(usize::from(id)).ok_or_else(|| Error::new(ErrorCode::NotFound, format!("no FAT entry for {rom_path}")))?;
        if !entry.used() {
            return Ok(Vec::new());
        }
        let length = u32::try_from(maximum).map_or(entry.size(), |maximum| entry.size().min(maximum));
        self.read_region(entry.start_address, length, "NitroFS file")
    }

    /// Decompressed, whatever the ROM stores. An overlay the ROM keeps raw comes back unchanged, because BLZ marks its
    /// own absence.
    pub fn read_overlay(&mut self, overlay_id: u32) -> Result<Vec<u8>> {
        if let Some(cached) = self.overlay_cache.promote(&overlay_id) {
            return Ok(cached.clone());
        }
        let entry = self
            .overlays
            .iter()
            .find(|entry| entry.overlay_id == overlay_id)
            .copied()
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("the ROM has no overlay {overlay_id}")))?;
        let file_id = u16::try_from(entry.file_id)
            .ok()
            .filter(|id| *id <= 0xEFFF)
            .ok_or_else(|| Error::new(ErrorCode::ParseError, "overlay file id is outside the NitroFS range"))?;
        let raw = decompress_blz_overlay(&self.read_id(file_id)?)?;
        self.overlay_cache.remember(overlay_id, raw.clone());
        Ok(raw)
    }

    pub fn read_arm9(&mut self) -> Result<Vec<u8>> {
        let arm9 = self.header.arm9;
        self.read_region(arm9.rom_offset, arm9.size, "ARM9 binary")
    }

    pub fn read_banner(&mut self) -> Result<Vec<u8>> {
        let offset = self.header.banner_rom_offset;
        if offset == 0 || u64::from(offset) >= self.size {
            return Err(Error::new(ErrorCode::NotFound, "the ROM declares no banner"));
        }
        // A banner does not declare its own length. Its version implies one, and whatever region comes next bounds
        // it; the implied length wins only when it fits.
        let rom_size = usize::try_from(self.size).map_err(|_| Error::new(ErrorCode::OutOfBounds, "ROM is too large"))?;
        let version = self.read_region(offset, 2, "banner version")?;
        let size = banner_size(&self.header, &self.fat, rom_size, &version);
        self.read_region(offset, u32::try_from(size).map_err(|_| Error::new(ErrorCode::OutOfBounds, "banner size"))?, "banner")
    }

    fn archive(&mut self, rom_path: &str) -> Result<&NarcArchive> {
        let key = rom_path.to_owned();
        if self.archive_cache.promote(&key).is_none() {
            // A container may have arrived compressed -- Mario Kart DS names those `.carc` -- and is an ordinary NARC
            // underneath. What decides is what the bytes decode to, never the extension.
            let (_, data) =
                unwrap_narc(&self.read(rom_path)?).ok_or_else(|| Error::new(ErrorCode::Unsupported, format!("{rom_path} is not a Nitro archive")))?;
            let parsed = parse_narc(&data)?;
            self.archive_cache.remember(key, parsed);
        }
        self.archive_cache.0.first().map(|(_, archive)| archive).ok_or_else(|| Error::new(ErrorCode::NotFound, "archive cache is empty"))
    }

    /// Members in id order, from the archive's own name table. A member the name table does not name keeps its row
    /// with an empty path rather than being skipped, so a member's position in this list is its id -- which is how the
    /// game loads it and how a build reports it.
    pub fn list_archive(&mut self, rom_path: &str) -> Result<Vec<ArchiveMember>> {
        let narc = self.archive(rom_path)?;
        let names = parse_fnt(&narc.name_table)?;
        let mut members = narc
            .files
            .iter()
            .enumerate()
            .map(|(index, data)| {
                Ok(ArchiveMember {
                    id: u16::try_from(index).map_err(|_| Error::new(ErrorCode::OutOfBounds, "archive has too many members"))?,
                    path: String::new(),
                    size: u32::try_from(data.len()).map_err(|_| Error::new(ErrorCode::OutOfBounds, "archive member is too large"))?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        for name in names {
            if let Some(member) = members.get_mut(usize::from(name.id)) {
                member.path = name.path;
            }
        }
        Ok(members)
    }

    pub fn read_member_id(&mut self, rom_path: &str, member_id: usize) -> Result<Vec<u8>> {
        self.archive(rom_path)?
            .files
            .get(member_id)
            .cloned()
            .ok_or_else(|| Error::new(ErrorCode::NotFound, format!("{rom_path} has no member {member_id}")))
    }

    pub fn read_member(&mut self, rom_path: &str, member: &str) -> Result<Vec<u8>> {
        let archive = self.archive(rom_path)?;
        let index = archive.find(member)?;
        archive.files.get(index).cloned().ok_or_else(|| Error::new(ErrorCode::NotFound, format!("{rom_path} names a member its FAT does not hold")))
    }
}
