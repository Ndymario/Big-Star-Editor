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

fn write_text(path: &std::path::Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn plan_entry(path: &str, source: &str) -> bigstar_patching::RomFileEntry {
    bigstar_patching::RomFileEntry {
        path: path.to_owned(),
        source: source.to_owned(),
        module: "coop".to_owned(),
        action: bigstar_patching::FileAction::Modified,
        ..Default::default()
    }
}

#[test]
fn effective_filesystem_prefers_module_sources() {
    let dir = TempDir::new("effective-filesystem");
    let vs_bmg = [b'M', b'E', b'S', b'G', 1, 2, 3, 4];
    let hd_bin = [b'H', b'D', 0, 0];
    let fresh = b"FRESH";
    let plain = make_narc(&[("vs.bmg", &vs_bmg), ("menu/hd.bin", &hd_bin)]);
    let packed = compress_nitro_lz10(&make_narc(&[("kart.bin", b"KART")])).unwrap();
    let overlay_raw: Vec<u8> = (0..0x400usize).map(|i| (i % 5) as u8).collect();
    let overlay = compress_blz_overlay(&overlay_raw).unwrap();
    let rom_path = dir.path().join("big-star-build.nds");
    write_bytes(
        &rom_path,
        &make_rom(&[RomFile::new("plain.narc", &plain), RomFile::new("packed.carc", &packed), RomFile::new("z_new/fresh.bin", fresh)], &overlay),
    );

    let mut files = EffectiveFilesystem::new();
    files.set_rom(Some(RomReader::open(&rom_path).unwrap()));

    // Split first, because everything below turns on it.
    assert_eq!(split_rom_address("plain.narc").container, "plain.narc");
    assert!(!split_rom_address("plain.narc").inside_archive());
    assert_eq!(split_rom_address("plain.narc!menu/hd.bin").member, "menu/hd.bin");

    // With no plan, everything is the ROM's answer.
    assert_eq!(files.read("z_new/fresh.bin").unwrap(), fresh);
    assert_eq!(files.read("plain.narc!vs.bmg").unwrap(), vs_bmg);
    assert!(files.source("z_new/fresh.bin").is_none());
    assert_eq!(files.path_for_id(0).as_deref(), Some("plain.narc"));
    assert!(files.path_for_id(9).is_none());

    write_text(&dir.path().join("modules/coop/nitrofs/z_new/fresh.bin"), "EDITED");
    write_text(&dir.path().join("modules/message/nitrofs/plain_narc/vs.bmg"), "PENDING");

    let mut plan = bigstar_patching::FileManifest { planned: true, ..Default::default() };
    plan.files.push(plan_entry("z_new/fresh.bin", "modules/coop/nitrofs/z_new/fresh.bin"));
    // A source the plan names but the disk does not hold has no bytes to offer, so the ROM still answers.
    plan.files.push(plan_entry("packed.carc", "build/generated/packed.carc"));
    let mut archive = plan_entry("plain.narc", "");
    archive.members.push(bigstar_patching::RomArchiveMember {
        id: 0,
        path: "vs.bmg".to_owned(),
        source: "modules/message/nitrofs/plain_narc/vs.bmg".to_owned(),
        module: "message".to_owned(),
        ..Default::default()
    });
    plan.files.push(archive);
    files.set_plan(dir.path(), Some(plan.clone()));

    // A module edit that has not been built yet is what the editor has to show.
    assert_eq!(files.read("z_new/fresh.bin").unwrap(), b"EDITED");
    assert_eq!(files.source("z_new/fresh.bin"), Some(dir.path().join("modules/coop/nitrofs/z_new/fresh.bin")));
    assert_eq!(files.read_prefix("z_new/fresh.bin", 3).unwrap(), b"EDI");

    // One edited member does not take its siblings out of the ROM.
    assert_eq!(files.read("plain.narc!vs.bmg").unwrap(), b"PENDING");
    assert_eq!(files.read("plain.narc!menu/hd.bin").unwrap(), hd_bin);
    assert_eq!(files.read_prefix("plain.narc!menu/hd.bin", 2).unwrap(), b"HD");
    assert!(files.source("plain.narc!menu/hd.bin").is_none());

    // The container's own bytes are still the ROM's.
    assert_eq!(files.read("plain.narc").unwrap(), plain);
    // A source the plan names and the disk lacks falls back rather than failing.
    assert!(files.source("packed.carc").is_none());
    assert_eq!(files.read("packed.carc").unwrap(), packed);
    assert!(files.read("nowhere/at/all.bin").is_err());

    // Without a ROM only what a module supplies can be read, the state before a first build.
    let mut unbuilt = EffectiveFilesystem::new();
    unbuilt.set_plan(dir.path(), Some(plan));
    assert_eq!(unbuilt.read("z_new/fresh.bin").unwrap(), b"EDITED");
    assert!(unbuilt.read("plain.narc!menu/hd.bin").is_err());
    assert!(unbuilt.path_for_id(0).is_none());
}

mod filesystem_view {
    use std::path::{Path, PathBuf};

    use bigstar_formats::compress_nitro_lz10;
    use bigstar_patching::{FileAction, FileManifest, ModuleGraph, ModuleInfo, ModuleNitroFs, RomArchiveMember, RomFileEntry};

    use crate::test_support::{RomFile, TempDir, make_narc, make_rom, write_bytes};
    use crate::*;

    fn write(path: &Path, value: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, value).unwrap();
    }

    fn module(key: &str, root: PathBuf, layered: bool, base: &str, into: &str) -> ModuleInfo {
        ModuleInfo {
            key: key.to_owned(),
            id: key.to_owned(),
            enabled: true,
            nitrofs: Some(ModuleNitroFs { dir: root, layered, base_variant: base.to_owned(), into: into.to_owned() }),
            ..Default::default()
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn file(id: i64, path: &str, size: i64, action: FileAction, source: &str, module: &str, component: &str, variant: &str) -> RomFileEntry {
        RomFileEntry {
            id,
            path: path.to_owned(),
            size,
            action,
            source: source.to_owned(),
            module: module.to_owned(),
            component: component.to_owned(),
            from_variant: variant.to_owned(),
            ..Default::default()
        }
    }

    fn member(id: i64, path: &str, size: i64, source: &str) -> RomArchiveMember {
        RomArchiveMember {
            id,
            path: path.to_owned(),
            size,
            action: FileAction::Modified,
            source: source.to_owned(),
            module: "second".to_owned(),
            component: "Assets".to_owned(),
            from_variant: "fr".to_owned(),
            source_missing: false,
        }
    }

    fn archive(id: i64, path: &str, size: i64, members: Vec<RomArchiveMember>) -> RomFileEntry {
        RomFileEntry { id, path: path.to_owned(), size, action: FileAction::Modified, members, ..Default::default() }
    }

    fn write_rom(rom_path: &Path, stale: &str, other: &str, with_fresh: bool) {
        let one = |contents: &str| make_narc(&[("member.bin", contents.as_bytes())]);
        let two = |first: &str| make_narc(&[("member.bin", first.as_bytes()), ("sibling.bin", b"SIB")]);
        let mut files = vec![
            RomFile::new("vanilla.bin", b"V"),
            RomFile::new("shared.bin", b"FR"),
            RomFile::new("moved.bin", b"SAME"),
            RomFile::new("explicit.bin", b"EXPLICIT"),
            RomFile::new("generated.bin", b"BUILT"),
            RomFile::new("stale.bin", stale.as_bytes()),
            RomFile::new("gone.bin", b"LEFTOVER"),
            RomFile::new("z_new/reserved", b""),
            RomFile::new("z_new/add.bin", b"ADD"),
            RomFile::new("ARCHIVE/menu.narc", &one("M")),
            RomFile::new("ARCHIVE/other.narc", &two(other)),
            // A compressed container is an ordinary archive under an LZ10 wrapper, and has to be compared through it.
            RomFile::new("ARCHIVE/menu.carc", &compress_nitro_lz10(&one("C")).unwrap()),
        ];
        if with_fresh {
            files.push(RomFile::new("z_new/fresh.bin", b"FRESH"));
        }
        write_bytes(rom_path, &make_rom(&files, &[]));
    }

    #[test]
    fn the_view_joins_plan_manifest_and_rom() {
        let dir = TempDir::new("filesystem-view");
        let root = dir.path();
        let rom_path = root.join("big-star-build.nds");
        write_rom(&rom_path, "OLD", "OLD", false);
        let mut rom = RomReader::open(&rom_path).unwrap();

        let first = module("first", root.join("modules/first/nitrofs"), false, "", "");
        let first_dir = first.nitrofs.as_ref().unwrap().dir.clone();
        write(&first_dir.join("moved.bin"), "SAME");
        let second = module("second", root.join("modules/second/nitrofs"), true, "en", "");
        let second_dir = second.nitrofs.as_ref().unwrap().dir.clone();
        for (path, contents) in [
            ("fr/shared.bin", "FR"),
            ("fr/stale.bin", "NEW"),
            ("fr/z_new/add.bin", "ADD"),
            ("fr/z_new/fresh.bin", "FRESH"),
            ("fr/ARCHIVE/menu_narc/member.bin", "M"),
            ("fr/ARCHIVE/menu_carc/member.bin", "C"),
            ("fr/ARCHIVE/other_narc/member.bin", "NEW"),
            ("fr/ARCHIVE/other_narc/sibling.bin", "SIB"),
        ] {
            write(&second_dir.join(path), contents);
        }
        let prefixed = module("prefixed", root.join("modules/prefixed/nitrofs"), true, "common", "assets");
        let prefixed_dir = prefixed.nitrofs.as_ref().unwrap().dir.clone();
        write(&prefixed_dir.join("french/picture.bin"), "PIC");
        write(&root.join("project-assets/explicit.bin"), "EXPLICIT");
        // A pre-build hook writes this one differently every time, so its bytes never match the ROM.
        write(&root.join("build/generated/generated.bin"), "REGENERATED");
        let graph = ModuleGraph { dir: PathBuf::new(), modules: vec![first, second.clone(), prefixed.clone()] };

        let src = "modules/second/nitrofs/fr";
        let manifest = FileManifest {
            variant: "fr".to_owned(),
            planned: false,
            files: vec![
                file(1, "vanilla.bin", 1, FileAction::Unchanged, "", "", "", ""),
                file(2, "shared.bin", 2, FileAction::Modified, &format!("{src}/shared.bin"), "second", "Assets", "fr"),
                file(3, "moved.bin", 4, FileAction::Modified, &format!("{src}/moved.bin"), "second", "", "fr"),
                file(4, "explicit.bin", 8, FileAction::Modified, "project-assets/explicit.bin", "", "", ""),
                file(5, "generated.bin", 5, FileAction::Modified, "build/generated/generated.bin", "glue", "", ""),
                file(6, "stale.bin", 3, FileAction::Modified, &format!("{src}/stale.bin"), "second", "", "fr"),
                file(7, "gone.bin", 8, FileAction::Modified, "modules/removed/gone.bin", "removed", "", "fr"),
                archive(8, "ARCHIVE/menu.narc", 64, vec![member(0, "member.bin", 1, &format!("{src}/ARCHIVE/menu_narc/member.bin"))]),
                archive(9, "ARCHIVE/menu.carc", 48, vec![member(0, "member.bin", 1, &format!("{src}/ARCHIVE/menu_carc/member.bin"))]),
                archive(
                    10,
                    "ARCHIVE/other.narc",
                    96,
                    vec![
                        member(0, "member.bin", 3, &format!("{src}/ARCHIVE/other_narc/member.bin")),
                        member(1, "sibling.bin", 3, &format!("{src}/ARCHIVE/other_narc/sibling.bin")),
                    ],
                ),
                file(11, "z_new/reserved", 0, FileAction::Created, "", "", "", ""),
                file(12, "z_new/add.bin", 3, FileAction::Created, &format!("{src}/z_new/add.bin"), "second", "Assets", "fr"),
            ],
        };

        // What a build would do now: `stale.bin` and `ARCHIVE/other.narc` have new bytes behind them, `moved.bin`
        // changed hands, `z_new/fresh.bin` is new, and `gone.bin` is no longer supplied at all.
        let mut plan = manifest.clone();
        plan.planned = true;
        let at = |plan: &mut FileManifest, path: &str| -> usize { plan.files.iter().position(|f| f.path == path).unwrap() };
        let i = at(&mut plan, "moved.bin");
        plan.files[i] = file(3, "moved.bin", 4, FileAction::Modified, "modules/first/nitrofs/moved.bin", "first", "", "");
        let i = at(&mut plan, "generated.bin");
        plan.files[i].source_missing = true;
        let i = at(&mut plan, "gone.bin");
        plan.files[i] = file(7, "gone.bin", 8, FileAction::Unchanged, "", "", "", "");
        plan.files.push(file(13, "z_new/fresh.bin", 5, FileAction::Created, &format!("{src}/z_new/fresh.bin"), "second", "Assets", "fr"));

        let view = build_filesystem_view(FilesystemViewRequest {
            rom: Some(&mut rom),
            project_dir: root.to_path_buf(),
            modules: graph.clone(),
            plan: Some(plan.clone()),
            manifest: Some(manifest.clone()),
            variant: "fr".to_owned(),
        })
        .unwrap();
        let find = |path: &str| view.find(path).unwrap_or_else(|| panic!("{path}"));

        assert_eq!(find("vanilla.bin").status, FilesystemStatus::Vanilla);
        assert!(find("shared.bin").status == FilesystemStatus::Replaced && find("shared.bin").module == "second");
        // A project-level `files:` entry: a source with no module behind it.
        assert!(find("explicit.bin").status == FilesystemStatus::Replaced && find("explicit.bin").module.is_empty());
        assert_eq!(find("explicit.bin").source, root.join("project-assets/explicit.bin"));
        assert_eq!(find("stale.bin").status, FilesystemStatus::Pending);
        // Same bytes, different module: a handover is a change even when nothing moves.
        assert!(find("moved.bin").status == FilesystemStatus::Pending && find("moved.bin").module == "first");
        // A hook writes this one, so there is nothing on disk to compare it with.
        assert_eq!(find("generated.bin").status, FilesystemStatus::Replaced);
        assert_eq!(find("gone.bin").status, FilesystemStatus::Orphaned);
        assert_eq!(find("z_new/reserved").status, FilesystemStatus::Added);
        assert_eq!(find("z_new/add.bin").status, FilesystemStatus::Added);

        // An id the ROM does not hold yet, marked as the prediction it is.
        let fresh = find("z_new/fresh.bin");
        assert_eq!(fresh.status, FilesystemStatus::Pending);
        assert!(fresh.id == Some(13) && fresh.id_is_planned);
        assert!(!find("z_new/add.bin").id_is_planned && !find("shared.bin").id_is_planned);

        // Archives, reported one row per member.
        assert_eq!(find("ARCHIVE/menu.narc").status, FilesystemStatus::Replaced);
        assert_eq!(find("ARCHIVE/menu.narc!member.bin").status, FilesystemStatus::Replaced);
        assert!(find("ARCHIVE/menu.narc").id == Some(8) && !find("ARCHIVE/menu.narc").inside_archive());
        assert_eq!(find("ARCHIVE/menu.narc!member.bin").member_id, Some(0));
        assert!(find("ARCHIVE/menu.narc!member.bin").id.is_none());
        assert_eq!(find("ARCHIVE/menu.carc").status, FilesystemStatus::Replaced);
        assert_eq!(find("ARCHIVE/menu.carc!member.bin").status, FilesystemStatus::Replaced);
        // One stale member is named; its sibling stays Replaced even though the container is Pending.
        assert_eq!(find("ARCHIVE/other.narc").status, FilesystemStatus::Pending);
        let stale_member = find("ARCHIVE/other.narc!member.bin");
        assert!(stale_member.status == FilesystemStatus::Pending && stale_member.member_id == Some(0));
        assert!(stale_member.module == "second" && stale_member.component == "Assets" && stale_member.variant == "fr");
        assert_eq!(stale_member.source, second_dir.join("fr/ARCHIVE/other_narc/member.bin"));
        assert_eq!(find("ARCHIVE/other.narc!sibling.bin").status, FilesystemStatus::Replaced);
        assert_eq!(find("ARCHIVE/other.narc!sibling.bin").member_id, Some(1));
        assert!(view.has_pending());

        // When the plan is the document the last build wrote, the view has nothing to report.
        {
            let built = FileManifest { planned: false, ..plan.clone() };
            write_rom(&rom_path, "NEW", "NEW", true);
            let mut rebuilt = RomReader::open(&rom_path).unwrap();
            let same = build_filesystem_view(FilesystemViewRequest {
                rom: Some(&mut rebuilt),
                project_dir: root.to_path_buf(),
                modules: graph.clone(),
                plan: Some(plan.clone()),
                manifest: Some(built),
                variant: "fr".to_owned(),
            })
            .unwrap();
            assert!(!same.has_pending());
            assert_eq!(same.find("gone.bin").unwrap().status, FilesystemStatus::Vanilla);
            assert!(!same.find("z_new/fresh.bin").unwrap().id_is_planned);
        }

        // A project that has never been built has no manifest, so everything a build would place is pending.
        write_rom(&rom_path, "OLD", "OLD", false);
        let mut reopened = RomReader::open(&rom_path).unwrap();
        let unbuilt = build_filesystem_view(FilesystemViewRequest {
            rom: Some(&mut reopened),
            project_dir: root.to_path_buf(),
            modules: graph.clone(),
            plan: Some(plan.clone()),
            manifest: None,
            variant: "fr".to_owned(),
        })
        .unwrap();
        assert_eq!(unbuilt.find("shared.bin").unwrap().status, FilesystemStatus::Pending);
        assert_eq!(unbuilt.find("vanilla.bin").unwrap().status, FilesystemStatus::Vanilla);
        assert!(unbuilt.find("z_new/add.bin").unwrap().id_is_planned);

        // Without NCPatcher there is no plan, and the view reports the last build.
        let reported = build_filesystem_view(FilesystemViewRequest {
            rom: Some(&mut reopened),
            project_dir: root.to_path_buf(),
            modules: graph,
            plan: None,
            manifest: Some(manifest),
            variant: "fr".to_owned(),
        })
        .unwrap();
        assert_eq!(reported.find("shared.bin").unwrap().status, FilesystemStatus::Orphaned);
        assert_eq!(reported.find("vanilla.bin").unwrap().status, FilesystemStatus::Vanilla);

        // Watching is the one thing that still needs the module graph.
        assert!(view.watched_directories.contains(&second_dir));
        assert!(view.watched_directories.contains(&second_dir.join("fr/ARCHIVE/other_narc")));

        // Writing an edit back is the direction no build document answers.
        let mappings = [ModuleVariantMapping { module: "prefixed".to_owned(), layer: "french".to_owned() }];
        assert_eq!(
            module_file_target(&second, "fr", "ARCHIVE/menu.narc!member.bin", &[]).unwrap(),
            second_dir.join("fr/ARCHIVE/menu_narc/member.bin")
        );
        // The folder convention is `<stem>_<ext>` for whatever extension a game gives its archives.
        assert_eq!(
            module_file_target(&second, "fr", "ARCHIVE/menu.carc!member.bin", &[]).unwrap(),
            second_dir.join("fr/ARCHIVE/menu_carc/member.bin")
        );
        assert_eq!(module_file_target(&prefixed, "fr", "assets/picture.bin", &mappings).unwrap(), prefixed_dir.join("french/picture.bin"));
        assert!(module_file_target(&prefixed, "fr", "outside.bin", &mappings).is_err());
        assert!(module_file_target(&second, "fr", "bad//path.bin", &[]).is_err());
        assert!(module_file_target(&second, "fr", "ARCHIVE/menu!member.bin", &[]).is_err());
        assert_eq!(module_file_target(&second, "", "a.bin", &[]).unwrap(), second_dir.join("en/a.bin"));
        assert_eq!(filesystem_status_name(FilesystemStatus::Orphaned), "Orphaned");
    }
}
