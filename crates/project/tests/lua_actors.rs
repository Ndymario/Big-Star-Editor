//! Lua actors: the loader, the renderer, the caches, and every shipped actor checked against what NSMB Editor draws.
//! Port of `libs/nsmb-project/tests/lua_actors_tests.cpp`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing, clippy::panic, unused_mut, clippy::bool_assert_comparison)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use bigstar_core::{Error, ErrorCode, Result, read_file};
use bigstar_formats::RgbaImage;
use bigstar_level::SpriteBounds;
use bigstar_project::{
    ActorResource, ActorResourceKind, LuaActorDefinition, LuaActorDrawCommand, LuaActorDrawKind, LuaActorLayer, LuaActorRender, LuaActorRenderInput,
    has_lua_renderer, load_lua_actor_layers, lua_actor_bounds, render_lua_actor,
};

/// The repository root, where `data/actors` and the C++ test fixtures live.
fn source_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The host reads the bytes now, so the test plays the host: an address, the file it came from, and what is in it.
fn resource_at(path: &Path, name: &str) -> Result<ActorResource> {
    let data = read_file(path).map_err(|_| Error::new(ErrorCode::NotFound, format!("missing {name}")))?;
    Ok(ActorResource { rom_path: name.to_owned(), source: path.to_path_buf(), data })
}

/// A 2x2 red PNG, so the injected decoder has something real to chew on.
const RED_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02,
    0x08, 0x02, 0x00, 0x00, 0x00, 0xfd, 0xd4, 0x9a, 0x73, 0x00, 0x00, 0x00, 0x19, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xc0,
    0xc0, 0xf0, 0x9f, 0x81, 0x81, 0x81, 0xe1, 0x3f, 0x00, 0x00, 0x2a, 0x0d, 0x04, 0x03, 0x2b, 0x2a, 0x66, 0xd8, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// The catalog carries no image codec of its own, so the test plays the host and answers with an opaque image of the
/// size the PNG header declares. Real dimensions matter: a port that reads a frame out of a strip has to be checked
/// against the strip it actually ships with.
fn decode_image(bytes: &[u8]) -> Result<RgbaImage> {
    if bytes.len() < 8 || &bytes[1..4] != b"PNG" {
        return Err(Error::new(ErrorCode::ParseError, "not a PNG"));
    }
    let (mut width, mut height) = (2usize, 2usize);
    if bytes.len() >= 24 && &bytes[12..16] == b"IHDR" {
        let read = |at: usize| u32::from_be_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        width = read(16);
        height = read(20);
    }
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(Error::new(ErrorCode::ParseError, "bad PNG size"));
    }
    Ok(RgbaImage { width, height, pixels: vec![0xff; width * height * 4] })
}

/// A render's outcome, read the way the C++ test reads a `Result`.
struct Rendered(Result<LuaActorRender>);

impl Rendered {
    fn ok(&self) -> bool {
        self.0.is_ok()
    }

    fn value(&self) -> &Vec<LuaActorDrawCommand> {
        &self.0.as_ref().unwrap().commands
    }
}

fn render(actor: &LuaActorDefinition, input: &LuaActorRenderInput) -> Rendered {
    Rendered(render_lua_actor(actor, input))
}

fn render_flag(actor: &LuaActorDefinition, input: &LuaActorRenderInput, custom_rendered: &mut bool) -> Rendered {
    let result = render_lua_actor(actor, input);
    *custom_rendered = result.as_ref().is_ok_and(|render| render.custom_rendered);
    Rendered(result)
}

struct Boxed(Result<Option<SpriteBounds>>);

impl Boxed {
    fn ok(&self) -> bool {
        self.0.is_ok()
    }

    fn value(&self) -> Option<&SpriteBounds> {
        self.0.as_ref().unwrap().as_ref()
    }
}

fn bounds(actor: &LuaActorDefinition, input: &LuaActorRenderInput, render_box: bool) -> Boxed {
    Boxed(lua_actor_bounds(actor, input, render_box))
}

/// One comparison for every type the C++ compares with `==`: numbers of any width as `f64`, and images by identity,
/// as `shared_ptr` equality does.
#[derive(Debug, PartialEq)]
enum Compared {
    Number(f64),
    Flag(bool),
    Text(String),
    Kind(LuaActorDrawKind),
    Image(Option<usize>),
}

trait Comparable {
    fn compared(&self) -> Compared;
}

macro_rules! numbers {
    ($($t:ty),*) => { $(impl Comparable for $t { fn compared(&self) -> Compared { Compared::Number(*self as f64) } })* };
}
numbers!(f64, i16, i32, i64, u8, u16, u32, u64, usize);

impl Comparable for bool {
    fn compared(&self) -> Compared {
        Compared::Flag(*self)
    }
}

impl Comparable for String {
    fn compared(&self) -> Compared {
        Compared::Text(self.clone())
    }
}

impl Comparable for &str {
    fn compared(&self) -> Compared {
        Compared::Text((*self).to_owned())
    }
}

impl Comparable for LuaActorDrawKind {
    fn compared(&self) -> Compared {
        Compared::Kind(*self)
    }
}

impl Comparable for Option<Arc<RgbaImage>> {
    fn compared(&self) -> Compared {
        Compared::Image(self.as_ref().map(|image| Arc::as_ptr(image) as usize))
    }
}

fn eq<A: Comparable + ?Sized, B: Comparable + ?Sized>(a: &A, b: &B) -> bool {
    a.compared() == b.compared()
}

fn write(path: &Path, text: impl AsRef<[u8]>) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn layer(name: &str, directory: PathBuf) -> LuaActorLayer {
    LuaActorLayer { name: name.to_owned(), directory }
}

#[test]
fn lua_actors() {
    let root = std::env::temp_dir().join(format!("big-star-lua-actor-tests-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let mut tiles = [0u8; 64];
    tiles[..32].fill(0x33);
    tiles[32..].fill(0x22);
    write(&root.join("rom/goomba.bin"), tiles);
    let mut palette = [0u8; 32];
    palette[2] = 0x1f; // red, followed by green
    palette[4] = 0xe0;
    palette[5] = 0x03;
    write(&root.join("rom/enemy.bin"), palette);
    write(&root.join("first/sign.png"), RED_PNG);
    write(
        &root.join("first/goomba.lua"),
        r#"local gfx = {}
local function init(ctx)
 local palette = ctx.loadPalette("enemy.bin", {colors=16})
 gfx.bitmap = ctx.loadBitmap("goomba.bin", {bpp=4, width=16, height=8, palette=palette})
 gfx.sign = ctx.loadImage("sign.png")
end
local function bounds(actor) return {x=actor.pixelX-8, y=actor.pixelY-8, width=16+(actor.fields.size==1 and 8 or 0), height=16} end
local function render(actor, renderer)
 renderer:drawTile(gfx.bitmap, actor.data[5]&1, {x=actor.pixelX+2, y=actor.pixelY-3, width=16, height=12, rotation=90, opacity=0.5, flipX=true})
 renderer:drawFrame(gfx.bitmap, 0, 8, 8, {x=actor.pixelX-8, y=actor.pixelY})
 renderer:draw(gfx.sign, {x=actor.pixelX, y=actor.pixelY})
 renderer:drawText(tostring(actor.fields.size), {x=actor.pixelX, y=actor.pixelY})
 return true
end
return {objectId=83, name="Goomba", category="Enemies / Ground", notes="Walks.",
 fields={{key="size", label="Size", notes="A field note.", bitOffset=40, bitWidth=8, valueOffset=1, signed=true,
 choices={{value=0,label="Small"},{value=1,label="Big"}}}, {key="big", label="Big", bitOffset=0, bitWidth=1, checkboxValue=1}},
 init=init, render=render, grabBounds=bounds, renderBounds=bounds}"#,
    );
    write(&root.join("first/custom.lua"), r#"return {hash=0x12345678, name="Module Actor"}"#);
    write(&root.join("last/duplicate.lua"), r#"return {objectId=83, name="Loses"}"#);
    let layers = vec![layer("first", root.join("first")), layer("last", root.join("last"))];
    let rom = root.join("rom");
    let resolve = |_: ActorResourceKind, name: &str| resource_at(&rom.join(name), name);
    let catalog = load_lua_actor_layers(&layers, &resolve, Some(&decode_image), None).unwrap();
    assert_eq!(catalog.actors.len(), 2);
    let goomba = catalog.by_object_id(83).unwrap();
    assert!(goomba.sprite.name == "Goomba" && goomba.init_error.is_empty(), "{}", goomba.init_error);
    assert!(goomba.sprite.fields.len() == 2 && goomba.sprite.fields[0].choices.len() == 2);
    assert_eq!(goomba.sprite.notes, "Walks.");
    assert_eq!(goomba.sprite.fields[0].notes, "A field note.");
    assert!(goomba.sprite.fields[0].bit_offset == 40 && goomba.sprite.fields[0].signed_value);
    assert!(has_lua_renderer(goomba));
    // The ROM graphics, its palette and the still beside the script.
    assert_eq!(goomba.render_resources.len(), 3);

    let mut input = LuaActorRenderInput::new();
    input.x = 10;
    input.y = 20;
    input.pixel_x = 160.0;
    input.pixel_y = 320.0;
    input.object_id = 83;
    input.r#type = 7;
    input.settings = 1;
    input.data[5] = 1;
    input.data[0] = 0; // the target event holds the "size" field, so it reads as 0 + 1
    let commands = render(goomba, &input);
    assert!(commands.ok() && commands.value().len() == 4, "{:?}", commands.0.as_ref().err());
    let c = commands.value();
    assert!(c[0].source_x == 8 && c[0].source_y == 0);
    assert!(c[0].x == 162.0 && c[0].y == 317.0);
    assert!(c[0].width == 16.0 && c[0].height == 12.0);
    assert!(c[0].rotation == 90.0 && c[0].opacity == 0.5 && c[0].flip_x);
    // Both ROM draws share one rasterized image; the still is its own.
    assert!(eq(&c[0].image, &c[1].image));
    assert!(c[2].image.as_ref().is_some_and(|image| image.width == 2));
    // drawText carries text rather than an image.
    assert!(c[3].text == "1" && c[3].image.is_none());

    // Bounds come back in scene pixels, and read the declared field.
    let boxed = bounds(goomba, &input, false);
    assert!(boxed.ok() && boxed.value().is_some_and(|b| b.x == 152 && b.y == 312));
    assert_eq!(boxed.value().unwrap().width, 24);
    // The cached definition bounds are the same box evaluated at the origin.
    assert!(goomba.sprite.grab_bounds.x == -8 && goomba.sprite.grab_bounds.y == -8);

    let commands_again = render(goomba, &input);
    assert!(commands_again.ok() && eq(&commands_again.value()[0].image, &commands.value()[0].image));

    // A repaint re-asks for every visible sprite, so a render is kept and replayed rather than re-run. The counter is
    // the only way to see it: it advances once per actual call into the script.
    write(
        &root.join("cache/counter.lua"),
        r#"local runs = 0
local function render(actor, renderer)
 runs = runs + 1
 renderer:drawText(tostring(runs), {x=actor.pixelX, y=actor.pixelY})
 return true
end
return {objectId=84, name="Counter", render=render}"#,
    );
    let cache_catalog = load_lua_actor_layers(&[layer("cache", root.join("cache"))], &resolve, Some(&decode_image), None).unwrap();
    let counter = cache_catalog.by_object_id(84).unwrap();
    assert!(has_lua_renderer(counter));
    let runs = |at: &LuaActorRenderInput| {
        let result = render(counter, at);
        assert!(result.ok() && result.value().len() == 1);
        result.value()[0].text.clone()
    };
    let mut counted = LuaActorRenderInput::new();
    counted.object_id = 84;
    counted.pixel_x = 32.0;
    assert_eq!(runs(&counted), "1");
    assert_eq!(runs(&counted), "1");
    // Settings, position and the event bytes all reach the script, so each one is part of the key.
    counted.settings = 4;
    assert_eq!(runs(&counted), "2");
    counted.pixel_x = 48.0;
    assert_eq!(runs(&counted), "3");
    counted.data[2] = 9;
    assert_eq!(runs(&counted), "4");
    // And going back to a key already seen replays it instead of running again.
    counted.data[2] = 0;
    counted.pixel_x = 32.0;
    counted.settings = 0;
    assert_eq!(runs(&counted), "1");
    // custom_rendered is part of what is kept, not recomputed as false.
    let mut counter_custom_rendered = false;
    let replayed = render_flag(counter, &counted, &mut counter_custom_rendered);
    assert!(replayed.ok() && counter_custom_rendered);

    // Bounds are kept the same way, and the paint path asks for them twice per sprite. The width counts calls, so a
    // repeat is visible in the answer.
    write(
        &root.join("boxes/shared.lua"),
        r#"local runs = 0
local function bounds(actor)
 runs = runs + 1
 return {x=actor.pixelX, y=actor.pixelY, width=runs, height=16}
end
return {objectId=85, name="Shared Box", grabBounds=bounds}"#,
    );
    write(
        &root.join("boxes/split.lua"),
        r#"local grabs, renders = 0, 0
local function grab(actor) grabs = grabs + 1 return {x=0, y=0, width=grabs, height=16} end
local function render(actor) renders = renders + 1 return {x=0, y=0, width=100+renders, height=16} end
return {objectId=86, name="Split Box", grabBounds=grab, renderBounds=render}"#,
    );
    let box_catalog = load_lua_actor_layers(&[layer("boxes", root.join("boxes"))], &resolve, Some(&decode_image), None).unwrap();
    let width_of = |actor: &LuaActorDefinition, at: &LuaActorRenderInput, render_box: bool| {
        let boxed = bounds(actor, at, render_box);
        assert!(boxed.ok() && boxed.value().is_some());
        boxed.value().unwrap().width
    };
    let mut boxed_input = LuaActorRenderInput::new();
    boxed_input.pixel_x = 64.0;
    // Loading probes both boxes at the origin to fill the definition, so the count starts above zero; what matters is
    // that it stops moving.
    let shared = box_catalog.by_object_id(85).unwrap();
    let first = width_of(shared, &boxed_input, false);
    assert_eq!(width_of(shared, &boxed_input, false), first);
    // An actor with no renderBounds answers both questions from grabBounds, and the key follows the function that
    // ran, so the render box hits too.
    assert_eq!(width_of(shared, &boxed_input, true), first);
    boxed_input.pixel_x = 96.0;
    assert_eq!(width_of(shared, &boxed_input, false), first + 1);

    // Two functions on one input are two entries, not one overwriting the other.
    let split = box_catalog.by_object_id(86).unwrap();
    let grab_width = width_of(split, &boxed_input, false);
    let render_width = width_of(split, &boxed_input, true);
    assert!(grab_width < 100 && render_width > 100);
    assert_eq!(width_of(split, &boxed_input, false), grab_width);
    assert_eq!(width_of(split, &boxed_input, true), render_width);
    // An actor that declares no bounds function grabs as the tile it sits on, which is where NSMBStageObj.cs starts
    // before its per-class adjustments.
    let definitions = catalog.sprite_definitions();
    let custom = definitions.by_hash(0x12345678).unwrap();
    assert!(custom.grab_bounds.x == 0 && custom.grab_bounds.y == 0);
    assert!(custom.grab_bounds.width == 16 && custom.grab_bounds.height == 16);
    assert!(custom.render_bounds.x == 0 && custom.render_bounds.y == 0);

    let shipped = vec![layer("shipped", source_dir().join("data/actors"))];
    // The shipped catalog draws from stills on disk, so the resolver points at the real ones: a script naming an
    // image that is not there must fail here.
    let shipped_resolve = |kind: ActorResourceKind, name: &str| -> Result<ActorResource> {
        if kind != ActorResourceKind::Image {
            return resolve(kind, name);
        }
        resource_at(&source_dir().join("data/actors").join(name), name)
    };
    let shipped_catalog = load_lua_actor_layers(&shipped, &shipped_resolve, Some(&decode_image), None).unwrap();
    assert_eq!(shipped_catalog.actors.len(), 305);

    // Actor 285 reverses its length nybbles to draw a loop guide, then leaves
    // the stock numbered marker visible over it.
    let mut loop_input = LuaActorRenderInput::new();
    loop_input.pixel_x = 160.0;
    loop_input.pixel_y = 320.0;
    loop_input.settings = 0x11 << 8;
    loop_input.data[4] = 0x11;
    let mut loop_custom_rendered = true;
    let mut loop_commands = render_flag(shipped_catalog.by_object_id(285).unwrap(), &loop_input, &mut loop_custom_rendered);
    assert!(loop_commands.ok() && eq(&loop_commands.value().len(), &4) && !loop_custom_rendered);
    assert!(eq(&loop_commands.value()[0].kind, &LuaActorDrawKind::FillRect));
    assert!(eq(&loop_commands.value()[0].x, &160) && eq(&loop_commands.value()[0].y, &326));
    assert!(eq(&loop_commands.value()[0].width, &32) && eq(&loop_commands.value()[0].height, &4));
    assert!(eq(&loop_commands.value()[1].x, &188) && eq(&loop_commands.value()[1].y, &320));
    assert!(eq(&loop_commands.value()[2].x, &161) && eq(&loop_commands.value()[2].y, &327));
    assert!(eq(&loop_commands.value()[3].x, &189) && eq(&loop_commands.value()[3].y, &321));

    // Actor 286 draws the wrong-path guide upward from its position and also
    // leaves the stock marker visible.
    let mut wrong_path_input = LuaActorRenderInput::new();
    wrong_path_input.pixel_x = 160.0;
    wrong_path_input.pixel_y = 320.0;
    wrong_path_input.settings = 3 << 4;
    wrong_path_input.data[5] = 0x30;
    let mut wrong_path_custom_rendered = true;
    let mut wrong_path_commands = render_flag(shipped_catalog.by_object_id(286).unwrap(), &wrong_path_input, &mut wrong_path_custom_rendered);
    assert!(wrong_path_commands.ok() && eq(&wrong_path_commands.value().len(), &4) && !wrong_path_custom_rendered);
    assert!(eq(&wrong_path_commands.value()[0].kind, &LuaActorDrawKind::FillRect));
    assert!(eq(&wrong_path_commands.value()[0].x, &166) && eq(&wrong_path_commands.value()[0].y, &288));
    assert!(eq(&wrong_path_commands.value()[0].width, &4) && eq(&wrong_path_commands.value()[0].height, &48));
    assert!(eq(&wrong_path_commands.value()[1].x, &160) && eq(&wrong_path_commands.value()[1].y, &288));
    assert!(eq(&wrong_path_commands.value()[2].x, &167) && eq(&wrong_path_commands.value()[2].y, &289));
    assert!(eq(&wrong_path_commands.value()[3].x, &161) && eq(&wrong_path_commands.value()[3].y, &289));

    // Actor 208 deliberately keeps a 48x48 selection box around a 49x49 still.
    let mut barrel_input = LuaActorRenderInput::new();
    barrel_input.pixel_x = 160.0;
    barrel_input.pixel_y = 320.0;
    let barrel = shipped_catalog.by_object_id(208).unwrap();
    let mut barrel_bounds = bounds(barrel, &barrel_input, false);
    let mut barrel_commands = render(barrel, &barrel_input);
    assert!(barrel_bounds.ok() && barrel_bounds.value().is_some());
    assert!(eq(&barrel_bounds.value().unwrap().x, &144) && eq(&barrel_bounds.value().unwrap().y, &312));
    assert!(eq(&barrel_bounds.value().unwrap().width, &48) && eq(&barrel_bounds.value().unwrap().height, &48));
    assert!(barrel_commands.ok() && eq(&barrel_commands.value().len(), &1));
    assert!(eq(&barrel_commands.value()[0].x, &144) && eq(&barrel_commands.value()[0].y, &312));
    assert!(eq(&barrel_commands.value()[0].width, &49) && eq(&barrel_commands.value()[0].height, &49));

    // The boss key also renders one pixel taller than its selection box.
    let boss_key = shipped_catalog.by_object_id(93).unwrap();
    let mut boss_key_bounds = bounds(boss_key, &barrel_input, false);
    let mut boss_key_commands = render(boss_key, &barrel_input);
    assert!(boss_key_bounds.ok() && boss_key_bounds.value().is_some());
    assert!(eq(&boss_key_bounds.value().unwrap().x, &140) && eq(&boss_key_bounds.value().unwrap().y, &305));
    assert!(eq(&boss_key_bounds.value().unwrap().width, &25) && eq(&boss_key_bounds.value().unwrap().height, &31));
    assert!(boss_key_commands.ok() && eq(&boss_key_commands.value().len(), &1));
    assert!(eq(&boss_key_commands.value()[0].x, &140) && eq(&boss_key_commands.value()[0].y, &305));
    assert!(eq(&boss_key_commands.value()[0].width, &25) && eq(&boss_key_commands.value()[0].height, &32));

    // Mini Goomba's half-block shifts apply to both geometries, while its
    // selection box remains one pixel smaller than the still.
    let mut mini_goomba_input = barrel_input.clone();
    mini_goomba_input.settings = 0x11;
    let mini_goomba = shipped_catalog.by_object_id(85).unwrap();
    let mut mini_goomba_bounds = bounds(mini_goomba, &mini_goomba_input, false);
    let mut mini_goomba_commands = render(mini_goomba, &mini_goomba_input);
    assert!(mini_goomba_bounds.ok() && mini_goomba_bounds.value().is_some());
    assert!(eq(&mini_goomba_bounds.value().unwrap().x, &168) && eq(&mini_goomba_bounds.value().unwrap().y, &328));
    assert!(eq(&mini_goomba_bounds.value().unwrap().width, &8) && eq(&mini_goomba_bounds.value().unwrap().height, &8));
    assert!(mini_goomba_commands.ok() && eq(&mini_goomba_commands.value().len(), &1));
    assert!(eq(&mini_goomba_commands.value()[0].x, &168) && eq(&mini_goomba_commands.value()[0].y, &328));
    assert!(eq(&mini_goomba_commands.value()[0].width, &9) && eq(&mini_goomba_commands.value()[0].height, &9));

    // Red Climbing Koopa's still is one pixel narrower than its shared
    // selection box.
    let mut climbing_koopa_input = barrel_input.clone();
    climbing_koopa_input.settings = 1;
    let climbing_koopa = shipped_catalog.by_object_id(96).unwrap();
    let mut climbing_koopa_bounds = bounds(climbing_koopa, &climbing_koopa_input, false);
    let mut climbing_koopa_commands = render(climbing_koopa, &climbing_koopa_input);
    assert!(climbing_koopa_bounds.ok() && climbing_koopa_bounds.value().is_some());
    assert!(eq(&climbing_koopa_bounds.value().unwrap().width, &18) && eq(&climbing_koopa_bounds.value().unwrap().height, &26));
    assert!(climbing_koopa_commands.ok() && eq(&climbing_koopa_commands.value().len(), &1));
    assert!(eq(&climbing_koopa_commands.value()[0].x, &160) && eq(&climbing_koopa_commands.value()[0].y, &320));
    assert!(eq(&climbing_koopa_commands.value()[0].width, &17) && eq(&climbing_koopa_commands.value()[0].height, &26));

    // Amp's image dimensions are intentionally transposed relative to its
    // selection box, but both share the field-controlled origin.
    let mut amp_input = barrel_input.clone();
    amp_input.settings = 0x30;
    let amp = shipped_catalog.by_object_id(110).unwrap();
    let mut amp_bounds = bounds(amp, &amp_input, false);
    let mut amp_commands = render(amp, &amp_input);
    assert!(amp_bounds.ok() && amp_bounds.value().is_some());
    assert!(eq(&amp_bounds.value().unwrap().x, &162) && eq(&amp_bounds.value().unwrap().y, &322));
    assert!(eq(&amp_bounds.value().unwrap().width, &27) && eq(&amp_bounds.value().unwrap().height, &28));
    assert!(amp_commands.ok() && eq(&amp_commands.value().len(), &1));
    assert!(eq(&amp_commands.value()[0].x, &162) && eq(&amp_commands.value()[0].y, &322));
    assert!(eq(&amp_commands.value()[0].width, &28) && eq(&amp_commands.value()[0].height, &27));

    // Wiggler's bounds use the low-nibble size flag, while its image selection
    // also requires the neighboring nibble to be zero.
    let mut wiggler_input = barrel_input.clone();
    wiggler_input.settings = 1;
    let wiggler = shipped_catalog.by_object_id(111).unwrap();
    let mut wiggler_bounds = bounds(wiggler, &wiggler_input, false);
    let mut wiggler_commands = render(wiggler, &wiggler_input);
    assert!(wiggler_bounds.ok() && wiggler_bounds.value().is_some());
    assert!(eq(&wiggler_bounds.value().unwrap().y, &276) && eq(&wiggler_bounds.value().unwrap().width, &92));
    assert!(wiggler_commands.ok() && eq(&wiggler_commands.value().len(), &1));
    assert!(eq(&wiggler_commands.value()[0].y, &276) && eq(&wiggler_commands.value()[0].width, &92));

    wiggler_input.settings = 0x11;
    wiggler_bounds = bounds(wiggler, &wiggler_input, false);
    wiggler_commands = render(wiggler, &wiggler_input);
    assert!(wiggler_bounds.ok() && wiggler_bounds.value().is_some());
    assert!(eq(&wiggler_bounds.value().unwrap().y, &276) && eq(&wiggler_bounds.value().unwrap().width, &92));
    assert!(wiggler_commands.ok() && eq(&wiggler_commands.value().len(), &1));
    assert!(eq(&wiggler_commands.value()[0].y, &306) && eq(&wiggler_commands.value()[0].width, &52));

    // Bowser Jr. behaviours zero and three share the masked still; normal
    // modes select the other resource.
    let mut bowser_jr_input = barrel_input.clone();
    let bowser_jr = shipped_catalog.by_object_id(113).unwrap();
    let mut bowser_jr_masked = render(bowser_jr, &bowser_jr_input);
    bowser_jr_input.settings = 0x30;
    let mut bowser_jr_masked_jumping = render(bowser_jr, &bowser_jr_input);
    bowser_jr_input.settings = 0x10;
    let mut bowser_jr_normal = render(bowser_jr, &bowser_jr_input);
    assert!(bowser_jr_masked.ok() && eq(&bowser_jr_masked.value().len(), &1));
    assert!(bowser_jr_masked_jumping.ok() && eq(&bowser_jr_masked_jumping.value().len(), &1));
    assert!(bowser_jr_normal.ok() && eq(&bowser_jr_normal.value().len(), &1));
    assert!(eq(&bowser_jr_masked.value()[0].image, &bowser_jr_masked_jumping.value()[0].image));
    assert!(!eq(&bowser_jr_masked.value()[0].image, &bowser_jr_normal.value()[0].image));

    // Swooper's size is only the low nibble; its independent spawn-shift
    // nibble must not change the preview selection.
    let swooper = shipped_catalog.by_object_id(90).unwrap();
    let mut swooper_input = barrel_input.clone();
    swooper_input.settings = 0x21;
    let mut swooper_bounds = bounds(swooper, &swooper_input, false);
    let mut swooper_commands = render(swooper, &swooper_input);
    assert!(swooper_bounds.ok() && swooper_bounds.value().is_some());
    assert!(eq(&swooper_bounds.value().unwrap().x, &154) && eq(&swooper_bounds.value().unwrap().y, &304));
    assert!(eq(&swooper_bounds.value().unwrap().width, &26) && eq(&swooper_bounds.value().unwrap().height, &37));
    assert!(swooper_commands.ok() && eq(&swooper_commands.value()[0].width, &26));

    swooper_input.settings = 0x02;
    swooper_bounds = bounds(swooper, &swooper_input, false);
    swooper_commands = render(swooper, &swooper_input);
    assert!(swooper_bounds.ok() && swooper_bounds.value().is_some());
    assert!(eq(&swooper_bounds.value().unwrap().x, &160) && eq(&swooper_bounds.value().unwrap().y, &320));
    assert!(eq(&swooper_bounds.value().unwrap().width, &16) && eq(&swooper_bounds.value().unwrap().height, &18));
    assert!(swooper_commands.ok() && eq(&swooper_commands.value()[0].width, &16));

    // Empty-shell colors two and three both use the blue shell, while color
    // zero remains green.
    let koopa = shipped_catalog.by_object_id(94).unwrap();
    let mut koopa_input = barrel_input.clone();
    koopa_input.settings = 0x10;
    let mut green_shell = render(koopa, &koopa_input);
    koopa_input.settings = 0x12;
    let mut blue_shell = render(koopa, &koopa_input);
    koopa_input.settings = 0x13;
    let mut blue_ledge_shell = render(koopa, &koopa_input);
    let mut blue_ledge_bounds = bounds(koopa, &koopa_input, false);
    assert!(green_shell.ok() && blue_shell.ok() && blue_ledge_shell.ok());
    assert!(!eq(&green_shell.value()[0].image, &blue_shell.value()[0].image));
    assert!(eq(&blue_shell.value()[0].image, &blue_ledge_shell.value()[0].image));
    assert!(blue_ledge_bounds.ok() && blue_ledge_bounds.value().is_some());
    assert!(eq(&blue_ledge_bounds.value().unwrap().x, &160) && eq(&blue_ledge_bounds.value().unwrap().y, &320));
    assert!(eq(&blue_ledge_bounds.value().unwrap().width, &16) && eq(&blue_ledge_bounds.value().unwrap().height, &16));

    // Paratroopa color three intentionally falls back to green despite its
    // metadata label; only color two selects the blue still.
    let paratroopa = shipped_catalog.by_object_id(95).unwrap();
    let mut paratroopa_input = barrel_input.clone();
    let mut green_paratroopa = render(paratroopa, &paratroopa_input);
    paratroopa_input.settings = 2;
    let mut blue_paratroopa = render(paratroopa, &paratroopa_input);
    paratroopa_input.settings = 3;
    let mut ledge_paratroopa = render(paratroopa, &paratroopa_input);
    assert!(green_paratroopa.ok() && blue_paratroopa.ok() && ledge_paratroopa.ok());
    assert!(!eq(&green_paratroopa.value()[0].image, &blue_paratroopa.value()[0].image));
    assert!(eq(&green_paratroopa.value()[0].image, &ledge_paratroopa.value()[0].image));

    // Toadsworth's selection excludes the distant house preview that follows
    // him in draw order.
    let toadsworth = shipped_catalog.by_object_id(134).unwrap();
    let mut toadsworth_input = barrel_input.clone();
    let mut toadsworth_bounds = bounds(toadsworth, &toadsworth_input, false);
    let mut toadsworth_commands = render(toadsworth, &toadsworth_input);
    assert!(toadsworth_bounds.ok() && toadsworth_bounds.value().is_some());
    assert!(eq(&toadsworth_bounds.value().unwrap().x, &160) && eq(&toadsworth_bounds.value().unwrap().y, &305));
    assert!(eq(&toadsworth_bounds.value().unwrap().width, &20) && eq(&toadsworth_bounds.value().unwrap().height, &31));
    assert!(toadsworth_commands.ok() && eq(&toadsworth_commands.value().len(), &2));
    assert!(eq(&toadsworth_commands.value()[0].x, &160) && eq(&toadsworth_commands.value()[0].y, &305));
    assert!(eq(&toadsworth_commands.value()[1].x, &-28) && eq(&toadsworth_commands.value()[1].y, &232));
    assert!(eq(&toadsworth_commands.value()[1].width, &184) && eq(&toadsworth_commands.value()[1].height, &31));

    toadsworth_input.settings = 2;
    toadsworth_commands = render(toadsworth, &toadsworth_input);
    assert!(toadsworth_commands.ok() && eq(&toadsworth_commands.value().len(), &2));
    assert!(eq(&toadsworth_commands.value()[1].x, &44) && eq(&toadsworth_commands.value()[1].y, &244));
    assert!(eq(&toadsworth_commands.value()[1].width, &40) && eq(&toadsworth_commands.value()[1].height, &40));
    toadsworth_input.settings = 1;
    toadsworth_commands = render(toadsworth, &toadsworth_input);
    assert!(toadsworth_commands.ok() && eq(&toadsworth_commands.value().len(), &1));

    // Dorrie size reads only two bits, and left-facing previews reuse the side
    // geometry with a horizontal flip.
    let dorrie = shipped_catalog.by_object_id(79).unwrap();
    let mut dorrie_input = barrel_input.clone();
    dorrie_input.settings = 0x01000000;
    let mut dorrie_bounds = bounds(dorrie, &dorrie_input, false);
    let mut dorrie_commands = render(dorrie, &dorrie_input);
    assert!(dorrie_bounds.ok() && dorrie_bounds.value().is_some());
    assert!(eq(&dorrie_bounds.value().unwrap().x, &109) && eq(&dorrie_bounds.value().unwrap().y, &261));
    assert!(eq(&dorrie_bounds.value().unwrap().width, &115) && eq(&dorrie_bounds.value().unwrap().height, &76));
    assert!(dorrie_commands.ok() && dorrie_commands.value()[0].flip_x);

    dorrie_input.settings = 0x03f00000;
    dorrie_bounds = bounds(dorrie, &dorrie_input, false);
    dorrie_commands = render(dorrie, &dorrie_input);
    assert!(dorrie_bounds.ok() && dorrie_bounds.value().is_some());
    assert!(eq(&dorrie_bounds.value().unwrap().x, &61) && eq(&dorrie_bounds.value().unwrap().y, &208));
    assert!(eq(&dorrie_bounds.value().unwrap().width, &202) && eq(&dorrie_bounds.value().unwrap().height, &181));
    assert!(dorrie_commands.ok() && !dorrie_commands.value()[0].flip_x);

    // Jumping Cheep Cheep keeps a wide, short selection around its natural
    // narrow, tall still.
    let jumping_cheep = shipped_catalog.by_object_id(136).unwrap();
    let mut jumping_cheep_bounds = bounds(jumping_cheep, &barrel_input, false);
    let mut jumping_cheep_commands = render(jumping_cheep, &barrel_input);
    assert!(jumping_cheep_bounds.ok() && jumping_cheep_bounds.value().is_some());
    assert!(eq(&jumping_cheep_bounds.value().unwrap().x, &160) && eq(&jumping_cheep_bounds.value().unwrap().y, &320));
    assert!(eq(&jumping_cheep_bounds.value().unwrap().width, &21) && eq(&jumping_cheep_bounds.value().unwrap().height, &16));
    assert!(jumping_cheep_commands.ok() && eq(&jumping_cheep_commands.value().len(), &1));
    assert!(eq(&jumping_cheep_commands.value()[0].x, &160) && eq(&jumping_cheep_commands.value()[0].y, &320));
    assert!(eq(&jumping_cheep_commands.value()[0].width, &19) && eq(&jumping_cheep_commands.value()[0].height, &18));

    // Wobbly lift bounds cover only the platform, while six chain links extend
    // upward in draw order.
    let wobbly_lift = shipped_catalog.by_object_id(169).unwrap();
    let mut wobbly_lift_bounds = bounds(wobbly_lift, &barrel_input, false);
    let mut wobbly_lift_commands = render(wobbly_lift, &barrel_input);
    assert!(wobbly_lift_bounds.ok() && wobbly_lift_bounds.value().is_some());
    assert!(eq(&wobbly_lift_bounds.value().unwrap().x, &160) && eq(&wobbly_lift_bounds.value().unwrap().y, &320));
    assert!(eq(&wobbly_lift_bounds.value().unwrap().width, &192) && eq(&wobbly_lift_bounds.value().unwrap().height, &32));
    assert!(wobbly_lift_commands.ok() && eq(&wobbly_lift_commands.value().len(), &7));
    assert!(eq(&wobbly_lift_commands.value()[0].x, &160) && eq(&wobbly_lift_commands.value()[0].y, &320));
    for link in 1..=6 {
        assert!(eq(&wobbly_lift_commands.value()[link].x, &252));
        assert!(eq(&wobbly_lift_commands.value()[link].y, &(320.0 - ((32 * link) as f64))));
        assert!(eq(&wobbly_lift_commands.value()[link].width, &8));
        assert!(eq(&wobbly_lift_commands.value()[link].height, &32));
    }

    // Spike Top separates visual orientation from travel direction and falls
    // back to the upward still for undocumented orientations.
    let spike_top = shipped_catalog.by_object_id(147).unwrap();
    let spike_x: [f64; 5] = [160.0, 156.0, 160.0, 160.0, 160.0];
    let spike_y: [f64; 5] = [316.0, 320.0, 320.0, 320.0, 316.0];
    let spike_width: [f64; 5] = [16.0, 20.0, 16.0, 20.0, 16.0];
    let spike_height: [f64; 5] = [20.0, 16.0, 20.0, 16.0, 20.0];
    let mut spike_images: Vec<Option<Arc<RgbaImage>>> = vec![None; 5];
    for direction in 0..5 {
        let mut spike_top_input = barrel_input.clone();
        spike_top_input.settings = direction << 4;
        let mut spike_top_bounds = bounds(spike_top, &spike_top_input, false);
        let mut spike_top_commands = render(spike_top, &spike_top_input);
        assert!(spike_top_bounds.ok() && spike_top_bounds.value().is_some());
        assert!(eq(&spike_top_bounds.value().unwrap().x, &spike_x[direction as usize]));
        assert!(eq(&spike_top_bounds.value().unwrap().y, &spike_y[direction as usize]));
        assert!(eq(&spike_top_bounds.value().unwrap().width, &spike_width[direction as usize]));
        assert!(eq(&spike_top_bounds.value().unwrap().height, &spike_height[direction as usize]));
        assert!(spike_top_commands.ok() && eq(&spike_top_commands.value().len(), &1));
        spike_images[direction as usize] = spike_top_commands.value()[0].image.clone();
    }
    assert!(!eq(&spike_images[0], &spike_images[1]) && !eq(&spike_images[1], &spike_images[2]));
    assert!(!eq(&spike_images[2], &spike_images[3]) && eq(&spike_images[4], &spike_images[0]));
    let mut spike_top_reverse_input = barrel_input.clone();
    spike_top_reverse_input.settings = 1;
    let mut spike_top_reverse = render(spike_top, &spike_top_reverse_input);
    assert!(spike_top_reverse.ok() && eq(&spike_top_reverse.value()[0].image, &spike_images[0]));

    // Scale-lift bounds cover asymmetric ropes, bolts, and both platforms.
    let mut scale_lift_input = barrel_input.clone();
    scale_lift_input.settings = 0x143;
    let scale_lift = shipped_catalog.by_object_id(172).unwrap();
    let mut scale_lift_bounds = bounds(scale_lift, &scale_lift_input, false);
    let mut scale_lift_commands = render(scale_lift, &scale_lift_input);
    assert!(scale_lift_bounds.ok() && scale_lift_bounds.value().is_some());
    assert!(eq(&scale_lift_bounds.value().unwrap().x, &136) && eq(&scale_lift_bounds.value().unwrap().y, &310));
    assert!(eq(&scale_lift_bounds.value().unwrap().width, &96) && eq(&scale_lift_bounds.value().unwrap().height, &82));
    assert!(scale_lift_commands.ok() && eq(&scale_lift_commands.value().len(), &10));
    assert!(eq(&scale_lift_commands.value()[0].x, &158) && eq(&scale_lift_commands.value()[0].y, &310));
    assert!(eq(&scale_lift_commands.value()[1].x, &198) && eq(&scale_lift_commands.value()[1].y, &310));
    assert!(eq(&scale_lift_commands.value()[2].x, &168) && eq(&scale_lift_commands.value()[2].y, &311));
    assert!(eq(&scale_lift_commands.value()[2].x2, &200) && eq(&scale_lift_commands.value()[2].y2, &311));
    assert!(eq(&scale_lift_commands.value()[3].x, &169) && eq(&scale_lift_commands.value()[3].x2, &199));
    assert!(eq(&scale_lift_commands.value()[4].x, &159) && eq(&scale_lift_commands.value()[4].y2, &327));
    assert!(eq(&scale_lift_commands.value()[5].x, &160) && eq(&scale_lift_commands.value()[5].y2, &327));
    assert!(eq(&scale_lift_commands.value()[6].x, &207) && eq(&scale_lift_commands.value()[6].y2, &375));
    assert!(eq(&scale_lift_commands.value()[7].x, &208) && eq(&scale_lift_commands.value()[7].y2, &375));
    assert!(eq(&scale_lift_commands.value()[8].x, &184) && eq(&scale_lift_commands.value()[8].y, &376));
    assert!(eq(&scale_lift_commands.value()[9].x, &136) && eq(&scale_lift_commands.value()[9].y, &328));

    // Ball 'n' Chain keeps native 96x39 draw geometry while quarter-turn
    // bounds are 39x96, including gfx.blit's half-pixel center compensation.
    let ball_chain = shipped_catalog.by_object_id(130).unwrap();
    let ball_bounds_x: [f64; 4] = [164.0, 149.0, 77.0, 149.0];
    let ball_bounds_y: [f64; 4] = [309.0, 237.0, 309.0, 324.0];
    let ball_draw_x: [f64; 4] = [164.0, 120.5, 77.0, 120.5];
    let ball_draw_y: [f64; 4] = [309.0, 265.5, 309.0, 352.5];
    let ball_rotation: [f64; 4] = [0.0, 270.0, 180.0, 90.0];
    for position in 0..4 {
        let mut ball_chain_input = barrel_input.clone();
        ball_chain_input.settings = position;
        let mut ball_chain_bounds = bounds(ball_chain, &ball_chain_input, false);
        let mut ball_chain_commands = render(ball_chain, &ball_chain_input);
        assert!(ball_chain_bounds.ok() && ball_chain_bounds.value().is_some());
        assert!(eq(&ball_chain_bounds.value().unwrap().x, &ball_bounds_x[position as usize]));
        assert!(eq(&ball_chain_bounds.value().unwrap().y, &ball_bounds_y[position as usize]));
        assert!(ball_chain_commands.ok() && eq(&ball_chain_commands.value().len(), &2));
        assert!(eq(&ball_chain_commands.value()[0].x, &ball_draw_x[position as usize]));
        assert!(eq(&ball_chain_commands.value()[0].y, &ball_draw_y[position as usize]));
        assert!(eq(&ball_chain_commands.value()[0].width, &96) && eq(&ball_chain_commands.value()[0].height, &39));
        assert!(eq(&ball_chain_commands.value()[0].rotation, &ball_rotation[position as usize]));
        assert!(eq(&ball_chain_commands.value()[1].x, &152) && eq(&ball_chain_commands.value()[1].y, &312));
        assert!(ball_chain_commands.value()[1].flip_x);
    }
    let mut ball_chain_mask_input = barrel_input.clone();
    ball_chain_mask_input.settings = 0xff;
    let mut ball_chain_mask_commands = render(ball_chain, &ball_chain_mask_input);
    assert!(ball_chain_mask_commands.ok() && eq(&ball_chain_mask_commands.value()[0].rotation, &90));
    assert!(!ball_chain_mask_commands.value()[1].flip_x);

    // Vertical moving-lift length uses only three bits; raw low nibble F still
    // produces eight tiles and a vertical ghost target.
    let mut vertical_lift_input = barrel_input.clone();
    vertical_lift_input.settings = 0x20f;
    let vertical_lift = shipped_catalog.by_object_id(161).unwrap();
    let mut vertical_lift_bounds = bounds(vertical_lift, &vertical_lift_input, false);
    let mut vertical_lift_commands = render(vertical_lift, &vertical_lift_input);
    assert!(vertical_lift_bounds.ok() && vertical_lift_bounds.value().is_some());
    assert!(eq(&vertical_lift_bounds.value().unwrap().x, &160) && eq(&vertical_lift_bounds.value().unwrap().y, &320));
    assert!(eq(&vertical_lift_bounds.value().unwrap().width, &128) && eq(&vertical_lift_bounds.value().unwrap().height, &16));
    assert!(vertical_lift_commands.ok() && eq(&vertical_lift_commands.value().len(), &18));
    assert!(eq(&vertical_lift_commands.value()[0].kind, &LuaActorDrawKind::Line));
    assert!(eq(&vertical_lift_commands.value()[0].x, &224) && eq(&vertical_lift_commands.value()[0].y, &328));
    assert!(eq(&vertical_lift_commands.value()[0].x2, &224) && eq(&vertical_lift_commands.value()[0].y2, &296));
    assert!(eq(&vertical_lift_commands.value()[1].x, &220) && eq(&vertical_lift_commands.value()[1].y, &292));
    assert!(eq(&vertical_lift_commands.value()[2].x, &160) && eq(&vertical_lift_commands.value()[2].y, &320));
    assert!(eq(&vertical_lift_commands.value()[9].x, &272) && eq(&vertical_lift_commands.value()[9].y, &320));
    assert!(eq(&vertical_lift_commands.value()[10].x, &160) && eq(&vertical_lift_commands.value()[10].y, &288));
    assert!(eq(&vertical_lift_commands.value()[17].x, &272) && eq(&vertical_lift_commands.value()[17].y, &288));

    // Horizontal moving lift uses the offset length field directly and draws
    // a matching ghost platform at its target.
    let mut horizontal_lift_input = barrel_input.clone();
    horizontal_lift_input.settings = 0x203;
    let horizontal_lift = shipped_catalog.by_object_id(162).unwrap();
    let mut horizontal_lift_bounds = bounds(horizontal_lift, &horizontal_lift_input, false);
    let mut horizontal_lift_commands = render(horizontal_lift, &horizontal_lift_input);
    assert!(horizontal_lift_bounds.ok() && horizontal_lift_bounds.value().is_some());
    assert!(eq(&horizontal_lift_bounds.value().unwrap().width, &64) && eq(&horizontal_lift_bounds.value().unwrap().height, &16));
    assert!(horizontal_lift_commands.ok() && eq(&horizontal_lift_commands.value().len(), &10));
    assert!(eq(&horizontal_lift_commands.value()[0].x, &192) && eq(&horizontal_lift_commands.value()[0].x2, &224));
    assert!(eq(&horizontal_lift_commands.value()[0].y, &328) && eq(&horizontal_lift_commands.value()[0].y2, &328));
    assert!(eq(&horizontal_lift_commands.value()[1].x, &220) && eq(&horizontal_lift_commands.value()[1].y, &324));
    assert!(eq(&horizontal_lift_commands.value()[2].x, &160) && eq(&horizontal_lift_commands.value()[5].x, &208));
    assert!(eq(&horizontal_lift_commands.value()[6].x, &192) && eq(&horizontal_lift_commands.value()[9].x, &240));

    let unused_horizontal_lift = shipped_catalog.by_object_id(163).unwrap();
    let mut unused_horizontal_bounds = bounds(unused_horizontal_lift, &horizontal_lift_input, false);
    let mut unused_horizontal_commands = render(unused_horizontal_lift, &horizontal_lift_input);
    assert!(unused_horizontal_bounds.ok() && unused_horizontal_bounds.value().is_some());
    assert!(eq(&unused_horizontal_bounds.value().unwrap().width, &horizontal_lift_bounds.value().unwrap().width));
    assert!(unused_horizontal_commands.ok() && eq(&unused_horizontal_commands.value().len(), &10));
    for command in 0..unused_horizontal_commands.value().len() {
        assert!(eq(&unused_horizontal_commands.value()[command].x, &horizontal_lift_commands.value()[command].x));
        assert!(eq(&unused_horizontal_commands.value()[command].y, &horizontal_lift_commands.value()[command].y));
    }

    // The vertical stone wall uses three length bits and two height bits. Its
    // render order walks the perimeter clockwise before filling the interior.
    let vertical_stone_wall = shipped_catalog.by_object_id(165).unwrap();
    let mut flat_stone_wall_bounds = bounds(vertical_stone_wall, &barrel_input, false);
    let mut flat_stone_wall_commands = render(vertical_stone_wall, &barrel_input);
    assert!(flat_stone_wall_bounds.ok() && flat_stone_wall_bounds.value().is_some());
    assert!(eq(&flat_stone_wall_bounds.value().unwrap().width, &32) && eq(&flat_stone_wall_bounds.value().unwrap().height, &16));
    assert!(flat_stone_wall_commands.ok() && eq(&flat_stone_wall_commands.value().len(), &2));
    assert!(eq(&flat_stone_wall_commands.value()[0].x, &160) && eq(&flat_stone_wall_commands.value()[1].x, &176));

    let mut tall_stone_wall_input = barrel_input.clone();
    tall_stone_wall_input.settings = 0xf00f;
    let mut tall_stone_wall_bounds = bounds(vertical_stone_wall, &tall_stone_wall_input, false);
    let mut tall_stone_wall_commands = render(vertical_stone_wall, &tall_stone_wall_input);
    assert!(tall_stone_wall_bounds.ok() && tall_stone_wall_bounds.value().is_some());
    assert!(eq(&tall_stone_wall_bounds.value().unwrap().width, &128) && eq(&tall_stone_wall_bounds.value().unwrap().height, &128));
    assert!(tall_stone_wall_commands.ok() && eq(&tall_stone_wall_commands.value().len(), &64));
    assert!(eq(&tall_stone_wall_commands.value()[0].x, &160) && eq(&tall_stone_wall_commands.value()[0].y, &320));
    assert!(eq(&tall_stone_wall_commands.value()[7].x, &272) && eq(&tall_stone_wall_commands.value()[7].y, &320));
    assert!(eq(&tall_stone_wall_commands.value()[14].x, &272) && eq(&tall_stone_wall_commands.value()[14].y, &432));
    assert!(eq(&tall_stone_wall_commands.value()[21].x, &160) && eq(&tall_stone_wall_commands.value()[21].y, &432));
    assert!(eq(&tall_stone_wall_commands.value()[28].x, &176) && eq(&tall_stone_wall_commands.value()[28].y, &336));
    assert!(!eq(&tall_stone_wall_commands.value()[0].image, &tall_stone_wall_commands.value()[7].image));
    assert!(!eq(&tall_stone_wall_commands.value()[14].image, &tall_stone_wall_commands.value()[21].image));
    assert!(!eq(&tall_stone_wall_commands.value()[28].image, &tall_stone_wall_commands.value()[0].image));

    // Actor 166 uses the full length nybble but only two height bits.
    let horizontal_stone_wall = shipped_catalog.by_object_id(166).unwrap();
    let mut horizontal_stone_wall_input = barrel_input.clone();
    horizontal_stone_wall_input.settings = 0x2003;
    let mut horizontal_stone_wall_bounds = bounds(horizontal_stone_wall, &horizontal_stone_wall_input, false);
    let mut horizontal_stone_wall_commands = render(horizontal_stone_wall, &horizontal_stone_wall_input);
    assert!(horizontal_stone_wall_bounds.ok() && horizontal_stone_wall_bounds.value().is_some());
    assert!(eq(&horizontal_stone_wall_bounds.value().unwrap().width, &64) && eq(&horizontal_stone_wall_bounds.value().unwrap().height, &64));
    assert!(horizontal_stone_wall_commands.ok() && eq(&horizontal_stone_wall_commands.value().len(), &16));
    assert!(eq(&horizontal_stone_wall_commands.value()[0].x, &160) && eq(&horizontal_stone_wall_commands.value()[0].y, &320));
    assert!(eq(&horizontal_stone_wall_commands.value()[3].x, &208) && eq(&horizontal_stone_wall_commands.value()[3].y, &320));
    assert!(eq(&horizontal_stone_wall_commands.value()[6].x, &208) && eq(&horizontal_stone_wall_commands.value()[6].y, &368));
    assert!(eq(&horizontal_stone_wall_commands.value()[9].x, &160) && eq(&horizontal_stone_wall_commands.value()[9].y, &368));

    let mut masked_stone_wall_input = barrel_input.clone();
    masked_stone_wall_input.settings = 0x4000;
    let mut masked_stone_wall_bounds = bounds(horizontal_stone_wall, &masked_stone_wall_input, false);
    let mut masked_stone_wall_commands = render(horizontal_stone_wall, &masked_stone_wall_input);
    assert!(masked_stone_wall_bounds.ok() && masked_stone_wall_bounds.value().is_some());
    assert!(eq(&masked_stone_wall_bounds.value().unwrap().width, &32) && eq(&masked_stone_wall_bounds.value().unwrap().height, &16));
    assert!(masked_stone_wall_commands.ok() && eq(&masked_stone_wall_commands.value().len(), &2));

    // Actor 167 is the unused duplicate of actor 166 and must keep identical
    // bounds and draw commands for the same settings.
    let unused_stone_wall = shipped_catalog.by_object_id(167).unwrap();
    let mut unused_stone_wall_bounds = bounds(unused_stone_wall, &horizontal_stone_wall_input, false);
    let mut unused_stone_wall_commands = render(unused_stone_wall, &horizontal_stone_wall_input);
    assert!(unused_stone_wall_bounds.ok() && unused_stone_wall_bounds.value().is_some());
    assert!(eq(&unused_stone_wall_bounds.value().unwrap().x, &horizontal_stone_wall_bounds.value().unwrap().x));
    assert!(eq(&unused_stone_wall_bounds.value().unwrap().y, &horizontal_stone_wall_bounds.value().unwrap().y));
    assert!(eq(&unused_stone_wall_bounds.value().unwrap().width, &horizontal_stone_wall_bounds.value().unwrap().width));
    assert!(eq(&unused_stone_wall_bounds.value().unwrap().height, &horizontal_stone_wall_bounds.value().unwrap().height));
    assert!(unused_stone_wall_commands.ok() && eq(&unused_stone_wall_commands.value().len(), &horizontal_stone_wall_commands.value().len()));
    for command in 0..unused_stone_wall_commands.value().len() {
        assert!(eq(&unused_stone_wall_commands.value()[command].x, &horizontal_stone_wall_commands.value()[command].x));
        assert!(eq(&unused_stone_wall_commands.value()[command].y, &horizontal_stone_wall_commands.value()[command].y));
        assert!(eq(&unused_stone_wall_commands.value()[command].image, &horizontal_stone_wall_commands.value()[command].image));
    }

    // Actor 168 deliberately keeps odd selection expansion: vertical spikes
    // enlarge width and horizontal spikes enlarge height.
    let spiked_stone_wall = shipped_catalog.by_object_id(168).unwrap();
    let spiked_wall_width: [f64; 7] = [32.0, 48.0, 48.0, 64.0, 32.0, 32.0, 32.0];
    let spiked_wall_height: [f64; 7] = [16.0, 16.0, 16.0, 16.0, 32.0, 32.0, 48.0];
    let spiked_wall_count: [usize; 7] = [2, 4, 3, 5, 3, 3, 4];
    for spikes in 0..7 {
        let mut spiked_wall_input = barrel_input.clone();
        spiked_wall_input.settings = spikes << 24;
        let mut spiked_wall_bounds = bounds(spiked_stone_wall, &spiked_wall_input, false);
        let mut spiked_wall_commands = render(spiked_stone_wall, &spiked_wall_input);
        assert!(spiked_wall_bounds.ok() && spiked_wall_bounds.value().is_some());
        assert!(eq(&spiked_wall_bounds.value().unwrap().width, &spiked_wall_width[spikes as usize]));
        assert!(eq(&spiked_wall_bounds.value().unwrap().height, &spiked_wall_height[spikes as usize]));
        assert!(spiked_wall_commands.ok() && eq(&spiked_wall_commands.value().len(), &spiked_wall_count[spikes as usize]));
    }

    let mut both_vertical_spikes_input = barrel_input.clone();
    both_vertical_spikes_input.settings = 3 << 24;
    let mut both_vertical_spikes = render(spiked_stone_wall, &both_vertical_spikes_input);
    assert!(both_vertical_spikes.ok() && eq(&both_vertical_spikes.value().len(), &5));
    assert!(eq(&both_vertical_spikes.value()[0].x, &160) && eq(&both_vertical_spikes.value()[0].y, &336));
    assert!(eq(&both_vertical_spikes.value()[1].x, &176) && eq(&both_vertical_spikes.value()[1].y, &336));
    assert!(eq(&both_vertical_spikes.value()[2].x, &160) && eq(&both_vertical_spikes.value()[2].y, &320));
    assert!(eq(&both_vertical_spikes.value()[3].x, &176) && eq(&both_vertical_spikes.value()[3].y, &320));
    assert!(eq(&both_vertical_spikes.value()[4].x, &160) && eq(&both_vertical_spikes.value()[4].y, &352));
    assert!(both_vertical_spikes.value()[4].flip_y);

    let mut both_horizontal_spikes_input = barrel_input.clone();
    both_horizontal_spikes_input.settings = 6 << 24;
    let mut both_horizontal_spikes = render(spiked_stone_wall, &both_horizontal_spikes_input);
    assert!(both_horizontal_spikes.ok() && eq(&both_horizontal_spikes.value().len(), &4));
    assert!(eq(&both_horizontal_spikes.value()[0].x, &176) && eq(&both_horizontal_spikes.value()[0].y, &320));
    assert!(eq(&both_horizontal_spikes.value()[1].x, &192) && eq(&both_horizontal_spikes.value()[1].y, &320));
    assert!(eq(&both_horizontal_spikes.value()[2].x, &208) && eq(&both_horizontal_spikes.value()[2].rotation, &90));
    assert!(eq(&both_horizontal_spikes.value()[3].x, &160) && eq(&both_horizontal_spikes.value()[3].rotation, &90));
    assert!(both_horizontal_spikes.value()[3].flip_x);

    let mut square_stone_wall_input = barrel_input.clone();
    square_stone_wall_input.settings = 0x22;
    let mut square_stone_wall_commands = render(spiked_stone_wall, &square_stone_wall_input);
    assert!(square_stone_wall_commands.ok() && eq(&square_stone_wall_commands.value().len(), &9));
    let square_stone_wall_x: [f64; 9] = [160.0, 176.0, 192.0, 192.0, 192.0, 176.0, 160.0, 160.0, 176.0];
    let square_stone_wall_y: [f64; 9] = [320.0, 320.0, 320.0, 336.0, 352.0, 352.0, 352.0, 336.0, 336.0];
    for command in 0..square_stone_wall_commands.value().len() {
        assert!(eq(&square_stone_wall_commands.value()[command].x, &square_stone_wall_x[command]));
        assert!(eq(&square_stone_wall_commands.value()[command].y, &square_stone_wall_y[command]));
    }

    // The flimsy lift reads only two length bits -- `FallWhenTouchedPlatform`
    // takes `(settings & 3) + 1` -- and doubles the logical value into its tile
    // count, so the low nybble's top two bits change nothing. Settings 0xf is
    // the same four-long, eight-tile lift that 0x3 is.
    let mut flimsy_lift_input = barrel_input.clone();
    flimsy_lift_input.settings = 0xf;
    let flimsy_lift = shipped_catalog.by_object_id(173).unwrap();
    let mut flimsy_lift_bounds = bounds(flimsy_lift, &flimsy_lift_input, false);
    let mut flimsy_lift_commands = render(flimsy_lift, &flimsy_lift_input);
    assert!(flimsy_lift_bounds.ok() && flimsy_lift_bounds.value().is_some());
    assert!(eq(&flimsy_lift_bounds.value().unwrap().x, &104) && eq(&flimsy_lift_bounds.value().unwrap().y, &320));
    assert!(eq(&flimsy_lift_bounds.value().unwrap().width, &128) && eq(&flimsy_lift_bounds.value().unwrap().height, &16));
    assert!(flimsy_lift_commands.ok() && eq(&flimsy_lift_commands.value().len(), &8));
    assert!(eq(&flimsy_lift_commands.value()[0].x, &104) && eq(&flimsy_lift_commands.value()[0].y, &320));
    assert!(eq(&flimsy_lift_commands.value()[1].x, &120) && eq(&flimsy_lift_commands.value()[1].y, &320));
    assert!(eq(&flimsy_lift_commands.value()[7].x, &216) && eq(&flimsy_lift_commands.value()[7].y, &320));

    // The two ignored bits are not merely masked out of the value: 0x3 renders
    // identically, which is the whole of what narrowing the field bought.
    let mut flimsy_lift_low_input = barrel_input.clone();
    flimsy_lift_low_input.settings = 0x3;
    let mut flimsy_lift_low_bounds = bounds(flimsy_lift, &flimsy_lift_low_input, false);
    assert!(flimsy_lift_low_bounds.ok() && flimsy_lift_low_bounds.value().is_some());
    assert!(eq(&flimsy_lift_low_bounds.value().unwrap().x, &flimsy_lift_bounds.value().unwrap().x));
    assert!(eq(&flimsy_lift_low_bounds.value().unwrap().width, &flimsy_lift_bounds.value().unwrap().width));

    // The one-way lift clamps its logical length to two tiles and remains
    // centered on the actor origin.
    let mut one_way_lift_input = barrel_input.clone();
    one_way_lift_input.settings = 0xf;
    let one_way_lift = shipped_catalog.by_object_id(174).unwrap();
    let mut one_way_lift_bounds = bounds(one_way_lift, &one_way_lift_input, false);
    let mut one_way_lift_commands = render(one_way_lift, &one_way_lift_input);
    assert!(one_way_lift_bounds.ok() && one_way_lift_bounds.value().is_some());
    assert!(eq(&one_way_lift_bounds.value().unwrap().x, &32) && eq(&one_way_lift_bounds.value().unwrap().y, &320));
    assert!(eq(&one_way_lift_bounds.value().unwrap().width, &256) && eq(&one_way_lift_bounds.value().unwrap().height, &16));
    assert!(one_way_lift_commands.ok() && eq(&one_way_lift_commands.value().len(), &16));
    assert!(eq(&one_way_lift_commands.value()[0].x, &32) && eq(&one_way_lift_commands.value()[0].y, &320));
    assert!(eq(&one_way_lift_commands.value()[15].x, &272) && eq(&one_way_lift_commands.value()[15].y, &320));

    // The three-platform rickshaw uses one bit for platform size and one for
    // rotation; the direction marker is flipped for counterclockwise motion.
    let mut three_rickshaw_input = barrel_input.clone();
    three_rickshaw_input.settings = 1;
    let three_rickshaw = shipped_catalog.by_object_id(175).unwrap();
    let mut three_rickshaw_bounds = bounds(three_rickshaw, &three_rickshaw_input, false);
    let mut three_rickshaw_commands = render(three_rickshaw, &three_rickshaw_input);
    assert!(three_rickshaw_bounds.ok() && three_rickshaw_bounds.value().is_some());
    assert!(eq(&three_rickshaw_bounds.value().unwrap().x, &119) && eq(&three_rickshaw_bounds.value().unwrap().y, &279));
    assert!(eq(&three_rickshaw_bounds.value().unwrap().width, &121) && eq(&three_rickshaw_bounds.value().unwrap().height, &99));
    assert!(three_rickshaw_commands.ok() && eq(&three_rickshaw_commands.value().len(), &2));
    assert!(eq(&three_rickshaw_commands.value()[0].x, &119) && eq(&three_rickshaw_commands.value()[0].y, &279));
    assert!(eq(&three_rickshaw_commands.value()[1].x, &153) && eq(&three_rickshaw_commands.value()[1].y, &312));
    assert!(eq(&three_rickshaw_commands.value()[1].width, &32) && eq(&three_rickshaw_commands.value()[1].height, &32));
    assert!(three_rickshaw_commands.value()[1].flip_x);

    three_rickshaw_input.settings = 1 | (1 << 16);
    three_rickshaw_commands = render(three_rickshaw, &three_rickshaw_input);
    assert!(three_rickshaw_commands.ok() && !three_rickshaw_commands.value()[1].flip_x);

    // Lift-spawner direction is a two-bit field; upper nybble bits do not
    // change the left-spawning platform or its arrow.
    let mut lift_spawner_input = barrel_input.clone();
    lift_spawner_input.settings = 7 << 16;
    let lift_spawner = shipped_catalog.by_object_id(177).unwrap();
    let mut lift_spawner_bounds = bounds(lift_spawner, &lift_spawner_input, false);
    let mut lift_spawner_commands = render(lift_spawner, &lift_spawner_input);
    assert!(lift_spawner_bounds.ok() && lift_spawner_bounds.value().is_some());
    assert!(eq(&lift_spawner_bounds.value().unwrap().x, &224) && eq(&lift_spawner_bounds.value().unwrap().y, &320));
    assert!(eq(&lift_spawner_bounds.value().unwrap().width, &64) && eq(&lift_spawner_bounds.value().unwrap().height, &16));
    assert!(lift_spawner_commands.ok() && eq(&lift_spawner_commands.value().len(), &5));
    assert!(eq(&lift_spawner_commands.value()[0].x, &224) && eq(&lift_spawner_commands.value()[0].y, &320));
    assert!(eq(&lift_spawner_commands.value()[3].x, &272) && eq(&lift_spawner_commands.value()[3].y, &320));
    assert!(eq(&lift_spawner_commands.value()[4].x, &208) && eq(&lift_spawner_commands.value()[4].y, &320));
    assert!(eq(&lift_spawner_commands.value()[4].rotation, &90));

    // Rectangle-lift shift and rotation are single bits even when their raw
    // nybbles contain other set bits.
    let mut rotating_rectangle_input = barrel_input.clone();
    rotating_rectangle_input.settings = 0x30020000;
    let rotating_rectangle = shipped_catalog.by_object_id(179).unwrap();
    let mut rotating_rectangle_bounds = bounds(rotating_rectangle, &rotating_rectangle_input, false);
    let mut rotating_rectangle_commands = render(rotating_rectangle, &rotating_rectangle_input);
    assert!(rotating_rectangle_bounds.ok() && rotating_rectangle_bounds.value().is_some());
    assert!(eq(&rotating_rectangle_bounds.value().unwrap().x, &136) && eq(&rotating_rectangle_bounds.value().unwrap().y, &320));
    assert!(eq(&rotating_rectangle_bounds.value().unwrap().width, &64) && eq(&rotating_rectangle_bounds.value().unwrap().height, &32));
    assert!(rotating_rectangle_commands.ok() && eq(&rotating_rectangle_commands.value().len(), &2));
    assert!(eq(&rotating_rectangle_commands.value()[0].x, &136) && eq(&rotating_rectangle_commands.value()[0].y, &320));
    assert!(eq(&rotating_rectangle_commands.value()[1].x, &152) && eq(&rotating_rectangle_commands.value()[1].y, &320));
    assert!(rotating_rectangle_commands.value()[1].flip_x);

    // The triangle's direction marker intentionally begins two pixels above
    // the platform bounds.
    let rotating_triangle = shipped_catalog.by_object_id(180).unwrap();
    let mut rotating_triangle_bounds = bounds(rotating_triangle, &rotating_rectangle_input, false);
    let mut rotating_triangle_commands = render(rotating_triangle, &rotating_rectangle_input);
    assert!(rotating_triangle_bounds.ok() && rotating_triangle_bounds.value().is_some());
    assert!(eq(&rotating_triangle_bounds.value().unwrap().x, &146) && eq(&rotating_triangle_bounds.value().unwrap().y, &322));
    assert!(eq(&rotating_triangle_bounds.value().unwrap().width, &44) && eq(&rotating_triangle_bounds.value().unwrap().height, &39));
    assert!(rotating_triangle_commands.ok() && eq(&rotating_triangle_commands.value().len(), &2));
    assert!(eq(&rotating_triangle_commands.value()[0].x, &146) && eq(&rotating_triangle_commands.value()[0].y, &322));
    assert!(eq(&rotating_triangle_commands.value()[1].x, &152) && eq(&rotating_triangle_commands.value()[1].y, &320));
    assert!(rotating_triangle_commands.value()[1].flip_x);

    let rotating_square = shipped_catalog.by_object_id(181).unwrap();
    let mut rotating_square_bounds = bounds(rotating_square, &rotating_rectangle_input, false);
    let mut rotating_square_commands = render(rotating_square, &rotating_rectangle_input);
    assert!(rotating_square_bounds.ok() && rotating_square_bounds.value().is_some());
    assert!(eq(&rotating_square_bounds.value().unwrap().x, &144) && eq(&rotating_square_bounds.value().unwrap().y, &312));
    assert!(eq(&rotating_square_bounds.value().unwrap().width, &48) && eq(&rotating_square_bounds.value().unwrap().height, &48));
    assert!(rotating_square_commands.ok() && eq(&rotating_square_commands.value().len(), &2));
    assert!(eq(&rotating_square_commands.value()[0].x, &144) && eq(&rotating_square_commands.value()[0].y, &312));
    assert!(eq(&rotating_square_commands.value()[1].x, &152) && eq(&rotating_square_commands.value()[1].y, &320));
    assert!(rotating_square_commands.value()[1].flip_x);

    // The track-controlled lift is a fixed four-tile platform shifted down by
    // half a tile.
    let track_lift = shipped_catalog.by_object_id(182).unwrap();
    let mut track_lift_bounds = bounds(track_lift, &barrel_input, false);
    let mut track_lift_commands = render(track_lift, &barrel_input);
    assert!(track_lift_bounds.ok() && track_lift_bounds.value().is_some());
    assert!(eq(&track_lift_bounds.value().unwrap().x, &160) && eq(&track_lift_bounds.value().unwrap().y, &328));
    assert!(eq(&track_lift_bounds.value().unwrap().width, &64) && eq(&track_lift_bounds.value().unwrap().height, &16));
    assert!(track_lift_commands.ok() && eq(&track_lift_commands.value().len(), &4));
    for command in 0..track_lift_commands.value().len() {
        assert!(eq(&track_lift_commands.value()[command].x, &(160 + 16 * command)));
        assert!(eq(&track_lift_commands.value()[command].y, &328));
    }

    let four_rickshaw = shipped_catalog.by_object_id(183).unwrap();
    let mut four_rickshaw_bounds = bounds(four_rickshaw, &barrel_input, false);
    let mut four_rickshaw_commands = render(four_rickshaw, &barrel_input);
    assert!(four_rickshaw_bounds.ok() && four_rickshaw_bounds.value().is_some());
    assert!(eq(&four_rickshaw_bounds.value().unwrap().x, &122) && eq(&four_rickshaw_bounds.value().unwrap().y, &291));
    assert!(eq(&four_rickshaw_bounds.value().unwrap().width, &91) && eq(&four_rickshaw_bounds.value().unwrap().height, &75));
    assert!(four_rickshaw_commands.ok() && eq(&four_rickshaw_commands.value().len(), &1));
    assert!(eq(&four_rickshaw_commands.value()[0].x, &122) && eq(&four_rickshaw_commands.value()[0].y, &291));
    assert!(eq(&four_rickshaw_commands.value()[0].width, &91) && eq(&four_rickshaw_commands.value()[0].height, &75));

    // Swinging-pole height uses only three low bits; a raw F still produces
    // eleven pole segments plus the attachment drawn last.
    let mut swinging_pole_input = barrel_input.clone();
    swinging_pole_input.settings = 0xf;
    let swinging_pole = shipped_catalog.by_object_id(184).unwrap();
    let mut swinging_pole_bounds = bounds(swinging_pole, &swinging_pole_input, false);
    let mut swinging_pole_commands = render(swinging_pole, &swinging_pole_input);
    assert!(swinging_pole_bounds.ok() && swinging_pole_bounds.value().is_some());
    assert!(eq(&swinging_pole_bounds.value().unwrap().x, &149) && eq(&swinging_pole_bounds.value().unwrap().y, &309));
    assert!(eq(&swinging_pole_bounds.value().unwrap().width, &22) && eq(&swinging_pole_bounds.value().unwrap().height, &187));
    assert!(swinging_pole_commands.ok() && eq(&swinging_pole_commands.value().len(), &12));
    for segment in 0..11 {
        assert!(eq(&swinging_pole_commands.value()[segment].x, &157));
        assert!(eq(&swinging_pole_commands.value()[segment].y, &(320 + 16 * segment)));
        assert!(eq(&swinging_pole_commands.value()[segment].width, &7));
        assert!(eq(&swinging_pole_commands.value()[segment].height, &16));
    }
    assert!(eq(&swinging_pole_commands.value()[11].x, &149) && eq(&swinging_pole_commands.value()[11].y, &309));
    assert!(eq(&swinging_pole_commands.value()[11].width, &22) && eq(&swinging_pole_commands.value()[11].height, &22));

    let large_spiked_ball = shipped_catalog.by_object_id(185).unwrap();
    let mut large_spiked_ball_bounds = bounds(large_spiked_ball, &barrel_input, false);
    let mut large_spiked_ball_commands = render(large_spiked_ball, &barrel_input);
    assert!(large_spiked_ball_bounds.ok() && large_spiked_ball_bounds.value().is_some());
    assert!(eq(&large_spiked_ball_bounds.value().unwrap().x, &160) && eq(&large_spiked_ball_bounds.value().unwrap().y, &278));
    assert!(eq(&large_spiked_ball_bounds.value().unwrap().width, &63) && eq(&large_spiked_ball_bounds.value().unwrap().height, &58));
    assert!(large_spiked_ball_commands.ok() && eq(&large_spiked_ball_commands.value().len(), &1));
    assert!(eq(&large_spiked_ball_commands.value()[0].x, &160) && eq(&large_spiked_ball_commands.value()[0].y, &278));
    assert!(eq(&large_spiked_ball_commands.value()[0].width, &63) && eq(&large_spiked_ball_commands.value()[0].height, &58));

    // The giant rickshaw's rotation uses only bit 16; other bits in that
    // nybble leave the counterclockwise marker flipped.
    let mut giant_rickshaw_input = barrel_input.clone();
    giant_rickshaw_input.settings = 2 << 16;
    let giant_rickshaw = shipped_catalog.by_object_id(186).unwrap();
    let mut giant_rickshaw_bounds = bounds(giant_rickshaw, &giant_rickshaw_input, false);
    let mut giant_rickshaw_commands = render(giant_rickshaw, &giant_rickshaw_input);
    assert!(giant_rickshaw_bounds.ok() && giant_rickshaw_bounds.value().is_some());
    assert!(eq(&giant_rickshaw_bounds.value().unwrap().x, &16) && eq(&giant_rickshaw_bounds.value().unwrap().y, &216));
    assert!(eq(&giant_rickshaw_bounds.value().unwrap().width, &288) && eq(&giant_rickshaw_bounds.value().unwrap().height, &208));
    assert!(giant_rickshaw_commands.ok() && eq(&giant_rickshaw_commands.value().len(), &2));
    assert!(eq(&giant_rickshaw_commands.value()[0].x, &16) && eq(&giant_rickshaw_commands.value()[0].y, &216));
    assert!(eq(&giant_rickshaw_commands.value()[1].x, &144) && eq(&giant_rickshaw_commands.value()[1].y, &304));
    assert!(giant_rickshaw_commands.value()[1].flip_x);

    giant_rickshaw_input.settings = 1 << 16;
    giant_rickshaw_commands = render(giant_rickshaw, &giant_rickshaw_input);
    assert!(giant_rickshaw_commands.ok() && !giant_rickshaw_commands.value()[1].flip_x);

    // The Mario Bros.-style platform consumes the full low byte for length,
    // not just its declared low nybble.
    let mut bump_platform_input = barrel_input.clone();
    bump_platform_input.settings = 0x12;
    let bump_platform = shipped_catalog.by_object_id(188).unwrap();
    let mut bump_platform_bounds = bounds(bump_platform, &bump_platform_input, false);
    let mut bump_platform_commands = render(bump_platform, &bump_platform_input);
    assert!(bump_platform_bounds.ok() && bump_platform_bounds.value().is_some());
    assert!(eq(&bump_platform_bounds.value().unwrap().x, &10) && eq(&bump_platform_bounds.value().unwrap().y, &320));
    assert!(eq(&bump_platform_bounds.value().unwrap().width, &300) && eq(&bump_platform_bounds.value().unwrap().height, &10));
    assert!(bump_platform_commands.ok() && eq(&bump_platform_commands.value().len(), &19));
    assert!(eq(&bump_platform_commands.value()[0].x, &10) && eq(&bump_platform_commands.value()[0].width, &14));
    for section in 0..17 {
        assert!(eq(&bump_platform_commands.value()[section + 1].x, &(24 + 16 * section)));
        assert!(eq(&bump_platform_commands.value()[section + 1].width, &16));
    }
    assert!(eq(&bump_platform_commands.value()[18].x, &296) && eq(&bump_platform_commands.value()[18].width, &14));

    // Mushroom actor 189 uses the full stalk-height nybble; value eight draws
    // ten stalk pieces before the green platform cap.
    let mut tilting_mushroom_input = barrel_input.clone();
    tilting_mushroom_input.settings = (1 << 12) | (8 << 16) | (1 << 23);
    let tilting_mushroom = shipped_catalog.by_object_id(189).unwrap();
    let mut tilting_mushroom_bounds = bounds(tilting_mushroom, &tilting_mushroom_input, false);
    let mut tilting_mushroom_commands = render(tilting_mushroom, &tilting_mushroom_input);
    assert!(tilting_mushroom_bounds.ok() && tilting_mushroom_bounds.value().is_some());
    assert!(eq(&tilting_mushroom_bounds.value().unwrap().x, &128) && eq(&tilting_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&tilting_mushroom_bounds.value().unwrap().width, &96) && eq(&tilting_mushroom_bounds.value().unwrap().height, &192));
    assert!(tilting_mushroom_commands.ok() && eq(&tilting_mushroom_commands.value().len(), &14));
    assert!(eq(&tilting_mushroom_commands.value()[0].x, &166) && eq(&tilting_mushroom_commands.value()[0].y, &344));
    for segment in 0..10 {
        assert!(eq(&tilting_mushroom_commands.value()[segment + 1].x, &166));
        assert!(eq(&tilting_mushroom_commands.value()[segment + 1].y, &(352 + 16 * segment)));
    }
    assert!(eq(&tilting_mushroom_commands.value()[11].x, &128) && eq(&tilting_mushroom_commands.value()[11].y, &320));
    assert!(eq(&tilting_mushroom_commands.value()[12].x, &160) && eq(&tilting_mushroom_commands.value()[12].y, &320));
    assert!(eq(&tilting_mushroom_commands.value()[13].x, &192) && tilting_mushroom_commands.value()[13].flip_x);

    // Purple-mushroom length uses only bits 4-5; upper bits in the low byte
    // remain ignored.
    let mut purple_mushroom_input = barrel_input.clone();
    purple_mushroom_input.settings = 0xd0;
    let purple_mushroom = shipped_catalog.by_object_id(190).unwrap();
    let mut purple_mushroom_bounds = bounds(purple_mushroom, &purple_mushroom_input, false);
    let mut purple_mushroom_commands = render(purple_mushroom, &purple_mushroom_input);
    assert!(purple_mushroom_bounds.ok() && purple_mushroom_bounds.value().is_some());
    assert!(eq(&purple_mushroom_bounds.value().unwrap().x, &128) && eq(&purple_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&purple_mushroom_bounds.value().unwrap().width, &96) && eq(&purple_mushroom_bounds.value().unwrap().height, &112));
    assert!(purple_mushroom_commands.ok() && eq(&purple_mushroom_commands.value().len(), &9));
    assert!(eq(&purple_mushroom_commands.value()[0].x, &166) && eq(&purple_mushroom_commands.value()[0].y, &344));
    for segment in 0..5 {
        assert!(eq(&purple_mushroom_commands.value()[segment + 1].x, &166));
        assert!(eq(&purple_mushroom_commands.value()[segment + 1].y, &(352 + 16 * segment)));
    }
    assert!(eq(&purple_mushroom_commands.value()[6].x, &128));
    assert!(eq(&purple_mushroom_commands.value()[7].x, &160));
    assert!(eq(&purple_mushroom_commands.value()[8].x, &192) && purple_mushroom_commands.value()[8].flip_x);

    // Equal stalk height and travel distance select the rising mushroom art.
    let mut rising_mushroom_input = barrel_input.clone();
    rising_mushroom_input.settings = 0x332;
    let rising_mushroom = shipped_catalog.by_object_id(191).unwrap();
    let mut rising_mushroom_bounds = bounds(rising_mushroom, &rising_mushroom_input, false);
    let mut rising_mushroom_commands = render(rising_mushroom, &rising_mushroom_input);
    assert!(rising_mushroom_bounds.ok() && rising_mushroom_bounds.value().is_some());
    assert!(eq(&rising_mushroom_bounds.value().unwrap().x, &136) && eq(&rising_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&rising_mushroom_bounds.value().unwrap().width, &64) && eq(&rising_mushroom_bounds.value().unwrap().height, &96));
    assert!(rising_mushroom_commands.ok() && eq(&rising_mushroom_commands.value().len(), &9));
    assert!(eq(&rising_mushroom_commands.value()[0].x, &160) && eq(&rising_mushroom_commands.value()[0].y, &344));
    for segment in 0..4 {
        assert!(eq(&rising_mushroom_commands.value()[segment + 1].x, &160));
        assert!(eq(&rising_mushroom_commands.value()[segment + 1].y, &(352 + 16 * segment)));
    }
    assert!(eq(&rising_mushroom_commands.value()[5].x, &136));
    assert!(eq(&rising_mushroom_commands.value()[6].x, &152));
    assert!(eq(&rising_mushroom_commands.value()[7].x, &168));
    assert!(eq(&rising_mushroom_commands.value()[8].x, &184) && rising_mushroom_commands.value()[8].flip_x);
    assert!(eq(&rising_mushroom_commands.value()[5].image, &rising_mushroom_commands.value()[8].image));
    let mut lowering_mushroom_input = barrel_input.clone();
    lowering_mushroom_input.settings = 0x342;
    let mut lowering_mushroom_commands = render(rising_mushroom, &lowering_mushroom_input);
    assert!(lowering_mushroom_commands.ok() && eq(&lowering_mushroom_commands.value().len(), &10));
    assert!(!eq(&lowering_mushroom_commands.value()[6].image, &rising_mushroom_commands.value()[5].image));

    let mut mechanical_mushroom_input = barrel_input.clone();
    mechanical_mushroom_input.settings = 0x32;
    let mechanical_mushroom = shipped_catalog.by_object_id(192).unwrap();
    let mut mechanical_mushroom_bounds = bounds(mechanical_mushroom, &mechanical_mushroom_input, false);
    let mut mechanical_mushroom_commands = render(mechanical_mushroom, &mechanical_mushroom_input);
    assert!(mechanical_mushroom_bounds.ok() && mechanical_mushroom_bounds.value().is_some());
    assert!(eq(&mechanical_mushroom_bounds.value().unwrap().x, &136) && eq(&mechanical_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&mechanical_mushroom_bounds.value().unwrap().width, &64) && eq(&mechanical_mushroom_bounds.value().unwrap().height, &97));
    assert!(mechanical_mushroom_commands.ok() && eq(&mechanical_mushroom_commands.value().len(), &8));
    assert!(eq(&mechanical_mushroom_commands.value()[0].x, &136));
    assert!(eq(&mechanical_mushroom_commands.value()[1].x, &152));
    assert!(eq(&mechanical_mushroom_commands.value()[2].x, &168));
    assert!(eq(&mechanical_mushroom_commands.value()[3].x, &184) && mechanical_mushroom_commands.value()[3].flip_x);
    assert!(eq(&mechanical_mushroom_commands.value()[4].x, &160) && eq(&mechanical_mushroom_commands.value()[4].y, &344));
    for segment in 0..3 {
        assert!(eq(&mechanical_mushroom_commands.value()[segment + 5].x, &160));
        assert!(eq(&mechanical_mushroom_commands.value()[segment + 5].y, &(369 + 16 * segment)));
    }

    // Expanded blue mushrooms start their stalk one tile higher and add a top
    // segment beyond the configured stalk height.
    let mut expanding_mushroom_input = barrel_input.clone();
    expanding_mushroom_input.settings = 0x112;
    let expanding_mushroom = shipped_catalog.by_object_id(193).unwrap();
    let mut expanding_mushroom_bounds = bounds(expanding_mushroom, &expanding_mushroom_input, false);
    let mut expanding_mushroom_commands = render(expanding_mushroom, &expanding_mushroom_input);
    assert!(expanding_mushroom_bounds.ok() && expanding_mushroom_bounds.value().is_some());
    assert!(eq(&expanding_mushroom_bounds.value().unwrap().x, &128) && eq(&expanding_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&expanding_mushroom_bounds.value().unwrap().width, &80) && eq(&expanding_mushroom_bounds.value().unwrap().height, &64));
    assert!(expanding_mushroom_commands.ok() && eq(&expanding_mushroom_commands.value().len(), &4));
    assert!(eq(&expanding_mushroom_commands.value()[0].x, &128) && eq(&expanding_mushroom_commands.value()[0].y, &320));
    assert!(eq(&expanding_mushroom_commands.value()[0].width, &80) && eq(&expanding_mushroom_commands.value()[0].height, &16));
    assert!(eq(&expanding_mushroom_commands.value()[1].x, &160) && eq(&expanding_mushroom_commands.value()[1].y, &336));
    assert!(eq(&expanding_mushroom_commands.value()[2].x, &160) && eq(&expanding_mushroom_commands.value()[2].y, &352));
    assert!(eq(&expanding_mushroom_commands.value()[3].x, &160) && eq(&expanding_mushroom_commands.value()[3].y, &368));

    let mut bouncy_mushroom_input = barrel_input.clone();
    bouncy_mushroom_input.settings = 0x12;
    let bouncy_mushroom = shipped_catalog.by_object_id(194).unwrap();
    let mut bouncy_mushroom_bounds = bounds(bouncy_mushroom, &bouncy_mushroom_input, false);
    let mut bouncy_mushroom_commands = render(bouncy_mushroom, &bouncy_mushroom_input);
    assert!(bouncy_mushroom_bounds.ok() && bouncy_mushroom_bounds.value().is_some());
    assert!(eq(&bouncy_mushroom_bounds.value().unwrap().x, &112) && eq(&bouncy_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&bouncy_mushroom_bounds.value().unwrap().width, &96) && eq(&bouncy_mushroom_bounds.value().unwrap().height, &64));
    assert!(bouncy_mushroom_commands.ok() && eq(&bouncy_mushroom_commands.value().len(), &3));
    assert!(eq(&bouncy_mushroom_commands.value()[0].x, &112) && eq(&bouncy_mushroom_commands.value()[0].y, &320));
    assert!(eq(&bouncy_mushroom_commands.value()[1].x, &155) && eq(&bouncy_mushroom_commands.value()[1].y, &352));
    assert!(eq(&bouncy_mushroom_commands.value()[2].x, &155) && eq(&bouncy_mushroom_commands.value()[2].y, &368));

    let mut moving_mushroom_input = barrel_input.clone();
    moving_mushroom_input.settings = (2 << 12) | (1 << 20);
    let moving_mushroom = shipped_catalog.by_object_id(195).unwrap();
    let mut moving_mushroom_bounds = bounds(moving_mushroom, &moving_mushroom_input, false);
    let mut moving_mushroom_commands = render(moving_mushroom, &moving_mushroom_input);
    assert!(moving_mushroom_bounds.ok() && moving_mushroom_bounds.value().is_some());
    assert!(eq(&moving_mushroom_bounds.value().unwrap().x, &112) && eq(&moving_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&moving_mushroom_bounds.value().unwrap().width, &128) && eq(&moving_mushroom_bounds.value().unwrap().height, &64));
    assert!(moving_mushroom_commands.ok() && eq(&moving_mushroom_commands.value().len(), &7));
    assert!(eq(&moving_mushroom_commands.value()[0].x, &166) && eq(&moving_mushroom_commands.value()[0].y, &344));
    assert!(eq(&moving_mushroom_commands.value()[1].x, &166) && eq(&moving_mushroom_commands.value()[1].y, &352));
    assert!(eq(&moving_mushroom_commands.value()[2].x, &166) && eq(&moving_mushroom_commands.value()[2].y, &368));
    assert!(eq(&moving_mushroom_commands.value()[3].x, &112));
    assert!(eq(&moving_mushroom_commands.value()[4].x, &144));
    assert!(eq(&moving_mushroom_commands.value()[5].x, &176));
    assert!(eq(&moving_mushroom_commands.value()[6].x, &208) && moving_mushroom_commands.value()[6].flip_x);

    // Swinging rope length saturates at eight tiles rather than wrapping.
    let mut swinging_rope_input = barrel_input.clone();
    swinging_rope_input.settings = 0xf;
    let swinging_rope = shipped_catalog.by_object_id(196).unwrap();
    let mut swinging_rope_bounds = bounds(swinging_rope, &swinging_rope_input, false);
    let mut swinging_rope_commands = render(swinging_rope, &swinging_rope_input);
    assert!(swinging_rope_bounds.ok() && swinging_rope_bounds.value().is_some());
    assert!(eq(&swinging_rope_bounds.value().unwrap().x, &157) && eq(&swinging_rope_bounds.value().unwrap().y, &320));
    assert!(eq(&swinging_rope_bounds.value().unwrap().width, &8) && eq(&swinging_rope_bounds.value().unwrap().height, &128));
    assert!(swinging_rope_commands.ok() && eq(&swinging_rope_commands.value().len(), &8));
    for segment in 0..8 {
        assert!(eq(&swinging_rope_commands.value()[segment].x, &157));
        assert!(eq(&swinging_rope_commands.value()[segment].y, &(320 + 16 * segment)));
        assert!(eq(&swinging_rope_commands.value()[segment].width, &8));
        assert!(eq(&swinging_rope_commands.value()[segment].height, &16));
    }

    // Swinging vine length also saturates at four extra middle segments; its
    // 14px bottom leaves two pixels of selection padding.
    let mut swinging_vine_input = barrel_input.clone();
    swinging_vine_input.settings = 0xf;
    let swinging_vine = shipped_catalog.by_object_id(197).unwrap();
    let mut swinging_vine_bounds = bounds(swinging_vine, &swinging_vine_input, false);
    let mut swinging_vine_commands = render(swinging_vine, &swinging_vine_input);
    assert!(swinging_vine_bounds.ok() && swinging_vine_bounds.value().is_some());
    assert!(eq(&swinging_vine_bounds.value().unwrap().x, &144) && eq(&swinging_vine_bounds.value().unwrap().y, &320));
    assert!(eq(&swinging_vine_bounds.value().unwrap().width, &32) && eq(&swinging_vine_bounds.value().unwrap().height, &128));
    assert!(swinging_vine_commands.ok() && eq(&swinging_vine_commands.value().len(), &8));
    assert!(eq(&swinging_vine_commands.value()[0].x, &144) && eq(&swinging_vine_commands.value()[0].y, &320));
    for segment in 1..=6 {
        assert!(eq(&swinging_vine_commands.value()[segment].x, &144));
        assert!(eq(&swinging_vine_commands.value()[segment].y, &(320 + 16 * segment)));
    }
    assert!(eq(&swinging_vine_commands.value()[7].x, &144) && eq(&swinging_vine_commands.value()[7].y, &432));
    assert!(eq(&swinging_vine_commands.value()[7].height, &14));

    // Balance-lift arrow mode 2 deliberately reuses the up arrow; mode 3 omits
    // the arrow while retaining the platform.
    let mut balance_lift_input = barrel_input.clone();
    balance_lift_input.settings = 2;
    let balance_lift = shipped_catalog.by_object_id(198).unwrap();
    let mut balance_lift_bounds = bounds(balance_lift, &balance_lift_input, false);
    let mut balance_lift_commands = render(balance_lift, &balance_lift_input);
    assert!(balance_lift_bounds.ok() && balance_lift_bounds.value().is_some());
    assert!(eq(&balance_lift_bounds.value().unwrap().x, &160) && eq(&balance_lift_bounds.value().unwrap().y, &320));
    assert!(eq(&balance_lift_bounds.value().unwrap().width, &112) && eq(&balance_lift_bounds.value().unwrap().height, &16));
    assert!(balance_lift_commands.ok() && eq(&balance_lift_commands.value().len(), &2));
    assert!(eq(&balance_lift_commands.value()[0].x, &160) && eq(&balance_lift_commands.value()[0].y, &320));
    assert!(eq(&balance_lift_commands.value()[1].x, &211) && eq(&balance_lift_commands.value()[1].y, &324));

    balance_lift_input.settings = 3;
    balance_lift_commands = render(balance_lift, &balance_lift_input);
    assert!(balance_lift_commands.ok() && eq(&balance_lift_commands.value().len(), &1));

    let mut sinking_snow_input = barrel_input.clone();
    sinking_snow_input.settings = 2;
    let sinking_snow = shipped_catalog.by_object_id(200).unwrap();
    let mut sinking_snow_bounds = bounds(sinking_snow, &sinking_snow_input, false);
    let mut sinking_snow_commands = render(sinking_snow, &sinking_snow_input);
    assert!(sinking_snow_bounds.ok() && sinking_snow_bounds.value().is_some());
    assert!(eq(&sinking_snow_bounds.value().unwrap().x, &144) && eq(&sinking_snow_bounds.value().unwrap().y, &320));
    assert!(eq(&sinking_snow_bounds.value().unwrap().width, &80) && eq(&sinking_snow_bounds.value().unwrap().height, &35));
    assert!(sinking_snow_commands.ok() && eq(&sinking_snow_commands.value().len(), &5));
    assert!(eq(&sinking_snow_commands.value()[0].x, &144));
    assert!(eq(&sinking_snow_commands.value()[1].x, &160));
    assert!(eq(&sinking_snow_commands.value()[2].x, &176));
    assert!(eq(&sinking_snow_commands.value()[3].x, &192));
    assert!(eq(&sinking_snow_commands.value()[4].x, &208) && sinking_snow_commands.value()[4].flip_x);

    // Squishy-cloud length consumes the full low byte.
    let mut squishy_cloud_input = barrel_input.clone();
    squishy_cloud_input.settings = 0x10;
    let squishy_cloud = shipped_catalog.by_object_id(201).unwrap();
    let mut squishy_cloud_bounds = bounds(squishy_cloud, &squishy_cloud_input, false);
    let mut squishy_cloud_commands = render(squishy_cloud, &squishy_cloud_input);
    assert!(squishy_cloud_bounds.ok() && squishy_cloud_bounds.value().is_some());
    assert!(eq(&squishy_cloud_bounds.value().unwrap().x, &160) && eq(&squishy_cloud_bounds.value().unwrap().y, &320));
    assert!(eq(&squishy_cloud_bounds.value().unwrap().width, &576) && eq(&squishy_cloud_bounds.value().unwrap().height, &32));
    assert!(squishy_cloud_commands.ok() && eq(&squishy_cloud_commands.value().len(), &19));
    assert!(eq(&squishy_cloud_commands.value()[0].x, &160) && eq(&squishy_cloud_commands.value()[0].height, &26));
    for section in 0..17 {
        assert!(eq(&squishy_cloud_commands.value()[section + 1].x, &(176 + 32 * section)));
        assert!(eq(&squishy_cloud_commands.value()[section + 1].width, &32));
        assert!(eq(&squishy_cloud_commands.value()[section + 1].height, &32));
    }
    assert!(eq(&squishy_cloud_commands.value()[18].x, &720) && eq(&squishy_cloud_commands.value()[18].height, &25));

    // Pipe-cannon angle uses only three bits; raw D selects the long-right
    // cannon through its low three bits.
    let mut pipe_cannon_input = barrel_input.clone();
    pipe_cannon_input.settings = 0xd;
    let pipe_cannon = shipped_catalog.by_object_id(202).unwrap();
    let mut pipe_cannon_bounds = bounds(pipe_cannon, &pipe_cannon_input, false);
    let mut pipe_cannon_commands = render(pipe_cannon, &pipe_cannon_input);
    assert!(pipe_cannon_bounds.ok() && pipe_cannon_bounds.value().is_some());
    assert!(eq(&pipe_cannon_bounds.value().unwrap().x, &161) && eq(&pipe_cannon_bounds.value().unwrap().y, &273));
    assert!(eq(&pipe_cannon_bounds.value().unwrap().width, &57) && eq(&pipe_cannon_bounds.value().unwrap().height, &63));
    assert!(pipe_cannon_commands.ok() && eq(&pipe_cannon_commands.value().len(), &1));
    assert!(eq(&pipe_cannon_commands.value()[0].x, &161) && eq(&pipe_cannon_commands.value()[0].y, &273));
    assert!(eq(&pipe_cannon_commands.value()[0].width, &57) && eq(&pipe_cannon_commands.value()[0].height, &63));

    // Pump shifts use only the low bit of their nybbles; distance fields remain
    // full numeric nybbles.
    let mut corked_pipe_input = barrel_input.clone();
    corked_pipe_input.settings = 0x00032330;
    let corked_pipe = shipped_catalog.by_object_id(204).unwrap();
    let mut corked_pipe_bounds = bounds(corked_pipe, &corked_pipe_input, false);
    let mut corked_pipe_commands = render(corked_pipe, &corked_pipe_input);
    assert!(corked_pipe_bounds.ok() && corked_pipe_bounds.value().is_some());
    assert!(eq(&corked_pipe_bounds.value().unwrap().x, &160) && eq(&corked_pipe_bounds.value().unwrap().y, &264));
    assert!(eq(&corked_pipe_bounds.value().unwrap().width, &32) && eq(&corked_pipe_bounds.value().unwrap().height, &72));
    assert!(corked_pipe_commands.ok() && eq(&corked_pipe_commands.value().len(), &2));
    assert!(eq(&corked_pipe_commands.value()[0].x, &160) && eq(&corked_pipe_commands.value()[0].y, &264));
    assert!(eq(&corked_pipe_commands.value()[1].x, &-80) && eq(&corked_pipe_commands.value()[1].y, &529));

    // Wall-jump lift dimensions use two bits apiece despite their raw nybbles.
    let mut wall_jump_lift_input = barrel_input.clone();
    wall_jump_lift_input.settings = 0xf00f;
    let wall_jump_lift = shipped_catalog.by_object_id(209).unwrap();
    let mut wall_jump_lift_bounds = bounds(wall_jump_lift, &wall_jump_lift_input, false);
    let mut wall_jump_lift_commands = render(wall_jump_lift, &wall_jump_lift_input);
    assert!(wall_jump_lift_bounds.ok() && wall_jump_lift_bounds.value().is_some());
    assert!(eq(&wall_jump_lift_bounds.value().unwrap().x, &112) && eq(&wall_jump_lift_bounds.value().unwrap().y, &325));
    assert!(eq(&wall_jump_lift_bounds.value().unwrap().width, &112) && eq(&wall_jump_lift_bounds.value().unwrap().height, &131));
    assert!(wall_jump_lift_commands.ok() && eq(&wall_jump_lift_commands.value().len(), &20));
    for row in 0..7 {
        assert!(eq(&wall_jump_lift_commands.value()[2 * row].x, &112));
        assert!(eq(&wall_jump_lift_commands.value()[2 * row + 1].x, &208));
        assert!(eq(&wall_jump_lift_commands.value()[2 * row].y, &(440 - 16 * row)));
        assert!(eq(&wall_jump_lift_commands.value()[2 * row + 1].y, &(440 - 16 * row)));
    }
    for column in 0..5 {
        assert!(eq(&wall_jump_lift_commands.value()[14 + column].x, &(128 + 16 * column)));
        assert!(eq(&wall_jump_lift_commands.value()[14 + column].y, &344));
    }
    assert!(eq(&wall_jump_lift_commands.value()[19].x, &157) && eq(&wall_jump_lift_commands.value()[19].y, &325));

    // Carry-through-wall direction is an exact-zero versus nonzero field.
    let through_wall_lift = shipped_catalog.by_object_id(210).unwrap();
    let mut through_wall_bounds = bounds(through_wall_lift, &barrel_input, false);
    let mut through_wall_commands = render(through_wall_lift, &barrel_input);
    assert!(through_wall_bounds.ok() && through_wall_bounds.value().is_some());
    assert!(eq(&through_wall_bounds.value().unwrap().x, &80) && eq(&through_wall_bounds.value().unwrap().y, &320));
    assert!(through_wall_commands.ok() && eq(&through_wall_commands.value().len(), &1));
    assert!(eq(&through_wall_commands.value()[0].x, &80) && !through_wall_commands.value()[0].flip_x);

    let mut reversed_through_wall_input = barrel_input.clone();
    reversed_through_wall_input.settings = 2 << 20;
    through_wall_bounds = bounds(through_wall_lift, &reversed_through_wall_input, false);
    through_wall_commands = render(through_wall_lift, &reversed_through_wall_input);
    assert!(through_wall_bounds.ok() && through_wall_bounds.value().is_some() && eq(&through_wall_bounds.value().unwrap().x, &160));
    assert!(through_wall_commands.ok() && eq(&through_wall_commands.value()[0].x, &160));
    assert!(through_wall_commands.value()[0].flip_x);

    // Snowy branch selects the left art only for exact value one.
    let mut snowy_branch_input = barrel_input.clone();
    snowy_branch_input.settings = 1;
    let snowy_branch = shipped_catalog.by_object_id(212).unwrap();
    let mut snowy_branch_bounds = bounds(snowy_branch, &snowy_branch_input, false);
    let mut snowy_branch_commands = render(snowy_branch, &snowy_branch_input);
    assert!(snowy_branch_bounds.ok() && snowy_branch_bounds.value().is_some());
    assert!(eq(&snowy_branch_bounds.value().unwrap().x, &129) && eq(&snowy_branch_bounds.value().unwrap().y, &308));
    assert!(eq(&snowy_branch_bounds.value().unwrap().width, &34) && eq(&snowy_branch_bounds.value().unwrap().height, &23));
    assert!(snowy_branch_commands.ok() && eq(&snowy_branch_commands.value()[0].x, &129));
    let left_branch_image = snowy_branch_commands.value()[0].image.clone();

    snowy_branch_input.settings = 3;
    snowy_branch_bounds = bounds(snowy_branch, &snowy_branch_input, false);
    snowy_branch_commands = render(snowy_branch, &snowy_branch_input);
    assert!(snowy_branch_bounds.ok() && snowy_branch_bounds.value().is_some());
    assert!(eq(&snowy_branch_bounds.value().unwrap().x, &175) && eq(&snowy_branch_bounds.value().unwrap().y, &308));
    assert!(eq(&snowy_branch_bounds.value().unwrap().width, &30) && eq(&snowy_branch_bounds.value().unwrap().height, &23));
    assert!(snowy_branch_commands.ok() && !eq(&snowy_branch_commands.value()[0].image, &left_branch_image));

    // The one-way gate preserves the original non-composing transform helper,
    // including quarter-turn half-pixel commands and bounds mismatches.
    let one_way_gate = shipped_catalog.by_object_id(213).unwrap();
    let gate_bounds_x: [f64; 8] = [160.0, 160.0, 160.0, 160.0, 176.0, 176.0, 121.0, 121.0];
    let gate_bounds_y: [f64; 8] = [281.0, 281.0, 336.0, 336.0, 320.0, 320.0, 320.0, 320.0];
    let gate_bounds_width: [f64; 8] = [16.0, 16.0, 16.0, 16.0, 39.0, 39.0, 39.0, 39.0];
    let gate_bounds_height: [f64; 8] = [39.0, 39.0, 39.0, 39.0, 16.0, 16.0, 16.0, 16.0];
    let gate_command_x: [f64; 8] = [160.0, 160.0, 160.0, 160.0, 176.0, 187.5, 121.0, 132.5];
    let gate_command_y: [f64; 8] = [281.0, 281.0, 336.0, 336.0, 320.0, 308.5, 320.0, 308.5];
    let gate_rotation: [f64; 8] = [0.0, 0.0, 0.0, 0.0, 0.0, 270.0, 0.0, 270.0];
    let gate_flip_x: [bool; 8] = [true, false, true, false, false, false, false, true];
    let gate_flip_y: [bool; 8] = [false, true, false, false, true, false, true, false];
    for gate_type in 0..8 {
        let mut gate_input = barrel_input.clone();
        gate_input.settings = gate_type;
        let mut gate_bounds = bounds(one_way_gate, &gate_input, false);
        let mut gate_commands = render(one_way_gate, &gate_input);
        assert!(gate_bounds.ok() && gate_bounds.value().is_some());
        assert!(
            eq(&gate_bounds.value().unwrap().x, &gate_bounds_x[gate_type as usize])
                && eq(&gate_bounds.value().unwrap().y, &gate_bounds_y[gate_type as usize])
        );
        assert!(eq(&gate_bounds.value().unwrap().width, &gate_bounds_width[gate_type as usize]));
        assert!(eq(&gate_bounds.value().unwrap().height, &gate_bounds_height[gate_type as usize]));
        assert!(gate_commands.ok() && eq(&gate_commands.value().len(), &1));
        assert!(eq(&gate_commands.value()[0].x, &gate_command_x[gate_type as usize]));
        assert!(eq(&gate_commands.value()[0].y, &gate_command_y[gate_type as usize]));
        assert!(eq(&gate_commands.value()[0].rotation, &gate_rotation[gate_type as usize]));
        assert!(eq(&gate_commands.value()[0].flip_x, &gate_flip_x[gate_type as usize]));
        assert!(eq(&gate_commands.value()[0].flip_y, &gate_flip_y[gate_type as usize]));
    }
    let mut gate_high_nybble_input = barrel_input.clone();
    gate_high_nybble_input.settings = 0x10;
    let mut gate_high_nybble_bounds = bounds(one_way_gate, &gate_high_nybble_input, false);
    let mut gate_high_nybble_commands = render(one_way_gate, &gate_high_nybble_input);
    assert!(gate_high_nybble_bounds.ok() && gate_high_nybble_bounds.value().is_some());
    assert!(eq(&gate_high_nybble_bounds.value().unwrap().y, &281));
    assert!(gate_high_nybble_commands.ok() && eq(&gate_high_nybble_commands.value()[0].x, &160));
    assert!(eq(&gate_high_nybble_commands.value()[0].y, &320));
    assert!(!gate_high_nybble_commands.value()[0].flip_x && !gate_high_nybble_commands.value()[0].flip_y);

    let mut snake_block_input = barrel_input.clone();
    snake_block_input.settings = 2 << 12;
    let snake_block = shipped_catalog.by_object_id(214).unwrap();
    let mut snake_block_bounds = bounds(snake_block, &snake_block_input, false);
    let mut snake_block_commands = render(snake_block, &snake_block_input);
    assert!(snake_block_bounds.ok() && snake_block_bounds.value().is_some());
    assert!(eq(&snake_block_bounds.value().unwrap().x, &160) && eq(&snake_block_bounds.value().unwrap().y, &320));
    assert!(eq(&snake_block_bounds.value().unwrap().width, &80) && eq(&snake_block_bounds.value().unwrap().height, &16));
    assert!(snake_block_commands.ok() && eq(&snake_block_commands.value().len(), &5));
    for block in 0..5 {
        assert!(eq(&snake_block_commands.value()[block].x, &(160 + 16 * block)));
        assert!(eq(&snake_block_commands.value()[block].y, &320));
    }

    // Slow and fast spike pillars share exact directional geometry. Horizontal
    // variants retain the 40px gfx.blit quarter-turn correction.
    let spike_pillar_ids: [u32; 8] = [216, 217, 218, 219, 220, 221, 222, 223];
    let spike_pillar_bounds_x: [f64; 8] = [160.0, 160.0, 160.0, 32.0, 160.0, 160.0, 160.0, 32.0];
    let spike_pillar_bounds_y: [f64; 8] = [192.0, 320.0, 320.0, 320.0, 192.0, 320.0, 320.0, 320.0];
    let spike_pillar_bounds_width: [f64; 8] = [64.0, 64.0, 144.0, 144.0, 64.0, 64.0, 144.0, 144.0];
    let spike_pillar_bounds_height: [f64; 8] = [144.0, 144.0, 64.0, 64.0, 144.0, 144.0, 64.0, 64.0];
    let spike_pillar_command_x: [f64; 8] = [160.0, 160.0, 200.0, 72.0, 160.0, 160.0, 200.0, 72.0];
    let spike_pillar_command_y: [f64; 8] = [192.0, 320.0, 280.0, 280.0, 192.0, 320.0, 280.0, 280.0];
    let spike_pillar_rotation: [f64; 8] = [0.0, 180.0, 90.0, 270.0, 0.0, 180.0, 90.0, 270.0];
    for pillar in 0..spike_pillar_ids.len() {
        let spike_pillar = shipped_catalog.by_object_id(spike_pillar_ids[pillar]).unwrap();
        let mut spike_pillar_bounds = bounds(spike_pillar, &barrel_input, false);
        let mut spike_pillar_commands = render(spike_pillar, &barrel_input);
        assert!(spike_pillar_bounds.ok() && spike_pillar_bounds.value().is_some());
        assert!(eq(&spike_pillar_bounds.value().unwrap().x, &spike_pillar_bounds_x[pillar]));
        assert!(eq(&spike_pillar_bounds.value().unwrap().y, &spike_pillar_bounds_y[pillar]));
        assert!(eq(&spike_pillar_bounds.value().unwrap().width, &spike_pillar_bounds_width[pillar]));
        assert!(eq(&spike_pillar_bounds.value().unwrap().height, &spike_pillar_bounds_height[pillar]));
        assert!(spike_pillar_commands.ok() && eq(&spike_pillar_commands.value().len(), &1));
        assert!(eq(&spike_pillar_commands.value()[0].x, &spike_pillar_command_x[pillar]));
        assert!(eq(&spike_pillar_commands.value()[0].y, &spike_pillar_command_y[pillar]));
        assert!(eq(&spike_pillar_commands.value()[0].width, &64) && eq(&spike_pillar_commands.value()[0].height, &144));
        assert!(eq(&spike_pillar_commands.value()[0].rotation, &spike_pillar_rotation[pillar]));
    }

    let whirlpool = shipped_catalog.by_object_id(226).unwrap();
    let mut whirlpool_bounds = bounds(whirlpool, &barrel_input, false);
    let mut whirlpool_commands = render(whirlpool, &barrel_input);
    assert!(whirlpool_bounds.ok() && whirlpool_bounds.value().is_some());
    assert!(eq(&whirlpool_bounds.value().unwrap().x, &82) && eq(&whirlpool_bounds.value().unwrap().y, &156));
    assert!(eq(&whirlpool_bounds.value().unwrap().width, &165) && eq(&whirlpool_bounds.value().unwrap().height, &182));
    assert!(whirlpool_commands.ok() && eq(&whirlpool_commands.value().len(), &1));
    assert!(eq(&whirlpool_commands.value()[0].x, &82) && eq(&whirlpool_commands.value()[0].y, &156));
    assert!(eq(&whirlpool_commands.value()[0].width, &165) && eq(&whirlpool_commands.value()[0].height, &182));

    // Swelling-ground bounds cover the base but intentionally exclude the
    // upward overlay drawn 112 pixels above it.
    let swelling_ground = shipped_catalog.by_object_id(231).unwrap();
    let mut swelling_ground_bounds = bounds(swelling_ground, &barrel_input, false);
    let mut swelling_ground_commands = render(swelling_ground, &barrel_input);
    assert!(swelling_ground_bounds.ok() && swelling_ground_bounds.value().is_some());
    assert!(eq(&swelling_ground_bounds.value().unwrap().x, &32) && eq(&swelling_ground_bounds.value().unwrap().y, &320));
    assert!(eq(&swelling_ground_bounds.value().unwrap().width, &276) && eq(&swelling_ground_bounds.value().unwrap().height, &128));
    assert!(swelling_ground_commands.ok() && eq(&swelling_ground_commands.value().len(), &2));
    assert!(eq(&swelling_ground_commands.value()[0].x, &32) && eq(&swelling_ground_commands.value()[0].y, &320));
    assert!(eq(&swelling_ground_commands.value()[1].x, &32) && eq(&swelling_ground_commands.value()[1].y, &208));
    let outward_swelling_image = swelling_ground_commands.value()[1].image.clone();

    let mut inward_swelling_input = barrel_input.clone();
    inward_swelling_input.settings = 1 << 24;
    swelling_ground_commands = render(swelling_ground, &inward_swelling_input);
    assert!(swelling_ground_commands.ok() && eq(&swelling_ground_commands.value()[1].y, &320));
    assert!(!eq(&swelling_ground_commands.value()[1].image, &outward_swelling_image));

    // Tightrope height is a signed nybble; -2 places the second endpoint below
    // the actor while bounds cover both 16px endpoint cells.
    let mut tightrope_input = barrel_input.clone();
    tightrope_input.settings = (3 << 28) | (0xe << 24);
    let tightrope = shipped_catalog.by_object_id(232).unwrap();
    let mut tightrope_bounds = bounds(tightrope, &tightrope_input, false);
    let mut tightrope_commands = render(tightrope, &tightrope_input);
    assert!(tightrope_bounds.ok() && tightrope_bounds.value().is_some());
    assert!(eq(&tightrope_bounds.value().unwrap().x, &152) && eq(&tightrope_bounds.value().unwrap().y, &320));
    assert!(eq(&tightrope_bounds.value().unwrap().width, &64) && eq(&tightrope_bounds.value().unwrap().height, &48));
    assert!(tightrope_commands.ok() && eq(&tightrope_commands.value().len(), &3));
    assert!(eq(&tightrope_commands.value()[0].kind, &LuaActorDrawKind::Line));
    assert!(eq(&tightrope_commands.value()[0].x, &160) && eq(&tightrope_commands.value()[0].y, &328));
    assert!(eq(&tightrope_commands.value()[0].x2, &208) && eq(&tightrope_commands.value()[0].y2, &360));
    assert!(eq(&tightrope_commands.value()[1].x, &152) && eq(&tightrope_commands.value()[1].y, &320));
    assert!(eq(&tightrope_commands.value()[2].x, &200) && eq(&tightrope_commands.value()[2].y, &352));

    // Burners preserve the port's up-facing bounds/render mismatch. The
    // direction field includes all six bits consumed by the original modulo.
    let burner_ids: [u32; 2] = [234, 235];
    let burner_bounds_y: [f64; 2] = [272.0, 208.0];
    let burner_bounds_height: [f64; 2] = [64.0, 128.0];
    let burner_command_x: [f64; 2] = [136.0, 104.0];
    let burner_command_y: [f64; 2] = [344.0, 376.0];
    let burner_command_width: [f64; 2] = [64.0, 128.0];
    for index in 0..burner_ids.len() {
        let mut burner_input = barrel_input.clone();
        burner_input.settings = 2;
        let burner = shipped_catalog.by_object_id(burner_ids[index]).unwrap();
        let mut burner_bounds = bounds(burner, &burner_input, false);
        let mut burner_commands = render(burner, &burner_input);
        assert!(burner_bounds.ok() && burner_bounds.value().is_some());
        assert!(eq(&burner_bounds.value().unwrap().x, &160) && eq(&burner_bounds.value().unwrap().y, &burner_bounds_y[index]));
        assert!(eq(&burner_bounds.value().unwrap().width, &16));
        assert!(eq(&burner_bounds.value().unwrap().height, &burner_bounds_height[index]));
        assert!(burner_commands.ok() && eq(&burner_commands.value().len(), &1));
        assert!(eq(&burner_commands.value()[0].x, &burner_command_x[index]));
        assert!(eq(&burner_commands.value()[0].y, &burner_command_y[index]));
        assert!(eq(&burner_commands.value()[0].width, &burner_command_width[index]));
        assert!(eq(&burner_commands.value()[0].height, &16));
        assert!(eq(&burner_commands.value()[0].rotation, &270));

        burner_input.settings = 4;
        burner_commands = render(burner, &burner_input);
        assert!(burner_commands.ok() && burner_commands.value().is_empty());
    }

    let mut trampoline_input = barrel_input.clone();
    trampoline_input.settings = 0x11;
    let trampoline = shipped_catalog.by_object_id(237).unwrap();
    let mut trampoline_bounds = bounds(trampoline, &trampoline_input, false);
    let mut trampoline_commands = render(trampoline, &trampoline_input);
    assert!(trampoline_bounds.ok() && trampoline_bounds.value().is_some());
    assert!(eq(&trampoline_bounds.value().unwrap().x, &160) && eq(&trampoline_bounds.value().unwrap().y, &304));
    assert!(eq(&trampoline_bounds.value().unwrap().width, &32) && eq(&trampoline_bounds.value().unwrap().height, &32));
    assert!(trampoline_commands.ok() && eq(&trampoline_commands.value().len(), &1));
    assert!(eq(&trampoline_commands.value()[0].x, &160) && eq(&trampoline_commands.value()[0].y, &304));
    assert!(eq(&trampoline_commands.value()[0].width, &32) && eq(&trampoline_commands.value()[0].height, &32));

    let mut question_switch_input = barrel_input.clone();
    question_switch_input.settings = 0x11;
    let question_switch = shipped_catalog.by_object_id(242).unwrap();
    let mut question_switch_bounds = bounds(question_switch, &question_switch_input, false);
    let mut question_switch_commands = render(question_switch, &question_switch_input);
    assert!(question_switch_bounds.ok() && question_switch_bounds.value().is_some());
    assert!(eq(&question_switch_bounds.value().unwrap().x, &168) && eq(&question_switch_bounds.value().unwrap().y, &320));
    assert!(eq(&question_switch_bounds.value().unwrap().width, &16) && eq(&question_switch_bounds.value().unwrap().height, &18));
    assert!(question_switch_commands.ok() && eq(&question_switch_commands.value().len(), &1));
    assert!(eq(&question_switch_commands.value()[0].x, &168) && eq(&question_switch_commands.value()[0].y, &320));
    assert!(eq(&question_switch_commands.value()[0].rotation, &180));

    let p_switch = shipped_catalog.by_object_id(244).unwrap();
    let mut p_switch_bounds = bounds(p_switch, &question_switch_input, false);
    let mut p_switch_commands = render(p_switch, &question_switch_input);
    assert!(p_switch_bounds.ok() && p_switch_bounds.value().is_some());
    assert!(eq(&p_switch_bounds.value().unwrap().x, &168) && eq(&p_switch_bounds.value().unwrap().y, &320));
    assert!(eq(&p_switch_bounds.value().unwrap().width, &16) && eq(&p_switch_bounds.value().unwrap().height, &18));
    assert!(p_switch_commands.ok() && eq(&p_switch_commands.value().len(), &1));
    assert!(eq(&p_switch_commands.value()[0].x, &168) && eq(&p_switch_commands.value()[0].y, &320));
    assert!(eq(&p_switch_commands.value()[0].rotation, &180));

    let red_switch = shipped_catalog.by_object_id(246).unwrap();
    let mut red_switch_bounds = bounds(red_switch, &question_switch_input, false);
    let mut red_switch_commands = render(red_switch, &question_switch_input);
    assert!(red_switch_bounds.ok() && red_switch_bounds.value().is_some());
    assert!(eq(&red_switch_bounds.value().unwrap().x, &168) && eq(&red_switch_bounds.value().unwrap().y, &320));
    assert!(eq(&red_switch_bounds.value().unwrap().width, &16) && eq(&red_switch_bounds.value().unwrap().height, &18));
    assert!(red_switch_commands.ok() && eq(&red_switch_commands.value().len(), &1));
    assert!(eq(&red_switch_commands.value()[0].x, &168) && eq(&red_switch_commands.value()[0].y, &320));
    assert!(eq(&red_switch_commands.value()[0].rotation, &180));

    let mut bill_blaster_input = barrel_input.clone();
    bill_blaster_input.settings = 3 << 4;
    let bill_blaster = shipped_catalog.by_object_id(248).unwrap();
    let mut bill_blaster_bounds = bounds(bill_blaster, &bill_blaster_input, false);
    let mut bill_blaster_commands = render(bill_blaster, &bill_blaster_input);
    assert!(bill_blaster_bounds.ok() && bill_blaster_bounds.value().is_some());
    assert!(eq(&bill_blaster_bounds.value().unwrap().x, &160) && eq(&bill_blaster_bounds.value().unwrap().y, &256));
    assert!(eq(&bill_blaster_bounds.value().unwrap().width, &16) && eq(&bill_blaster_bounds.value().unwrap().height, &80));
    assert!(bill_blaster_commands.ok() && eq(&bill_blaster_commands.value().len(), &4));
    assert!(eq(&bill_blaster_commands.value()[0].y, &320));
    assert!(eq(&bill_blaster_commands.value()[1].y, &304));
    assert!(eq(&bill_blaster_commands.value()[2].y, &288));
    assert!(eq(&bill_blaster_commands.value()[3].y, &256));
    assert!(!eq(&bill_blaster_commands.value()[0].image, &bill_blaster_commands.value()[3].image));

    let banzai_blaster = shipped_catalog.by_object_id(249).unwrap();
    let mut banzai_blaster_bounds = bounds(banzai_blaster, &barrel_input, false);
    let mut banzai_blaster_commands = render(banzai_blaster, &barrel_input);
    assert!(banzai_blaster_bounds.ok() && banzai_blaster_bounds.value().is_some());
    assert!(eq(&banzai_blaster_bounds.value().unwrap().x, &128) && eq(&banzai_blaster_bounds.value().unwrap().y, &256));
    assert!(eq(&banzai_blaster_bounds.value().unwrap().width, &64) && eq(&banzai_blaster_bounds.value().unwrap().height, &80));
    assert!(banzai_blaster_commands.ok() && eq(&banzai_blaster_commands.value().len(), &1));
    assert!(eq(&banzai_blaster_commands.value()[0].x, &128) && eq(&banzai_blaster_commands.value()[0].y, &256));
    assert!(eq(&banzai_blaster_commands.value()[0].width, &64));
    assert!(eq(&banzai_blaster_commands.value()[0].height, &80));

    let hanging_block = shipped_catalog.by_object_id(250).unwrap();
    let mut hanging_block_bounds = bounds(hanging_block, &barrel_input, false);
    let mut hanging_block_commands = render(hanging_block, &barrel_input);
    assert!(hanging_block_bounds.ok() && hanging_block_bounds.value().is_some());
    assert!(eq(&hanging_block_bounds.value().unwrap().x, &160) && eq(&hanging_block_bounds.value().unwrap().y, &240));
    assert!(eq(&hanging_block_bounds.value().unwrap().width, &16) && eq(&hanging_block_bounds.value().unwrap().height, &144));
    assert!(hanging_block_commands.ok() && eq(&hanging_block_commands.value().len(), &4));
    assert!(eq(&hanging_block_commands.value()[0].x, &160) && eq(&hanging_block_commands.value()[0].y, &368));
    assert!(eq(&hanging_block_commands.value()[1].x, &160) && eq(&hanging_block_commands.value()[1].y, &368));
    assert!(eq(&hanging_block_commands.value()[1].source_x, &0));
    assert!(eq(&hanging_block_commands.value()[2].kind, &LuaActorDrawKind::Line));
    assert!(eq(&hanging_block_commands.value()[2].x, &167) && eq(&hanging_block_commands.value()[2].y, &368));
    assert!(eq(&hanging_block_commands.value()[2].x2, &167) && eq(&hanging_block_commands.value()[2].y2, &240));
    assert!(eq(&hanging_block_commands.value()[3].x, &168) && eq(&hanging_block_commands.value()[3].x2, &168));

    let mut spiked_block_input = barrel_input.clone();
    spiked_block_input.settings = 2;
    let unused_spiked_block = shipped_catalog.by_object_id(251).unwrap();
    let mut unused_spiked_block_bounds = bounds(unused_spiked_block, &spiked_block_input, false);
    let mut unused_spiked_block_commands = render(unused_spiked_block, &spiked_block_input);
    assert!(unused_spiked_block_bounds.ok() && unused_spiked_block_bounds.value().is_some());
    assert!(eq(&unused_spiked_block_bounds.value().unwrap().x, &158) && eq(&unused_spiked_block_bounds.value().unwrap().y, &313));
    assert!(eq(&unused_spiked_block_bounds.value().unwrap().width, &19) && eq(&unused_spiked_block_bounds.value().unwrap().height, &26));
    assert!(unused_spiked_block_commands.ok() && eq(&unused_spiked_block_commands.value().len(), &2));
    assert!(eq(&unused_spiked_block_commands.value()[0].x, &158) && eq(&unused_spiked_block_commands.value()[0].y, &313));
    assert!(eq(&unused_spiked_block_commands.value()[1].x, &160) && eq(&unused_spiked_block_commands.value()[1].y, &321));
    assert!(eq(&unused_spiked_block_commands.value()[1].source_x, &48));

    spiked_block_input.settings = 4;
    let spiked_block = shipped_catalog.by_object_id(252).unwrap();
    let mut spiked_block_bounds = bounds(spiked_block, &spiked_block_input, false);
    let mut spiked_block_commands = render(spiked_block, &spiked_block_input);
    assert!(spiked_block_bounds.ok() && spiked_block_bounds.value().is_some());
    assert!(eq(&spiked_block_bounds.value().unwrap().x, &158) && eq(&spiked_block_bounds.value().unwrap().y, &313));
    assert!(eq(&spiked_block_bounds.value().unwrap().width, &19) && eq(&spiked_block_bounds.value().unwrap().height, &26));
    assert!(spiked_block_commands.ok() && eq(&spiked_block_commands.value().len(), &1));
    assert!(eq(&spiked_block_commands.value()[0].x, &158) && eq(&spiked_block_commands.value()[0].y, &313));

    spiked_block_input.settings = 3;
    let second_unused_spiked_block = shipped_catalog.by_object_id(253).unwrap();
    let mut second_unused_spiked_block_commands = render(second_unused_spiked_block, &spiked_block_input);
    assert!(second_unused_spiked_block_commands.ok() && eq(&second_unused_spiked_block_commands.value().len(), &2));
    assert!(eq(&second_unused_spiked_block_commands.value()[0].x, &158));
    assert!(eq(&second_unused_spiked_block_commands.value()[0].y, &313));
    assert!(eq(&second_unused_spiked_block_commands.value()[1].x, &160));
    assert!(eq(&second_unused_spiked_block_commands.value()[1].y, &321));
    assert!(eq(&second_unused_spiked_block_commands.value()[1].source_x, &48));

    let spin_block = shipped_catalog.by_object_id(254).unwrap();
    let mut spin_block_bounds = bounds(spin_block, &barrel_input, false);
    let mut spin_block_commands = render(spin_block, &barrel_input);
    assert!(spin_block_bounds.ok() && spin_block_bounds.value().is_some());
    assert!(eq(&spin_block_bounds.value().unwrap().x, &154) && eq(&spin_block_bounds.value().unwrap().y, &320));
    assert!(eq(&spin_block_bounds.value().unwrap().width, &26) && eq(&spin_block_bounds.value().unwrap().height, &26));
    assert!(spin_block_commands.ok() && eq(&spin_block_commands.value().len(), &1));
    assert!(eq(&spin_block_commands.value()[0].x, &154) && eq(&spin_block_commands.value()[0].y, &320));
    assert!(eq(&spin_block_commands.value()[0].width, &26) && eq(&spin_block_commands.value()[0].height, &26));

    let unused_spin_block = shipped_catalog.by_object_id(255).unwrap();
    let mut unused_spin_block_commands = render(unused_spin_block, &barrel_input);
    assert!(unused_spin_block_commands.ok() && eq(&unused_spin_block_commands.value().len(), &1));
    assert!(eq(&unused_spin_block_commands.value()[0].x, &154));
    assert!(eq(&unused_spin_block_commands.value()[0].y, &320));
    assert!(eq(&unused_spin_block_commands.value()[0].width, &26));
    assert!(eq(&unused_spin_block_commands.value()[0].height, &26));

    let second_unused_spin_block = shipped_catalog.by_object_id(256).unwrap();
    let mut second_unused_spin_block_commands = render(second_unused_spin_block, &barrel_input);
    assert!(second_unused_spin_block_commands.ok() && eq(&second_unused_spin_block_commands.value().len(), &1));
    assert!(eq(&second_unused_spin_block_commands.value()[0].x, &154));
    assert!(eq(&second_unused_spin_block_commands.value()[0].y, &320));
    assert!(eq(&second_unused_spin_block_commands.value()[0].width, &26));
    assert!(eq(&second_unused_spin_block_commands.value()[0].height, &26));

    let mut hanging_question_block_input = barrel_input.clone();
    hanging_question_block_input.settings = 9;
    let hanging_question_block = shipped_catalog.by_object_id(258).unwrap();
    let mut hanging_question_block_bounds = bounds(hanging_question_block, &hanging_question_block_input, false);
    let mut hanging_question_block_commands = render(hanging_question_block, &hanging_question_block_input);
    assert!(hanging_question_block_bounds.ok() && hanging_question_block_bounds.value().is_some());
    assert!(eq(&hanging_question_block_bounds.value().unwrap().x, &160) && eq(&hanging_question_block_bounds.value().unwrap().y, &192));
    assert!(eq(&hanging_question_block_bounds.value().unwrap().width, &16));
    assert!(eq(&hanging_question_block_bounds.value().unwrap().height, &144));
    assert!(hanging_question_block_commands.ok() && eq(&hanging_question_block_commands.value().len(), &4));
    assert!(eq(&hanging_question_block_commands.value()[0].x, &160));
    assert!(eq(&hanging_question_block_commands.value()[1].source_x, &144));
    assert!(eq(&hanging_question_block_commands.value()[2].x, &167));
    assert!(eq(&hanging_question_block_commands.value()[2].y, &192));
    assert!(eq(&hanging_question_block_commands.value()[2].y2, &320));

    let mut blockhopper_input = barrel_input.clone();
    blockhopper_input.settings = 0x332;
    let blockhopper = shipped_catalog.by_object_id(259).unwrap();
    let mut blockhopper_bounds = bounds(blockhopper, &blockhopper_input, false);
    let mut blockhopper_commands = render(blockhopper, &blockhopper_input);
    assert!(blockhopper_bounds.ok() && blockhopper_bounds.value().is_some());
    assert!(eq(&blockhopper_bounds.value().unwrap().x, &160) && eq(&blockhopper_bounds.value().unwrap().y, &274));
    assert!(eq(&blockhopper_bounds.value().unwrap().width, &16) && eq(&blockhopper_bounds.value().unwrap().height, &62));
    assert!(blockhopper_commands.ok() && eq(&blockhopper_commands.value().len(), &4));
    assert!(eq(&blockhopper_commands.value()[0].x, &160) && eq(&blockhopper_commands.value()[0].y, &306));
    assert!(eq(&blockhopper_commands.value()[0].width, &16) && eq(&blockhopper_commands.value()[0].height, &30));
    assert!(eq(&blockhopper_commands.value()[1].x, &160) && eq(&blockhopper_commands.value()[1].y, &309));
    assert!(eq(&blockhopper_commands.value()[1].source_x, &48));
    assert!(eq(&blockhopper_commands.value()[2].y, &290));
    assert!(eq(&blockhopper_commands.value()[3].y, &274));

    let mut flying_block_input = barrel_input.clone();
    flying_block_input.settings = 7;
    let flying_block = shipped_catalog.by_object_id(260).unwrap();
    let mut flying_block_bounds = bounds(flying_block, &flying_block_input, false);
    let mut flying_block_commands = render(flying_block, &flying_block_input);
    assert!(flying_block_bounds.ok() && flying_block_bounds.value().is_some());
    assert!(eq(&flying_block_bounds.value().unwrap().x, &151) && eq(&flying_block_bounds.value().unwrap().y, &307));
    assert!(eq(&flying_block_bounds.value().unwrap().width, &38) && eq(&flying_block_bounds.value().unwrap().height, &28));
    assert!(flying_block_commands.ok() && eq(&flying_block_commands.value().len(), &2));
    assert!(eq(&flying_block_commands.value()[0].x, &151) && eq(&flying_block_commands.value()[0].y, &307));
    assert!(eq(&flying_block_commands.value()[1].x, &160) && eq(&flying_block_commands.value()[1].y, &320));
    assert!(eq(&flying_block_commands.value()[1].source_x, &112));

    let map_flying_block = shipped_catalog.by_object_id(261).unwrap();
    let mut map_flying_block_bounds = bounds(map_flying_block, &barrel_input, false);
    let mut map_flying_block_commands = render(map_flying_block, &barrel_input);
    assert!(map_flying_block_bounds.ok() && map_flying_block_bounds.value().is_some());
    assert!(eq(&map_flying_block_bounds.value().unwrap().x, &148) && eq(&map_flying_block_bounds.value().unwrap().y, &312));
    assert!(eq(&map_flying_block_bounds.value().unwrap().width, &44) && eq(&map_flying_block_bounds.value().unwrap().height, &23));
    assert!(map_flying_block_commands.ok() && eq(&map_flying_block_commands.value().len(), &1));
    assert!(eq(&map_flying_block_commands.value()[0].x, &148) && eq(&map_flying_block_commands.value()[0].y, &312));
    assert!(eq(&map_flying_block_commands.value()[0].width, &44));
    assert!(eq(&map_flying_block_commands.value()[0].height, &23));

    let mut rotating_blaster_input = barrel_input.clone();
    rotating_blaster_input.settings = 0x000B0507;
    let rotating_blaster = shipped_catalog.by_object_id(262).unwrap();
    let mut rotating_blaster_bounds = bounds(rotating_blaster, &rotating_blaster_input, false);
    let mut rotating_blaster_commands = render(rotating_blaster, &rotating_blaster_input);
    assert!(rotating_blaster_bounds.ok() && rotating_blaster_bounds.value().is_some());
    assert!(eq(&rotating_blaster_bounds.value().unwrap().x, &157) && eq(&rotating_blaster_bounds.value().unwrap().y, &272));
    assert!(eq(&rotating_blaster_bounds.value().unwrap().width, &23) && eq(&rotating_blaster_bounds.value().unwrap().height, &64));
    assert!(rotating_blaster_commands.ok() && eq(&rotating_blaster_commands.value().len(), &4));
    assert!(eq(&rotating_blaster_commands.value()[0].x, &162) && eq(&rotating_blaster_commands.value()[0].y, &320));
    assert!(eq(&rotating_blaster_commands.value()[1].x, &157) && eq(&rotating_blaster_commands.value()[1].y, &304));
    assert!(rotating_blaster_commands.value()[1].flip_x);
    assert!(eq(&rotating_blaster_commands.value()[2].x, &162) && eq(&rotating_blaster_commands.value()[2].y, &288));
    assert!(eq(&rotating_blaster_commands.value()[3].x, &161) && eq(&rotating_blaster_commands.value()[3].y, &272));
    assert!(!rotating_blaster_commands.value()[3].flip_x);

    let event_door = shipped_catalog.by_object_id(263).unwrap();
    let mut event_door_bounds = bounds(event_door, &barrel_input, false);
    let mut event_door_commands = render(event_door, &barrel_input);
    assert!(event_door_bounds.ok() && event_door_bounds.value().is_some());
    assert!(eq(&event_door_bounds.value().unwrap().x, &160) && eq(&event_door_bounds.value().unwrap().y, &288));
    assert!(eq(&event_door_bounds.value().unwrap().width, &32) && eq(&event_door_bounds.value().unwrap().height, &48));
    assert!(event_door_commands.ok() && eq(&event_door_commands.value().len(), &1));
    assert!(eq(&event_door_commands.value()[0].x, &160) && eq(&event_door_commands.value()[0].y, &288));
    assert!(eq(&event_door_commands.value()[0].width, &32) && eq(&event_door_commands.value()[0].height, &48));

    let mut toad_house_block_input = barrel_input.clone();
    toad_house_block_input.settings = 0x30000034;
    let toad_house_block = shipped_catalog.by_object_id(264).unwrap();
    let mut toad_house_block_bounds = bounds(toad_house_block, &toad_house_block_input, false);
    let mut toad_house_block_commands = render(toad_house_block, &toad_house_block_input);
    assert!(toad_house_block_bounds.ok() && toad_house_block_bounds.value().is_some());
    assert!(eq(&toad_house_block_bounds.value().unwrap().x, &156) && eq(&toad_house_block_bounds.value().unwrap().y, &308));
    assert!(eq(&toad_house_block_bounds.value().unwrap().width, &40) && eq(&toad_house_block_bounds.value().unwrap().height, &40));
    assert!(toad_house_block_commands.ok() && eq(&toad_house_block_commands.value().len(), &1));
    assert!(eq(&toad_house_block_commands.value()[0].x, &156) && eq(&toad_house_block_commands.value()[0].y, &308));
    assert!(eq(&toad_house_block_commands.value()[0].width, &40));
    assert!(eq(&toad_house_block_commands.value()[0].height, &40));

    let bowser_switch = shipped_catalog.by_object_id(265).unwrap();
    let mut bowser_switch_bounds = bounds(bowser_switch, &barrel_input, false);
    let mut bowser_switch_commands = render(bowser_switch, &barrel_input);
    assert!(bowser_switch_bounds.ok() && bowser_switch_bounds.value().is_some());
    assert!(eq(&bowser_switch_bounds.value().unwrap().x, &164) && eq(&bowser_switch_bounds.value().unwrap().y, &313));
    assert!(eq(&bowser_switch_bounds.value().unwrap().width, &24) && eq(&bowser_switch_bounds.value().unwrap().height, &23));
    assert!(bowser_switch_commands.ok() && eq(&bowser_switch_commands.value().len(), &1));
    assert!(eq(&bowser_switch_commands.value()[0].x, &164) && eq(&bowser_switch_commands.value()[0].y, &313));
    assert!(eq(&bowser_switch_commands.value()[0].width, &24));
    assert!(eq(&bowser_switch_commands.value()[0].height, &23));

    let mut warp_cannon_input = barrel_input.clone();
    warp_cannon_input.settings = 8 << 28;
    let warp_cannon = shipped_catalog.by_object_id(269).unwrap();
    let mut warp_cannon_bounds = bounds(warp_cannon, &warp_cannon_input, false);
    let mut warp_cannon_commands = render(warp_cannon, &warp_cannon_input);
    assert!(warp_cannon_bounds.ok() && warp_cannon_bounds.value().is_some());
    assert!(eq(&warp_cannon_bounds.value().unwrap().x, &151) && eq(&warp_cannon_bounds.value().unwrap().y, &294));
    assert!(eq(&warp_cannon_bounds.value().unwrap().width, &66) && eq(&warp_cannon_bounds.value().unwrap().height, &90));
    assert!(warp_cannon_commands.ok() && eq(&warp_cannon_commands.value().len(), &2));
    assert!(eq(&warp_cannon_commands.value()[0].x, &151) && eq(&warp_cannon_commands.value()[0].y, &294));
    assert!(eq(&warp_cannon_commands.value()[1].x, &186) && eq(&warp_cannon_commands.value()[1].y, &343));
    assert!(eq(&warp_cannon_commands.value()[1].source_x, &54));
    assert!(eq(&warp_cannon_commands.value()[1].source_width, &18));
    assert!(eq(&warp_cannon_commands.value()[1].source_height, &19));

    let mut warp_entrance_input = barrel_input.clone();
    warp_entrance_input.settings = (2 << 8) | (3 << 4);
    let warp_entrance = shipped_catalog.by_object_id(87).unwrap();
    let mut warp_entrance_bounds = bounds(warp_entrance, &warp_entrance_input, false);
    let mut warp_entrance_commands = render(warp_entrance, &warp_entrance_input);
    assert!(warp_entrance_bounds.ok() && warp_entrance_bounds.value().is_some());
    assert!(eq(&warp_entrance_bounds.value().unwrap().x, &160) && eq(&warp_entrance_bounds.value().unwrap().y, &320));
    assert!(eq(&warp_entrance_bounds.value().unwrap().width, &48) && eq(&warp_entrance_bounds.value().unwrap().height, &64));
    assert!(warp_entrance_commands.ok() && eq(&warp_entrance_commands.value().len(), &3));
    assert!(eq(&warp_entrance_commands.value()[0].kind, &LuaActorDrawKind::StrokeRect));
    assert!(eq(&warp_entrance_commands.value()[0].x, &161) && eq(&warp_entrance_commands.value()[0].y, &321));
    assert!(eq(&warp_entrance_commands.value()[1].kind, &LuaActorDrawKind::StrokeRect));
    assert!(eq(&warp_entrance_commands.value()[1].x, &160) && eq(&warp_entrance_commands.value()[1].y, &320));
    assert!(eq(&warp_entrance_commands.value()[2].kind, &LuaActorDrawKind::Text));
    assert!(eq(&warp_entrance_commands.value()[2].x, &160) && eq(&warp_entrance_commands.value()[2].y, &320));

    let mut unused_purple_mushroom_input = barrel_input.clone();
    unused_purple_mushroom_input.settings = 0x30;
    let unused_purple_mushroom = shipped_catalog.by_object_id(105).unwrap();
    let mut unused_purple_mushroom_bounds = bounds(unused_purple_mushroom, &unused_purple_mushroom_input, false);
    let mut unused_purple_mushroom_commands = render(unused_purple_mushroom, &unused_purple_mushroom_input);
    assert!(unused_purple_mushroom_bounds.ok() && unused_purple_mushroom_bounds.value().is_some());
    assert!(eq(&unused_purple_mushroom_bounds.value().unwrap().x, &96));
    assert!(eq(&unused_purple_mushroom_bounds.value().unwrap().y, &320));
    assert!(eq(&unused_purple_mushroom_bounds.value().unwrap().width, &160));
    assert!(eq(&unused_purple_mushroom_bounds.value().unwrap().height, &112));
    assert!(unused_purple_mushroom_commands.ok() && eq(&unused_purple_mushroom_commands.value().len(), &11));
    assert!(eq(&unused_purple_mushroom_commands.value()[0].x, &166));
    assert!(eq(&unused_purple_mushroom_commands.value()[0].y, &344));
    for section in 0..5 {
        assert!(eq(&unused_purple_mushroom_commands.value()[section + 1].x, &166));
        assert!(eq(&unused_purple_mushroom_commands.value()[section + 1].y, &(352 + 16 * section)));
    }
    assert!(eq(&unused_purple_mushroom_commands.value()[6].x, &96));
    assert!(eq(&unused_purple_mushroom_commands.value()[7].x, &128));
    assert!(eq(&unused_purple_mushroom_commands.value()[8].x, &160));
    assert!(eq(&unused_purple_mushroom_commands.value()[9].x, &192));
    assert!(eq(&unused_purple_mushroom_commands.value()[10].x, &224));
    assert!(unused_purple_mushroom_commands.value()[10].flip_x);

    let mut zoom_input = barrel_input.clone();
    zoom_input.settings = (2 << 20) | (1 << 16) | (1 << 5);
    let zoom = shipped_catalog.by_object_id(49).unwrap();
    let mut zoom_bounds = bounds(zoom, &zoom_input, false);
    let mut zoom_commands = render(zoom, &zoom_input);
    assert!(zoom_bounds.ok() && zoom_bounds.value().is_some());
    assert!(eq(&zoom_bounds.value().unwrap().x, &128) && eq(&zoom_bounds.value().unwrap().y, &304));
    assert!(eq(&zoom_bounds.value().unwrap().width, &64) && eq(&zoom_bounds.value().unwrap().height, &16));
    assert!(zoom_commands.ok() && eq(&zoom_commands.value().len(), &2));
    assert!(eq(&zoom_commands.value()[0].x, &128) && eq(&zoom_commands.value()[0].y, &304));
    assert!(eq(&zoom_commands.value()[1].x, &176) && eq(&zoom_commands.value()[1].y, &304));

    let mut infinite_zoom_bounds = bounds(zoom, &barrel_input, false);
    let mut infinite_zoom_commands = render(zoom, &barrel_input);
    assert!(infinite_zoom_bounds.ok() && infinite_zoom_bounds.value().is_some());
    assert!(eq(&infinite_zoom_bounds.value().unwrap().x, &32) && eq(&infinite_zoom_bounds.value().unwrap().y, &304));
    assert!(eq(&infinite_zoom_bounds.value().unwrap().width, &256) && eq(&infinite_zoom_bounds.value().unwrap().height, &16));
    assert!(infinite_zoom_commands.ok() && eq(&infinite_zoom_commands.value().len(), &24));
    assert!(eq(&infinite_zoom_commands.value()[16].x, &16) && eq(&infinite_zoom_commands.value()[16].y, &304));
    assert!(eq(&infinite_zoom_commands.value()[16].rotation, &90));
    assert!(eq(&infinite_zoom_commands.value()[17].x, &288) && eq(&infinite_zoom_commands.value()[17].rotation, &270));
    assert!(eq(&infinite_zoom_commands.value()[20].x, &152) && eq(&infinite_zoom_commands.value()[20].y, &320));
    assert!(eq(&infinite_zoom_commands.value()[21].y, &288) && eq(&infinite_zoom_commands.value()[21].rotation, &180));

    zoom_input.settings = (1 << 20) | (1 << 16) | (1 << 5);
    zoom_bounds = bounds(zoom, &zoom_input, false);
    zoom_commands = render(zoom, &zoom_input);
    assert!(zoom_bounds.ok() && zoom_bounds.value().is_some());
    assert!(eq(&zoom_bounds.value().unwrap().x, &160) && eq(&zoom_bounds.value().unwrap().y, &320));
    assert!(eq(&zoom_bounds.value().unwrap().width, &72) && eq(&zoom_bounds.value().unwrap().height, &28));
    assert!(zoom_commands.ok() && eq(&zoom_commands.value().len(), &2));
    assert!(eq(&zoom_commands.value()[0].kind, &LuaActorDrawKind::FillRect));
    assert!(eq(&zoom_commands.value()[1].kind, &LuaActorDrawKind::Text));

    // Chain Chomp draws the post first, then expands the selection to include
    // the Chomp layered in front when the count is one.
    let mut chain_chomp_input = barrel_input.clone();
    chain_chomp_input.settings = 1;
    let chain_chomp = shipped_catalog.by_object_id(44).unwrap();
    let mut chain_chomp_bounds = bounds(chain_chomp, &chain_chomp_input, false);
    let mut chain_chomp_commands = render(chain_chomp, &chain_chomp_input);
    assert!(chain_chomp_bounds.ok() && chain_chomp_bounds.value().is_some());
    assert!(eq(&chain_chomp_bounds.value().unwrap().x, &123) && eq(&chain_chomp_bounds.value().unwrap().y, &272));
    assert!(eq(&chain_chomp_bounds.value().unwrap().width, &45) && eq(&chain_chomp_bounds.value().unwrap().height, &48));
    assert!(chain_chomp_commands.ok() && eq(&chain_chomp_commands.value().len(), &2));
    assert!(eq(&chain_chomp_commands.value()[0].x, &152) && eq(&chain_chomp_commands.value()[0].y, &272));
    assert!(eq(&chain_chomp_commands.value()[0].width, &16) && eq(&chain_chomp_commands.value()[0].height, &48));
    assert!(eq(&chain_chomp_commands.value()[1].x, &123) && eq(&chain_chomp_commands.value()[1].y, &292));
    assert!(eq(&chain_chomp_commands.value()[1].width, &44) && eq(&chain_chomp_commands.value()[1].height, &28));

    // The three-bubble coin layout ignores the regular coin's shift flags.
    let mut coin_input = barrel_input.clone();
    coin_input.settings = 0x01100002;
    let coin = shipped_catalog.by_object_id(66).unwrap();
    let mut coin_bounds = bounds(coin, &coin_input, false);
    let mut coin_commands = render(coin, &coin_input);
    assert!(coin_bounds.ok() && coin_bounds.value().is_some());
    assert!(eq(&coin_bounds.value().unwrap().x, &156) && eq(&coin_bounds.value().unwrap().y, &300));
    assert!(eq(&coin_bounds.value().unwrap().width, &72) && eq(&coin_bounds.value().unwrap().height, &40));
    assert!(coin_commands.ok() && eq(&coin_commands.value().len(), &3));
    assert!(eq(&coin_commands.value()[0].x, &156) && eq(&coin_commands.value()[0].y, &316));
    assert!(eq(&coin_commands.value()[1].x, &180) && eq(&coin_commands.value()[1].y, &300));
    assert!(eq(&coin_commands.value()[2].x, &204) && eq(&coin_commands.value()[2].y, &316));
    for command in coin_commands.value() {
        assert!(eq(&command.width, &24) && eq(&command.height, &24));
    }

    // Unagi's three direction flags are separate nybbles with stationary,
    // left, then right precedence; no direction renders the fallback label.
    let unagi = shipped_catalog.by_object_id(75).unwrap();
    let mut unagi_input = barrel_input.clone();
    unagi_input.settings = 1;
    let mut unagi_bounds = bounds(unagi, &unagi_input, false);
    let mut unagi_commands = render(unagi, &unagi_input);
    assert!(unagi_bounds.ok() && unagi_bounds.value().is_some());
    assert!(eq(&unagi_bounds.value().unwrap().x, &163) && eq(&unagi_bounds.value().unwrap().y, &327));
    assert!(eq(&unagi_bounds.value().unwrap().width, &25) && eq(&unagi_bounds.value().unwrap().height, &23));
    assert!(unagi_commands.ok() && eq(&unagi_commands.value().len(), &1));
    assert!(eq(&unagi_commands.value()[0].x, &163) && eq(&unagi_commands.value()[0].y, &327));

    unagi_input.settings = 0x10;
    unagi_bounds = bounds(unagi, &unagi_input, false);
    unagi_commands = render(unagi, &unagi_input);
    assert!(unagi_bounds.ok() && unagi_bounds.value().is_some());
    assert!(eq(&unagi_bounds.value().unwrap().x, &205) && eq(&unagi_bounds.value().unwrap().y, &323));
    assert!(unagi_commands.ok() && eq(&unagi_commands.value().len(), &1));
    assert!(eq(&unagi_commands.value()[0].x, &205) && unagi_commands.value()[0].flip_x);

    unagi_input.settings = 0x10000;
    unagi_bounds = bounds(unagi, &unagi_input, false);
    unagi_commands = render(unagi, &unagi_input);
    assert!(unagi_bounds.ok() && unagi_bounds.value().is_some());
    assert!(eq(&unagi_bounds.value().unwrap().x, &172) && eq(&unagi_bounds.value().unwrap().y, &323));
    assert!(unagi_commands.ok() && eq(&unagi_commands.value().len(), &1));
    assert!(eq(&unagi_commands.value()[0].x, &172) && !unagi_commands.value()[0].flip_x);

    unagi_input.settings = 0;
    unagi_commands = render(unagi, &unagi_input);
    assert!(unagi_commands.ok() && eq(&unagi_commands.value().len(), &2));
    assert!(eq(&unagi_commands.value()[0].kind, &LuaActorDrawKind::FillRect));
    assert!(eq(&unagi_commands.value()[1].kind, &LuaActorDrawKind::Text));
    assert!(eq(&unagi_commands.value()[1].text, &"Eel"));

    unagi_input.settings = 0x10011;
    unagi_commands = render(unagi, &unagi_input);
    assert!(unagi_commands.ok() && eq(&unagi_commands.value().len(), &1));
    assert!(eq(&unagi_commands.value()[0].x, &163) && eq(&unagi_commands.value()[0].y, &327));

    // Pipe-generator direction wraps to the two low bits, matching the
    // original byte mask even for otherwise undocumented nibble values.
    let mut pipe_generator_input = barrel_input.clone();
    pipe_generator_input.settings = 0x1f;
    let pipe_generator = shipped_catalog.by_object_id(64).unwrap();
    let mut pipe_generator_bounds = bounds(pipe_generator, &pipe_generator_input, false);
    let mut pipe_generator_commands = render(pipe_generator, &pipe_generator_input);
    assert!(pipe_generator_bounds.ok() && pipe_generator_bounds.value().is_some());
    assert!(eq(&pipe_generator_bounds.value().unwrap().x, &160) && eq(&pipe_generator_bounds.value().unwrap().y, &320));
    assert!(eq(&pipe_generator_bounds.value().unwrap().width, &32) && eq(&pipe_generator_bounds.value().unwrap().height, &32));
    assert!(pipe_generator_commands.ok() && eq(&pipe_generator_commands.value().len(), &3));
    assert!(eq(&pipe_generator_commands.value()[0].x, &167) && eq(&pipe_generator_commands.value()[0].y, &328));
    assert!(eq(&pipe_generator_commands.value()[0].width, &16) && eq(&pipe_generator_commands.value()[0].height, &18));
    assert!(eq(&pipe_generator_commands.value()[1].x, &160) && eq(&pipe_generator_commands.value()[1].y, &320));
    assert!(eq(&pipe_generator_commands.value()[2].x, &192) && eq(&pipe_generator_commands.value()[2].y, &328));
    assert!(eq(&pipe_generator_commands.value()[2].rotation, &270));

    // A two-sided fire bar walks through the center in layer order, then
    // overlays its direction marker using the declared rotation field.
    let mut fire_bar_input = barrel_input.clone();
    fire_bar_input.settings = 0x12;
    let fire_bar = shipped_catalog.by_object_id(65).unwrap();
    let mut fire_bar_bounds = bounds(fire_bar, &fire_bar_input, false);
    let mut fire_bar_commands = render(fire_bar, &fire_bar_input);
    assert!(fire_bar_bounds.ok() && fire_bar_bounds.value().is_some());
    assert!(eq(&fire_bar_bounds.value().unwrap().x, &140) && eq(&fire_bar_bounds.value().unwrap().y, &300));
    assert!(eq(&fire_bar_bounds.value().unwrap().width, &52) && eq(&fire_bar_bounds.value().unwrap().height, &52));
    assert!(fire_bar_commands.ok() && eq(&fire_bar_commands.value().len(), &6));
    assert!(eq(&fire_bar_commands.value()[0].x, &140) && eq(&fire_bar_commands.value()[0].y, &340));
    assert!(eq(&fire_bar_commands.value()[1].x, &150) && eq(&fire_bar_commands.value()[1].y, &330));
    assert!(eq(&fire_bar_commands.value()[2].x, &164) && eq(&fire_bar_commands.value()[2].y, &324));
    assert!(eq(&fire_bar_commands.value()[2].width, &8) && eq(&fire_bar_commands.value()[2].height, &8));
    assert!(eq(&fire_bar_commands.value()[3].x, &170) && eq(&fire_bar_commands.value()[3].y, &310));
    assert!(eq(&fire_bar_commands.value()[4].x, &180) && eq(&fire_bar_commands.value()[4].y, &300));
    assert!(eq(&fire_bar_commands.value()[5].x, &152) && eq(&fire_bar_commands.value()[5].y, &312));
    assert!(fire_bar_commands.value()[5].flip_x);

    // Down-facing Squiggler keeps its nonsquare image at the shifted origin
    // and rotates it in place.
    let mut squiggler_input = barrel_input.clone();
    squiggler_input.settings = 1;
    let squiggler = shipped_catalog.by_object_id(126).unwrap();
    let mut squiggler_bounds = bounds(squiggler, &squiggler_input, false);
    let mut squiggler_commands = render(squiggler, &squiggler_input);
    assert!(squiggler_bounds.ok() && squiggler_bounds.value().is_some());
    assert!(eq(&squiggler_bounds.value().unwrap().x, &170) && eq(&squiggler_bounds.value().unwrap().y, &336));
    assert!(eq(&squiggler_bounds.value().unwrap().width, &13) && eq(&squiggler_bounds.value().unwrap().height, &15));
    assert!(squiggler_commands.ok() && eq(&squiggler_commands.value().len(), &1));
    assert!(eq(&squiggler_commands.value()[0].x, &170) && eq(&squiggler_commands.value()[0].y, &336));
    assert!(eq(&squiggler_commands.value()[0].width, &13) && eq(&squiggler_commands.value()[0].height, &15));
    assert!(eq(&squiggler_commands.value()[0].rotation, &180));

    // Scuttlebug's offset field already includes four blocks; the web ends at
    // the image origin while selection bounds exclude the web itself.
    let mut scuttlebug_input = barrel_input.clone();
    scuttlebug_input.settings = 0x20;
    let scuttlebug = shipped_catalog.by_object_id(117).unwrap();
    let mut scuttlebug_bounds = bounds(scuttlebug, &scuttlebug_input, false);
    let mut scuttlebug_commands = render(scuttlebug, &scuttlebug_input);
    assert!(scuttlebug_bounds.ok() && scuttlebug_bounds.value().is_some());
    assert!(eq(&scuttlebug_bounds.value().unwrap().x, &146) && eq(&scuttlebug_bounds.value().unwrap().y, &416));
    assert!(eq(&scuttlebug_bounds.value().unwrap().width, &44) && eq(&scuttlebug_bounds.value().unwrap().height, &37));
    assert!(scuttlebug_commands.ok() && eq(&scuttlebug_commands.value().len(), &3));
    assert!(eq(&scuttlebug_commands.value()[0].kind, &LuaActorDrawKind::Line));
    assert!(eq(&scuttlebug_commands.value()[0].x, &167) && eq(&scuttlebug_commands.value()[0].y, &320));
    assert!(eq(&scuttlebug_commands.value()[0].x2, &167) && eq(&scuttlebug_commands.value()[0].y2, &416));
    assert!(eq(&scuttlebug_commands.value()[1].x, &168) && eq(&scuttlebug_commands.value()[1].x2, &168));
    assert!(eq(&scuttlebug_commands.value()[2].x, &146) && eq(&scuttlebug_commands.value()[2].y, &416));

    // Bounce-bubble distance and direction are independent fields; ignored
    // low-byte bits do not change the preview offset.
    let bounce_bubble = shipped_catalog.by_object_id(124).unwrap();
    let mut bounce_bubble_input = barrel_input.clone();
    bounce_bubble_input.settings = 0x30;
    let mut bounce_bubble_bounds = bounds(bounce_bubble, &bounce_bubble_input, false);
    let mut bounce_bubble_commands = render(bounce_bubble, &bounce_bubble_input);
    assert!(bounce_bubble_bounds.ok() && bounce_bubble_bounds.value().is_some());
    assert!(eq(&bounce_bubble_bounds.value().unwrap().x, &140) && eq(&bounce_bubble_bounds.value().unwrap().y, &294));
    assert!(bounce_bubble_commands.ok() && eq(&bounce_bubble_commands.value()[0].x, &140));

    bounce_bubble_input.settings = 0x31;
    bounce_bubble_bounds = bounds(bounce_bubble, &bounce_bubble_input, false);
    assert!(bounce_bubble_bounds.ok() && eq(&bounce_bubble_bounds.value().unwrap().x, &128));
    bounce_bubble_input.settings = 0x3f;
    bounce_bubble_bounds = bounds(bounce_bubble, &bounce_bubble_input, false);
    assert!(bounce_bubble_bounds.ok() && eq(&bounce_bubble_bounds.value().unwrap().x, &128));

    // Arrow signs alternate two base images and rotate each pair by 90°;
    // the unused high direction bit remains ignored.
    let arrow_sign = shipped_catalog.by_object_id(125).unwrap();
    let mut straight_arrow: Option<Arc<RgbaImage>> = None;
    let mut diagonal_arrow: Option<Arc<RgbaImage>> = None;
    for direction in 0..8 {
        let mut arrow_input = barrel_input.clone();
        arrow_input.settings = direction << 24;
        let mut arrow_commands = render(arrow_sign, &arrow_input);
        assert!(arrow_commands.ok() && eq(&arrow_commands.value().len(), &1));
        let command = &arrow_commands.value()[0];
        assert!(eq(&command.x, &160) && eq(&command.y, &320) && eq(&command.width, &32) && eq(&command.height, &32));
        assert!(eq(&command.rotation, &(((direction / 2) * 90) as f64)));
        if direction == 0 {
            straight_arrow = command.image.clone();
        }
        if direction == 1 {
            diagonal_arrow = command.image.clone();
        }
        assert!(eq(&command.image, &(if direction % 2 == 0 { straight_arrow.clone() } else { diagonal_arrow.clone() })));
    }
    let mut arrow_high_bit_input = barrel_input.clone();
    arrow_high_bit_input.settings = 0x08000000;
    let mut arrow_high_bit_commands = render(arrow_sign, &arrow_high_bit_input);
    assert!(arrow_high_bit_commands.ok() && eq(&arrow_high_bit_commands.value()[0].image, &straight_arrow));
    assert!(eq(&arrow_high_bit_commands.value()[0].rotation, &0));

    // Bubble currents keep native image dimensions while transforms rotate
    // them into their visible selection boxes.
    let down_current = shipped_catalog.by_object_id(51).unwrap();
    let mut down_bounds = bounds(down_current, &barrel_input, false);
    let mut down_commands = render(down_current, &barrel_input);
    assert!(down_bounds.ok() && down_bounds.value().is_some());
    assert!(eq(&down_bounds.value().unwrap().x, &160) && eq(&down_bounds.value().unwrap().y, &352));
    assert!(eq(&down_bounds.value().unwrap().width, &32) && eq(&down_bounds.value().unwrap().height, &74));
    assert!(down_commands.ok() && eq(&down_commands.value().len(), &1));
    assert!(eq(&down_commands.value()[0].x, &160) && eq(&down_commands.value()[0].y, &352));
    assert!(eq(&down_commands.value()[0].width, &32) && eq(&down_commands.value()[0].height, &74));
    assert!(eq(&down_commands.value()[0].rotation, &0) && down_commands.value()[0].flip_y);

    let right_current = shipped_catalog.by_object_id(52).unwrap();
    let mut right_bounds = bounds(right_current, &barrel_input, false);
    let mut right_commands = render(right_current, &barrel_input);
    assert!(right_bounds.ok() && right_bounds.value().is_some());
    assert!(eq(&right_bounds.value().unwrap().x, &192) && eq(&right_bounds.value().unwrap().y, &320));
    assert!(eq(&right_bounds.value().unwrap().width, &74) && eq(&right_bounds.value().unwrap().height, &32));
    assert!(right_commands.ok() && eq(&right_commands.value().len(), &1));
    assert!(eq(&right_commands.value()[0].x, &213) && eq(&right_commands.value()[0].y, &299));
    assert!(eq(&right_commands.value()[0].width, &32) && eq(&right_commands.value()[0].height, &74));
    assert!(eq(&right_commands.value()[0].rotation, &90));

    let left_current = shipped_catalog.by_object_id(53).unwrap();
    let mut left_bounds = bounds(left_current, &barrel_input, false);
    let mut left_commands = render(left_current, &barrel_input);
    assert!(left_bounds.ok() && left_bounds.value().is_some());
    assert!(eq(&left_bounds.value().unwrap().x, &70) && eq(&left_bounds.value().unwrap().y, &320));
    assert!(eq(&left_bounds.value().unwrap().width, &74) && eq(&left_bounds.value().unwrap().height, &32));
    assert!(left_commands.ok() && eq(&left_commands.value().len(), &1));
    assert!(eq(&left_commands.value()[0].x, &91) && eq(&left_commands.value()[0].y, &299));
    assert!(eq(&left_commands.value()[0].width, &32) && eq(&left_commands.value()[0].height, &74));
    assert!(eq(&left_commands.value()[0].rotation, &270));

    // A resource the project does not supply costs the actor its drawing, not its metadata, and never the whole
    // catalog.
    let missing = |_: ActorResourceKind, _: &str| -> Result<ActorResource> { Err(Error::new(ErrorCode::NotFound, "missing")) };
    let missing_resources = load_lua_actor_layers(&layers, &missing, Some(&decode_image), None).unwrap();
    assert_eq!(missing_resources.actors.len(), 2);
    let without = missing_resources.by_object_id(83).unwrap();
    assert_eq!(without.sprite.name, "Goomba");
    let missing_commands = render(without, &input);
    assert!(missing_commands.ok() && missing_commands.value().len() == 2);

    // init() failing leaves the metadata and drops the renderer.
    write(
        &root.join("broken/broken.lua"),
        r#"local function init(ctx) error("no") end
return {objectId=5, name="Broken", init=init, render=function() return true end}"#,
    );
    let broken_catalog = load_lua_actor_layers(&[layer("broken", root.join("broken"))], &resolve, Some(&decode_image), None).unwrap();
    assert_eq!(broken_catalog.actors.len(), 1);
    let broken_actor = broken_catalog.by_object_id(5).unwrap();
    assert_eq!(broken_actor.sprite.name, "Broken");
    assert!(!broken_actor.init_error.is_empty() && !has_lua_renderer(broken_actor));

    // A mistyped field name is an error, not a silent nil.
    write(&root.join("typo/typo.lua"), r#"return {objectId=6, name="Typo", render=function(actor) return actor.fields.nope end}"#);
    let typo_catalog = load_lua_actor_layers(&[layer("typo", root.join("typo"))], &resolve, Some(&decode_image), None).unwrap();
    assert!(!render(typo_catalog.by_object_id(6).unwrap(), &input).ok());

    // Stills may not climb out of their own actor directory.
    write(
        &root.join("escape/escape.lua"),
        r#"return {objectId=2, name="Escape", init=function(ctx) ctx.loadImage("../sign.png") end,
render=function() return true end}"#,
    );
    let escaped = load_lua_actor_layers(&[layer("escape", root.join("escape"))], &resolve, Some(&decode_image), None).unwrap();
    assert!(!escaped.actors.is_empty());
    assert!(!escaped.by_object_id(2).unwrap().init_error.is_empty());

    // Resource loading is unavailable once init() has returned.
    write(
        &root.join("late/late.lua"),
        r#"local saved
local function init(ctx) saved = ctx end
return {objectId=7, name="Late", init=init, render=function() saved.loadImage("sign.png") return true end}"#,
    );
    let late_catalog = load_lua_actor_layers(&[layer("late", root.join("late"))], &resolve, Some(&decode_image), None).unwrap();
    assert!(!render(late_catalog.by_object_id(7).unwrap(), &input).ok());

    // A module actor overriding a vanilla class, ported from the NSMBe Signboard plugin: the same settings must pick
    // the same still and the same box.
    let signboard = vec![
        layer("module signboard", source_dir().join("libs/nsmb-project/tests/fixtures/signboard-module/actors")),
        layer("shipped", source_dir().join("data/actors")),
    ];
    let signboard_catalog = load_lua_actor_layers(&signboard, &resolve, Some(&decode_image), None).unwrap();
    let sign = signboard_catalog.by_object_id(77).unwrap();
    assert!(sign.sprite.name == "Arrow signboard" && sign.init_error.is_empty(), "{}", sign.init_error);
    assert!(has_lua_renderer(sign));

    let mut sign_input = LuaActorRenderInput::new();
    sign_input.object_id = 77;
    sign_input.pixel_x = 160.0;
    sign_input.pixel_y = 320.0;
    // Model type 2, no pipe mode, full size, unshifted: x -= 8, y -= 16.
    sign_input.settings = 2 << 24;
    let sign_bounds = bounds(sign, &sign_input, false);
    assert!(sign_bounds.ok() && sign_bounds.value().is_some());
    assert!(sign_bounds.value().unwrap().x == 152 && sign_bounds.value().unwrap().y == 304);
    assert!(sign_bounds.value().unwrap().width == 32 && sign_bounds.value().unwrap().height == 32);
    let sign_commands = render(sign, &sign_input);
    assert!(sign_commands.ok() && sign_commands.value().len() == 1);
    assert!(sign_commands.value()[0].x == 152.0 && sign_commands.value()[0].width == 32.0);

    // Smaller, shifted left and down: the half-block shift halves with the size.
    sign_input.settings = (1 << 16) | (13 << 12) | (14 << 8);
    let sign_bounds = bounds(sign, &sign_input, false);
    assert!(sign_bounds.ok() && sign_bounds.value().is_some());
    assert!(sign_bounds.value().unwrap().x == 156 && sign_bounds.value().unwrap().y == 328);
    assert!(sign_bounds.value().unwrap().width == 16 && sign_bounds.value().unwrap().height == 16);

    // Model type 10 with pipe mode off is the joint rather than an arrow.
    sign_input.settings = 10 << 24;
    assert!(render(sign, &sign_input).ok());

    // Every shipped program has to survive a spread of settings, not just zero.
    let mut with_renderer = 0;
    for definition in &shipped_catalog.actors {
        let filename = definition.source.file_name().unwrap().to_string_lossy();
        assert!(definition.init_error.is_empty(), "{filename}: {}", definition.init_error);
        if has_lua_renderer(definition) {
            with_renderer += 1;
        }
        for settings in [0x0000_0000u32, 0xFFFF_FFFF, 0x1234_5678, 0x8888_8888, 0x0101_0101] {
            let mut probe = LuaActorRenderInput::new();
            probe.x = 10;
            probe.y = 20;
            probe.pixel_x = 160.0;
            probe.pixel_y = 320.0;
            probe.object_id = definition.sprite.object_id;
            probe.settings = settings;
            probe.data =
                [settings as u8, (settings >> 8) as u8, (settings >> 24) as u8, (settings >> 16) as u8, (settings >> 8) as u8, settings as u8];
            for render_box in [false, true] {
                if let Err(error) = lua_actor_bounds(definition, &probe, render_box) {
                    panic!("{filename} bounds: {}", error.message);
                }
            }
            if let Err(error) = render_lua_actor(definition, &probe) {
                panic!("{filename} render: {}", error.message);
            }
        }
    }
    assert!(with_renderer > 190);

    let _ = std::fs::remove_dir_all(&root);
}
