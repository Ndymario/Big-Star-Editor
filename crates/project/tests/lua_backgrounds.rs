//! Lua background scripts: every shape one can take, how layers override each other, the check that every shipped
//! script loads, and the frames an animation is played out into.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic)]

use std::path::{Path, PathBuf};

use bigstar_level::{
    BACKGROUND_SLOT_COUNT, BackgroundLayer, BackgroundMotion, BackgroundPaletteAnimation, BackgroundTileAnimation, BackgroundTileCopy,
};
use bigstar_project::{
    BackgroundResourceFiles, LuaBackgroundLayer, ResourceFile, background_palette_animator, background_tile_animator, load_background_frames,
    load_lua_background_layers,
};

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn layer(name: &str, directory: PathBuf) -> LuaBackgroundLayer {
    LuaBackgroundLayer { name: name.to_owned(), directory }
}

#[test]
fn background_scripts_load_layer_and_report() {
    let root = std::env::temp_dir().join(format!("big-star-lua-background-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);

    // A script is data. Every shape it can take is here.
    write(
        &root.join("base/bg-000-hills.lua"),
        r#"
return {
    layer = "background",
    id = 0,
    name = "Hills",
    strips = {
        { throughRow = 279, drift = 0.0625 },
        { throughRow = 335, drift = 0.125, channel = 3 },
        { throughRow = 364, drift = -0.5, channel = 3 },
        { throughRow = 400, wave = { amplitude = 2.5, speed = 224, step = 1792 } },
        { scroll = 1 },
    },
}
"#,
    );
    write(
        &root.join("base/bg-001-castle.lua"),
        r#"
local frames = {}
for step = 0, 2 do frames[#frames + 1] = { step * 16, ticks = 10 + step } end
frames[#frames + 1] = 64
return {
    layer = "background",
    id = 1,
    name = "Castle",
    tiles = {
        file = "BG_ncg/anim.bin",
        interval = 5,
        frames = frames,
        copies = {
            { layer = "foreground", to = 14, count = 4 },
            { to = 46, count = 4, from = 16 },
        },
    },
    palette = { file = "BG_ncl/anim.bin", interval = 16, frames = 8, to = 80, count = 8, stride = 16 },
}
"#,
    );
    write(&root.join("base/fg-001-castle.lua"), r#"return { layer = "foreground", id = 1, name = "Castle front" }"#);
    // Mistakes name themselves and cost nothing else.
    write(&root.join("base/zz-broken.lua"), "return {");
    write(&root.join("base/zz-no-layer.lua"), r#"return { id = 2, name = "Nowhere" }"#);
    write(&root.join("base/zz-two-motions.lua"), r#"return { layer = "background", id = 3, name = "Both", strips = { { scroll = 1, drift = 1 } } }"#);
    write(
        &root.join("base/zz-no-interval.lua"),
        r#"return { layer = "background", id = 4, name = "Stuck", tiles = { file = "a", frames = { 0 }, copies = { { to = 0, count = 1 } } } }"#,
    );
    // A module renames one background and leaves the rest alone.
    write(&root.join("module/castle.lua"), r#"return { layer = "background", id = 1, name = "Haunted castle" }"#);

    let layers = [layer("module", root.join("module")), layer("missing", root.join("missing")), layer("base", root.join("base"))];
    let loaded = load_lua_background_layers(&layers).unwrap();
    assert_eq!(loaded.sources.len(), 8);
    assert_eq!(loaded.errors.len(), 4, "{:?}", loaded.errors);
    assert!(loaded.errors[0].starts_with("base / zz-broken.lua: "), "{:?}", loaded.errors);
    assert!(loaded.errors[2].contains("needs a layer"), "{:?}", loaded.errors);
    assert!(loaded.errors[3].contains("background 3 strip 1 needs exactly one of scroll, drift and wave"), "{:?}", loaded.errors);
    assert!(loaded.errors[1].contains("background 4 tiles frame 1 has no ticks"), "{:?}", loaded.errors);

    let catalog = &loaded.catalog;
    assert_eq!(catalog.definitions.len(), 3);
    // The module's castle is the one found, and it brought no animation, so the castle has none.
    let castle = catalog.find(BackgroundLayer::Background, 1).unwrap();
    assert_eq!(castle.name, "Haunted castle");
    assert!(castle.tiles.is_none() && !castle.animated());
    assert_eq!(catalog.name(BackgroundLayer::Foreground, 1), Some("Castle front"));

    let hills = catalog.find(BackgroundLayer::Background, 0).unwrap();
    assert!(hills.animated());
    let motions: Vec<(u16, BackgroundMotion)> = hills.strips.iter().map(|strip| (strip.through_row, strip.motion)).collect();
    assert_eq!(motions[1], (335, BackgroundMotion::Drift { speed: 512, channel: 3 }));
    assert_eq!(motions[2], (364, BackgroundMotion::Drift { speed: -2048, channel: 3 }));
    assert_eq!(motions[3], (400, BackgroundMotion::Wave { amplitude: 10240, speed: 224, step: 1792 }));
    assert_eq!(motions[4].1, BackgroundMotion::Scroll { factor: 4096 });
    // A band that names no channel moves on one of its own.
    let BackgroundMotion::Drift { speed: 256, channel } = motions[0].1 else { panic!("{:?}", motions[0]) };
    assert!(channel >= 128);

    // The shipped castle, read on its own, keeps everything it said.
    let alone = load_lua_background_layers(&[layer("base", root.join("base"))]).unwrap();
    let castle = alone.catalog.find(BackgroundLayer::Background, 1).unwrap();
    let tiles = castle.tiles.as_ref().unwrap();
    assert_eq!(tiles.steps.iter().map(|step| (step.tile, step.ticks)).collect::<Vec<_>>(), [(0, 10), (16, 11), (32, 12), (64, 5)]);
    assert_eq!(tiles.copies[0], BackgroundTileCopy { layer: BackgroundLayer::Foreground, to: 14, count: 4, from: 0 });
    assert_eq!(
        tiles.copies[1],
        BackgroundTileCopy { layer: BackgroundLayer::Background, to: 46, count: 4, from: 16 },
        "a copy lands in its own layer unless it says"
    );
    assert_eq!(castle.palette.as_ref().unwrap().stride, 16);

    let _ = std::fs::remove_dir_all(&root);
}

/// Every background and foreground the game's tables hold has a shipped script, so each has a name.
#[test]
fn every_shipped_background_script_loads() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/backgrounds");
    let loaded = load_lua_background_layers(&[layer("shipped", data)]).unwrap();
    assert!(loaded.errors.is_empty(), "{:?}", loaded.errors);
    for which in [BackgroundLayer::Background, BackgroundLayer::Foreground] {
        let ids: Vec<u16> = loaded.catalog.layer(which).iter().map(|definition| definition.id).collect();
        assert_eq!(ids, (0..BACKGROUND_SLOT_COUNT).collect::<Vec<_>>(), "{which:?}");
    }
    assert_eq!(loaded.catalog.name(BackgroundLayer::Background, 5), Some("Underwater"));
    assert_eq!(loaded.catalog.name(BackgroundLayer::Foreground, 8), Some("Grassland (Peach Castle)"));
    // The volcano's eruption: 22 and 16 short steps, then two long ones.
    let volcano = loaded.catalog.find(BackgroundLayer::Foreground, 15).unwrap().tiles.as_ref().unwrap();
    assert_eq!(volcano.steps.len(), 40);
    assert_eq!(volcano.steps.iter().map(|step| u32::from(step.ticks)).sum::<u32>(), 448);
    // The castle's background animates tiles of its foreground.
    let castle = loaded.catalog.find(BackgroundLayer::Background, 1).unwrap().tiles.as_ref().unwrap();
    assert!(castle.copies.iter().all(|copy| copy.layer == BackgroundLayer::Foreground));
}

fn colors(values: &[u16], length: usize) -> Vec<u8> {
    let mut data: Vec<u8> = values.iter().flat_map(|value| value.to_le_bytes()).collect();
    data.resize(length * 2, 0);
    data
}

#[test]
fn an_animation_is_played_out_into_frames() {
    const RED: u16 = 0x001f;
    const GREEN: u16 = 0x03e0;
    const BLUE: u16 = 0x7c00;
    const WHITE: u16 = 0x7fff;
    let file = |data: Vec<u8>| ResourceFile { rom_path: "x".into(), data };
    // One row of 64 tiles, every one tile 0, which is all colour 1: red.
    let files = BackgroundResourceFiles {
        graphics: file([vec![1u8; 64], vec![0u8; 64]].concat()),
        palette: file(colors(&[0, RED, GREEN, BLUE], 256)),
        screen: file(vec![0u8; 128]),
    };
    let pixel = |image: &bigstar_formats::RgbaImage| [image.pixels[0], image.pixels[1], image.pixels[2]];
    let is = |rgb: [u8; 3], red: bool, green: bool, blue: bool| (rgb[0] > 128, rgb[1] > 128, rgb[2] > 128) == (red, green, blue);

    // Nothing animating it: no frames, and the still is all there is.
    assert!(load_background_frames(&files, 0, 0, &[]).unwrap().frames.is_empty());

    // Two steps, a tile of colour 2 for two frames and one of colour 3 for three, written over tile 0.
    let animation = BackgroundTileAnimation {
        file: "anim".into(),
        steps: vec![bigstar_level::BackgroundTileStep { tile: 0, ticks: 2 }, bigstar_level::BackgroundTileStep { tile: 1, ticks: 3 }],
        copies: vec![BackgroundTileCopy { layer: BackgroundLayer::Background, to: 0, count: 1, from: 0 }],
    };
    let source = [vec![2u8; 64], vec![3u8; 64]].concat();
    assert!(background_tile_animator(&animation, BackgroundLayer::Foreground, &source).unwrap().is_none(), "it writes nothing in the foreground");
    let tiles = background_tile_animator(&animation, BackgroundLayer::Background, &source).unwrap().unwrap();
    let played = load_background_frames(&files, 0, 0, std::slice::from_ref(&tiles)).unwrap();
    assert_eq!(played.frames, [(0, 2), (1, 3)]);
    assert_eq!((played.images[0].width, played.images[0].height), (512, 8));
    assert!(is(pixel(&played.images[0]), false, true, false) && is(pixel(&played.images[1]), false, false, true));

    // Colour 2 turning white and back every five frames beside it: the two only line up again after ten.
    let cycle = BackgroundPaletteAnimation { file: "pal".into(), ticks: 5, frames: 2, to: 2, count: 1, stride: 2 };
    let palette = background_palette_animator(&cycle, &colors(&[WHITE, 0, GREEN, 0], 4)).unwrap();
    let played = load_background_frames(&files, 0, 0, &[tiles, palette]).unwrap();
    assert_eq!(played.frames.iter().map(|&(_, ticks)| ticks).collect::<Vec<_>>(), [2, 3, 2, 3]);
    let shown: Vec<[u8; 3]> = played.frames.iter().map(|&(image, _)| pixel(&played.images[image])).collect();
    assert!(
        is(shown[0], true, true, true) && is(shown[1], false, false, true) && is(shown[2], false, true, false) && is(shown[3], false, false, true)
    );
}
