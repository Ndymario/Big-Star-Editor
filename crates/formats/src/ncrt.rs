//! NCPatcher runtime modules (`.ncrt`): code linked into a file of its own, which the game loads, relocates and patches
//! itself with while it runs. The image is laid out by NCPatcher's linker script for runtime modules, so where each part
//! starts and ends follows from the header alone:
//!
//! ```text
//!   header (60 bytes)
//!   ncp_over payloads, .text, .rodata, .data
//!   .init_array, .fini_array
//!   ncprt_exports object
//!   hook bridges (32 bytes each), patch table (16 bytes per patch)
//!   .bss, room for the destructors of C++ static objects
//!   relocation table (u32 offsets), after `size` bytes
//! ```
//!
//! There are no symbols in the file, so the listing names what the header and the patch table point at, and what the
//! code it can follow from there branches to.

use std::collections::{BTreeMap, BTreeSet};

use bigstar_core::{Error, ErrorCode, LittleEndianReader, Result};
use unarm::{AddrLdrStr, BlxTarget, BranchTarget, FormatIns, Ins, LdrStrOffset, Options, Reg};

pub const NCRT_MAGIC: u32 = 0x5452_434E;
pub const NCRT_VERSION: u16 = 1;
pub const NCRT_HEADER_SIZE: u32 = 60;
const PATCH_ENTRY_SIZE: u32 = 16;

/// How a patch is written: copied, or encoded as a branch to its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NcrtPatchKind {
    Copy,
    ArmB,
    ArmBl,
    ArmBlx,
    ArmJump,
    ThumbBl,
    ThumbBlx,
    ThumbJump,
    ThumbJumpX,
    Unknown(u8),
}

impl NcrtPatchKind {
    pub fn from_byte(kind: u8) -> Self {
        match kind {
            0 => Self::Copy,
            1 => Self::ArmB,
            2 => Self::ArmBl,
            3 => Self::ArmBlx,
            4 => Self::ArmJump,
            5 => Self::ThumbBl,
            6 => Self::ThumbBlx,
            7 => Self::ThumbJump,
            8 => Self::ThumbJumpX,
            other => Self::Unknown(other),
        }
    }

    pub fn name(self) -> String {
        match self {
            Self::Copy => "copy".to_owned(),
            Self::ArmB => "ARM B".to_owned(),
            Self::ArmBl => "ARM BL".to_owned(),
            Self::ArmBlx => "ARM BLX".to_owned(),
            Self::ArmJump => "ARM jump".to_owned(),
            Self::ThumbBl => "THUMB BL".to_owned(),
            Self::ThumbBlx => "THUMB BLX".to_owned(),
            Self::ThumbJump => "THUMB jump".to_owned(),
            Self::ThumbJumpX => "THUMB jump to ARM".to_owned(),
            Self::Unknown(kind) => format!("unknown ({kind})"),
        }
    }

    /// The instruction set the destination of a branch is in; `None` for a copy.
    pub fn destination_mode(self) -> Option<CodeMode> {
        match self {
            Self::ArmB | Self::ArmBl | Self::ArmJump | Self::ThumbBlx | Self::ThumbJumpX => Some(CodeMode::Arm),
            Self::ArmBlx | Self::ThumbBl | Self::ThumbJump => Some(CodeMode::Thumb),
            Self::Copy | Self::Unknown(_) => None,
        }
    }
}

/// One entry of the module's patch table: what it writes into the game when the module is attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NcrtPatch {
    /// The game address the patch is written at.
    pub address: u32,
    pub size: u16,
    pub kind: NcrtPatchKind,
    /// A copy's payload, or a branch's destination, as an address for the module's `base`.
    pub value: u32,
    /// Where the bytes the patch overwrites are kept while it is applied.
    pub save: u32,
}

/// A part of the image, by offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NcrtRegion {
    pub name: &'static str,
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NcrtModule {
    pub version: u16,
    pub flags: u8,
    pub align_log2: u8,
    /// The address every pointer in the image is relocated for.
    pub base: u32,
    /// Code, data and zeroed bss, without the relocation table.
    pub image: Vec<u8>,
    /// Offset of the exports object, if the module has one.
    pub exports: Option<u32>,
    /// Offset just past the exports object.
    pub exports_end: u32,
    pub patch_table: u32,
    pub patches: Vec<NcrtPatch>,
    /// Offsets of the words that hold addresses in the module.
    pub relocs: BTreeSet<u32>,
    pub init: u32,
    /// The constructors, as addresses for `base` (bit 0 set for THUMB).
    pub constructors: Vec<u32>,
    pub fini: u32,
    pub destructors: Vec<u32>,
    pub atexit: u32,
    pub atexit_capacity: u16,
}

pub fn is_ncrt(data: &[u8]) -> bool {
    data.starts_with(b"NCRT")
}

fn parse_error(message: impl Into<String>) -> Error {
    Error::new(ErrorCode::ParseError, message)
}

fn align4(value: u32) -> u32 {
    value.saturating_add(3) & !3
}

pub fn parse_ncrt(data: &[u8]) -> Result<NcrtModule> {
    let r = LittleEndianReader::new(data);
    if r.u32(0)? != NCRT_MAGIC {
        return Err(parse_error("not an NCPatcher runtime module (no NCRT magic)"));
    }
    let version = r.u16(4)?;
    if version != NCRT_VERSION {
        return Err(parse_error(format!("runtime module version {version} is not one this editor reads (it reads {NCRT_VERSION})")));
    }
    let base = r.u32(8)?;
    let size = r.u32(12)?;
    let image = data.get(..size as usize).ok_or_else(|| parse_error(format!("the image is {size} bytes, but the file is {}", data.len())))?;
    if size < NCRT_HEADER_SIZE {
        return Err(parse_error("the image is smaller than its header"));
    }
    // Every pointer in the header is an address for `base`; the image is read by offset.
    let offset = |field: usize, what: &str| -> Result<u32> {
        let address = r.u32(field)?;
        address.checked_sub(base).filter(|o| *o <= size).ok_or_else(|| parse_error(format!("the {what} at 0x{address:08X} is outside the image")))
    };
    let words = |start: u32, count: u32, what: &str| -> Result<Vec<u32>> {
        let image = LittleEndianReader::new(image);
        (0..count).map(|i| image.u32((start + 4 * i) as usize).map_err(|_| parse_error(format!("the {what} run past the image")))).collect()
    };

    let exports_address = r.u32(16)?;
    let exports = if exports_address == 0 { None } else { Some(offset(16, "exports object")?) };
    let patch_table = offset(20, "patch table")?;
    let patch_count = u32::from(r.u16(24)?);
    let relocs_at = r.u32(28)?.checked_sub(base).ok_or_else(|| parse_error("the relocation table is before the image"))?;
    let reloc_count = r.u32(32)?;
    let init = offset(36, "constructor table")?;
    let init_count = r.u32(40)?;
    let fini = offset(44, "destructor table")?;
    let fini_count = r.u32(48)?;
    let atexit = offset(52, "atexit room")?;
    let atexit_capacity = r.u16(56)?;

    let table = LittleEndianReader::new(image);
    let patches = (0..patch_count)
        .map(|i| {
            let at = (patch_table + PATCH_ENTRY_SIZE * i) as usize;
            Ok(NcrtPatch {
                address: table.u32(at)?,
                size: table.u16(at + 4)?,
                kind: NcrtPatchKind::from_byte(table.u8(at + 6)?),
                value: table.u32(at + 8)?,
                save: table.u32(at + 12)?,
            })
        })
        .collect::<Result<Vec<_>>>()
        .map_err(|_| parse_error("the patch table runs past the image"))?;

    let file = LittleEndianReader::new(data);
    let relocs = (0..reloc_count)
        .map(|i| file.u32((relocs_at + 4 * i) as usize))
        .collect::<Result<BTreeSet<_>>>()
        .map_err(|_| parse_error("the relocation table runs past the file"))?;

    // The hook bridges follow the exports object, and a hook's patch branches to its bridge, so the first bridge a
    // patch points at is where the object ends. With no hooks it runs up to the patch table.
    let exports_end = match exports {
        Some(start) => patches
            .iter()
            .filter(|p| p.kind != NcrtPatchKind::Copy)
            .filter_map(|p| (p.value & !1).checked_sub(base))
            .filter(|o| (start..patch_table).contains(o))
            .fold(patch_table, u32::min)
            .max(start),
        None => align4(fini + 4 * fini_count),
    };

    Ok(NcrtModule {
        version,
        flags: r.u8(6)?,
        align_log2: r.u8(7)?,
        base,
        image: image.to_vec(),
        exports,
        exports_end,
        patch_table,
        patches,
        relocs,
        init,
        constructors: words(init, init_count, "constructors")?,
        fini,
        destructors: words(fini, fini_count, "destructors")?,
        atexit,
        atexit_capacity,
    })
}

impl NcrtModule {
    pub fn size(&self) -> u32 {
        self.image.len() as u32
    }

    pub fn word(&self, offset: u32) -> Option<u32> {
        LittleEndianReader::new(&self.image).u32(offset as usize).ok()
    }

    /// The offset in the image of an address for `base`, if it is in the image.
    pub fn offset_of(&self, address: u32) -> Option<u32> {
        address.checked_sub(self.base).filter(|o| *o < self.size())
    }

    /// Where the code and data the module's sources compiled to end, and its constructor table begins.
    pub fn code_end(&self) -> u32 {
        self.init
    }

    /// Where the hook bridges start.
    pub fn bridges_start(&self) -> u32 {
        align4(self.exports_end).min(self.patch_table)
    }

    pub fn patch_table_end(&self) -> u32 {
        self.patch_table + PATCH_ENTRY_SIZE * self.patches.len() as u32
    }

    /// Where the room for the bytes the patches overwrite ends, which is where the zeroed data starts.
    pub fn saves_end(&self) -> u32 {
        self.patches
            .iter()
            .filter_map(|p| self.offset_of(p.save))
            .zip(self.patches.iter().map(|p| u32::from(p.size)))
            .map(|(save, size)| save + size)
            .fold(self.patch_table_end(), u32::max)
    }

    pub fn regions(&self) -> Vec<NcrtRegion> {
        let fini_end = self.fini + 4 * self.destructors.len() as u32;
        let mut regions = vec![
            NcrtRegion { name: "Header", start: 0, end: NCRT_HEADER_SIZE },
            NcrtRegion { name: "Code and data", start: NCRT_HEADER_SIZE, end: self.code_end() },
            NcrtRegion { name: "Constructors", start: self.init, end: self.init + 4 * self.constructors.len() as u32 },
            NcrtRegion { name: "Destructors", start: self.fini, end: fini_end },
        ];
        if let Some(exports) = self.exports {
            regions.push(NcrtRegion { name: "Exports", start: exports, end: self.exports_end });
        }
        regions.extend([
            NcrtRegion { name: "Hook bridges", start: self.bridges_start(), end: self.patch_table },
            NcrtRegion { name: "Patch table", start: self.patch_table, end: self.patch_table_end() },
            NcrtRegion { name: "Saved game bytes", start: self.patch_table_end(), end: self.saves_end() },
            NcrtRegion { name: "Zeroed data (bss)", start: align4(self.saves_end()), end: self.atexit },
            NcrtRegion { name: "Static destructor room", start: self.atexit, end: self.atexit + 8 * u32::from(self.atexit_capacity) },
        ]);
        regions.retain(|r| r.end > r.start);
        regions
    }

    /// The words of the exports object. Its type is the module's own, so each word is all there is to say: an address
    /// in the module when it is relocated, otherwise a value.
    pub fn export_words(&self) -> Vec<ExportWord> {
        let Some(start) = self.exports else { return Vec::new() };
        (start..self.exports_end)
            .step_by(4)
            .filter_map(|offset| {
                let value = self.word(offset)?;
                let target = self.relocs.contains(&offset).then(|| self.offset_of(value & !1)).flatten();
                Some(ExportWord { offset, value, target })
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportWord {
    pub offset: u32,
    pub value: u32,
    /// Where in the module it points, when it is an address in the module.
    pub target: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodeMode {
    Arm,
    Thumb,
}

/// How the listing decides between ARM and THUMB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ListingMode {
    /// From what is known to point at the code, and what that code branches to.
    #[default]
    Auto,
    Arm,
    Thumb,
}

/// A line of the listing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsmLine {
    pub offset: u32,
    /// The instruction or word as a number, in hex: a THUMB pair as its two halves.
    pub bytes: String,
    pub text: String,
    pub comment: String,
    /// The names of the place, when something points at it.
    pub labels: Vec<String>,
    /// The offset in the module it branches to or holds the address of.
    pub target: Option<u32>,
}

/// Something known to point at an offset: its name, and the instruction set it expects there, when it says.
struct Entry {
    names: Vec<String>,
    mode: Option<CodeMode>,
}

/// Writes branch targets as the label of the place or its offset in the module, and anything else as unarm does.
struct Formatter<'a> {
    options: &'a Options,
    text: String,
    base: u32,
    size: u32,
    entries: &'a BTreeMap<u32, Entry>,
}

impl std::fmt::Write for Formatter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.text.push_str(s);
        Ok(())
    }
}

impl FormatIns for Formatter<'_> {
    fn options(&self) -> &Options {
        self.options
    }

    fn write_branch_target(&mut self, target: BranchTarget) -> std::fmt::Result {
        let text = describe_address(target.addr, self.base, self.size, self.entries);
        self.text.push_str(&text);
        Ok(())
    }

    fn write_blx_target(&mut self, target: BlxTarget) -> std::fmt::Result {
        match target {
            BlxTarget::Direct(target) => self.write_branch_target(target),
            BlxTarget::Indirect(reg) => self.write_reg(reg),
        }
    }
}

fn describe_address(address: u32, base: u32, size: u32, entries: &BTreeMap<u32, Entry>) -> String {
    match address.checked_sub(base).filter(|o| *o < size) {
        Some(offset) => match entries.get(&offset).and_then(|e| e.names.first()) {
            Some(name) => name.clone(),
            None => format!("+0x{offset:X}"),
        },
        None => format!("0x{address:08X}"),
    }
}

fn options() -> Options {
    Options { version: unarm::Version::V5Te, ual: false, ..Options::default() }
}

/// A range of the image that holds code, and the instruction set it starts in.
struct CodeRange {
    start: u32,
    end: u32,
    mode: CodeMode,
}

/// The module's code as assembly: what its sources compiled to and its hook bridges. Constant pools, and words that
/// hold addresses in the module, are shown as words.
pub fn disassemble_ncrt(module: &NcrtModule, mode: ListingMode) -> Vec<AsmLine> {
    let mut entries = known_entries(module);
    let ranges = [
        CodeRange { start: NCRT_HEADER_SIZE, end: module.code_end(), mode: CodeMode::Arm },
        CodeRange { start: module.bridges_start(), end: module.patch_table, mode: CodeMode::Arm },
    ];
    let mut literals = BTreeSet::new();
    // What the code branches to and loads only shows once it is read in the right instruction set, and that can
    // change what is read before it, so the sweep runs again while it finds something new.
    for _ in 0..4 {
        let before = (entries.len(), literals.len());
        for range in &ranges {
            sweep(module, range, &ranges, mode, &mut entries, &mut literals, None);
        }
        if (entries.len(), literals.len()) == before {
            break;
        }
    }
    let mut lines = Vec::new();
    for range in &ranges {
        sweep(module, range, &ranges, mode, &mut entries, &mut literals, Some(&mut lines));
    }
    lines
}

fn known_entries(module: &NcrtModule) -> BTreeMap<u32, Entry> {
    let mut entries: BTreeMap<u32, Entry> = BTreeMap::new();
    let mut add = |offset: u32, name: String, mode: Option<CodeMode>| {
        let entry = entries.entry(offset).or_insert(Entry { names: Vec::new(), mode: None });
        entry.names.push(name);
        entry.mode = entry.mode.or(mode);
    };
    let thumb_bit = |address: u32| Some(if address & 1 != 0 { CodeMode::Thumb } else { CodeMode::Arm });
    let bridges = module.bridges_start()..module.patch_table;
    for (index, patch) in module.patches.iter().enumerate() {
        let Some(offset) = module.offset_of(patch.value & !1) else { continue };
        let name = match patch.kind {
            NcrtPatchKind::Copy => format!("patch{index}_payload"),
            _ if bridges.contains(&offset) => format!("patch{index}_bridge"),
            _ => format!("patch{index}_{:08X}", patch.address),
        };
        add(offset, name, patch.kind.destination_mode());
    }
    for (index, address) in module.constructors.iter().enumerate() {
        if let Some(offset) = module.offset_of(address & !1) {
            add(offset, format!("constructor{index}"), thumb_bit(*address));
        }
    }
    for (index, address) in module.destructors.iter().enumerate() {
        if let Some(offset) = module.offset_of(address & !1) {
            add(offset, format!("destructor{index}"), thumb_bit(*address));
        }
    }
    for (index, word) in module.export_words().iter().enumerate() {
        // Only an address in the code can be a function; one elsewhere is a pointer to data.
        if let Some(target) = word.target.filter(|t| (NCRT_HEADER_SIZE..module.code_end()).contains(t)) {
            add(target, format!("export{index}"), thumb_bit(word.value));
        }
    }
    entries
}

/// Reads one code range. Collects the places its branches go and its loads read from into `entries` and `literals`,
/// and writes its lines to `lines` when given them.
fn sweep(
    module: &NcrtModule,
    range: &CodeRange,
    code: &[CodeRange],
    listing: ListingMode,
    entries: &mut BTreeMap<u32, Entry>,
    literals: &mut BTreeSet<u32>,
    mut lines: Option<&mut Vec<AsmLine>>,
) {
    let options = options();
    let mut mode = match listing {
        ListingMode::Thumb => CodeMode::Thumb,
        ListingMode::Arm | ListingMode::Auto => range.mode,
    };
    let mut found: Vec<(u32, CodeMode)> = Vec::new();
    let mut offset = range.start;
    while offset < range.end {
        let entry = entries.get(&offset);
        if listing == ListingMode::Auto
            && let Some(entry_mode) = entry.and_then(|e| e.mode)
        {
            mode = entry_mode;
        }
        let labels = entry.map(|e| e.names.clone()).unwrap_or_default();
        let word = module.word(offset).filter(|_| offset + 4 <= range.end);

        // A word another instruction loads, or one that holds an address in the module, is data.
        if offset.is_multiple_of(4)
            && let Some(value) = word
            && (literals.contains(&offset) || module.relocs.contains(&offset))
        {
            if let Some(lines) = lines.as_deref_mut() {
                let comment =
                    if module.relocs.contains(&offset) { describe_address(value & !1, module.base, module.size(), entries) } else { String::new() };
                let target = if module.relocs.contains(&offset) { module.offset_of(value & !1) } else { None };
                lines.push(AsmLine { offset, bytes: format!("{value:08X}"), text: format!(".word 0x{value:08X}"), comment, labels, target });
            }
            offset += 4;
            continue;
        }

        let pc = module.base.wrapping_add(offset);
        let (ins, size) = match mode {
            CodeMode::Arm => match word {
                Some(code) => (unarm::parse_arm(code, pc, &options), 4),
                None => (Ins::Illegal, 2),
            },
            CodeMode::Thumb => {
                let low = LittleEndianReader::new(&module.image).u16(offset as usize).unwrap_or(0);
                let high = if offset + 4 <= range.end { LittleEndianReader::new(&module.image).u16(offset as usize + 2).unwrap_or(0) } else { 0 };
                let (ins, size) = unarm::parse_thumb(u32::from(low) | (u32::from(high) << 16), pc, &options);
                (ins, size.max(2))
            }
        };
        let size = size.min(range.end - offset);

        let mut comment = String::new();
        let mut target = None;
        let target_mode = |exchange: bool| match (mode, exchange) {
            (CodeMode::Arm, false) | (CodeMode::Thumb, true) => CodeMode::Arm,
            _ => CodeMode::Thumb,
        };
        match &ins {
            Ins::B { target: to, .. } | Ins::Bl { target: to, .. } => {
                found.push((to.addr, target_mode(false)));
                target = module.offset_of(to.addr);
            }
            Ins::Blx { target: BlxTarget::Direct(to), .. } => {
                found.push((to.addr, target_mode(true)));
                target = module.offset_of(to.addr);
            }
            Ins::Ldr { rd, addr: AddrLdrStr::Pre { rn: Reg::Pc, offset: LdrStrOffset::Imm(imm), writeback: false }, .. } => {
                let pc_value = match mode {
                    CodeMode::Arm => pc.wrapping_add(8),
                    CodeMode::Thumb => pc.wrapping_add(4) & !3,
                };
                let address = pc_value.wrapping_add_signed(*imm);
                if let Some(literal) = module.offset_of(address).filter(|o| o.is_multiple_of(4) && (range.start..range.end).contains(o)) {
                    literals.insert(literal);
                    if let Some(value) = module.word(literal) {
                        comment = if module.relocs.contains(&literal) {
                            target = module.offset_of(value & !1);
                            let name = describe_address(value & !1, module.base, module.size(), entries);
                            // A load into the PC, or a pointer with the THUMB bit, is a jump to code.
                            if *rd == Reg::Pc || value & 1 != 0 {
                                found.push((value & !1, if value & 1 != 0 { CodeMode::Thumb } else { CodeMode::Arm }));
                            }
                            format!("={name}")
                        } else {
                            format!("=0x{value:08X}")
                        };
                    }
                }
            }
            _ => {}
        }

        if let Some(lines) = lines.as_deref_mut() {
            let bytes = match (mode, size) {
                (CodeMode::Arm, _) => format!("{:08X}", word.unwrap_or(0)),
                (CodeMode::Thumb, 4) => {
                    let w = word.unwrap_or(0);
                    format!("{:04X} {:04X}", w & 0xFFFF, w >> 16)
                }
                (CodeMode::Thumb, _) => format!("{:04X}", LittleEndianReader::new(&module.image).u16(offset as usize).unwrap_or(0)),
            };
            let mut formatter = Formatter { options: &options, text: String::new(), base: module.base, size: module.size(), entries };
            let text = match ins {
                Ins::Illegal => format!("<illegal {}>", if mode == CodeMode::Arm { "ARM" } else { "THUMB" }),
                ins => match formatter.write_ins(&ins) {
                    Ok(()) => formatter.text,
                    Err(_) => "<unprintable>".to_owned(),
                },
            };
            lines.push(AsmLine { offset, bytes, text, comment, labels, target });
        }
        offset += size;
    }

    for (address, mode) in found {
        let Some(offset) = module.offset_of(address & !1).filter(|o| code.iter().any(|r| (r.start..r.end).contains(o))) else { continue };
        let entry = entries.entry(offset).or_insert_with(|| Entry { names: vec![format!("fn_{offset:X}")], mode: None });
        if entry.mode.is_none() {
            entry.mode = Some(mode);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A module at 0x10000000 with: an ARM function at 0x3C that loads a module address and calls a THUMB one at 0x48
    /// with BLX, a constructor pointing at the THUMB one, an exports object holding both, and one ARM BL patch to the
    /// first.
    pub fn sample() -> Vec<u8> {
        let base = 0x1000_0000u32;
        let mut image = vec![0u8; 0x80];
        let mut put = |at: usize, value: u32| image.splice(at..at + 4, value.to_le_bytes()).for_each(drop);
        // header
        put(0, NCRT_MAGIC);
        put(4, u32::from(NCRT_VERSION) | (2 << 24));
        put(8, base);
        put(12, 0x80);
        put(16, base + 0x58); // exports
        put(20, base + 0x60); // patch table
        put(24, 1); // one patch
        put(28, base + 0x80); // relocs
        put(32, 5);
        put(36, base + 0x50); // init
        put(40, 1);
        put(44, base + 0x54); // fini
        put(48, 0);
        put(52, base + 0x74); // atexit
        put(56, 0);
        // 0x3C: ARM: ldr r0, [pc, #8] (the word at 0x4C) ; blx 0x48 ; bx lr
        put(0x3C, 0xE59F_0008);
        put(0x40, 0xFA00_0000); // blx to 0x40 + 8 + 0 = 0x48
        put(0x44, 0xE12F_FF1E);
        // 0x48: THUMB: movs r0, #1 ; bx lr
        put(0x48, 0x4770_2001);
        // 0x4C: the word the ldr loads, the address of the exports object
        put(0x4C, base + 0x58);
        put(0x50, base + 0x49); // constructor, THUMB
        put(0x58, base + 0x3C); // export 0
        put(0x5C, base + 0x49); // export 1
        // patch table
        put(0x60, 0x0200_1000);
        put(0x64, 4 | (2 << 16)); // size 4, ARM BL
        put(0x68, base + 0x3C);
        put(0x6C, base + 0x70);
        let mut file = image;
        for reloc in [0x4Cu32, 0x50, 0x58, 0x5C, 0x68] {
            file.extend(reloc.to_le_bytes());
        }
        file
    }

    #[test]
    fn a_module_reads_back_its_layout() {
        let module = parse_ncrt(&sample()).unwrap();
        assert_eq!(module.base, 0x1000_0000);
        assert_eq!(module.exports, Some(0x58));
        assert_eq!(module.exports_end, 0x60, "with no hooks the exports run up to the patch table");
        assert_eq!(module.patches.len(), 1);
        assert_eq!(module.patches[0].kind, NcrtPatchKind::ArmBl);
        assert_eq!(module.constructors, [0x1000_0049]);
        let exports = module.export_words();
        assert_eq!(exports.iter().map(|w| w.target).collect::<Vec<_>>(), [Some(0x3C), Some(0x48)]);
        assert!(module.regions().iter().any(|r| r.name == "Patch table" && r.start == 0x60 && r.end == 0x70));
        assert!(module.regions().iter().any(|r| r.name == "Saved game bytes" && r.start == 0x70 && r.end == 0x74));
    }

    #[test]
    fn the_listing_follows_the_code_into_thumb() {
        let module = parse_ncrt(&sample()).unwrap();
        let lines = disassemble_ncrt(&module, ListingMode::Auto);
        let at = |offset: u32| lines.iter().find(|l| l.offset == offset).unwrap();
        assert!(at(0x3C).labels.contains(&"patch0_02001000".to_owned()));
        assert!(at(0x3C).labels.contains(&"export0".to_owned()));
        assert!(at(0x40).text.starts_with("blx"), "{}", at(0x40).text);
        assert!(at(0x40).text.contains("constructor0"), "a branch names its target: {}", at(0x40).text);
        assert_eq!(at(0x48).bytes, "2001", "THUMB from the constructor's bit");
        assert!(at(0x48).text.starts_with("mov"), "{}", at(0x48).text);
        assert!(at(0x4A).text.starts_with("bx"), "{}", at(0x4A).text);
        assert_eq!(at(0x4C).text, ".word 0x10000058", "a loaded word is data");
        assert_eq!(at(0x3C).comment, "=+0x58");
        assert_eq!(at(0x40).target, Some(0x48), "a branch can be followed");
    }

    #[test]
    fn a_file_that_is_not_a_module_is_refused() {
        assert!(parse_ncrt(b"NCRT").is_err());
        assert!(parse_ncrt(b"NARC\0\0\0\0").is_err());
        let mut wrong = sample();
        wrong[4] = 9;
        assert!(parse_ncrt(&wrong).unwrap_err().message.contains("version 9"));
    }
}
