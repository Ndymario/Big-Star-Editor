use bigstar_core::ErrorCode;
use bigstar_formats::{compress_blz_overlay, compress_nitro_lz10};
use bigstar_nds::RomHeader;

use crate::test_support::{BANNER_SIZE, RomFile, TempDir, make_narc, make_rom, write_bytes};
use crate::*;

fn course_rom(extra: &[(&str, &[u8])]) -> Vec<u8> {
    let mut files = vec![RomFile::new("course/course1.bin", b"0123456789ab")];
    files.extend(extra.iter().map(|(path, data)| RomFile::new(path, data)));
    make_rom(&files, &[])
}

#[test]
fn rom_reader_reads_in_place() {
    let dir = TempDir::new("rom-reader");

    let vs_bmg = [b'M', b'E', b'S', b'G', 1, 2, 3, 4];
    let hd_bin = [b'H', b'D', 0, 0];
    let plain = make_narc(&[("vs.bmg", &vs_bmg), ("menu/hd.bin", &hd_bin)]);
    let kart = [b'K', b'A', b'R', b'T', 0, 0, 0, 0];
    let packed_raw = make_narc(&[("kart.bin", &kart)]);
    let packed = compress_nitro_lz10(&packed_raw).unwrap();
    let fresh = b"FRESH";

    // Compressible on purpose: BLZ declines to grow a file, and an overlay it declined is stored raw, which would not
    // exercise the decompression path.
    let overlay_raw: Vec<u8> = (0..0x800usize).map(|i| (i % 7) as u8).collect();
    let overlay = compress_blz_overlay(&overlay_raw).unwrap();
    assert!(!overlay.is_empty());

    let rom_path = dir.path().join("build.nds");
    let rom = make_rom(&[RomFile::new("plain.narc", &plain), RomFile::new("packed.carc", &packed), RomFile::new("z_new/fresh.bin", fresh)], &overlay);
    write_bytes(&rom_path, &rom);

    let mut reader = RomReader::open(&rom_path).unwrap();
    assert_eq!(reader.header().game_code, "A2DE");
    assert_eq!(reader.files().len(), 3);

    // By path, and by the id that path resolves to -- the two have to agree, or a resource table that names an id and a
    // dock that names a path disagree.
    assert_eq!(reader.file_id("z_new/fresh.bin"), Some(2));
    assert_eq!(reader.file_id("z_new/missing.bin"), None);
    assert_eq!(reader.read("z_new/fresh.bin").unwrap(), fresh);
    assert_eq!(reader.read_id(2).unwrap(), fresh);
    assert!(reader.read("nothing/here.bin").is_err());
    assert_eq!(reader.read_prefix("z_new/fresh.bin", 3).unwrap(), b"FRE");
    assert_eq!(reader.read_prefix("z_new/fresh.bin", 99).unwrap(), fresh);

    assert_eq!(reader.read_arm9().unwrap().first(), Some(&0x90));
    assert_eq!(reader.read_banner().unwrap().len(), BANNER_SIZE);
    assert_eq!(reader.read_banner().unwrap().get(2), Some(&0xAB));

    // The ROM stores the overlay compressed; the reader hands back what the game would run.
    assert_eq!(reader.read_overlay(0).unwrap(), overlay_raw);
    assert!(reader.read_overlay(7).is_err());

    // A plain container and an LZ-wrapped one are the same thing to a caller.
    assert!(reader.is_archive("plain.narc"));
    assert!(reader.is_archive("packed.carc"));
    assert!(!reader.is_archive("z_new/fresh.bin"));
    let listed = reader.list_archive("plain.narc").unwrap();
    assert_eq!(listed.len(), 2);
    // Every member carries the file id its container's name table gave it -- which is what game code loads it by -- and
    // the size comes from the container rather than from a second read of every member.
    assert_eq!((listed[0].id, listed[0].path.as_str(), listed[0].size as usize), (0, "vs.bmg", vs_bmg.len()));
    assert_eq!((listed[1].id, listed[1].path.as_str(), listed[1].size as usize), (1, "menu/hd.bin", hd_bin.len()));
    // An id is a position in this list, so reading a member by the id the listing reported reaches the member the
    // listing named.
    assert_eq!(reader.read_member_id("plain.narc", usize::from(listed[1].id)).unwrap(), hd_bin);
    // A compressed container numbers its members no differently.
    let packed_list = reader.list_archive("packed.carc").unwrap();
    assert_eq!((packed_list[0].id, packed_list[0].path.as_str()), (0, "kart.bin"));
    assert_eq!(reader.read_member("plain.narc", "vs.bmg").unwrap(), vs_bmg);
    assert_eq!(reader.read_member_id("plain.narc", 1).unwrap(), hd_bin);
    assert_eq!(reader.read_member("packed.carc", "kart.bin").unwrap(), kart);
    assert!(reader.read_member("plain.narc", "menu/nope.bin").is_err());
    assert!(reader.read_member_id("plain.narc", 9).is_err());

    // Repeat questions are what the cache exists for, and have to answer the same thing as the first one.
    assert_eq!(reader.read_overlay(0).unwrap(), overlay_raw);
    assert_eq!(reader.read_member("plain.narc", "vs.bmg").unwrap(), vs_bmg);

    assert!(RomReader::open(dir.path().join("absent.nds")).is_err());
}

#[test]
fn rom_reader_cache_is_bounded() {
    let dir = TempDir::new("rom-cache");
    let narcs: Vec<(String, Vec<u8>)> = (0..6).map(|i| (format!("a{i}.narc"), make_narc(&[("m.bin", &[i as u8])]))).collect();
    let files: Vec<RomFile> = narcs.iter().map(|(path, data)| RomFile::new(path, data)).collect();
    let rom_path = dir.path().join("many.nds");
    write_bytes(&rom_path, &make_rom(&files, &[]));
    let mut reader = RomReader::open(&rom_path).unwrap();
    for round in 0..2 {
        for (i, (path, _)) in narcs.iter().enumerate() {
            assert_eq!(reader.read_member(path, "m.bin").unwrap(), [i as u8], "round {round}");
        }
    }
}

#[test]
fn manifest_round_trip_and_indexing() {
    let dir = TempDir::new("project");
    let built_rom = dir.path().join("big-star-build.nds");
    write_bytes(&built_rom, &course_rom(&[]));

    let mut project = create_empty_project("Test \"Project\"\nName", dir.path()).unwrap();
    let header = RomHeader { game_title: "TEST \"ROM\"".into(), game_code: "TST1".into(), ..RomHeader::default() };
    project.attach_rom(&built_rom, Some(header));
    index_rom_files(&mut project).unwrap();

    let manifest = dir.path().join(MANIFEST_FILE_NAME);
    save_manifest(&project, &manifest).unwrap();
    let text = std::fs::read_to_string(&manifest).unwrap();
    assert!(text.contains("format: big-star-editor-project"));
    assert!(!text.contains("root:"));
    assert!(!text.contains(&dir.path().display().to_string()));
    assert!(text.contains("rom_path: \"big-star-build.nds\""));
    assert!(text.contains("name: \"Test \\\"Project\\\"\\nName\""));
    // The ROM's file table is derived output; the manifest must not mirror it.
    assert!(!text.contains("files:"));
    assert!(!text.contains("course1.bin"));

    let mut loaded = load_manifest(&manifest).unwrap();
    assert_eq!(loaded.name(), "Test \"Project\"\nName");
    let rom = loaded.rom().unwrap();
    assert_eq!(rom.rom_path, std::path::Path::new("big-star-build.nds"));
    assert_eq!(rom.header.as_ref().unwrap().game_title, "TEST \"ROM\"");
    assert_eq!(loaded.game_files().len(), 1);
    assert_eq!(loaded.game_files()[0].path, std::path::Path::new("course/course1.bin"));
    assert_eq!(loaded.game_files()[0].size, 12);
    assert!(!loaded.dirty());
    loaded.update_game_file_size(std::path::Path::new("course/course1.bin"), 99);
    assert_eq!(loaded.game_files()[0].size, 99);
    assert!(loaded.dirty());
    loaded.mark_clean();

    // A version 1 manifest still opens; its stale `files:` block is ignored, not parsed, and the directory its
    // `extracted_path` named is read and dropped: that tree is derived, so a version 3 project finds the ROM instead.
    let write = |name: &str, text: &str| write_bytes(&dir.path().join(name), text.as_bytes());
    write(
        "legacy-v1.yaml",
        "format: big-star-editor-project\nversion: 1\nname: \"Legacy\"\nrom:\n  extracted_path: \"extracted-rom\"\nfiles:\n  - path: \"extracted-rom/root/gone.bin\"\n    size: nope\n",
    );
    let legacy = load_manifest(&dir.path().join("legacy-v1.yaml")).unwrap();
    assert_eq!(legacy.rom().unwrap().rom_path, std::path::Path::new("big-star-build.nds"));
    assert_eq!(legacy.game_files().len(), 1);
    assert_eq!(legacy.game_files()[0].path, std::path::Path::new("course/course1.bin"));

    write("bad-path.yaml", "format: big-star-editor-project\nversion: 1\nname: \"Bad\"\nrom:\n  extracted_path: \"../outside\"\n");
    assert!(load_manifest(&dir.path().join("bad-path.yaml")).is_err());
    write("bad-drive-path.yaml", "format: big-star-editor-project\nversion: 1\nname: \"Bad\"\nrom:\n  extracted_path: \"C:/outside\"\n");
    assert!(load_manifest(&dir.path().join("bad-drive-path.yaml")).is_err());
    write("bad-version.yaml", "format: big-star-editor-project\nversion: 4\nname: \"Bad\"\n");
    assert!(load_manifest(&dir.path().join("bad-version.yaml")).is_err());
    assert!(make_project_relative(dir.path(), std::path::Path::new("")).is_err());

    // A project whose ROM has not been built yet still opens, with an empty index.
    write("unbuilt.yaml", "format: big-star-editor-project\nversion: 3\nname: \"Unbuilt\"\nrom:\n  rom_path: \"not-built-yet.nds\"\n");
    let unbuilt = load_manifest(&dir.path().join("unbuilt.yaml")).unwrap();
    assert!(unbuilt.rom().is_some());
    assert!(unbuilt.game_files().is_empty());

    // A rebuild that adds files re-indexes from the ROM, not from a directory somebody has to remember to refresh.
    write_bytes(&built_rom, &course_rom(&[("course/a.bin", b"abc"), ("course/z.bin", b"z")]));
    index_rom_files(&mut loaded).unwrap();
    assert_eq!(loaded.game_files().len(), 3);
    assert_eq!(loaded.game_files()[0].path, std::path::Path::new("course/a.bin"));
    assert_eq!(loaded.game_files()[0].size, 3);
    assert_eq!(loaded.game_files()[2].path, std::path::Path::new("course/z.bin"));
    assert_eq!(index_rom_files(&mut Project::default()).unwrap_err().code, ErrorCode::InvalidArgument);
}

#[test]
fn manifest_refuses_bad_values() {
    let dir = TempDir::new("manifest-bad");
    let manifest = dir.path().join("m.yaml");
    for (text, why) in [
        ("version: 3\nname: \"X\"\n", "format marker"),
        ("format: big-star-editor-project\nname: \"X\"\n", "version"),
        ("format: big-star-editor-project\nversion: 0\nname: \"X\"\n", "version zero"),
        ("format: big-star-editor-project\nversion: 3\n", "name"),
        ("format: big-star-editor-project\nversion: 3\nname: \"\"\n", "empty name"),
        ("format: big-star-editor-project\nversion: 3\nname: \"X\nrom:\n", "unterminated"),
        ("format: big-star-editor-project\nversion: 3\nname: \"a\\qb\"\n", "bad escape"),
        ("format: big-star-editor-project\nversion: 3\nname: \"a\"b\"\n", "bare quote"),
        ("format: big-star-editor-project\nversion: 3\nname: \"X\"\nrom:\n  rom_path: \"/etc/rom.nds\"\n", "absolute"),
        ("format: big-star-editor-project\nversion: 3\nname: \"X\"\nrom:\n  rom_path: \"a\\\\b.nds\"\n", "backslash"),
    ] {
        write_bytes(&manifest, text.as_bytes());
        assert!(load_manifest(&manifest).is_err(), "{why}");
    }
    assert_eq!(load_manifest(&dir.path().join("absent.yaml")).unwrap_err().code, ErrorCode::IoError);
}

#[test]
fn project_paths_stay_inside_the_project() {
    let dir = std::path::Path::new("/work/project");
    assert_eq!(make_project_relative(dir, std::path::Path::new("/work/project/a/../b.nds")).unwrap(), std::path::Path::new("b.nds"));
    assert!(make_project_relative(dir, std::path::Path::new("/work/other/b.nds")).is_err());
    assert!(make_project_relative(dir, std::path::Path::new("/work/project")).is_err());
    assert!(make_project_relative(dir, std::path::Path::new("../b.nds")).is_err());
    assert_eq!(lexically_normal(std::path::Path::new("a/./b/../c")), std::path::Path::new("a/c"));
}

#[test]
fn default_rom_prefers_the_built_rom() {
    let dir = TempDir::new("default-rom");
    assert_eq!(default_rom_path(dir.path()), std::path::Path::new(BUILT_ROM_FILE_NAME));
    write_bytes(&dir.path().join(INPUT_ROM_FILE_NAME), b"x");
    assert_eq!(default_rom_path(dir.path()), std::path::Path::new(INPUT_ROM_FILE_NAME));
    write_bytes(&dir.path().join(BUILT_ROM_FILE_NAME), b"x");
    assert_eq!(default_rom_path(dir.path()), std::path::Path::new(BUILT_ROM_FILE_NAME));
}

#[test]
fn level_catalog_lists_worlds_and_download_play_levels() {
    let catalog = nsmb_level_catalog();
    assert!(!catalog.is_empty());
    assert_eq!(catalog[0].world, "World 1");
    assert_eq!(catalog[0].display_name, "1-1");
    assert_eq!(catalog[0].areas.len(), 3);
    assert_eq!(catalog[0].areas[0].display_name, "Area 1");
    assert_eq!(catalog[0].areas[0].course_token, "A01_1");
    let ice = catalog.iter().flat_map(|entry| &entry.areas).find(|area| area.source_token == "J03_1@Dat_Ice").unwrap();
    assert_eq!(ice.course_token, "J03_1");
    assert_eq!(ice.narc_name.as_deref(), Some("Dat_Ice"));
    // A one-area level shows the level's own name.
    assert_eq!(catalog.iter().find(|entry| entry.display_name == "1-3").unwrap().areas[0].display_name, "1-3");
}

#[test]
fn module_data_catalog_loads_from_the_generated_directory() {
    // A project that has never been built has no generated data, which is an empty catalog and not an error.
    let missing = std::env::temp_dir().join("big-star-no-generated-data-anywhere");
    let catalog = load_module_data_catalog(&missing, std::path::Path::new(DEFAULT_GENERATED_DIR)).unwrap();
    assert!(catalog.level_data.is_empty() && catalog.stage_objects.is_empty());

    let dir = TempDir::new("module-data");
    let generated = dir.path().join("build/generated");
    write_bytes(
        &generated.join("stageobjects.json"),
        br#"{"schema":"nsmbtool.stageobjects/1","stageObjectIdBase":326,"stageObjects":[
            {"name":"coop.CoopFlagActor.Default","hash":"0xf46a44b9","module":"coop","component":"CustomWorldUnlock","object":"CoopFlagActor","variant":"Default","objectId":387,"info":"CoopFlagActor::ObjectInfo_Default","header":"coop_flag.hpp"}
        ]}"#,
    );
    let catalog = load_module_data_catalog(dir.path(), std::path::Path::new(DEFAULT_GENERATED_DIR)).unwrap();
    assert_eq!(catalog.stage_object_id_base, 326);
    assert_eq!(catalog.stage_objects.len(), 1);

    write_bytes(&generated.join("level_data.json"), b"not json");
    assert!(load_module_data_catalog(dir.path(), std::path::Path::new(DEFAULT_GENERATED_DIR)).is_err());
}
