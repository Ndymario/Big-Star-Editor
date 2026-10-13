//! Lua tile scripts. Port of `libs/nsmb-project/tests/lua_tiles_tests.cpp`, plus the check that every shipped script
//! loads.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result};
use bigstar_formats::RgbaImage;
use bigstar_project::{
    ActorResource, ActorResourceKind, LuaActorLayer, LuaTileLayer, TileOverlay, TileOverlayCatalog, TileOverlayScript, load_lua_actor_layers,
    load_lua_tile_layers,
};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// No script in these tests resolves a texture: building a valid TEX0 by hand would test the container parser rather
/// than the loader, and the loader's job is to report the failure per block and keep going.
fn no_resources(_: ActorResourceKind, name: &str) -> Result<ActorResource> {
    Err(Error::new(ErrorCode::NotFound, format!("missing {name}")))
}

fn layer(name: &str, directory: PathBuf) -> LuaTileLayer {
    LuaTileLayer { name: name.to_owned(), directory }
}

#[test]
fn tile_scripts_load_layer_and_filter() {
    let root = std::env::temp_dir().join(format!("big-star-lua-tile-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);

    // A script is data. Every shape it can take is here.
    write(
        &root.join("base/pipes.lua"),
        r#"
return {
    name = "test.pipes",
    blocks = {
        [171] = { "polygon_unit/I_dokan.nsbtx", "I_do_yoko_lu" },
        [172] = { "polygon_unit/I_dokan.nsbtx", "I_do_yoko_u", palette = 256 },
        [800] = {},
        [785] = { remap = 781 },
    },
}
"#,
    );
    write(
        &root.join("base/tower.lua"),
        r#"
return {
    name = "test.tower",
    replaces = false,
    tilesets = { 1, 0x2f },
    blocks = { [1456] = { "polygon_unit/I_toride_1.nsbtx", "I_toride_1" } },
}
"#,
    );

    let catalog = load_lua_tile_layers(&[layer("base", root.join("base"))], &no_resources).unwrap();
    assert_eq!(catalog.scripts.len(), 2);
    // Alphabetical within a layer, the way the actor loader orders scripts.
    assert_eq!(catalog.scripts[0].name, "test.pipes");
    assert_eq!(catalog.scripts[1].name, "test.tower");

    // Every entry shape survived, and the two that named a texture recorded the failure against their own block
    // rather than losing the script.
    let pipes = &catalog.scripts[0];
    assert_eq!(pipes.blocks.len(), 4);
    assert_eq!(pipes.errors.len(), 2);
    assert!(pipes.errors[0].contains("test.pipes block 171"), "{:?}", pipes.errors);

    let replaced = catalog.find(0, 171).unwrap();
    assert!(replaced.replaces && replaced.image.is_none() && replaced.remap.is_none());
    let blanked = catalog.find(0, 800).unwrap();
    assert!(blanked.replaces && blanked.image.is_none() && blanked.remap.is_none());
    assert_eq!(catalog.find(0, 785).unwrap().remap, Some(781));
    assert!(catalog.find(0, 999).is_none());

    // A tilesets list is a filter, not decoration.
    assert!(!catalog.find(1, 1456).unwrap().replaces);
    assert!(catalog.find(0x2f, 1456).is_some());
    assert!(catalog.find(2, 1456).is_none());
    assert!(catalog.block_count(0) == 4 && catalog.block_count(1) == 5);

    // A later layer overrides one block and inherits the rest.
    write(&root.join("module/override.lua"), r#"return { name = "module.pipe", blocks = { [171] = { remap = 42 } } }"#);
    let stacked = load_lua_tile_layers(&[layer("module", root.join("module")), layer("base", root.join("base"))], &no_resources).unwrap();
    assert_eq!(stacked.find(0, 171).unwrap().remap, Some(42));
    assert!(stacked.find(0, 172).is_some());

    // Bad scripts name themselves and take the load down, because a tile script that cannot be read is a mistake in
    // the file rather than a missing asset.
    for bad in [
        r#"return { name = "x", blocks = { [2000] = {} } }"#,
        r#"return { name = "x" }"#,
        r#"return { name = "x", tilesets = {}, blocks = {} }"#,
        r#"return { name = "x", blocks = { [1] = 5 } }"#,
        r#"return 7"#,
    ] {
        let _ = std::fs::remove_dir_all(root.join("bad"));
        write(&root.join("bad/bad.lua"), bad);
        let failed = load_lua_tile_layers(&[layer("bad", root.join("bad"))], &no_resources).unwrap_err();
        assert!(failed.message.contains("bad.lua"), "{}", failed.message);
    }

    // A missing layer is not an error: most projects ship no tile scripts.
    assert!(load_lua_tile_layers(&[layer("absent", root.join("nothing-here"))], &no_resources).unwrap().is_empty());

    // The sandbox is the actors' sandbox.
    write(&root.join("escape/escape.lua"), r#"return { name = "x", blocks = {}, danger = require("os") }"#);
    assert!(load_lua_tile_layers(&[layer("escape", root.join("escape"))], &no_resources).is_err());

    // What the catalog does to a Map16 sheet, built by hand so the four outcomes are checked without a TEX0.
    let texture = Arc::new(RgbaImage { width: 16, height: 16, pixels: vec![0x40; 16 * 16 * 4] });
    let mut script = TileOverlayScript { name: "test.apply".into(), ..TileOverlayScript::default() };
    script.blocks.insert(112, TileOverlay { image: Some(texture.clone()), remap: None, replaces: true });
    script.blocks.insert(113, TileOverlay { image: None, remap: None, replaces: true });
    script.blocks.insert(114, TileOverlay { image: Some(texture), remap: None, replaces: false });
    script.blocks.insert(115, TileOverlay { image: None, remap: Some(116), replaces: true });
    let applied = TileOverlayCatalog { scripts: vec![script] };
    assert!(applied.find(0, 112).is_some() && applied.block_count(0) == 4);

    let _ = std::fs::remove_dir_all(&root);
}

/// Every file in `data/actors` and `data/tiles` loads unmodified on the vendored Lua 5.4, which is what the C++ build
/// (linked against the system Lua) loaded too.
#[test]
fn every_shipped_script_loads() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data");
    let actors =
        load_lua_actor_layers(&[LuaActorLayer { name: "shipped".into(), directory: data.join("actors") }], &no_resources, None, None).unwrap();
    assert_eq!(actors.actors.len(), 305);
    for actor in &actors.actors {
        assert!(actor.init_error.is_empty(), "{}: {}", actor.source.display(), actor.init_error);
    }
    let tiles = load_lua_tile_layers(&[layer("shipped", data.join("tiles"))], &no_resources).unwrap();
    assert_eq!(tiles.scripts.len(), 8);
    // 176 blocks across the eight scripts (docs/lua-tiles.md); without a ROM only the textures fail to resolve.
    let blocks: usize = tiles.scripts.iter().map(|script| script.blocks.len()).sum();
    assert_eq!(blocks, 176);
}
