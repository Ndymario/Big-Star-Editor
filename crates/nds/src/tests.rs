use super::*;

fn put_u16(data: &mut [u8], offset: usize, value: u16) {
    data[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(data: &mut [u8], offset: usize, value: u32) {
    data[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn make_fnt() -> Vec<u8> {
    let mut fnt = vec![0u8; 8];
    put_u32(&mut fnt, 0, 8);
    put_u16(&mut fnt, 4, 0);
    put_u16(&mut fnt, 6, 1);
    let name = b"course.bin";
    fnt.push(name.len() as u8);
    fnt.extend_from_slice(name);
    fnt.push(0);
    fnt
}

fn make_test_rom() -> Vec<u8> {
    const ARM9_OFFSET: u32 = 0x4000;
    const ARM9_SIZE: u32 = 0x20;
    const ARM7_OFFSET: u32 = 0x8000;
    const ARM7_SIZE: u32 = 0x20;
    const FNT_OFFSET: u32 = 0x8400;
    const FAT_OFFSET: u32 = 0x8500;
    const FILE_OFFSET: u32 = 0x9000;
    let file_data = b"LEVEL";
    let fnt = make_fnt();

    let mut rom = vec![0xFFu8; 0x9200];
    rom[..DS_CARTRIDGE_HEADER_SIZE].fill(0);
    rom[..8].copy_from_slice(b"TEST ROM");
    rom[0x00C..0x010].copy_from_slice(b"TEST");
    rom[0x010..0x012].copy_from_slice(b"01");
    rom[0x014] = 7;

    put_u32(&mut rom, 0x020, ARM9_OFFSET);
    put_u32(&mut rom, 0x024, 0x0200_0000);
    put_u32(&mut rom, 0x028, 0x0200_0000);
    put_u32(&mut rom, 0x02C, ARM9_SIZE);
    put_u32(&mut rom, 0x030, ARM7_OFFSET);
    put_u32(&mut rom, 0x034, 0x0200_1000);
    put_u32(&mut rom, 0x038, 0x0200_1000);
    put_u32(&mut rom, 0x03C, ARM7_SIZE);
    put_u32(&mut rom, 0x040, FNT_OFFSET);
    put_u32(&mut rom, 0x044, fnt.len() as u32);
    put_u32(&mut rom, 0x048, FAT_OFFSET);
    put_u32(&mut rom, 0x04C, 8);
    let rom_len = rom.len() as u32;
    put_u32(&mut rom, 0x080, rom_len);
    put_u32(&mut rom, 0x084, DEFAULT_HEADER_REGION_SIZE as u32);

    for i in 0..ARM9_SIZE {
        rom[(ARM9_OFFSET + i) as usize] = (0x90 + i) as u8;
    }
    for i in 0..ARM7_SIZE {
        rom[(ARM7_OFFSET + i) as usize] = (0x70 + i) as u8;
    }
    rom[FNT_OFFSET as usize..FNT_OFFSET as usize + fnt.len()].copy_from_slice(&fnt);
    put_u32(&mut rom, FAT_OFFSET as usize, FILE_OFFSET);
    put_u32(&mut rom, FAT_OFFSET as usize + 4, FILE_OFFSET + file_data.len() as u32);
    rom[FILE_OFFSET as usize..FILE_OFFSET as usize + file_data.len()].copy_from_slice(file_data);

    let secure = crc16(&rom[ARM9_OFFSET as usize..0x8000], 0xFFFF);
    put_u16(&mut rom, 0x06C, secure);
    let header = crc16(&rom[..0x15E], 0xFFFF);
    put_u16(&mut rom, 0x15E, header);
    rom
}

fn slice(rom: &[u8], region: RomRegion) -> &[u8] {
    &rom[region.rom_offset as usize..(region.rom_offset + region.size) as usize]
}

#[test]
fn parses_the_synthetic_rom() {
    let rom = make_test_rom();
    let header = parse_rom_header(&rom).unwrap();
    assert_eq!(header.game_title, "TEST ROM");
    assert_eq!(header.game_code, "TEST");
    assert_eq!(header.arm9.rom_offset, 0x4000);
    assert_eq!(header.fnt.rom_offset, 0x8400);

    let fnt = parse_fnt(slice(&rom, header.fnt)).unwrap();
    assert_eq!(fnt.len(), 1);
    assert_eq!(fnt[0].id, 0);
    assert_eq!(fnt[0].path, "course.bin");

    let fat = parse_fat(slice(&rom, header.fat)).unwrap();
    assert!(fat.len() == 1 && fat[0].used() && fat[0].size() == 5);

    // The banner's own version says how long it is; the next region after it says how much room there is.
    // A reader with only the header and the FAT needs both, and this ROM declares no banner at all.
    assert_eq!(banner_size_from_version(&[1, 0]), 0x0A00);
    assert_eq!(banner_size_from_version(&[9, 9]), 0);
    assert_eq!(banner_size_from_version(&[3]), 0);
    assert_eq!(banner_region_limit(&header, &fat, rom.len()), 0x4000);
}

#[test]
fn refuses_malformed_tables() {
    assert!(parse_rom_header(&[0; 0x1FF]).is_err());
    assert!(parse_fat(&[0; 7]).is_err());
    assert!(parse_fat(&[5, 0, 0, 0, 1, 0, 0, 0]).is_err());
    assert!(parse_overlay_table(&[0; 0x21]).is_err());
    assert_eq!(parse_overlay_table(&[0; 0x40]).unwrap().len(), 2);
    assert!(parse_fnt(&[0; 4]).is_err());
    // A directory entry pointing back at the root is a cycle.
    let mut fnt = vec![0u8; 8];
    put_u32(&mut fnt, 0, 8);
    put_u16(&mut fnt, 6, 1);
    fnt.extend_from_slice(&[0x81, b'd']);
    fnt.extend_from_slice(&0xF000u16.to_le_bytes());
    fnt.push(0);
    assert!(parse_fnt(&fnt).is_err());
}

#[test]
fn walks_nested_directories() {
    let mut fnt = vec![0u8; 16];
    put_u32(&mut fnt, 0, 16);
    put_u16(&mut fnt, 4, 0);
    put_u16(&mut fnt, 6, 2);
    put_u16(&mut fnt, 12, 0);
    put_u16(&mut fnt, 14, 0xF000);
    fnt.extend_from_slice(&[0x83, b'd', b'i', b'r']);
    fnt.extend_from_slice(&0xF001u16.to_le_bytes());
    fnt.push(0);
    let sub = fnt.len() as u32;
    put_u32(&mut fnt, 8, sub);
    put_u16(&mut fnt, 12, 0);
    fnt.extend_from_slice(&[1, b'a', 0]);
    let files = parse_fnt(&fnt).unwrap();
    assert_eq!(files, vec![NitroFileEntry { id: 0, path: "dir/a".to_owned() }]);
}

fn header_checksum_holds(rom: &[u8]) -> bool {
    crc16(&rom[..0x15E], 0xFFFF) == u16::from_le_bytes([rom[0x15E], rom[0x15F]])
}

fn file_bytes(rom: &[u8], id: usize) -> Vec<u8> {
    let header = parse_rom_header(rom).unwrap();
    let entry = parse_fat(slice(rom, header.fat)).unwrap()[id];
    rom[entry.start_address as usize..entry.end_address as usize].to_vec()
}

#[test]
fn a_writer_with_nothing_staged_returns_the_rom_untouched() {
    let rom = make_test_rom();
    let writer = NdsRomWriter::new(rom.clone()).unwrap();
    assert!(!writer.dirty());
    assert_eq!(writer.commit().unwrap(), rom);
}

#[test]
fn a_file_that_still_fits_stays_in_its_extent() {
    let rom = make_test_rom();
    let mut writer = NdsRomWriter::new(rom.clone()).unwrap();
    writer.set_file(0, b"LEVEL-EDITED".to_vec()).unwrap();
    let out = writer.commit().unwrap();

    assert_eq!(out.len(), rom.len());
    let header = parse_rom_header(&out).unwrap();
    let fat = parse_fat(slice(&out, header.fat)).unwrap();
    assert_eq!(fat[0], FatEntry { start_address: 0x9000, end_address: 0x9000 + 12 });
    assert_eq!(file_bytes(&out, 0), b"LEVEL-EDITED");
    assert_eq!(header.total_used_rom_size, rom.len() as u32);
    assert!(header_checksum_holds(&out));
    // The binaries and the name table are where they were, byte for byte.
    let before = parse_rom_header(&rom).unwrap();
    assert_eq!(out[0x4000..0x8500], rom[0x4000..0x8500]);
    assert_eq!(slice(&out, header.fnt), slice(&rom, before.fnt));
    assert_eq!(parse_fnt(slice(&out, header.fnt)).unwrap()[0].path, "course.bin");
}

#[test]
fn a_file_that_outgrows_its_extent_is_appended_after_the_used_rom() {
    let rom = make_test_rom();
    let mut writer = NdsRomWriter::new(rom.clone()).unwrap();
    writer.set_file(0, vec![0xDD; 0x400]).unwrap();
    let out = writer.commit().unwrap();

    let header = parse_rom_header(&out).unwrap();
    let fat = parse_fat(slice(&out, header.fat)).unwrap();
    assert_eq!(fat[0], FatEntry { start_address: 0x9200, end_address: 0x9600 });
    assert_eq!(header.total_used_rom_size, 0x9600);
    assert_eq!(out.len(), 0x9600);
    assert_eq!(file_bytes(&out, 0), vec![0xDD; 0x400]);
    // The abandoned extent is left as it was, as NCPatcher leaves it.
    assert_eq!(&out[0x9000..0x9005], b"LEVEL");
    assert_eq!(header.device_capacity, 7);
    assert!(header_checksum_holds(&out));
}

#[test]
fn appending_past_the_chip_size_raises_the_device_capacity() {
    let mut rom = make_test_rom();
    rom[0x014] = 0;
    let mut writer = NdsRomWriter::new(rom).unwrap();
    writer.set_file(0, vec![0xAB; 0x20000]).unwrap();
    let out = writer.commit().unwrap();
    let header = parse_rom_header(&out).unwrap();
    assert_eq!(header.total_used_rom_size, 0x9200 + 0x20000);
    assert_eq!(header.device_capacity, 1);
    assert!(header_checksum_holds(&out));
}

#[test]
fn the_writer_refuses_unknown_files_and_banners_of_another_size() {
    let mut writer = NdsRomWriter::new(make_test_rom()).unwrap();
    assert!(writer.set_file(1, vec![1]).is_err());
    assert!(writer.set_banner(vec![0; 0xA00]).is_err(), "this ROM has no banner");

    let mut rom = make_test_rom();
    put_u32(&mut rom, 0x068, 0x8600);
    put_u16(&mut rom, 0x8600, 1);
    let mut writer = NdsRomWriter::new(rom).unwrap();
    assert_eq!(writer.banner_size(), 0xA00);
    assert!(writer.set_banner(vec![0; 0x840]).is_err());
    let mut banner = vec![0x11; 0xA00];
    banner[..2].copy_from_slice(&1u16.to_le_bytes());
    writer.set_banner(banner.clone()).unwrap();
    let out = writer.commit().unwrap();
    assert_eq!(out[0x8600..0x9000], banner[..]);
    assert_eq!(file_bytes(&out, 0), b"LEVEL");
    assert!(header_checksum_holds(&out));
}

/// Against a real ROM, named by `BIGSTAR_NSMB_ROM`: an untouched write is the same file, and a grown level comes back
/// out of the written ROM while every other file stays put. Skipped without one.
#[test]
fn rewrites_a_real_rom() {
    let Some(path) = std::env::var_os("BIGSTAR_NSMB_ROM") else { return };
    let rom = std::fs::read(path).unwrap();
    assert_eq!(NdsRomWriter::new(rom.clone()).unwrap().commit().unwrap(), rom);

    let header = parse_rom_header(&rom).unwrap();
    let files = parse_fnt(slice(&rom, header.fnt)).unwrap();
    let course = files.iter().find(|file| file.path == "course/A01_1.bin").expect("the ROM holds course/A01_1.bin");
    let id = usize::from(course.id);
    let mut grown = file_bytes(&rom, id);
    grown.extend_from_slice(&[0x5A; 0x1000]);
    let mut shrunk = file_bytes(&rom, id + 1);
    shrunk.truncate(shrunk.len() / 2);

    let mut writer = NdsRomWriter::new(rom.clone()).unwrap();
    writer.set_file(course.id, grown.clone()).unwrap();
    writer.set_file(course.id + 1, shrunk.clone()).unwrap();
    let out = writer.commit().unwrap();

    assert!(header_checksum_holds(&out));
    assert_eq!(file_bytes(&out, id), grown);
    assert_eq!(file_bytes(&out, id + 1), shrunk);
    let fat = parse_fat(slice(&rom, header.fat)).unwrap();
    for other in (0..fat.len()).filter(|&i| i != id && i != id + 1) {
        assert_eq!(file_bytes(&out, other), file_bytes(&rom, other), "file {other} changed");
    }
    let written = parse_rom_header(&out).unwrap();
    assert_eq!(parse_fnt(slice(&out, written.fnt)).unwrap(), files);
    let arm9 = |arm: RomArmBinary| RomRegion { rom_offset: arm.rom_offset, size: arm.size };
    assert_eq!(slice(&out, arm9(written.arm9)), slice(&rom, arm9(header.arm9)));
}
