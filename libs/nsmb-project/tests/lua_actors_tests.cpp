#include "nsmb/project/lua_actors.hpp"

#include <algorithm>
#include <array>
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>

namespace {

// A 2x2 red PNG, so the injected decoder has something real to chew on.
constexpr std::array<unsigned char, 82> red_png{
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0x08, 0x02, 0x00, 0x00, 0x00, 0xfd, 0xd4, 0x9a,
    0x73, 0x00, 0x00, 0x00, 0x19, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xc0,
    0xc0, 0xf0, 0x9f, 0x81, 0x81, 0x81, 0xe1, 0x3f, 0x00, 0x00, 0x2a, 0x0d, 0x04, 0x03, 0x2b, 0x2a,
    0x66, 0xd8, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82};

// The catalog carries no image codec of its own, so the test plays the host and
// answers with a fixed 2x2 image.
nsmb::core::Result<nsmb::formats::RgbaImage> decode_image(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 8 || bytes[1] != 'P' || bytes[2] != 'N' || bytes[3] != 'G')
        return nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "not a PNG");
    // Real dimensions matter: a port that reads a frame out of a strip has to be
    // checked against the strip it actually ships with.
    nsmb::formats::RgbaImage image;
    image.width = 2;
    image.height = 2;
    if (bytes.size() >= 24 && std::equal(bytes.begin() + 12, bytes.begin() + 16,
                                         std::begin("IHDR"), std::begin("IHDR") + 4)) {
        const auto read = [&](std::size_t at) {
            return static_cast<std::size_t>(bytes[at]) << 24U | static_cast<std::size_t>(bytes[at + 1]) << 16U |
                   static_cast<std::size_t>(bytes[at + 2]) << 8U | static_cast<std::size_t>(bytes[at + 3]);
        };
        image.width = read(16);
        image.height = read(20);
    }
    if (!image.width || !image.height || image.width > 8192U || image.height > 8192U)
        return nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "bad PNG size");
    image.pixels.assign(image.width * image.height * 4U, 0xff);
    return image;
}

} // namespace

int main() {
    namespace fs = std::filesystem;
    const auto root = fs::temp_directory_path() / "big-star-lua-actor-tests";
    fs::remove_all(root);
    fs::create_directories(root / "first");
    fs::create_directories(root / "last");
    fs::create_directories(root / "rom");
    {
        std::array<std::uint8_t, 64> tiles{};
        std::fill_n(tiles.begin(), 32, 0x33);
        std::fill_n(tiles.begin() + 32, 32, 0x22);
        std::ofstream file(root / "rom" / "goomba.bin", std::ios::binary);
        file.write(reinterpret_cast<const char*>(tiles.data()), static_cast<std::streamsize>(tiles.size()));
    }
    {
        std::array<std::uint8_t, 32> palette{};
        palette[2] = 0x1f; // red, followed by green
        palette[4] = 0xe0;
        palette[5] = 0x03;
        std::ofstream file(root / "rom" / "enemy.bin", std::ios::binary);
        file.write(reinterpret_cast<const char*>(palette.data()), static_cast<std::streamsize>(palette.size()));
    }
    {
        std::ofstream file(root / "first" / "sign.png", std::ios::binary);
        file.write(reinterpret_cast<const char*>(red_png.data()), static_cast<std::streamsize>(red_png.size()));
    }
    {
        std::ofstream file(root / "first" / "goomba.lua");
        file << R"(local gfx = {}
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
 init=init, render=render, grabBounds=bounds, renderBounds=bounds})";
    }
    {
        std::ofstream file(root / "first" / "custom.lua");
        file << R"(return {hash=0x12345678, name="Module Actor"})";
    }
    {
        std::ofstream file(root / "last" / "duplicate.lua");
        file << R"(return {objectId=83, name="Loses"})";
    }
    const std::vector<nsmb::project::LuaActorLayer> layers{{"first", root / "first"}, {"last", root / "last"}};
    const auto resolve = [&](auto, std::string_view name) { return nsmb::core::Result<fs::path>(root / "rom" / name); };
    auto catalog = nsmb::project::load_lua_actor_layers(layers, resolve, decode_image);
    if (!catalog) std::cerr << catalog.error().message << '\n';
    assert(catalog);
    assert(catalog.value().actors.size() == 2);
    const auto* goomba = catalog.value().by_object_id(83);
    assert(goomba && goomba->sprite.name == "Goomba" && goomba->init_error.empty());
    assert(goomba->sprite.fields.size() == 2 && goomba->sprite.fields.front().choices.size() == 2);
    assert(goomba->sprite.notes == "Walks.");
    assert(goomba->sprite.fields.front().notes == "A field note.");
    assert(goomba->sprite.fields.front().bit_offset == 40 && goomba->sprite.fields.front().signed_value);
    assert(nsmb::project::has_lua_renderer(*goomba));
    // The ROM graphics, its palette and the still beside the script.
    assert(goomba->render_resources.size() == 3);

    nsmb::project::LuaActorRenderInput input;
    input.x = 10;
    input.y = 20;
    input.pixel_x = 160;
    input.pixel_y = 320;
    input.object_id = 83;
    input.type = 7;
    input.settings = 1;
    input.data[5] = 1;
    input.data[0] = 0; // Event 1 holds the "size" field, so it reads as 0 + 1
    auto commands = nsmb::project::render_lua_actor(*goomba, input);
    if (!commands) std::cerr << commands.error().message << '\n';
    assert(commands && commands.value().size() == 4);
    assert(commands.value()[0].source_x == 8 && commands.value()[0].source_y == 0);
    assert(commands.value()[0].x == 162 && commands.value()[0].y == 317);
    assert(commands.value()[0].width == 16 && commands.value()[0].height == 12);
    assert(commands.value()[0].rotation == 90 && commands.value()[0].opacity == 0.5 && commands.value()[0].flip_x);
    // Both ROM draws share one rasterized image; the still is its own.
    assert(commands.value()[0].image == commands.value()[1].image);
    assert(commands.value()[2].image && commands.value()[2].image->width == 2);
    // drawText carries text rather than an image.
    assert(commands.value()[3].text == "1" && !commands.value()[3].image);

    // Bounds come back in scene pixels, and read the declared field.
    auto bounds = nsmb::project::lua_actor_bounds(*goomba, input);
    assert(bounds && bounds.value() && bounds.value()->x == 152 && bounds.value()->y == 312);
    assert(bounds.value()->width == 24);
    // The cached definition bounds are the same box evaluated at the origin.
    assert(goomba->sprite.grab_bounds.x == -8 && goomba->sprite.grab_bounds.y == -8);

    auto commands_again = nsmb::project::render_lua_actor(*goomba, input);
    assert(commands_again && commands_again.value().front().image == commands.value().front().image);
    assert(catalog.value().sprite_definitions().by_hash(0x12345678));

    const std::vector<nsmb::project::LuaActorLayer> shipped{{"shipped", fs::path(BSE_TEST_SOURCE_DIR) / "data" / "actors"}};
    // The shipped catalog draws from stills on disk, so the resolver points at
    // the real ones: a script naming an image that is not there must fail here.
    const auto shipped_resolve = [&](nsmb::project::ActorResourceKind kind,
                                     std::string_view name) -> nsmb::core::Result<fs::path> {
        if (kind != nsmb::project::ActorResourceKind::Image) return resolve(kind, name);
        auto path = fs::path(BSE_TEST_SOURCE_DIR) / "data" / "actors" / name;
        if (!fs::exists(path))
            return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, std::string("missing ") + std::string(name));
        return path;
    };
    auto shipped_catalog = nsmb::project::load_lua_actor_layers(shipped, shipped_resolve, decode_image);
    if (!shipped_catalog) std::cerr << shipped_catalog.error().message << '\n';
    assert(shipped_catalog && shipped_catalog.value().actors.size() == 305);

    // Actor 285 reverses its length nybbles to draw a loop guide, then leaves
    // the stock numbered marker visible over it.
    nsmb::project::LuaActorRenderInput loop_input;
    loop_input.pixel_x = 160;
    loop_input.pixel_y = 320;
    loop_input.settings = 0x11U << 8U;
    loop_input.data[4] = 0x11;
    bool loop_custom_rendered = true;
    auto loop_commands = nsmb::project::render_lua_actor(
        *shipped_catalog.value().by_object_id(285), loop_input, &loop_custom_rendered);
    assert(loop_commands && loop_commands.value().size() == 4 && !loop_custom_rendered);
    assert(loop_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::FillRect);
    assert(loop_commands.value()[0].x == 160 && loop_commands.value()[0].y == 326);
    assert(loop_commands.value()[0].width == 32 && loop_commands.value()[0].height == 4);
    assert(loop_commands.value()[1].x == 188 && loop_commands.value()[1].y == 320);
    assert(loop_commands.value()[2].x == 161 && loop_commands.value()[2].y == 327);
    assert(loop_commands.value()[3].x == 189 && loop_commands.value()[3].y == 321);

    // Actor 286 draws the wrong-path guide upward from its position and also
    // leaves the stock marker visible.
    nsmb::project::LuaActorRenderInput wrong_path_input;
    wrong_path_input.pixel_x = 160;
    wrong_path_input.pixel_y = 320;
    wrong_path_input.settings = 3U << 4U;
    wrong_path_input.data[5] = 0x30;
    bool wrong_path_custom_rendered = true;
    auto wrong_path_commands = nsmb::project::render_lua_actor(
        *shipped_catalog.value().by_object_id(286), wrong_path_input, &wrong_path_custom_rendered);
    assert(wrong_path_commands && wrong_path_commands.value().size() == 4 && !wrong_path_custom_rendered);
    assert(wrong_path_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::FillRect);
    assert(wrong_path_commands.value()[0].x == 166 && wrong_path_commands.value()[0].y == 288);
    assert(wrong_path_commands.value()[0].width == 4 && wrong_path_commands.value()[0].height == 48);
    assert(wrong_path_commands.value()[1].x == 160 && wrong_path_commands.value()[1].y == 288);
    assert(wrong_path_commands.value()[2].x == 167 && wrong_path_commands.value()[2].y == 289);
    assert(wrong_path_commands.value()[3].x == 161 && wrong_path_commands.value()[3].y == 289);

    // Actor 208 deliberately keeps a 48x48 selection box around a 49x49 still.
    nsmb::project::LuaActorRenderInput barrel_input;
    barrel_input.pixel_x = 160;
    barrel_input.pixel_y = 320;
    const auto* barrel = shipped_catalog.value().by_object_id(208);
    auto barrel_bounds = nsmb::project::lua_actor_bounds(*barrel, barrel_input);
    auto barrel_commands = nsmb::project::render_lua_actor(*barrel, barrel_input);
    assert(barrel_bounds && barrel_bounds.value());
    assert(barrel_bounds.value()->x == 144 && barrel_bounds.value()->y == 312);
    assert(barrel_bounds.value()->width == 48 && barrel_bounds.value()->height == 48);
    assert(barrel_commands && barrel_commands.value().size() == 1);
    assert(barrel_commands.value()[0].x == 144 && barrel_commands.value()[0].y == 312);
    assert(barrel_commands.value()[0].width == 49 && barrel_commands.value()[0].height == 49);

    // The boss key also renders one pixel taller than its selection box.
    const auto* boss_key = shipped_catalog.value().by_object_id(93);
    auto boss_key_bounds = nsmb::project::lua_actor_bounds(*boss_key, barrel_input);
    auto boss_key_commands = nsmb::project::render_lua_actor(*boss_key, barrel_input);
    assert(boss_key_bounds && boss_key_bounds.value());
    assert(boss_key_bounds.value()->x == 140 && boss_key_bounds.value()->y == 305);
    assert(boss_key_bounds.value()->width == 25 && boss_key_bounds.value()->height == 31);
    assert(boss_key_commands && boss_key_commands.value().size() == 1);
    assert(boss_key_commands.value()[0].x == 140 && boss_key_commands.value()[0].y == 305);
    assert(boss_key_commands.value()[0].width == 25 && boss_key_commands.value()[0].height == 32);

    // Mini Goomba's half-block shifts apply to both geometries, while its
    // selection box remains one pixel smaller than the still.
    auto mini_goomba_input = barrel_input;
    mini_goomba_input.settings = 0x11;
    const auto* mini_goomba = shipped_catalog.value().by_object_id(85);
    auto mini_goomba_bounds = nsmb::project::lua_actor_bounds(*mini_goomba, mini_goomba_input);
    auto mini_goomba_commands = nsmb::project::render_lua_actor(*mini_goomba, mini_goomba_input);
    assert(mini_goomba_bounds && mini_goomba_bounds.value());
    assert(mini_goomba_bounds.value()->x == 168 && mini_goomba_bounds.value()->y == 328);
    assert(mini_goomba_bounds.value()->width == 8 && mini_goomba_bounds.value()->height == 8);
    assert(mini_goomba_commands && mini_goomba_commands.value().size() == 1);
    assert(mini_goomba_commands.value()[0].x == 168 && mini_goomba_commands.value()[0].y == 328);
    assert(mini_goomba_commands.value()[0].width == 9 && mini_goomba_commands.value()[0].height == 9);

    // Red Climbing Koopa's still is one pixel narrower than its shared
    // selection box.
    auto climbing_koopa_input = barrel_input;
    climbing_koopa_input.settings = 1;
    const auto* climbing_koopa = shipped_catalog.value().by_object_id(96);
    auto climbing_koopa_bounds = nsmb::project::lua_actor_bounds(*climbing_koopa, climbing_koopa_input);
    auto climbing_koopa_commands = nsmb::project::render_lua_actor(*climbing_koopa, climbing_koopa_input);
    assert(climbing_koopa_bounds && climbing_koopa_bounds.value());
    assert(climbing_koopa_bounds.value()->width == 18 && climbing_koopa_bounds.value()->height == 26);
    assert(climbing_koopa_commands && climbing_koopa_commands.value().size() == 1);
    assert(climbing_koopa_commands.value()[0].x == 160 && climbing_koopa_commands.value()[0].y == 320);
    assert(climbing_koopa_commands.value()[0].width == 17 && climbing_koopa_commands.value()[0].height == 26);

    // Amp's image dimensions are intentionally transposed relative to its
    // selection box, but both share the field-controlled origin.
    auto amp_input = barrel_input;
    amp_input.settings = 0x30;
    const auto* amp = shipped_catalog.value().by_object_id(110);
    auto amp_bounds = nsmb::project::lua_actor_bounds(*amp, amp_input);
    auto amp_commands = nsmb::project::render_lua_actor(*amp, amp_input);
    assert(amp_bounds && amp_bounds.value());
    assert(amp_bounds.value()->x == 162 && amp_bounds.value()->y == 322);
    assert(amp_bounds.value()->width == 27 && amp_bounds.value()->height == 28);
    assert(amp_commands && amp_commands.value().size() == 1);
    assert(amp_commands.value()[0].x == 162 && amp_commands.value()[0].y == 322);
    assert(amp_commands.value()[0].width == 28 && amp_commands.value()[0].height == 27);

    // Wiggler's bounds use the low-nibble size flag, while its image selection
    // also requires the neighboring nibble to be zero.
    auto wiggler_input = barrel_input;
    wiggler_input.settings = 1;
    const auto* wiggler = shipped_catalog.value().by_object_id(111);
    auto wiggler_bounds = nsmb::project::lua_actor_bounds(*wiggler, wiggler_input);
    auto wiggler_commands = nsmb::project::render_lua_actor(*wiggler, wiggler_input);
    assert(wiggler_bounds && wiggler_bounds.value());
    assert(wiggler_bounds.value()->y == 276 && wiggler_bounds.value()->width == 92);
    assert(wiggler_commands && wiggler_commands.value().size() == 1);
    assert(wiggler_commands.value()[0].y == 276 && wiggler_commands.value()[0].width == 92);

    wiggler_input.settings = 0x11;
    wiggler_bounds = nsmb::project::lua_actor_bounds(*wiggler, wiggler_input);
    wiggler_commands = nsmb::project::render_lua_actor(*wiggler, wiggler_input);
    assert(wiggler_bounds && wiggler_bounds.value());
    assert(wiggler_bounds.value()->y == 276 && wiggler_bounds.value()->width == 92);
    assert(wiggler_commands && wiggler_commands.value().size() == 1);
    assert(wiggler_commands.value()[0].y == 306 && wiggler_commands.value()[0].width == 52);

    // Bowser Jr. behaviours zero and three share the masked still; normal
    // modes select the other resource.
    auto bowser_jr_input = barrel_input;
    const auto* bowser_jr = shipped_catalog.value().by_object_id(113);
    auto bowser_jr_masked = nsmb::project::render_lua_actor(*bowser_jr, bowser_jr_input);
    bowser_jr_input.settings = 0x30;
    auto bowser_jr_masked_jumping = nsmb::project::render_lua_actor(*bowser_jr, bowser_jr_input);
    bowser_jr_input.settings = 0x10;
    auto bowser_jr_normal = nsmb::project::render_lua_actor(*bowser_jr, bowser_jr_input);
    assert(bowser_jr_masked && bowser_jr_masked.value().size() == 1);
    assert(bowser_jr_masked_jumping && bowser_jr_masked_jumping.value().size() == 1);
    assert(bowser_jr_normal && bowser_jr_normal.value().size() == 1);
    assert(bowser_jr_masked.value()[0].image == bowser_jr_masked_jumping.value()[0].image);
    assert(bowser_jr_masked.value()[0].image != bowser_jr_normal.value()[0].image);

    // Swooper's size is only the low nibble; its independent spawn-shift
    // nibble must not change the preview selection.
    const auto* swooper = shipped_catalog.value().by_object_id(90);
    auto swooper_input = barrel_input;
    swooper_input.settings = 0x21;
    auto swooper_bounds = nsmb::project::lua_actor_bounds(*swooper, swooper_input);
    auto swooper_commands = nsmb::project::render_lua_actor(*swooper, swooper_input);
    assert(swooper_bounds && swooper_bounds.value());
    assert(swooper_bounds.value()->x == 154 && swooper_bounds.value()->y == 304);
    assert(swooper_bounds.value()->width == 26 && swooper_bounds.value()->height == 37);
    assert(swooper_commands && swooper_commands.value()[0].width == 26);

    swooper_input.settings = 0x02;
    swooper_bounds = nsmb::project::lua_actor_bounds(*swooper, swooper_input);
    swooper_commands = nsmb::project::render_lua_actor(*swooper, swooper_input);
    assert(swooper_bounds && swooper_bounds.value());
    assert(swooper_bounds.value()->x == 160 && swooper_bounds.value()->y == 320);
    assert(swooper_bounds.value()->width == 16 && swooper_bounds.value()->height == 18);
    assert(swooper_commands && swooper_commands.value()[0].width == 16);

    // Empty-shell colors two and three both use the blue shell, while color
    // zero remains green.
    const auto* koopa = shipped_catalog.value().by_object_id(94);
    auto koopa_input = barrel_input;
    koopa_input.settings = 0x10;
    auto green_shell = nsmb::project::render_lua_actor(*koopa, koopa_input);
    koopa_input.settings = 0x12;
    auto blue_shell = nsmb::project::render_lua_actor(*koopa, koopa_input);
    koopa_input.settings = 0x13;
    auto blue_ledge_shell = nsmb::project::render_lua_actor(*koopa, koopa_input);
    auto blue_ledge_bounds = nsmb::project::lua_actor_bounds(*koopa, koopa_input);
    assert(green_shell && blue_shell && blue_ledge_shell);
    assert(green_shell.value()[0].image != blue_shell.value()[0].image);
    assert(blue_shell.value()[0].image == blue_ledge_shell.value()[0].image);
    assert(blue_ledge_bounds && blue_ledge_bounds.value());
    assert(blue_ledge_bounds.value()->x == 160 && blue_ledge_bounds.value()->y == 320);
    assert(blue_ledge_bounds.value()->width == 16 && blue_ledge_bounds.value()->height == 16);

    // Paratroopa color three intentionally falls back to green despite its
    // metadata label; only color two selects the blue still.
    const auto* paratroopa = shipped_catalog.value().by_object_id(95);
    auto paratroopa_input = barrel_input;
    auto green_paratroopa = nsmb::project::render_lua_actor(*paratroopa, paratroopa_input);
    paratroopa_input.settings = 2;
    auto blue_paratroopa = nsmb::project::render_lua_actor(*paratroopa, paratroopa_input);
    paratroopa_input.settings = 3;
    auto ledge_paratroopa = nsmb::project::render_lua_actor(*paratroopa, paratroopa_input);
    assert(green_paratroopa && blue_paratroopa && ledge_paratroopa);
    assert(green_paratroopa.value()[0].image != blue_paratroopa.value()[0].image);
    assert(green_paratroopa.value()[0].image == ledge_paratroopa.value()[0].image);

    // Toadsworth's selection excludes the distant house preview that follows
    // him in draw order.
    const auto* toadsworth = shipped_catalog.value().by_object_id(134);
    auto toadsworth_input = barrel_input;
    auto toadsworth_bounds = nsmb::project::lua_actor_bounds(*toadsworth, toadsworth_input);
    auto toadsworth_commands = nsmb::project::render_lua_actor(*toadsworth, toadsworth_input);
    assert(toadsworth_bounds && toadsworth_bounds.value());
    assert(toadsworth_bounds.value()->x == 160 && toadsworth_bounds.value()->y == 305);
    assert(toadsworth_bounds.value()->width == 20 && toadsworth_bounds.value()->height == 31);
    assert(toadsworth_commands && toadsworth_commands.value().size() == 2);
    assert(toadsworth_commands.value()[0].x == 160 && toadsworth_commands.value()[0].y == 305);
    assert(toadsworth_commands.value()[1].x == -28 && toadsworth_commands.value()[1].y == 232);
    assert(toadsworth_commands.value()[1].width == 184 && toadsworth_commands.value()[1].height == 31);

    toadsworth_input.settings = 2;
    toadsworth_commands = nsmb::project::render_lua_actor(*toadsworth, toadsworth_input);
    assert(toadsworth_commands && toadsworth_commands.value().size() == 2);
    assert(toadsworth_commands.value()[1].x == 44 && toadsworth_commands.value()[1].y == 244);
    assert(toadsworth_commands.value()[1].width == 40 && toadsworth_commands.value()[1].height == 40);
    toadsworth_input.settings = 1;
    toadsworth_commands = nsmb::project::render_lua_actor(*toadsworth, toadsworth_input);
    assert(toadsworth_commands && toadsworth_commands.value().size() == 1);

    // Dorrie size reads only two bits, and left-facing previews reuse the side
    // geometry with a horizontal flip.
    const auto* dorrie = shipped_catalog.value().by_object_id(79);
    auto dorrie_input = barrel_input;
    dorrie_input.settings = 0x01000000;
    auto dorrie_bounds = nsmb::project::lua_actor_bounds(*dorrie, dorrie_input);
    auto dorrie_commands = nsmb::project::render_lua_actor(*dorrie, dorrie_input);
    assert(dorrie_bounds && dorrie_bounds.value());
    assert(dorrie_bounds.value()->x == 109 && dorrie_bounds.value()->y == 261);
    assert(dorrie_bounds.value()->width == 115 && dorrie_bounds.value()->height == 76);
    assert(dorrie_commands && dorrie_commands.value()[0].flip_x);

    dorrie_input.settings = 0x03f00000;
    dorrie_bounds = nsmb::project::lua_actor_bounds(*dorrie, dorrie_input);
    dorrie_commands = nsmb::project::render_lua_actor(*dorrie, dorrie_input);
    assert(dorrie_bounds && dorrie_bounds.value());
    assert(dorrie_bounds.value()->x == 61 && dorrie_bounds.value()->y == 208);
    assert(dorrie_bounds.value()->width == 202 && dorrie_bounds.value()->height == 181);
    assert(dorrie_commands && !dorrie_commands.value()[0].flip_x);

    // Jumping Cheep Cheep keeps a wide, short selection around its natural
    // narrow, tall still.
    const auto* jumping_cheep = shipped_catalog.value().by_object_id(136);
    auto jumping_cheep_bounds = nsmb::project::lua_actor_bounds(*jumping_cheep, barrel_input);
    auto jumping_cheep_commands = nsmb::project::render_lua_actor(*jumping_cheep, barrel_input);
    assert(jumping_cheep_bounds && jumping_cheep_bounds.value());
    assert(jumping_cheep_bounds.value()->x == 160 && jumping_cheep_bounds.value()->y == 320);
    assert(jumping_cheep_bounds.value()->width == 21 && jumping_cheep_bounds.value()->height == 16);
    assert(jumping_cheep_commands && jumping_cheep_commands.value().size() == 1);
    assert(jumping_cheep_commands.value()[0].x == 160 && jumping_cheep_commands.value()[0].y == 320);
    assert(jumping_cheep_commands.value()[0].width == 19 && jumping_cheep_commands.value()[0].height == 18);

    // Wobbly lift bounds cover only the platform, while six chain links extend
    // upward in draw order.
    const auto* wobbly_lift = shipped_catalog.value().by_object_id(169);
    auto wobbly_lift_bounds = nsmb::project::lua_actor_bounds(*wobbly_lift, barrel_input);
    auto wobbly_lift_commands = nsmb::project::render_lua_actor(*wobbly_lift, barrel_input);
    assert(wobbly_lift_bounds && wobbly_lift_bounds.value());
    assert(wobbly_lift_bounds.value()->x == 160 && wobbly_lift_bounds.value()->y == 320);
    assert(wobbly_lift_bounds.value()->width == 192 && wobbly_lift_bounds.value()->height == 32);
    assert(wobbly_lift_commands && wobbly_lift_commands.value().size() == 7);
    assert(wobbly_lift_commands.value()[0].x == 160 && wobbly_lift_commands.value()[0].y == 320);
    for (std::size_t link = 1; link <= 6; ++link) {
        assert(wobbly_lift_commands.value()[link].x == 252);
        assert(wobbly_lift_commands.value()[link].y == 320 - static_cast<double>(32 * link));
        assert(wobbly_lift_commands.value()[link].width == 8);
        assert(wobbly_lift_commands.value()[link].height == 32);
    }

    // Spike Top separates visual orientation from travel direction and falls
    // back to the upward still for undocumented orientations.
    const auto* spike_top = shipped_catalog.value().by_object_id(147);
    constexpr std::array<double, 5> spike_x{160, 156, 160, 160, 160};
    constexpr std::array<double, 5> spike_y{316, 320, 320, 320, 316};
    constexpr std::array<double, 5> spike_width{16, 20, 16, 20, 16};
    constexpr std::array<double, 5> spike_height{20, 16, 20, 16, 20};
    std::array<std::shared_ptr<const nsmb::formats::RgbaImage>, 5> spike_images;
    for (std::uint32_t direction = 0; direction < 5; ++direction) {
        auto spike_top_input = barrel_input;
        spike_top_input.settings = direction << 4U;
        auto spike_top_bounds = nsmb::project::lua_actor_bounds(*spike_top, spike_top_input);
        auto spike_top_commands = nsmb::project::render_lua_actor(*spike_top, spike_top_input);
        assert(spike_top_bounds && spike_top_bounds.value());
        assert(spike_top_bounds.value()->x == spike_x[direction]);
        assert(spike_top_bounds.value()->y == spike_y[direction]);
        assert(spike_top_bounds.value()->width == spike_width[direction]);
        assert(spike_top_bounds.value()->height == spike_height[direction]);
        assert(spike_top_commands && spike_top_commands.value().size() == 1);
        spike_images[direction] = spike_top_commands.value()[0].image;
    }
    assert(spike_images[0] != spike_images[1] && spike_images[1] != spike_images[2]);
    assert(spike_images[2] != spike_images[3] && spike_images[4] == spike_images[0]);
    auto spike_top_reverse_input = barrel_input;
    spike_top_reverse_input.settings = 1;
    auto spike_top_reverse = nsmb::project::render_lua_actor(*spike_top, spike_top_reverse_input);
    assert(spike_top_reverse && spike_top_reverse.value()[0].image == spike_images[0]);

    // Scale-lift bounds cover asymmetric ropes, bolts, and both platforms.
    auto scale_lift_input = barrel_input;
    scale_lift_input.settings = 0x143;
    const auto* scale_lift = shipped_catalog.value().by_object_id(172);
    auto scale_lift_bounds = nsmb::project::lua_actor_bounds(*scale_lift, scale_lift_input);
    auto scale_lift_commands = nsmb::project::render_lua_actor(*scale_lift, scale_lift_input);
    assert(scale_lift_bounds && scale_lift_bounds.value());
    assert(scale_lift_bounds.value()->x == 136 && scale_lift_bounds.value()->y == 310);
    assert(scale_lift_bounds.value()->width == 96 && scale_lift_bounds.value()->height == 82);
    assert(scale_lift_commands && scale_lift_commands.value().size() == 10);
    assert(scale_lift_commands.value()[0].x == 158 && scale_lift_commands.value()[0].y == 310);
    assert(scale_lift_commands.value()[1].x == 198 && scale_lift_commands.value()[1].y == 310);
    assert(scale_lift_commands.value()[2].x == 168 && scale_lift_commands.value()[2].y == 311);
    assert(scale_lift_commands.value()[2].x2 == 200 && scale_lift_commands.value()[2].y2 == 311);
    assert(scale_lift_commands.value()[3].x == 169 && scale_lift_commands.value()[3].x2 == 199);
    assert(scale_lift_commands.value()[4].x == 159 && scale_lift_commands.value()[4].y2 == 327);
    assert(scale_lift_commands.value()[5].x == 160 && scale_lift_commands.value()[5].y2 == 327);
    assert(scale_lift_commands.value()[6].x == 207 && scale_lift_commands.value()[6].y2 == 375);
    assert(scale_lift_commands.value()[7].x == 208 && scale_lift_commands.value()[7].y2 == 375);
    assert(scale_lift_commands.value()[8].x == 184 && scale_lift_commands.value()[8].y == 376);
    assert(scale_lift_commands.value()[9].x == 136 && scale_lift_commands.value()[9].y == 328);

    // Ball 'n' Chain keeps native 96x39 draw geometry while quarter-turn
    // bounds are 39x96, including gfx.blit's half-pixel center compensation.
    const auto* ball_chain = shipped_catalog.value().by_object_id(130);
    constexpr std::array<double, 4> ball_bounds_x{164, 149, 77, 149};
    constexpr std::array<double, 4> ball_bounds_y{309, 237, 309, 324};
    constexpr std::array<double, 4> ball_draw_x{164, 120.5, 77, 120.5};
    constexpr std::array<double, 4> ball_draw_y{309, 265.5, 309, 352.5};
    constexpr std::array<double, 4> ball_rotation{0, 270, 180, 90};
    for (std::uint32_t position = 0; position < 4; ++position) {
        auto ball_chain_input = barrel_input;
        ball_chain_input.settings = position;
        auto ball_chain_bounds = nsmb::project::lua_actor_bounds(*ball_chain, ball_chain_input);
        auto ball_chain_commands = nsmb::project::render_lua_actor(*ball_chain, ball_chain_input);
        assert(ball_chain_bounds && ball_chain_bounds.value());
        assert(ball_chain_bounds.value()->x == ball_bounds_x[position]);
        assert(ball_chain_bounds.value()->y == ball_bounds_y[position]);
        assert(ball_chain_commands && ball_chain_commands.value().size() == 2);
        assert(ball_chain_commands.value()[0].x == ball_draw_x[position]);
        assert(ball_chain_commands.value()[0].y == ball_draw_y[position]);
        assert(ball_chain_commands.value()[0].width == 96 && ball_chain_commands.value()[0].height == 39);
        assert(ball_chain_commands.value()[0].rotation == ball_rotation[position]);
        assert(ball_chain_commands.value()[1].x == 152 && ball_chain_commands.value()[1].y == 312);
        assert(ball_chain_commands.value()[1].flip_x);
    }
    auto ball_chain_mask_input = barrel_input;
    ball_chain_mask_input.settings = 0xff;
    auto ball_chain_mask_commands = nsmb::project::render_lua_actor(*ball_chain, ball_chain_mask_input);
    assert(ball_chain_mask_commands && ball_chain_mask_commands.value()[0].rotation == 90);
    assert(!ball_chain_mask_commands.value()[1].flip_x);

    // Vertical moving-lift length uses only three bits; raw low nibble F still
    // produces eight tiles and a vertical ghost target.
    auto vertical_lift_input = barrel_input;
    vertical_lift_input.settings = 0x20f;
    const auto* vertical_lift = shipped_catalog.value().by_object_id(161);
    auto vertical_lift_bounds = nsmb::project::lua_actor_bounds(*vertical_lift, vertical_lift_input);
    auto vertical_lift_commands = nsmb::project::render_lua_actor(*vertical_lift, vertical_lift_input);
    assert(vertical_lift_bounds && vertical_lift_bounds.value());
    assert(vertical_lift_bounds.value()->x == 160 && vertical_lift_bounds.value()->y == 320);
    assert(vertical_lift_bounds.value()->width == 128 && vertical_lift_bounds.value()->height == 16);
    assert(vertical_lift_commands && vertical_lift_commands.value().size() == 18);
    assert(vertical_lift_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::Line);
    assert(vertical_lift_commands.value()[0].x == 224 && vertical_lift_commands.value()[0].y == 328);
    assert(vertical_lift_commands.value()[0].x2 == 224 && vertical_lift_commands.value()[0].y2 == 296);
    assert(vertical_lift_commands.value()[1].x == 220 && vertical_lift_commands.value()[1].y == 292);
    assert(vertical_lift_commands.value()[2].x == 160 && vertical_lift_commands.value()[2].y == 320);
    assert(vertical_lift_commands.value()[9].x == 272 && vertical_lift_commands.value()[9].y == 320);
    assert(vertical_lift_commands.value()[10].x == 160 && vertical_lift_commands.value()[10].y == 288);
    assert(vertical_lift_commands.value()[17].x == 272 && vertical_lift_commands.value()[17].y == 288);

    // Horizontal moving lift uses the offset length field directly and draws
    // a matching ghost platform at its target.
    auto horizontal_lift_input = barrel_input;
    horizontal_lift_input.settings = 0x203;
    const auto* horizontal_lift = shipped_catalog.value().by_object_id(162);
    auto horizontal_lift_bounds = nsmb::project::lua_actor_bounds(*horizontal_lift, horizontal_lift_input);
    auto horizontal_lift_commands = nsmb::project::render_lua_actor(*horizontal_lift, horizontal_lift_input);
    assert(horizontal_lift_bounds && horizontal_lift_bounds.value());
    assert(horizontal_lift_bounds.value()->width == 64 && horizontal_lift_bounds.value()->height == 16);
    assert(horizontal_lift_commands && horizontal_lift_commands.value().size() == 10);
    assert(horizontal_lift_commands.value()[0].x == 192 && horizontal_lift_commands.value()[0].x2 == 224);
    assert(horizontal_lift_commands.value()[0].y == 328 && horizontal_lift_commands.value()[0].y2 == 328);
    assert(horizontal_lift_commands.value()[1].x == 220 && horizontal_lift_commands.value()[1].y == 324);
    assert(horizontal_lift_commands.value()[2].x == 160 && horizontal_lift_commands.value()[5].x == 208);
    assert(horizontal_lift_commands.value()[6].x == 192 && horizontal_lift_commands.value()[9].x == 240);

    const auto* unused_horizontal_lift = shipped_catalog.value().by_object_id(163);
    auto unused_horizontal_bounds = nsmb::project::lua_actor_bounds(*unused_horizontal_lift, horizontal_lift_input);
    auto unused_horizontal_commands = nsmb::project::render_lua_actor(*unused_horizontal_lift, horizontal_lift_input);
    assert(unused_horizontal_bounds && unused_horizontal_bounds.value());
    assert(unused_horizontal_bounds.value()->width == horizontal_lift_bounds.value()->width);
    assert(unused_horizontal_commands && unused_horizontal_commands.value().size() == 10);
    for (std::size_t command = 0; command < unused_horizontal_commands.value().size(); ++command) {
        assert(unused_horizontal_commands.value()[command].x == horizontal_lift_commands.value()[command].x);
        assert(unused_horizontal_commands.value()[command].y == horizontal_lift_commands.value()[command].y);
    }

    // The vertical stone wall uses three length bits and two height bits. Its
    // render order walks the perimeter clockwise before filling the interior.
    const auto* vertical_stone_wall = shipped_catalog.value().by_object_id(165);
    auto flat_stone_wall_bounds = nsmb::project::lua_actor_bounds(*vertical_stone_wall, barrel_input);
    auto flat_stone_wall_commands = nsmb::project::render_lua_actor(*vertical_stone_wall, barrel_input);
    assert(flat_stone_wall_bounds && flat_stone_wall_bounds.value());
    assert(flat_stone_wall_bounds.value()->width == 32 && flat_stone_wall_bounds.value()->height == 16);
    assert(flat_stone_wall_commands && flat_stone_wall_commands.value().size() == 2);
    assert(flat_stone_wall_commands.value()[0].x == 160 && flat_stone_wall_commands.value()[1].x == 176);

    auto tall_stone_wall_input = barrel_input;
    tall_stone_wall_input.settings = 0xf00f;
    auto tall_stone_wall_bounds = nsmb::project::lua_actor_bounds(*vertical_stone_wall, tall_stone_wall_input);
    auto tall_stone_wall_commands = nsmb::project::render_lua_actor(*vertical_stone_wall, tall_stone_wall_input);
    assert(tall_stone_wall_bounds && tall_stone_wall_bounds.value());
    assert(tall_stone_wall_bounds.value()->width == 128 && tall_stone_wall_bounds.value()->height == 128);
    assert(tall_stone_wall_commands && tall_stone_wall_commands.value().size() == 64);
    assert(tall_stone_wall_commands.value()[0].x == 160 && tall_stone_wall_commands.value()[0].y == 320);
    assert(tall_stone_wall_commands.value()[7].x == 272 && tall_stone_wall_commands.value()[7].y == 320);
    assert(tall_stone_wall_commands.value()[14].x == 272 && tall_stone_wall_commands.value()[14].y == 432);
    assert(tall_stone_wall_commands.value()[21].x == 160 && tall_stone_wall_commands.value()[21].y == 432);
    assert(tall_stone_wall_commands.value()[28].x == 176 && tall_stone_wall_commands.value()[28].y == 336);
    assert(tall_stone_wall_commands.value()[0].image != tall_stone_wall_commands.value()[7].image);
    assert(tall_stone_wall_commands.value()[14].image != tall_stone_wall_commands.value()[21].image);
    assert(tall_stone_wall_commands.value()[28].image != tall_stone_wall_commands.value()[0].image);

    // Actor 166 uses the full length nybble but only two height bits.
    const auto* horizontal_stone_wall = shipped_catalog.value().by_object_id(166);
    auto horizontal_stone_wall_input = barrel_input;
    horizontal_stone_wall_input.settings = 0x2003;
    auto horizontal_stone_wall_bounds =
        nsmb::project::lua_actor_bounds(*horizontal_stone_wall, horizontal_stone_wall_input);
    auto horizontal_stone_wall_commands =
        nsmb::project::render_lua_actor(*horizontal_stone_wall, horizontal_stone_wall_input);
    assert(horizontal_stone_wall_bounds && horizontal_stone_wall_bounds.value());
    assert(horizontal_stone_wall_bounds.value()->width == 64 && horizontal_stone_wall_bounds.value()->height == 64);
    assert(horizontal_stone_wall_commands && horizontal_stone_wall_commands.value().size() == 16);
    assert(horizontal_stone_wall_commands.value()[0].x == 160 && horizontal_stone_wall_commands.value()[0].y == 320);
    assert(horizontal_stone_wall_commands.value()[3].x == 208 && horizontal_stone_wall_commands.value()[3].y == 320);
    assert(horizontal_stone_wall_commands.value()[6].x == 208 && horizontal_stone_wall_commands.value()[6].y == 368);
    assert(horizontal_stone_wall_commands.value()[9].x == 160 && horizontal_stone_wall_commands.value()[9].y == 368);

    auto masked_stone_wall_input = barrel_input;
    masked_stone_wall_input.settings = 0x4000;
    auto masked_stone_wall_bounds = nsmb::project::lua_actor_bounds(*horizontal_stone_wall, masked_stone_wall_input);
    auto masked_stone_wall_commands = nsmb::project::render_lua_actor(*horizontal_stone_wall, masked_stone_wall_input);
    assert(masked_stone_wall_bounds && masked_stone_wall_bounds.value());
    assert(masked_stone_wall_bounds.value()->width == 32 && masked_stone_wall_bounds.value()->height == 16);
    assert(masked_stone_wall_commands && masked_stone_wall_commands.value().size() == 2);

    // Actor 167 is the unused duplicate of actor 166 and must keep identical
    // bounds and draw commands for the same settings.
    const auto* unused_stone_wall = shipped_catalog.value().by_object_id(167);
    auto unused_stone_wall_bounds = nsmb::project::lua_actor_bounds(*unused_stone_wall, horizontal_stone_wall_input);
    auto unused_stone_wall_commands =
        nsmb::project::render_lua_actor(*unused_stone_wall, horizontal_stone_wall_input);
    assert(unused_stone_wall_bounds && unused_stone_wall_bounds.value());
    assert(unused_stone_wall_bounds.value()->x == horizontal_stone_wall_bounds.value()->x);
    assert(unused_stone_wall_bounds.value()->y == horizontal_stone_wall_bounds.value()->y);
    assert(unused_stone_wall_bounds.value()->width == horizontal_stone_wall_bounds.value()->width);
    assert(unused_stone_wall_bounds.value()->height == horizontal_stone_wall_bounds.value()->height);
    assert(unused_stone_wall_commands &&
           unused_stone_wall_commands.value().size() == horizontal_stone_wall_commands.value().size());
    for (std::size_t command = 0; command < unused_stone_wall_commands.value().size(); ++command) {
        assert(unused_stone_wall_commands.value()[command].x == horizontal_stone_wall_commands.value()[command].x);
        assert(unused_stone_wall_commands.value()[command].y == horizontal_stone_wall_commands.value()[command].y);
        assert(unused_stone_wall_commands.value()[command].image == horizontal_stone_wall_commands.value()[command].image);
    }

    // Actor 168 deliberately keeps odd selection expansion: vertical spikes
    // enlarge width and horizontal spikes enlarge height.
    const auto* spiked_stone_wall = shipped_catalog.value().by_object_id(168);
    constexpr std::array<double, 7> spiked_wall_width{32, 48, 48, 64, 32, 32, 32};
    constexpr std::array<double, 7> spiked_wall_height{16, 16, 16, 16, 32, 32, 48};
    constexpr std::array<std::size_t, 7> spiked_wall_count{2, 4, 3, 5, 3, 3, 4};
    for (std::uint32_t spikes = 0; spikes < 7; ++spikes) {
        auto spiked_wall_input = barrel_input;
        spiked_wall_input.settings = spikes << 24U;
        auto spiked_wall_bounds = nsmb::project::lua_actor_bounds(*spiked_stone_wall, spiked_wall_input);
        auto spiked_wall_commands = nsmb::project::render_lua_actor(*spiked_stone_wall, spiked_wall_input);
        assert(spiked_wall_bounds && spiked_wall_bounds.value());
        assert(spiked_wall_bounds.value()->width == spiked_wall_width[spikes]);
        assert(spiked_wall_bounds.value()->height == spiked_wall_height[spikes]);
        assert(spiked_wall_commands && spiked_wall_commands.value().size() == spiked_wall_count[spikes]);
    }

    auto both_vertical_spikes_input = barrel_input;
    both_vertical_spikes_input.settings = 3U << 24U;
    auto both_vertical_spikes = nsmb::project::render_lua_actor(*spiked_stone_wall, both_vertical_spikes_input);
    assert(both_vertical_spikes && both_vertical_spikes.value().size() == 5);
    assert(both_vertical_spikes.value()[0].x == 160 && both_vertical_spikes.value()[0].y == 336);
    assert(both_vertical_spikes.value()[1].x == 176 && both_vertical_spikes.value()[1].y == 336);
    assert(both_vertical_spikes.value()[2].x == 160 && both_vertical_spikes.value()[2].y == 320);
    assert(both_vertical_spikes.value()[3].x == 176 && both_vertical_spikes.value()[3].y == 320);
    assert(both_vertical_spikes.value()[4].x == 160 && both_vertical_spikes.value()[4].y == 352);
    assert(both_vertical_spikes.value()[4].flip_y);

    auto both_horizontal_spikes_input = barrel_input;
    both_horizontal_spikes_input.settings = 6U << 24U;
    auto both_horizontal_spikes = nsmb::project::render_lua_actor(*spiked_stone_wall, both_horizontal_spikes_input);
    assert(both_horizontal_spikes && both_horizontal_spikes.value().size() == 4);
    assert(both_horizontal_spikes.value()[0].x == 176 && both_horizontal_spikes.value()[0].y == 320);
    assert(both_horizontal_spikes.value()[1].x == 192 && both_horizontal_spikes.value()[1].y == 320);
    assert(both_horizontal_spikes.value()[2].x == 208 && both_horizontal_spikes.value()[2].rotation == 90);
    assert(both_horizontal_spikes.value()[3].x == 160 && both_horizontal_spikes.value()[3].rotation == 90);
    assert(both_horizontal_spikes.value()[3].flip_x);

    auto square_stone_wall_input = barrel_input;
    square_stone_wall_input.settings = 0x22;
    auto square_stone_wall_commands = nsmb::project::render_lua_actor(*spiked_stone_wall, square_stone_wall_input);
    assert(square_stone_wall_commands && square_stone_wall_commands.value().size() == 9);
    constexpr std::array<double, 9> square_stone_wall_x{160, 176, 192, 192, 192, 176, 160, 160, 176};
    constexpr std::array<double, 9> square_stone_wall_y{320, 320, 320, 336, 352, 352, 352, 336, 336};
    for (std::size_t command = 0; command < square_stone_wall_commands.value().size(); ++command) {
        assert(square_stone_wall_commands.value()[command].x == square_stone_wall_x[command]);
        assert(square_stone_wall_commands.value()[command].y == square_stone_wall_y[command]);
    }

    // The flimsy lift reads only three length bits and doubles the logical
    // value into its tile count, leaving the low nybble's high bit ignored.
    auto flimsy_lift_input = barrel_input;
    flimsy_lift_input.settings = 0xf;
    const auto* flimsy_lift = shipped_catalog.value().by_object_id(173);
    auto flimsy_lift_bounds = nsmb::project::lua_actor_bounds(*flimsy_lift, flimsy_lift_input);
    auto flimsy_lift_commands = nsmb::project::render_lua_actor(*flimsy_lift, flimsy_lift_input);
    assert(flimsy_lift_bounds && flimsy_lift_bounds.value());
    assert(flimsy_lift_bounds.value()->x == 40 && flimsy_lift_bounds.value()->y == 320);
    assert(flimsy_lift_bounds.value()->width == 256 && flimsy_lift_bounds.value()->height == 16);
    assert(flimsy_lift_commands && flimsy_lift_commands.value().size() == 16);
    assert(flimsy_lift_commands.value()[0].x == 40 && flimsy_lift_commands.value()[0].y == 320);
    assert(flimsy_lift_commands.value()[1].x == 56 && flimsy_lift_commands.value()[1].y == 320);
    assert(flimsy_lift_commands.value()[15].x == 280 && flimsy_lift_commands.value()[15].y == 320);

    // The one-way lift clamps its logical length to two tiles and remains
    // centered on the actor origin.
    auto one_way_lift_input = barrel_input;
    one_way_lift_input.settings = 0xf;
    const auto* one_way_lift = shipped_catalog.value().by_object_id(174);
    auto one_way_lift_bounds = nsmb::project::lua_actor_bounds(*one_way_lift, one_way_lift_input);
    auto one_way_lift_commands = nsmb::project::render_lua_actor(*one_way_lift, one_way_lift_input);
    assert(one_way_lift_bounds && one_way_lift_bounds.value());
    assert(one_way_lift_bounds.value()->x == 32 && one_way_lift_bounds.value()->y == 320);
    assert(one_way_lift_bounds.value()->width == 256 && one_way_lift_bounds.value()->height == 16);
    assert(one_way_lift_commands && one_way_lift_commands.value().size() == 16);
    assert(one_way_lift_commands.value()[0].x == 32 && one_way_lift_commands.value()[0].y == 320);
    assert(one_way_lift_commands.value()[15].x == 272 && one_way_lift_commands.value()[15].y == 320);

    // The three-platform rickshaw uses one bit for platform size and one for
    // rotation; the direction marker is flipped for counterclockwise motion.
    auto three_rickshaw_input = barrel_input;
    three_rickshaw_input.settings = 1;
    const auto* three_rickshaw = shipped_catalog.value().by_object_id(175);
    auto three_rickshaw_bounds = nsmb::project::lua_actor_bounds(*three_rickshaw, three_rickshaw_input);
    auto three_rickshaw_commands = nsmb::project::render_lua_actor(*three_rickshaw, three_rickshaw_input);
    assert(three_rickshaw_bounds && three_rickshaw_bounds.value());
    assert(three_rickshaw_bounds.value()->x == 119 && three_rickshaw_bounds.value()->y == 279);
    assert(three_rickshaw_bounds.value()->width == 121 && three_rickshaw_bounds.value()->height == 99);
    assert(three_rickshaw_commands && three_rickshaw_commands.value().size() == 2);
    assert(three_rickshaw_commands.value()[0].x == 119 && three_rickshaw_commands.value()[0].y == 279);
    assert(three_rickshaw_commands.value()[1].x == 153 && three_rickshaw_commands.value()[1].y == 312);
    assert(three_rickshaw_commands.value()[1].width == 32 && three_rickshaw_commands.value()[1].height == 32);
    assert(three_rickshaw_commands.value()[1].flip_x);

    three_rickshaw_input.settings = 1 | (1U << 16U);
    three_rickshaw_commands = nsmb::project::render_lua_actor(*three_rickshaw, three_rickshaw_input);
    assert(three_rickshaw_commands && !three_rickshaw_commands.value()[1].flip_x);

    // Lift-spawner direction is a two-bit field; upper nybble bits do not
    // change the left-spawning platform or its arrow.
    auto lift_spawner_input = barrel_input;
    lift_spawner_input.settings = 7U << 16U;
    const auto* lift_spawner = shipped_catalog.value().by_object_id(177);
    auto lift_spawner_bounds = nsmb::project::lua_actor_bounds(*lift_spawner, lift_spawner_input);
    auto lift_spawner_commands = nsmb::project::render_lua_actor(*lift_spawner, lift_spawner_input);
    assert(lift_spawner_bounds && lift_spawner_bounds.value());
    assert(lift_spawner_bounds.value()->x == 224 && lift_spawner_bounds.value()->y == 320);
    assert(lift_spawner_bounds.value()->width == 64 && lift_spawner_bounds.value()->height == 16);
    assert(lift_spawner_commands && lift_spawner_commands.value().size() == 5);
    assert(lift_spawner_commands.value()[0].x == 224 && lift_spawner_commands.value()[0].y == 320);
    assert(lift_spawner_commands.value()[3].x == 272 && lift_spawner_commands.value()[3].y == 320);
    assert(lift_spawner_commands.value()[4].x == 208 && lift_spawner_commands.value()[4].y == 320);
    assert(lift_spawner_commands.value()[4].rotation == 90);

    // Rectangle-lift shift and rotation are single bits even when their raw
    // nybbles contain other set bits.
    auto rotating_rectangle_input = barrel_input;
    rotating_rectangle_input.settings = 0x30020000;
    const auto* rotating_rectangle = shipped_catalog.value().by_object_id(179);
    auto rotating_rectangle_bounds = nsmb::project::lua_actor_bounds(*rotating_rectangle, rotating_rectangle_input);
    auto rotating_rectangle_commands =
        nsmb::project::render_lua_actor(*rotating_rectangle, rotating_rectangle_input);
    assert(rotating_rectangle_bounds && rotating_rectangle_bounds.value());
    assert(rotating_rectangle_bounds.value()->x == 136 && rotating_rectangle_bounds.value()->y == 320);
    assert(rotating_rectangle_bounds.value()->width == 64 && rotating_rectangle_bounds.value()->height == 32);
    assert(rotating_rectangle_commands && rotating_rectangle_commands.value().size() == 2);
    assert(rotating_rectangle_commands.value()[0].x == 136 && rotating_rectangle_commands.value()[0].y == 320);
    assert(rotating_rectangle_commands.value()[1].x == 152 && rotating_rectangle_commands.value()[1].y == 320);
    assert(rotating_rectangle_commands.value()[1].flip_x);

    // The triangle's direction marker intentionally begins two pixels above
    // the platform bounds.
    const auto* rotating_triangle = shipped_catalog.value().by_object_id(180);
    auto rotating_triangle_bounds = nsmb::project::lua_actor_bounds(*rotating_triangle, rotating_rectangle_input);
    auto rotating_triangle_commands = nsmb::project::render_lua_actor(*rotating_triangle, rotating_rectangle_input);
    assert(rotating_triangle_bounds && rotating_triangle_bounds.value());
    assert(rotating_triangle_bounds.value()->x == 146 && rotating_triangle_bounds.value()->y == 322);
    assert(rotating_triangle_bounds.value()->width == 44 && rotating_triangle_bounds.value()->height == 39);
    assert(rotating_triangle_commands && rotating_triangle_commands.value().size() == 2);
    assert(rotating_triangle_commands.value()[0].x == 146 && rotating_triangle_commands.value()[0].y == 322);
    assert(rotating_triangle_commands.value()[1].x == 152 && rotating_triangle_commands.value()[1].y == 320);
    assert(rotating_triangle_commands.value()[1].flip_x);

    const auto* rotating_square = shipped_catalog.value().by_object_id(181);
    auto rotating_square_bounds = nsmb::project::lua_actor_bounds(*rotating_square, rotating_rectangle_input);
    auto rotating_square_commands = nsmb::project::render_lua_actor(*rotating_square, rotating_rectangle_input);
    assert(rotating_square_bounds && rotating_square_bounds.value());
    assert(rotating_square_bounds.value()->x == 144 && rotating_square_bounds.value()->y == 312);
    assert(rotating_square_bounds.value()->width == 48 && rotating_square_bounds.value()->height == 48);
    assert(rotating_square_commands && rotating_square_commands.value().size() == 2);
    assert(rotating_square_commands.value()[0].x == 144 && rotating_square_commands.value()[0].y == 312);
    assert(rotating_square_commands.value()[1].x == 152 && rotating_square_commands.value()[1].y == 320);
    assert(rotating_square_commands.value()[1].flip_x);

    // The track-controlled lift is a fixed four-tile platform shifted down by
    // half a tile.
    const auto* track_lift = shipped_catalog.value().by_object_id(182);
    auto track_lift_bounds = nsmb::project::lua_actor_bounds(*track_lift, barrel_input);
    auto track_lift_commands = nsmb::project::render_lua_actor(*track_lift, barrel_input);
    assert(track_lift_bounds && track_lift_bounds.value());
    assert(track_lift_bounds.value()->x == 160 && track_lift_bounds.value()->y == 328);
    assert(track_lift_bounds.value()->width == 64 && track_lift_bounds.value()->height == 16);
    assert(track_lift_commands && track_lift_commands.value().size() == 4);
    for (std::size_t command = 0; command < track_lift_commands.value().size(); ++command) {
        assert(track_lift_commands.value()[command].x == 160 + 16 * command);
        assert(track_lift_commands.value()[command].y == 328);
    }

    const auto* four_rickshaw = shipped_catalog.value().by_object_id(183);
    auto four_rickshaw_bounds = nsmb::project::lua_actor_bounds(*four_rickshaw, barrel_input);
    auto four_rickshaw_commands = nsmb::project::render_lua_actor(*four_rickshaw, barrel_input);
    assert(four_rickshaw_bounds && four_rickshaw_bounds.value());
    assert(four_rickshaw_bounds.value()->x == 122 && four_rickshaw_bounds.value()->y == 291);
    assert(four_rickshaw_bounds.value()->width == 91 && four_rickshaw_bounds.value()->height == 75);
    assert(four_rickshaw_commands && four_rickshaw_commands.value().size() == 1);
    assert(four_rickshaw_commands.value()[0].x == 122 && four_rickshaw_commands.value()[0].y == 291);
    assert(four_rickshaw_commands.value()[0].width == 91 && four_rickshaw_commands.value()[0].height == 75);

    // Swinging-pole height uses only three low bits; a raw F still produces
    // eleven pole segments plus the attachment drawn last.
    auto swinging_pole_input = barrel_input;
    swinging_pole_input.settings = 0xf;
    const auto* swinging_pole = shipped_catalog.value().by_object_id(184);
    auto swinging_pole_bounds = nsmb::project::lua_actor_bounds(*swinging_pole, swinging_pole_input);
    auto swinging_pole_commands = nsmb::project::render_lua_actor(*swinging_pole, swinging_pole_input);
    assert(swinging_pole_bounds && swinging_pole_bounds.value());
    assert(swinging_pole_bounds.value()->x == 149 && swinging_pole_bounds.value()->y == 309);
    assert(swinging_pole_bounds.value()->width == 22 && swinging_pole_bounds.value()->height == 187);
    assert(swinging_pole_commands && swinging_pole_commands.value().size() == 12);
    for (std::size_t segment = 0; segment < 11; ++segment) {
        assert(swinging_pole_commands.value()[segment].x == 157);
        assert(swinging_pole_commands.value()[segment].y == 320 + 16 * segment);
        assert(swinging_pole_commands.value()[segment].width == 7);
        assert(swinging_pole_commands.value()[segment].height == 16);
    }
    assert(swinging_pole_commands.value()[11].x == 149 && swinging_pole_commands.value()[11].y == 309);
    assert(swinging_pole_commands.value()[11].width == 22 && swinging_pole_commands.value()[11].height == 22);

    const auto* large_spiked_ball = shipped_catalog.value().by_object_id(185);
    auto large_spiked_ball_bounds = nsmb::project::lua_actor_bounds(*large_spiked_ball, barrel_input);
    auto large_spiked_ball_commands = nsmb::project::render_lua_actor(*large_spiked_ball, barrel_input);
    assert(large_spiked_ball_bounds && large_spiked_ball_bounds.value());
    assert(large_spiked_ball_bounds.value()->x == 160 && large_spiked_ball_bounds.value()->y == 278);
    assert(large_spiked_ball_bounds.value()->width == 63 && large_spiked_ball_bounds.value()->height == 58);
    assert(large_spiked_ball_commands && large_spiked_ball_commands.value().size() == 1);
    assert(large_spiked_ball_commands.value()[0].x == 160 && large_spiked_ball_commands.value()[0].y == 278);
    assert(large_spiked_ball_commands.value()[0].width == 63 && large_spiked_ball_commands.value()[0].height == 58);

    // The giant rickshaw's rotation uses only bit 16; other bits in that
    // nybble leave the counterclockwise marker flipped.
    auto giant_rickshaw_input = barrel_input;
    giant_rickshaw_input.settings = 2U << 16U;
    const auto* giant_rickshaw = shipped_catalog.value().by_object_id(186);
    auto giant_rickshaw_bounds = nsmb::project::lua_actor_bounds(*giant_rickshaw, giant_rickshaw_input);
    auto giant_rickshaw_commands = nsmb::project::render_lua_actor(*giant_rickshaw, giant_rickshaw_input);
    assert(giant_rickshaw_bounds && giant_rickshaw_bounds.value());
    assert(giant_rickshaw_bounds.value()->x == 16 && giant_rickshaw_bounds.value()->y == 216);
    assert(giant_rickshaw_bounds.value()->width == 288 && giant_rickshaw_bounds.value()->height == 208);
    assert(giant_rickshaw_commands && giant_rickshaw_commands.value().size() == 2);
    assert(giant_rickshaw_commands.value()[0].x == 16 && giant_rickshaw_commands.value()[0].y == 216);
    assert(giant_rickshaw_commands.value()[1].x == 144 && giant_rickshaw_commands.value()[1].y == 304);
    assert(giant_rickshaw_commands.value()[1].flip_x);

    giant_rickshaw_input.settings = 1U << 16U;
    giant_rickshaw_commands = nsmb::project::render_lua_actor(*giant_rickshaw, giant_rickshaw_input);
    assert(giant_rickshaw_commands && !giant_rickshaw_commands.value()[1].flip_x);

    // The Mario Bros.-style platform consumes the full low byte for length,
    // not just its declared low nybble.
    auto bump_platform_input = barrel_input;
    bump_platform_input.settings = 0x12;
    const auto* bump_platform = shipped_catalog.value().by_object_id(188);
    auto bump_platform_bounds = nsmb::project::lua_actor_bounds(*bump_platform, bump_platform_input);
    auto bump_platform_commands = nsmb::project::render_lua_actor(*bump_platform, bump_platform_input);
    assert(bump_platform_bounds && bump_platform_bounds.value());
    assert(bump_platform_bounds.value()->x == 10 && bump_platform_bounds.value()->y == 320);
    assert(bump_platform_bounds.value()->width == 300 && bump_platform_bounds.value()->height == 10);
    assert(bump_platform_commands && bump_platform_commands.value().size() == 19);
    assert(bump_platform_commands.value()[0].x == 10 && bump_platform_commands.value()[0].width == 14);
    for (std::size_t section = 0; section < 17; ++section) {
        assert(bump_platform_commands.value()[section + 1].x == 24 + 16 * section);
        assert(bump_platform_commands.value()[section + 1].width == 16);
    }
    assert(bump_platform_commands.value()[18].x == 296 && bump_platform_commands.value()[18].width == 14);

    // Mushroom actor 189 uses the full stalk-height nybble; value eight draws
    // ten stalk pieces before the green platform cap.
    auto tilting_mushroom_input = barrel_input;
    tilting_mushroom_input.settings = (1U << 12U) | (8U << 16U) | (1U << 23U);
    const auto* tilting_mushroom = shipped_catalog.value().by_object_id(189);
    auto tilting_mushroom_bounds = nsmb::project::lua_actor_bounds(*tilting_mushroom, tilting_mushroom_input);
    auto tilting_mushroom_commands =
        nsmb::project::render_lua_actor(*tilting_mushroom, tilting_mushroom_input);
    assert(tilting_mushroom_bounds && tilting_mushroom_bounds.value());
    assert(tilting_mushroom_bounds.value()->x == 128 && tilting_mushroom_bounds.value()->y == 320);
    assert(tilting_mushroom_bounds.value()->width == 96 && tilting_mushroom_bounds.value()->height == 192);
    assert(tilting_mushroom_commands && tilting_mushroom_commands.value().size() == 14);
    assert(tilting_mushroom_commands.value()[0].x == 166 && tilting_mushroom_commands.value()[0].y == 344);
    for (std::size_t segment = 0; segment < 10; ++segment) {
        assert(tilting_mushroom_commands.value()[segment + 1].x == 166);
        assert(tilting_mushroom_commands.value()[segment + 1].y == 352 + 16 * segment);
    }
    assert(tilting_mushroom_commands.value()[11].x == 128 && tilting_mushroom_commands.value()[11].y == 320);
    assert(tilting_mushroom_commands.value()[12].x == 160 && tilting_mushroom_commands.value()[12].y == 320);
    assert(tilting_mushroom_commands.value()[13].x == 192 && tilting_mushroom_commands.value()[13].flip_x);

    // Purple-mushroom length uses only bits 4-5; upper bits in the low byte
    // remain ignored.
    auto purple_mushroom_input = barrel_input;
    purple_mushroom_input.settings = 0xd0;
    const auto* purple_mushroom = shipped_catalog.value().by_object_id(190);
    auto purple_mushroom_bounds = nsmb::project::lua_actor_bounds(*purple_mushroom, purple_mushroom_input);
    auto purple_mushroom_commands = nsmb::project::render_lua_actor(*purple_mushroom, purple_mushroom_input);
    assert(purple_mushroom_bounds && purple_mushroom_bounds.value());
    assert(purple_mushroom_bounds.value()->x == 128 && purple_mushroom_bounds.value()->y == 320);
    assert(purple_mushroom_bounds.value()->width == 96 && purple_mushroom_bounds.value()->height == 112);
    assert(purple_mushroom_commands && purple_mushroom_commands.value().size() == 9);
    assert(purple_mushroom_commands.value()[0].x == 166 && purple_mushroom_commands.value()[0].y == 344);
    for (std::size_t segment = 0; segment < 5; ++segment) {
        assert(purple_mushroom_commands.value()[segment + 1].x == 166);
        assert(purple_mushroom_commands.value()[segment + 1].y == 352 + 16 * segment);
    }
    assert(purple_mushroom_commands.value()[6].x == 128);
    assert(purple_mushroom_commands.value()[7].x == 160);
    assert(purple_mushroom_commands.value()[8].x == 192 && purple_mushroom_commands.value()[8].flip_x);

    // Equal stalk height and travel distance select the rising mushroom art.
    auto rising_mushroom_input = barrel_input;
    rising_mushroom_input.settings = 0x332;
    const auto* rising_mushroom = shipped_catalog.value().by_object_id(191);
    auto rising_mushroom_bounds = nsmb::project::lua_actor_bounds(*rising_mushroom, rising_mushroom_input);
    auto rising_mushroom_commands = nsmb::project::render_lua_actor(*rising_mushroom, rising_mushroom_input);
    assert(rising_mushroom_bounds && rising_mushroom_bounds.value());
    assert(rising_mushroom_bounds.value()->x == 136 && rising_mushroom_bounds.value()->y == 320);
    assert(rising_mushroom_bounds.value()->width == 64 && rising_mushroom_bounds.value()->height == 96);
    assert(rising_mushroom_commands && rising_mushroom_commands.value().size() == 9);
    assert(rising_mushroom_commands.value()[0].x == 160 && rising_mushroom_commands.value()[0].y == 344);
    for (std::size_t segment = 0; segment < 4; ++segment) {
        assert(rising_mushroom_commands.value()[segment + 1].x == 160);
        assert(rising_mushroom_commands.value()[segment + 1].y == 352 + 16 * segment);
    }
    assert(rising_mushroom_commands.value()[5].x == 136);
    assert(rising_mushroom_commands.value()[6].x == 152);
    assert(rising_mushroom_commands.value()[7].x == 168);
    assert(rising_mushroom_commands.value()[8].x == 184 && rising_mushroom_commands.value()[8].flip_x);
    assert(rising_mushroom_commands.value()[5].image == rising_mushroom_commands.value()[8].image);
    auto lowering_mushroom_input = barrel_input;
    lowering_mushroom_input.settings = 0x342;
    auto lowering_mushroom_commands = nsmb::project::render_lua_actor(*rising_mushroom, lowering_mushroom_input);
    assert(lowering_mushroom_commands && lowering_mushroom_commands.value().size() == 10);
    assert(lowering_mushroom_commands.value()[6].image != rising_mushroom_commands.value()[5].image);

    auto mechanical_mushroom_input = barrel_input;
    mechanical_mushroom_input.settings = 0x32;
    const auto* mechanical_mushroom = shipped_catalog.value().by_object_id(192);
    auto mechanical_mushroom_bounds =
        nsmb::project::lua_actor_bounds(*mechanical_mushroom, mechanical_mushroom_input);
    auto mechanical_mushroom_commands =
        nsmb::project::render_lua_actor(*mechanical_mushroom, mechanical_mushroom_input);
    assert(mechanical_mushroom_bounds && mechanical_mushroom_bounds.value());
    assert(mechanical_mushroom_bounds.value()->x == 136 && mechanical_mushroom_bounds.value()->y == 320);
    assert(mechanical_mushroom_bounds.value()->width == 64 && mechanical_mushroom_bounds.value()->height == 97);
    assert(mechanical_mushroom_commands && mechanical_mushroom_commands.value().size() == 8);
    assert(mechanical_mushroom_commands.value()[0].x == 136);
    assert(mechanical_mushroom_commands.value()[1].x == 152);
    assert(mechanical_mushroom_commands.value()[2].x == 168);
    assert(mechanical_mushroom_commands.value()[3].x == 184 && mechanical_mushroom_commands.value()[3].flip_x);
    assert(mechanical_mushroom_commands.value()[4].x == 160 && mechanical_mushroom_commands.value()[4].y == 344);
    for (std::size_t segment = 0; segment < 3; ++segment) {
        assert(mechanical_mushroom_commands.value()[segment + 5].x == 160);
        assert(mechanical_mushroom_commands.value()[segment + 5].y == 369 + 16 * segment);
    }

    // Expanded blue mushrooms start their stalk one tile higher and add a top
    // segment beyond the configured stalk height.
    auto expanding_mushroom_input = barrel_input;
    expanding_mushroom_input.settings = 0x112;
    const auto* expanding_mushroom = shipped_catalog.value().by_object_id(193);
    auto expanding_mushroom_bounds =
        nsmb::project::lua_actor_bounds(*expanding_mushroom, expanding_mushroom_input);
    auto expanding_mushroom_commands =
        nsmb::project::render_lua_actor(*expanding_mushroom, expanding_mushroom_input);
    assert(expanding_mushroom_bounds && expanding_mushroom_bounds.value());
    assert(expanding_mushroom_bounds.value()->x == 128 && expanding_mushroom_bounds.value()->y == 320);
    assert(expanding_mushroom_bounds.value()->width == 80 && expanding_mushroom_bounds.value()->height == 64);
    assert(expanding_mushroom_commands && expanding_mushroom_commands.value().size() == 4);
    assert(expanding_mushroom_commands.value()[0].x == 128 && expanding_mushroom_commands.value()[0].y == 320);
    assert(expanding_mushroom_commands.value()[0].width == 80 && expanding_mushroom_commands.value()[0].height == 16);
    assert(expanding_mushroom_commands.value()[1].x == 160 && expanding_mushroom_commands.value()[1].y == 336);
    assert(expanding_mushroom_commands.value()[2].x == 160 && expanding_mushroom_commands.value()[2].y == 352);
    assert(expanding_mushroom_commands.value()[3].x == 160 && expanding_mushroom_commands.value()[3].y == 368);

    auto bouncy_mushroom_input = barrel_input;
    bouncy_mushroom_input.settings = 0x12;
    const auto* bouncy_mushroom = shipped_catalog.value().by_object_id(194);
    auto bouncy_mushroom_bounds = nsmb::project::lua_actor_bounds(*bouncy_mushroom, bouncy_mushroom_input);
    auto bouncy_mushroom_commands = nsmb::project::render_lua_actor(*bouncy_mushroom, bouncy_mushroom_input);
    assert(bouncy_mushroom_bounds && bouncy_mushroom_bounds.value());
    assert(bouncy_mushroom_bounds.value()->x == 112 && bouncy_mushroom_bounds.value()->y == 320);
    assert(bouncy_mushroom_bounds.value()->width == 96 && bouncy_mushroom_bounds.value()->height == 64);
    assert(bouncy_mushroom_commands && bouncy_mushroom_commands.value().size() == 3);
    assert(bouncy_mushroom_commands.value()[0].x == 112 && bouncy_mushroom_commands.value()[0].y == 320);
    assert(bouncy_mushroom_commands.value()[1].x == 155 && bouncy_mushroom_commands.value()[1].y == 352);
    assert(bouncy_mushroom_commands.value()[2].x == 155 && bouncy_mushroom_commands.value()[2].y == 368);

    auto moving_mushroom_input = barrel_input;
    moving_mushroom_input.settings = (2U << 12U) | (1U << 20U);
    const auto* moving_mushroom = shipped_catalog.value().by_object_id(195);
    auto moving_mushroom_bounds = nsmb::project::lua_actor_bounds(*moving_mushroom, moving_mushroom_input);
    auto moving_mushroom_commands = nsmb::project::render_lua_actor(*moving_mushroom, moving_mushroom_input);
    assert(moving_mushroom_bounds && moving_mushroom_bounds.value());
    assert(moving_mushroom_bounds.value()->x == 112 && moving_mushroom_bounds.value()->y == 320);
    assert(moving_mushroom_bounds.value()->width == 128 && moving_mushroom_bounds.value()->height == 64);
    assert(moving_mushroom_commands && moving_mushroom_commands.value().size() == 7);
    assert(moving_mushroom_commands.value()[0].x == 166 && moving_mushroom_commands.value()[0].y == 344);
    assert(moving_mushroom_commands.value()[1].x == 166 && moving_mushroom_commands.value()[1].y == 352);
    assert(moving_mushroom_commands.value()[2].x == 166 && moving_mushroom_commands.value()[2].y == 368);
    assert(moving_mushroom_commands.value()[3].x == 112);
    assert(moving_mushroom_commands.value()[4].x == 144);
    assert(moving_mushroom_commands.value()[5].x == 176);
    assert(moving_mushroom_commands.value()[6].x == 208 && moving_mushroom_commands.value()[6].flip_x);

    // Swinging rope length saturates at eight tiles rather than wrapping.
    auto swinging_rope_input = barrel_input;
    swinging_rope_input.settings = 0xf;
    const auto* swinging_rope = shipped_catalog.value().by_object_id(196);
    auto swinging_rope_bounds = nsmb::project::lua_actor_bounds(*swinging_rope, swinging_rope_input);
    auto swinging_rope_commands = nsmb::project::render_lua_actor(*swinging_rope, swinging_rope_input);
    assert(swinging_rope_bounds && swinging_rope_bounds.value());
    assert(swinging_rope_bounds.value()->x == 157 && swinging_rope_bounds.value()->y == 320);
    assert(swinging_rope_bounds.value()->width == 8 && swinging_rope_bounds.value()->height == 128);
    assert(swinging_rope_commands && swinging_rope_commands.value().size() == 8);
    for (std::size_t segment = 0; segment < 8; ++segment) {
        assert(swinging_rope_commands.value()[segment].x == 157);
        assert(swinging_rope_commands.value()[segment].y == 320 + 16 * segment);
        assert(swinging_rope_commands.value()[segment].width == 8);
        assert(swinging_rope_commands.value()[segment].height == 16);
    }

    // Swinging vine length also saturates at four extra middle segments; its
    // 14px bottom leaves two pixels of selection padding.
    auto swinging_vine_input = barrel_input;
    swinging_vine_input.settings = 0xf;
    const auto* swinging_vine = shipped_catalog.value().by_object_id(197);
    auto swinging_vine_bounds = nsmb::project::lua_actor_bounds(*swinging_vine, swinging_vine_input);
    auto swinging_vine_commands = nsmb::project::render_lua_actor(*swinging_vine, swinging_vine_input);
    assert(swinging_vine_bounds && swinging_vine_bounds.value());
    assert(swinging_vine_bounds.value()->x == 144 && swinging_vine_bounds.value()->y == 320);
    assert(swinging_vine_bounds.value()->width == 32 && swinging_vine_bounds.value()->height == 128);
    assert(swinging_vine_commands && swinging_vine_commands.value().size() == 8);
    assert(swinging_vine_commands.value()[0].x == 144 && swinging_vine_commands.value()[0].y == 320);
    for (std::size_t segment = 1; segment <= 6; ++segment) {
        assert(swinging_vine_commands.value()[segment].x == 144);
        assert(swinging_vine_commands.value()[segment].y == 320 + 16 * segment);
    }
    assert(swinging_vine_commands.value()[7].x == 144 && swinging_vine_commands.value()[7].y == 432);
    assert(swinging_vine_commands.value()[7].height == 14);

    // Balance-lift arrow mode 2 deliberately reuses the up arrow; mode 3 omits
    // the arrow while retaining the platform.
    auto balance_lift_input = barrel_input;
    balance_lift_input.settings = 2;
    const auto* balance_lift = shipped_catalog.value().by_object_id(198);
    auto balance_lift_bounds = nsmb::project::lua_actor_bounds(*balance_lift, balance_lift_input);
    auto balance_lift_commands = nsmb::project::render_lua_actor(*balance_lift, balance_lift_input);
    assert(balance_lift_bounds && balance_lift_bounds.value());
    assert(balance_lift_bounds.value()->x == 160 && balance_lift_bounds.value()->y == 320);
    assert(balance_lift_bounds.value()->width == 112 && balance_lift_bounds.value()->height == 16);
    assert(balance_lift_commands && balance_lift_commands.value().size() == 2);
    assert(balance_lift_commands.value()[0].x == 160 && balance_lift_commands.value()[0].y == 320);
    assert(balance_lift_commands.value()[1].x == 211 && balance_lift_commands.value()[1].y == 324);

    balance_lift_input.settings = 3;
    balance_lift_commands = nsmb::project::render_lua_actor(*balance_lift, balance_lift_input);
    assert(balance_lift_commands && balance_lift_commands.value().size() == 1);

    auto sinking_snow_input = barrel_input;
    sinking_snow_input.settings = 2;
    const auto* sinking_snow = shipped_catalog.value().by_object_id(200);
    auto sinking_snow_bounds = nsmb::project::lua_actor_bounds(*sinking_snow, sinking_snow_input);
    auto sinking_snow_commands = nsmb::project::render_lua_actor(*sinking_snow, sinking_snow_input);
    assert(sinking_snow_bounds && sinking_snow_bounds.value());
    assert(sinking_snow_bounds.value()->x == 144 && sinking_snow_bounds.value()->y == 320);
    assert(sinking_snow_bounds.value()->width == 80 && sinking_snow_bounds.value()->height == 35);
    assert(sinking_snow_commands && sinking_snow_commands.value().size() == 5);
    assert(sinking_snow_commands.value()[0].x == 144);
    assert(sinking_snow_commands.value()[1].x == 160);
    assert(sinking_snow_commands.value()[2].x == 176);
    assert(sinking_snow_commands.value()[3].x == 192);
    assert(sinking_snow_commands.value()[4].x == 208 && sinking_snow_commands.value()[4].flip_x);

    // Squishy-cloud length consumes the full low byte.
    auto squishy_cloud_input = barrel_input;
    squishy_cloud_input.settings = 0x10;
    const auto* squishy_cloud = shipped_catalog.value().by_object_id(201);
    auto squishy_cloud_bounds = nsmb::project::lua_actor_bounds(*squishy_cloud, squishy_cloud_input);
    auto squishy_cloud_commands = nsmb::project::render_lua_actor(*squishy_cloud, squishy_cloud_input);
    assert(squishy_cloud_bounds && squishy_cloud_bounds.value());
    assert(squishy_cloud_bounds.value()->x == 160 && squishy_cloud_bounds.value()->y == 320);
    assert(squishy_cloud_bounds.value()->width == 576 && squishy_cloud_bounds.value()->height == 32);
    assert(squishy_cloud_commands && squishy_cloud_commands.value().size() == 19);
    assert(squishy_cloud_commands.value()[0].x == 160 && squishy_cloud_commands.value()[0].height == 26);
    for (std::size_t section = 0; section < 17; ++section) {
        assert(squishy_cloud_commands.value()[section + 1].x == 176 + 32 * section);
        assert(squishy_cloud_commands.value()[section + 1].width == 32);
        assert(squishy_cloud_commands.value()[section + 1].height == 32);
    }
    assert(squishy_cloud_commands.value()[18].x == 720 && squishy_cloud_commands.value()[18].height == 25);

    // Pipe-cannon angle uses only three bits; raw D selects the long-right
    // cannon through its low three bits.
    auto pipe_cannon_input = barrel_input;
    pipe_cannon_input.settings = 0xd;
    const auto* pipe_cannon = shipped_catalog.value().by_object_id(202);
    auto pipe_cannon_bounds = nsmb::project::lua_actor_bounds(*pipe_cannon, pipe_cannon_input);
    auto pipe_cannon_commands = nsmb::project::render_lua_actor(*pipe_cannon, pipe_cannon_input);
    assert(pipe_cannon_bounds && pipe_cannon_bounds.value());
    assert(pipe_cannon_bounds.value()->x == 161 && pipe_cannon_bounds.value()->y == 273);
    assert(pipe_cannon_bounds.value()->width == 57 && pipe_cannon_bounds.value()->height == 63);
    assert(pipe_cannon_commands && pipe_cannon_commands.value().size() == 1);
    assert(pipe_cannon_commands.value()[0].x == 161 && pipe_cannon_commands.value()[0].y == 273);
    assert(pipe_cannon_commands.value()[0].width == 57 && pipe_cannon_commands.value()[0].height == 63);

    // Pump shifts use only the low bit of their nybbles; distance fields remain
    // full numeric nybbles.
    auto corked_pipe_input = barrel_input;
    corked_pipe_input.settings = 0x00032330;
    const auto* corked_pipe = shipped_catalog.value().by_object_id(204);
    auto corked_pipe_bounds = nsmb::project::lua_actor_bounds(*corked_pipe, corked_pipe_input);
    auto corked_pipe_commands = nsmb::project::render_lua_actor(*corked_pipe, corked_pipe_input);
    assert(corked_pipe_bounds && corked_pipe_bounds.value());
    assert(corked_pipe_bounds.value()->x == 160 && corked_pipe_bounds.value()->y == 264);
    assert(corked_pipe_bounds.value()->width == 32 && corked_pipe_bounds.value()->height == 72);
    assert(corked_pipe_commands && corked_pipe_commands.value().size() == 2);
    assert(corked_pipe_commands.value()[0].x == 160 && corked_pipe_commands.value()[0].y == 264);
    assert(corked_pipe_commands.value()[1].x == -80 && corked_pipe_commands.value()[1].y == 529);

    // Wall-jump lift dimensions use two bits apiece despite their raw nybbles.
    auto wall_jump_lift_input = barrel_input;
    wall_jump_lift_input.settings = 0xf00f;
    const auto* wall_jump_lift = shipped_catalog.value().by_object_id(209);
    auto wall_jump_lift_bounds = nsmb::project::lua_actor_bounds(*wall_jump_lift, wall_jump_lift_input);
    auto wall_jump_lift_commands = nsmb::project::render_lua_actor(*wall_jump_lift, wall_jump_lift_input);
    assert(wall_jump_lift_bounds && wall_jump_lift_bounds.value());
    assert(wall_jump_lift_bounds.value()->x == 112 && wall_jump_lift_bounds.value()->y == 325);
    assert(wall_jump_lift_bounds.value()->width == 112 && wall_jump_lift_bounds.value()->height == 131);
    assert(wall_jump_lift_commands && wall_jump_lift_commands.value().size() == 20);
    for (std::size_t row = 0; row < 7; ++row) {
        assert(wall_jump_lift_commands.value()[2 * row].x == 112);
        assert(wall_jump_lift_commands.value()[2 * row + 1].x == 208);
        assert(wall_jump_lift_commands.value()[2 * row].y == 440 - 16 * row);
        assert(wall_jump_lift_commands.value()[2 * row + 1].y == 440 - 16 * row);
    }
    for (std::size_t column = 0; column < 5; ++column) {
        assert(wall_jump_lift_commands.value()[14 + column].x == 128 + 16 * column);
        assert(wall_jump_lift_commands.value()[14 + column].y == 344);
    }
    assert(wall_jump_lift_commands.value()[19].x == 157 && wall_jump_lift_commands.value()[19].y == 325);

    // Carry-through-wall direction is an exact-zero versus nonzero field.
    const auto* through_wall_lift = shipped_catalog.value().by_object_id(210);
    auto through_wall_bounds = nsmb::project::lua_actor_bounds(*through_wall_lift, barrel_input);
    auto through_wall_commands = nsmb::project::render_lua_actor(*through_wall_lift, barrel_input);
    assert(through_wall_bounds && through_wall_bounds.value());
    assert(through_wall_bounds.value()->x == 80 && through_wall_bounds.value()->y == 320);
    assert(through_wall_commands && through_wall_commands.value().size() == 1);
    assert(through_wall_commands.value()[0].x == 80 && !through_wall_commands.value()[0].flip_x);

    auto reversed_through_wall_input = barrel_input;
    reversed_through_wall_input.settings = 2U << 20U;
    through_wall_bounds = nsmb::project::lua_actor_bounds(*through_wall_lift, reversed_through_wall_input);
    through_wall_commands = nsmb::project::render_lua_actor(*through_wall_lift, reversed_through_wall_input);
    assert(through_wall_bounds && through_wall_bounds.value() && through_wall_bounds.value()->x == 160);
    assert(through_wall_commands && through_wall_commands.value()[0].x == 160);
    assert(through_wall_commands.value()[0].flip_x);

    // Snowy branch selects the left art only for exact value one.
    auto snowy_branch_input = barrel_input;
    snowy_branch_input.settings = 1;
    const auto* snowy_branch = shipped_catalog.value().by_object_id(212);
    auto snowy_branch_bounds = nsmb::project::lua_actor_bounds(*snowy_branch, snowy_branch_input);
    auto snowy_branch_commands = nsmb::project::render_lua_actor(*snowy_branch, snowy_branch_input);
    assert(snowy_branch_bounds && snowy_branch_bounds.value());
    assert(snowy_branch_bounds.value()->x == 129 && snowy_branch_bounds.value()->y == 308);
    assert(snowy_branch_bounds.value()->width == 34 && snowy_branch_bounds.value()->height == 23);
    assert(snowy_branch_commands && snowy_branch_commands.value()[0].x == 129);
    const auto left_branch_image = snowy_branch_commands.value()[0].image;

    snowy_branch_input.settings = 3;
    snowy_branch_bounds = nsmb::project::lua_actor_bounds(*snowy_branch, snowy_branch_input);
    snowy_branch_commands = nsmb::project::render_lua_actor(*snowy_branch, snowy_branch_input);
    assert(snowy_branch_bounds && snowy_branch_bounds.value());
    assert(snowy_branch_bounds.value()->x == 175 && snowy_branch_bounds.value()->y == 308);
    assert(snowy_branch_bounds.value()->width == 30 && snowy_branch_bounds.value()->height == 23);
    assert(snowy_branch_commands && snowy_branch_commands.value()[0].image != left_branch_image);

    // The one-way gate preserves the original non-composing transform helper,
    // including quarter-turn half-pixel commands and bounds mismatches.
    const auto* one_way_gate = shipped_catalog.value().by_object_id(213);
    constexpr std::array<double, 8> gate_bounds_x{160, 160, 160, 160, 176, 176, 121, 121};
    constexpr std::array<double, 8> gate_bounds_y{281, 281, 336, 336, 320, 320, 320, 320};
    constexpr std::array<double, 8> gate_bounds_width{16, 16, 16, 16, 39, 39, 39, 39};
    constexpr std::array<double, 8> gate_bounds_height{39, 39, 39, 39, 16, 16, 16, 16};
    constexpr std::array<double, 8> gate_command_x{160, 160, 160, 160, 176, 187.5, 121, 132.5};
    constexpr std::array<double, 8> gate_command_y{281, 281, 336, 336, 320, 308.5, 320, 308.5};
    constexpr std::array<double, 8> gate_rotation{0, 0, 0, 0, 0, 270, 0, 270};
    constexpr std::array<bool, 8> gate_flip_x{true, false, true, false, false, false, false, true};
    constexpr std::array<bool, 8> gate_flip_y{false, true, false, false, true, false, true, false};
    for (std::uint32_t type = 0; type < 8; ++type) {
        auto gate_input = barrel_input;
        gate_input.settings = type;
        auto gate_bounds = nsmb::project::lua_actor_bounds(*one_way_gate, gate_input);
        auto gate_commands = nsmb::project::render_lua_actor(*one_way_gate, gate_input);
        assert(gate_bounds && gate_bounds.value());
        assert(gate_bounds.value()->x == gate_bounds_x[type] && gate_bounds.value()->y == gate_bounds_y[type]);
        assert(gate_bounds.value()->width == gate_bounds_width[type]);
        assert(gate_bounds.value()->height == gate_bounds_height[type]);
        assert(gate_commands && gate_commands.value().size() == 1);
        assert(gate_commands.value()[0].x == gate_command_x[type]);
        assert(gate_commands.value()[0].y == gate_command_y[type]);
        assert(gate_commands.value()[0].rotation == gate_rotation[type]);
        assert(gate_commands.value()[0].flip_x == gate_flip_x[type]);
        assert(gate_commands.value()[0].flip_y == gate_flip_y[type]);
    }
    auto gate_high_nybble_input = barrel_input;
    gate_high_nybble_input.settings = 0x10;
    auto gate_high_nybble_bounds = nsmb::project::lua_actor_bounds(*one_way_gate, gate_high_nybble_input);
    auto gate_high_nybble_commands = nsmb::project::render_lua_actor(*one_way_gate, gate_high_nybble_input);
    assert(gate_high_nybble_bounds && gate_high_nybble_bounds.value());
    assert(gate_high_nybble_bounds.value()->y == 281);
    assert(gate_high_nybble_commands && gate_high_nybble_commands.value()[0].x == 160);
    assert(gate_high_nybble_commands.value()[0].y == 320);
    assert(!gate_high_nybble_commands.value()[0].flip_x && !gate_high_nybble_commands.value()[0].flip_y);

    auto snake_block_input = barrel_input;
    snake_block_input.settings = 2U << 12U;
    const auto* snake_block = shipped_catalog.value().by_object_id(214);
    auto snake_block_bounds = nsmb::project::lua_actor_bounds(*snake_block, snake_block_input);
    auto snake_block_commands = nsmb::project::render_lua_actor(*snake_block, snake_block_input);
    assert(snake_block_bounds && snake_block_bounds.value());
    assert(snake_block_bounds.value()->x == 160 && snake_block_bounds.value()->y == 320);
    assert(snake_block_bounds.value()->width == 80 && snake_block_bounds.value()->height == 16);
    assert(snake_block_commands && snake_block_commands.value().size() == 5);
    for (std::size_t block = 0; block < 5; ++block) {
        assert(snake_block_commands.value()[block].x == 160 + 16 * block);
        assert(snake_block_commands.value()[block].y == 320);
    }

    // Slow and fast spike pillars share exact directional geometry. Horizontal
    // variants retain the 40px gfx.blit quarter-turn correction.
    constexpr std::array<std::uint32_t, 8> spike_pillar_ids{216, 217, 218, 219, 220, 221, 222, 223};
    constexpr std::array<double, 8> spike_pillar_bounds_x{160, 160, 160, 32, 160, 160, 160, 32};
    constexpr std::array<double, 8> spike_pillar_bounds_y{192, 320, 320, 320, 192, 320, 320, 320};
    constexpr std::array<double, 8> spike_pillar_bounds_width{64, 64, 144, 144, 64, 64, 144, 144};
    constexpr std::array<double, 8> spike_pillar_bounds_height{144, 144, 64, 64, 144, 144, 64, 64};
    constexpr std::array<double, 8> spike_pillar_command_x{160, 160, 200, 72, 160, 160, 200, 72};
    constexpr std::array<double, 8> spike_pillar_command_y{192, 320, 280, 280, 192, 320, 280, 280};
    constexpr std::array<double, 8> spike_pillar_rotation{0, 180, 90, 270, 0, 180, 90, 270};
    for (std::size_t pillar = 0; pillar < spike_pillar_ids.size(); ++pillar) {
        const auto* spike_pillar = shipped_catalog.value().by_object_id(spike_pillar_ids[pillar]);
        auto spike_pillar_bounds = nsmb::project::lua_actor_bounds(*spike_pillar, barrel_input);
        auto spike_pillar_commands = nsmb::project::render_lua_actor(*spike_pillar, barrel_input);
        assert(spike_pillar_bounds && spike_pillar_bounds.value());
        assert(spike_pillar_bounds.value()->x == spike_pillar_bounds_x[pillar]);
        assert(spike_pillar_bounds.value()->y == spike_pillar_bounds_y[pillar]);
        assert(spike_pillar_bounds.value()->width == spike_pillar_bounds_width[pillar]);
        assert(spike_pillar_bounds.value()->height == spike_pillar_bounds_height[pillar]);
        assert(spike_pillar_commands && spike_pillar_commands.value().size() == 1);
        assert(spike_pillar_commands.value()[0].x == spike_pillar_command_x[pillar]);
        assert(spike_pillar_commands.value()[0].y == spike_pillar_command_y[pillar]);
        assert(spike_pillar_commands.value()[0].width == 64 && spike_pillar_commands.value()[0].height == 144);
        assert(spike_pillar_commands.value()[0].rotation == spike_pillar_rotation[pillar]);
    }

    const auto* whirlpool = shipped_catalog.value().by_object_id(226);
    auto whirlpool_bounds = nsmb::project::lua_actor_bounds(*whirlpool, barrel_input);
    auto whirlpool_commands = nsmb::project::render_lua_actor(*whirlpool, barrel_input);
    assert(whirlpool_bounds && whirlpool_bounds.value());
    assert(whirlpool_bounds.value()->x == 82 && whirlpool_bounds.value()->y == 156);
    assert(whirlpool_bounds.value()->width == 165 && whirlpool_bounds.value()->height == 182);
    assert(whirlpool_commands && whirlpool_commands.value().size() == 1);
    assert(whirlpool_commands.value()[0].x == 82 && whirlpool_commands.value()[0].y == 156);
    assert(whirlpool_commands.value()[0].width == 165 && whirlpool_commands.value()[0].height == 182);

    // Swelling-ground bounds cover the base but intentionally exclude the
    // upward overlay drawn 112 pixels above it.
    const auto* swelling_ground = shipped_catalog.value().by_object_id(231);
    auto swelling_ground_bounds = nsmb::project::lua_actor_bounds(*swelling_ground, barrel_input);
    auto swelling_ground_commands = nsmb::project::render_lua_actor(*swelling_ground, barrel_input);
    assert(swelling_ground_bounds && swelling_ground_bounds.value());
    assert(swelling_ground_bounds.value()->x == 32 && swelling_ground_bounds.value()->y == 320);
    assert(swelling_ground_bounds.value()->width == 276 && swelling_ground_bounds.value()->height == 128);
    assert(swelling_ground_commands && swelling_ground_commands.value().size() == 2);
    assert(swelling_ground_commands.value()[0].x == 32 && swelling_ground_commands.value()[0].y == 320);
    assert(swelling_ground_commands.value()[1].x == 32 && swelling_ground_commands.value()[1].y == 208);
    const auto outward_swelling_image = swelling_ground_commands.value()[1].image;

    auto inward_swelling_input = barrel_input;
    inward_swelling_input.settings = 1U << 24U;
    swelling_ground_commands = nsmb::project::render_lua_actor(*swelling_ground, inward_swelling_input);
    assert(swelling_ground_commands && swelling_ground_commands.value()[1].y == 320);
    assert(swelling_ground_commands.value()[1].image != outward_swelling_image);

    // Tightrope height is a signed nybble; -2 places the second endpoint below
    // the actor while bounds cover both 16px endpoint cells.
    auto tightrope_input = barrel_input;
    tightrope_input.settings = (3U << 28U) | (0xeU << 24U);
    const auto* tightrope = shipped_catalog.value().by_object_id(232);
    auto tightrope_bounds = nsmb::project::lua_actor_bounds(*tightrope, tightrope_input);
    auto tightrope_commands = nsmb::project::render_lua_actor(*tightrope, tightrope_input);
    assert(tightrope_bounds && tightrope_bounds.value());
    assert(tightrope_bounds.value()->x == 152 && tightrope_bounds.value()->y == 320);
    assert(tightrope_bounds.value()->width == 64 && tightrope_bounds.value()->height == 48);
    assert(tightrope_commands && tightrope_commands.value().size() == 3);
    assert(tightrope_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::Line);
    assert(tightrope_commands.value()[0].x == 160 && tightrope_commands.value()[0].y == 328);
    assert(tightrope_commands.value()[0].x2 == 208 && tightrope_commands.value()[0].y2 == 360);
    assert(tightrope_commands.value()[1].x == 152 && tightrope_commands.value()[1].y == 320);
    assert(tightrope_commands.value()[2].x == 200 && tightrope_commands.value()[2].y == 352);

    // Burners preserve the port's up-facing bounds/render mismatch. The
    // direction field includes all six bits consumed by the original modulo.
    constexpr std::array<std::uint32_t, 2> burner_ids{234, 235};
    constexpr std::array<double, 2> burner_bounds_y{272, 208};
    constexpr std::array<double, 2> burner_bounds_height{64, 128};
    constexpr std::array<double, 2> burner_command_x{136, 104};
    constexpr std::array<double, 2> burner_command_y{344, 376};
    constexpr std::array<double, 2> burner_command_width{64, 128};
    for (std::size_t index = 0; index < burner_ids.size(); ++index) {
        auto burner_input = barrel_input;
        burner_input.settings = 2;
        const auto* burner = shipped_catalog.value().by_object_id(burner_ids[index]);
        auto burner_bounds = nsmb::project::lua_actor_bounds(*burner, burner_input);
        auto burner_commands = nsmb::project::render_lua_actor(*burner, burner_input);
        assert(burner_bounds && burner_bounds.value());
        assert(burner_bounds.value()->x == 160 && burner_bounds.value()->y == burner_bounds_y[index]);
        assert(burner_bounds.value()->width == 16);
        assert(burner_bounds.value()->height == burner_bounds_height[index]);
        assert(burner_commands && burner_commands.value().size() == 1);
        assert(burner_commands.value()[0].x == burner_command_x[index]);
        assert(burner_commands.value()[0].y == burner_command_y[index]);
        assert(burner_commands.value()[0].width == burner_command_width[index]);
        assert(burner_commands.value()[0].height == 16);
        assert(burner_commands.value()[0].rotation == 270);

        burner_input.settings = 4;
        burner_commands = nsmb::project::render_lua_actor(*burner, burner_input);
        assert(burner_commands && burner_commands.value().empty());
    }

    auto trampoline_input = barrel_input;
    trampoline_input.settings = 0x11;
    const auto* trampoline = shipped_catalog.value().by_object_id(237);
    auto trampoline_bounds = nsmb::project::lua_actor_bounds(*trampoline, trampoline_input);
    auto trampoline_commands = nsmb::project::render_lua_actor(*trampoline, trampoline_input);
    assert(trampoline_bounds && trampoline_bounds.value());
    assert(trampoline_bounds.value()->x == 160 && trampoline_bounds.value()->y == 304);
    assert(trampoline_bounds.value()->width == 32 && trampoline_bounds.value()->height == 32);
    assert(trampoline_commands && trampoline_commands.value().size() == 1);
    assert(trampoline_commands.value()[0].x == 160 && trampoline_commands.value()[0].y == 304);
    assert(trampoline_commands.value()[0].width == 32 && trampoline_commands.value()[0].height == 32);

    auto question_switch_input = barrel_input;
    question_switch_input.settings = 0x11;
    const auto* question_switch = shipped_catalog.value().by_object_id(242);
    auto question_switch_bounds = nsmb::project::lua_actor_bounds(*question_switch, question_switch_input);
    auto question_switch_commands = nsmb::project::render_lua_actor(*question_switch, question_switch_input);
    assert(question_switch_bounds && question_switch_bounds.value());
    assert(question_switch_bounds.value()->x == 168 && question_switch_bounds.value()->y == 320);
    assert(question_switch_bounds.value()->width == 16 && question_switch_bounds.value()->height == 18);
    assert(question_switch_commands && question_switch_commands.value().size() == 1);
    assert(question_switch_commands.value()[0].x == 168 && question_switch_commands.value()[0].y == 320);
    assert(question_switch_commands.value()[0].rotation == 180);

    const auto* p_switch = shipped_catalog.value().by_object_id(244);
    auto p_switch_bounds = nsmb::project::lua_actor_bounds(*p_switch, question_switch_input);
    auto p_switch_commands = nsmb::project::render_lua_actor(*p_switch, question_switch_input);
    assert(p_switch_bounds && p_switch_bounds.value());
    assert(p_switch_bounds.value()->x == 168 && p_switch_bounds.value()->y == 320);
    assert(p_switch_bounds.value()->width == 16 && p_switch_bounds.value()->height == 18);
    assert(p_switch_commands && p_switch_commands.value().size() == 1);
    assert(p_switch_commands.value()[0].x == 168 && p_switch_commands.value()[0].y == 320);
    assert(p_switch_commands.value()[0].rotation == 180);

    const auto* red_switch = shipped_catalog.value().by_object_id(246);
    auto red_switch_bounds = nsmb::project::lua_actor_bounds(*red_switch, question_switch_input);
    auto red_switch_commands = nsmb::project::render_lua_actor(*red_switch, question_switch_input);
    assert(red_switch_bounds && red_switch_bounds.value());
    assert(red_switch_bounds.value()->x == 168 && red_switch_bounds.value()->y == 320);
    assert(red_switch_bounds.value()->width == 16 && red_switch_bounds.value()->height == 18);
    assert(red_switch_commands && red_switch_commands.value().size() == 1);
    assert(red_switch_commands.value()[0].x == 168 && red_switch_commands.value()[0].y == 320);
    assert(red_switch_commands.value()[0].rotation == 180);

    auto bill_blaster_input = barrel_input;
    bill_blaster_input.settings = 3U << 4U;
    const auto* bill_blaster = shipped_catalog.value().by_object_id(248);
    auto bill_blaster_bounds = nsmb::project::lua_actor_bounds(*bill_blaster, bill_blaster_input);
    auto bill_blaster_commands = nsmb::project::render_lua_actor(*bill_blaster, bill_blaster_input);
    assert(bill_blaster_bounds && bill_blaster_bounds.value());
    assert(bill_blaster_bounds.value()->x == 160 && bill_blaster_bounds.value()->y == 256);
    assert(bill_blaster_bounds.value()->width == 16 && bill_blaster_bounds.value()->height == 80);
    assert(bill_blaster_commands && bill_blaster_commands.value().size() == 4);
    assert(bill_blaster_commands.value()[0].y == 320);
    assert(bill_blaster_commands.value()[1].y == 304);
    assert(bill_blaster_commands.value()[2].y == 288);
    assert(bill_blaster_commands.value()[3].y == 256);
    assert(bill_blaster_commands.value()[0].image != bill_blaster_commands.value()[3].image);

    const auto* banzai_blaster = shipped_catalog.value().by_object_id(249);
    auto banzai_blaster_bounds = nsmb::project::lua_actor_bounds(*banzai_blaster, barrel_input);
    auto banzai_blaster_commands = nsmb::project::render_lua_actor(*banzai_blaster, barrel_input);
    assert(banzai_blaster_bounds && banzai_blaster_bounds.value());
    assert(banzai_blaster_bounds.value()->x == 128 && banzai_blaster_bounds.value()->y == 256);
    assert(banzai_blaster_bounds.value()->width == 64 && banzai_blaster_bounds.value()->height == 80);
    assert(banzai_blaster_commands && banzai_blaster_commands.value().size() == 1);
    assert(banzai_blaster_commands.value()[0].x == 128 && banzai_blaster_commands.value()[0].y == 256);
    assert(banzai_blaster_commands.value()[0].width == 64);
    assert(banzai_blaster_commands.value()[0].height == 80);

    const auto* hanging_block = shipped_catalog.value().by_object_id(250);
    auto hanging_block_bounds = nsmb::project::lua_actor_bounds(*hanging_block, barrel_input);
    auto hanging_block_commands = nsmb::project::render_lua_actor(*hanging_block, barrel_input);
    assert(hanging_block_bounds && hanging_block_bounds.value());
    assert(hanging_block_bounds.value()->x == 160 && hanging_block_bounds.value()->y == 240);
    assert(hanging_block_bounds.value()->width == 16 && hanging_block_bounds.value()->height == 144);
    assert(hanging_block_commands && hanging_block_commands.value().size() == 4);
    assert(hanging_block_commands.value()[0].x == 160 && hanging_block_commands.value()[0].y == 368);
    assert(hanging_block_commands.value()[1].x == 160 && hanging_block_commands.value()[1].y == 368);
    assert(hanging_block_commands.value()[1].source_x == 0);
    assert(hanging_block_commands.value()[2].kind == nsmb::project::LuaActorDrawKind::Line);
    assert(hanging_block_commands.value()[2].x == 167 && hanging_block_commands.value()[2].y == 368);
    assert(hanging_block_commands.value()[2].x2 == 167 && hanging_block_commands.value()[2].y2 == 240);
    assert(hanging_block_commands.value()[3].x == 168 && hanging_block_commands.value()[3].x2 == 168);

    auto spiked_block_input = barrel_input;
    spiked_block_input.settings = 2;
    const auto* unused_spiked_block = shipped_catalog.value().by_object_id(251);
    auto unused_spiked_block_bounds = nsmb::project::lua_actor_bounds(*unused_spiked_block, spiked_block_input);
    auto unused_spiked_block_commands = nsmb::project::render_lua_actor(*unused_spiked_block, spiked_block_input);
    assert(unused_spiked_block_bounds && unused_spiked_block_bounds.value());
    assert(unused_spiked_block_bounds.value()->x == 158 && unused_spiked_block_bounds.value()->y == 313);
    assert(unused_spiked_block_bounds.value()->width == 19 && unused_spiked_block_bounds.value()->height == 26);
    assert(unused_spiked_block_commands && unused_spiked_block_commands.value().size() == 2);
    assert(unused_spiked_block_commands.value()[0].x == 158 && unused_spiked_block_commands.value()[0].y == 313);
    assert(unused_spiked_block_commands.value()[1].x == 160 && unused_spiked_block_commands.value()[1].y == 321);
    assert(unused_spiked_block_commands.value()[1].source_x == 48);

    spiked_block_input.settings = 4;
    const auto* spiked_block = shipped_catalog.value().by_object_id(252);
    auto spiked_block_bounds = nsmb::project::lua_actor_bounds(*spiked_block, spiked_block_input);
    auto spiked_block_commands = nsmb::project::render_lua_actor(*spiked_block, spiked_block_input);
    assert(spiked_block_bounds && spiked_block_bounds.value());
    assert(spiked_block_bounds.value()->x == 158 && spiked_block_bounds.value()->y == 313);
    assert(spiked_block_bounds.value()->width == 19 && spiked_block_bounds.value()->height == 26);
    assert(spiked_block_commands && spiked_block_commands.value().size() == 1);
    assert(spiked_block_commands.value()[0].x == 158 && spiked_block_commands.value()[0].y == 313);

    spiked_block_input.settings = 3;
    const auto* second_unused_spiked_block = shipped_catalog.value().by_object_id(253);
    auto second_unused_spiked_block_commands =
        nsmb::project::render_lua_actor(*second_unused_spiked_block, spiked_block_input);
    assert(second_unused_spiked_block_commands && second_unused_spiked_block_commands.value().size() == 2);
    assert(second_unused_spiked_block_commands.value()[0].x == 158);
    assert(second_unused_spiked_block_commands.value()[0].y == 313);
    assert(second_unused_spiked_block_commands.value()[1].x == 160);
    assert(second_unused_spiked_block_commands.value()[1].y == 321);
    assert(second_unused_spiked_block_commands.value()[1].source_x == 48);

    const auto* spin_block = shipped_catalog.value().by_object_id(254);
    auto spin_block_bounds = nsmb::project::lua_actor_bounds(*spin_block, barrel_input);
    auto spin_block_commands = nsmb::project::render_lua_actor(*spin_block, barrel_input);
    assert(spin_block_bounds && spin_block_bounds.value());
    assert(spin_block_bounds.value()->x == 154 && spin_block_bounds.value()->y == 320);
    assert(spin_block_bounds.value()->width == 26 && spin_block_bounds.value()->height == 26);
    assert(spin_block_commands && spin_block_commands.value().size() == 1);
    assert(spin_block_commands.value()[0].x == 154 && spin_block_commands.value()[0].y == 320);
    assert(spin_block_commands.value()[0].width == 26 && spin_block_commands.value()[0].height == 26);

    const auto* unused_spin_block = shipped_catalog.value().by_object_id(255);
    auto unused_spin_block_commands = nsmb::project::render_lua_actor(*unused_spin_block, barrel_input);
    assert(unused_spin_block_commands && unused_spin_block_commands.value().size() == 1);
    assert(unused_spin_block_commands.value()[0].x == 154);
    assert(unused_spin_block_commands.value()[0].y == 320);
    assert(unused_spin_block_commands.value()[0].width == 26);
    assert(unused_spin_block_commands.value()[0].height == 26);

    const auto* second_unused_spin_block = shipped_catalog.value().by_object_id(256);
    auto second_unused_spin_block_commands =
        nsmb::project::render_lua_actor(*second_unused_spin_block, barrel_input);
    assert(second_unused_spin_block_commands && second_unused_spin_block_commands.value().size() == 1);
    assert(second_unused_spin_block_commands.value()[0].x == 154);
    assert(second_unused_spin_block_commands.value()[0].y == 320);
    assert(second_unused_spin_block_commands.value()[0].width == 26);
    assert(second_unused_spin_block_commands.value()[0].height == 26);

    auto hanging_question_block_input = barrel_input;
    hanging_question_block_input.settings = 9;
    const auto* hanging_question_block = shipped_catalog.value().by_object_id(258);
    auto hanging_question_block_bounds =
        nsmb::project::lua_actor_bounds(*hanging_question_block, hanging_question_block_input);
    auto hanging_question_block_commands =
        nsmb::project::render_lua_actor(*hanging_question_block, hanging_question_block_input);
    assert(hanging_question_block_bounds && hanging_question_block_bounds.value());
    assert(hanging_question_block_bounds.value()->x == 160 && hanging_question_block_bounds.value()->y == 192);
    assert(hanging_question_block_bounds.value()->width == 16);
    assert(hanging_question_block_bounds.value()->height == 144);
    assert(hanging_question_block_commands && hanging_question_block_commands.value().size() == 4);
    assert(hanging_question_block_commands.value()[0].x == 160);
    assert(hanging_question_block_commands.value()[1].source_x == 144);
    assert(hanging_question_block_commands.value()[2].x == 167);
    assert(hanging_question_block_commands.value()[2].y == 192);
    assert(hanging_question_block_commands.value()[2].y2 == 320);

    auto blockhopper_input = barrel_input;
    blockhopper_input.settings = 0x332;
    const auto* blockhopper = shipped_catalog.value().by_object_id(259);
    auto blockhopper_bounds = nsmb::project::lua_actor_bounds(*blockhopper, blockhopper_input);
    auto blockhopper_commands = nsmb::project::render_lua_actor(*blockhopper, blockhopper_input);
    assert(blockhopper_bounds && blockhopper_bounds.value());
    assert(blockhopper_bounds.value()->x == 160 && blockhopper_bounds.value()->y == 274);
    assert(blockhopper_bounds.value()->width == 16 && blockhopper_bounds.value()->height == 62);
    assert(blockhopper_commands && blockhopper_commands.value().size() == 4);
    assert(blockhopper_commands.value()[0].x == 160 && blockhopper_commands.value()[0].y == 306);
    assert(blockhopper_commands.value()[0].width == 16 && blockhopper_commands.value()[0].height == 30);
    assert(blockhopper_commands.value()[1].x == 160 && blockhopper_commands.value()[1].y == 309);
    assert(blockhopper_commands.value()[1].source_x == 48);
    assert(blockhopper_commands.value()[2].y == 290);
    assert(blockhopper_commands.value()[3].y == 274);

    auto flying_block_input = barrel_input;
    flying_block_input.settings = 7;
    const auto* flying_block = shipped_catalog.value().by_object_id(260);
    auto flying_block_bounds = nsmb::project::lua_actor_bounds(*flying_block, flying_block_input);
    auto flying_block_commands = nsmb::project::render_lua_actor(*flying_block, flying_block_input);
    assert(flying_block_bounds && flying_block_bounds.value());
    assert(flying_block_bounds.value()->x == 151 && flying_block_bounds.value()->y == 307);
    assert(flying_block_bounds.value()->width == 38 && flying_block_bounds.value()->height == 28);
    assert(flying_block_commands && flying_block_commands.value().size() == 2);
    assert(flying_block_commands.value()[0].x == 151 && flying_block_commands.value()[0].y == 307);
    assert(flying_block_commands.value()[1].x == 160 && flying_block_commands.value()[1].y == 320);
    assert(flying_block_commands.value()[1].source_x == 112);

    const auto* map_flying_block = shipped_catalog.value().by_object_id(261);
    auto map_flying_block_bounds = nsmb::project::lua_actor_bounds(*map_flying_block, barrel_input);
    auto map_flying_block_commands = nsmb::project::render_lua_actor(*map_flying_block, barrel_input);
    assert(map_flying_block_bounds && map_flying_block_bounds.value());
    assert(map_flying_block_bounds.value()->x == 148 && map_flying_block_bounds.value()->y == 312);
    assert(map_flying_block_bounds.value()->width == 44 && map_flying_block_bounds.value()->height == 23);
    assert(map_flying_block_commands && map_flying_block_commands.value().size() == 1);
    assert(map_flying_block_commands.value()[0].x == 148 && map_flying_block_commands.value()[0].y == 312);
    assert(map_flying_block_commands.value()[0].width == 44);
    assert(map_flying_block_commands.value()[0].height == 23);

    auto rotating_blaster_input = barrel_input;
    rotating_blaster_input.settings = 0x000B0507;
    const auto* rotating_blaster = shipped_catalog.value().by_object_id(262);
    auto rotating_blaster_bounds = nsmb::project::lua_actor_bounds(*rotating_blaster, rotating_blaster_input);
    auto rotating_blaster_commands = nsmb::project::render_lua_actor(*rotating_blaster, rotating_blaster_input);
    assert(rotating_blaster_bounds && rotating_blaster_bounds.value());
    assert(rotating_blaster_bounds.value()->x == 157 && rotating_blaster_bounds.value()->y == 272);
    assert(rotating_blaster_bounds.value()->width == 23 && rotating_blaster_bounds.value()->height == 64);
    assert(rotating_blaster_commands && rotating_blaster_commands.value().size() == 4);
    assert(rotating_blaster_commands.value()[0].x == 162 && rotating_blaster_commands.value()[0].y == 320);
    assert(rotating_blaster_commands.value()[1].x == 157 && rotating_blaster_commands.value()[1].y == 304);
    assert(rotating_blaster_commands.value()[1].flip_x);
    assert(rotating_blaster_commands.value()[2].x == 162 && rotating_blaster_commands.value()[2].y == 288);
    assert(rotating_blaster_commands.value()[3].x == 161 && rotating_blaster_commands.value()[3].y == 272);
    assert(!rotating_blaster_commands.value()[3].flip_x);

    const auto* event_door = shipped_catalog.value().by_object_id(263);
    auto event_door_bounds = nsmb::project::lua_actor_bounds(*event_door, barrel_input);
    auto event_door_commands = nsmb::project::render_lua_actor(*event_door, barrel_input);
    assert(event_door_bounds && event_door_bounds.value());
    assert(event_door_bounds.value()->x == 160 && event_door_bounds.value()->y == 288);
    assert(event_door_bounds.value()->width == 32 && event_door_bounds.value()->height == 48);
    assert(event_door_commands && event_door_commands.value().size() == 1);
    assert(event_door_commands.value()[0].x == 160 && event_door_commands.value()[0].y == 288);
    assert(event_door_commands.value()[0].width == 32 && event_door_commands.value()[0].height == 48);

    auto toad_house_block_input = barrel_input;
    toad_house_block_input.settings = 0x30000034;
    const auto* toad_house_block = shipped_catalog.value().by_object_id(264);
    auto toad_house_block_bounds = nsmb::project::lua_actor_bounds(*toad_house_block, toad_house_block_input);
    auto toad_house_block_commands = nsmb::project::render_lua_actor(*toad_house_block, toad_house_block_input);
    assert(toad_house_block_bounds && toad_house_block_bounds.value());
    assert(toad_house_block_bounds.value()->x == 156 && toad_house_block_bounds.value()->y == 308);
    assert(toad_house_block_bounds.value()->width == 40 && toad_house_block_bounds.value()->height == 40);
    assert(toad_house_block_commands && toad_house_block_commands.value().size() == 1);
    assert(toad_house_block_commands.value()[0].x == 156 && toad_house_block_commands.value()[0].y == 308);
    assert(toad_house_block_commands.value()[0].width == 40);
    assert(toad_house_block_commands.value()[0].height == 40);

    const auto* bowser_switch = shipped_catalog.value().by_object_id(265);
    auto bowser_switch_bounds = nsmb::project::lua_actor_bounds(*bowser_switch, barrel_input);
    auto bowser_switch_commands = nsmb::project::render_lua_actor(*bowser_switch, barrel_input);
    assert(bowser_switch_bounds && bowser_switch_bounds.value());
    assert(bowser_switch_bounds.value()->x == 164 && bowser_switch_bounds.value()->y == 313);
    assert(bowser_switch_bounds.value()->width == 24 && bowser_switch_bounds.value()->height == 23);
    assert(bowser_switch_commands && bowser_switch_commands.value().size() == 1);
    assert(bowser_switch_commands.value()[0].x == 164 && bowser_switch_commands.value()[0].y == 313);
    assert(bowser_switch_commands.value()[0].width == 24);
    assert(bowser_switch_commands.value()[0].height == 23);

    auto warp_cannon_input = barrel_input;
    warp_cannon_input.settings = 8U << 28U;
    const auto* warp_cannon = shipped_catalog.value().by_object_id(269);
    auto warp_cannon_bounds = nsmb::project::lua_actor_bounds(*warp_cannon, warp_cannon_input);
    auto warp_cannon_commands = nsmb::project::render_lua_actor(*warp_cannon, warp_cannon_input);
    assert(warp_cannon_bounds && warp_cannon_bounds.value());
    assert(warp_cannon_bounds.value()->x == 151 && warp_cannon_bounds.value()->y == 294);
    assert(warp_cannon_bounds.value()->width == 66 && warp_cannon_bounds.value()->height == 90);
    assert(warp_cannon_commands && warp_cannon_commands.value().size() == 2);
    assert(warp_cannon_commands.value()[0].x == 151 && warp_cannon_commands.value()[0].y == 294);
    assert(warp_cannon_commands.value()[1].x == 186 && warp_cannon_commands.value()[1].y == 343);
    assert(warp_cannon_commands.value()[1].source_x == 54);
    assert(warp_cannon_commands.value()[1].source_width == 18);
    assert(warp_cannon_commands.value()[1].source_height == 19);

    auto warp_entrance_input = barrel_input;
    warp_entrance_input.settings = (2U << 8U) | (3U << 4U);
    const auto* warp_entrance = shipped_catalog.value().by_object_id(87);
    auto warp_entrance_bounds = nsmb::project::lua_actor_bounds(*warp_entrance, warp_entrance_input);
    auto warp_entrance_commands = nsmb::project::render_lua_actor(*warp_entrance, warp_entrance_input);
    assert(warp_entrance_bounds && warp_entrance_bounds.value());
    assert(warp_entrance_bounds.value()->x == 160 && warp_entrance_bounds.value()->y == 320);
    assert(warp_entrance_bounds.value()->width == 48 && warp_entrance_bounds.value()->height == 64);
    assert(warp_entrance_commands && warp_entrance_commands.value().size() == 4);
    assert(warp_entrance_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::StrokeRect);
    assert(warp_entrance_commands.value()[0].x == 161 && warp_entrance_commands.value()[0].y == 321);
    assert(warp_entrance_commands.value()[1].kind == nsmb::project::LuaActorDrawKind::StrokeRect);
    assert(warp_entrance_commands.value()[1].x == 160 && warp_entrance_commands.value()[1].y == 320);
    assert(warp_entrance_commands.value()[2].kind == nsmb::project::LuaActorDrawKind::Text);
    assert(warp_entrance_commands.value()[2].x == 161 && warp_entrance_commands.value()[2].y == 321);

    auto unused_purple_mushroom_input = barrel_input;
    unused_purple_mushroom_input.settings = 0x30;
    const auto* unused_purple_mushroom = shipped_catalog.value().by_object_id(105);
    auto unused_purple_mushroom_bounds =
        nsmb::project::lua_actor_bounds(*unused_purple_mushroom, unused_purple_mushroom_input);
    auto unused_purple_mushroom_commands =
        nsmb::project::render_lua_actor(*unused_purple_mushroom, unused_purple_mushroom_input);
    assert(unused_purple_mushroom_bounds && unused_purple_mushroom_bounds.value());
    assert(unused_purple_mushroom_bounds.value()->x == 96);
    assert(unused_purple_mushroom_bounds.value()->y == 320);
    assert(unused_purple_mushroom_bounds.value()->width == 160);
    assert(unused_purple_mushroom_bounds.value()->height == 112);
    assert(unused_purple_mushroom_commands && unused_purple_mushroom_commands.value().size() == 11);
    assert(unused_purple_mushroom_commands.value()[0].x == 166);
    assert(unused_purple_mushroom_commands.value()[0].y == 344);
    for (std::size_t section = 0; section < 5; ++section) {
        assert(unused_purple_mushroom_commands.value()[section + 1].x == 166);
        assert(unused_purple_mushroom_commands.value()[section + 1].y == 352 + 16 * section);
    }
    assert(unused_purple_mushroom_commands.value()[6].x == 96);
    assert(unused_purple_mushroom_commands.value()[7].x == 128);
    assert(unused_purple_mushroom_commands.value()[8].x == 160);
    assert(unused_purple_mushroom_commands.value()[9].x == 192);
    assert(unused_purple_mushroom_commands.value()[10].x == 224);
    assert(unused_purple_mushroom_commands.value()[10].flip_x);

    auto zoom_input = barrel_input;
    zoom_input.settings = (2U << 20U) | (1U << 16U) | (1U << 5U);
    const auto* zoom = shipped_catalog.value().by_object_id(49);
    auto zoom_bounds = nsmb::project::lua_actor_bounds(*zoom, zoom_input);
    auto zoom_commands = nsmb::project::render_lua_actor(*zoom, zoom_input);
    assert(zoom_bounds && zoom_bounds.value());
    assert(zoom_bounds.value()->x == 128 && zoom_bounds.value()->y == 304);
    assert(zoom_bounds.value()->width == 64 && zoom_bounds.value()->height == 16);
    assert(zoom_commands && zoom_commands.value().size() == 2);
    assert(zoom_commands.value()[0].x == 128 && zoom_commands.value()[0].y == 304);
    assert(zoom_commands.value()[1].x == 176 && zoom_commands.value()[1].y == 304);

    auto infinite_zoom_bounds = nsmb::project::lua_actor_bounds(*zoom, barrel_input);
    auto infinite_zoom_commands = nsmb::project::render_lua_actor(*zoom, barrel_input);
    assert(infinite_zoom_bounds && infinite_zoom_bounds.value());
    assert(infinite_zoom_bounds.value()->x == 32 && infinite_zoom_bounds.value()->y == 304);
    assert(infinite_zoom_bounds.value()->width == 256 && infinite_zoom_bounds.value()->height == 16);
    assert(infinite_zoom_commands && infinite_zoom_commands.value().size() == 24);
    assert(infinite_zoom_commands.value()[16].x == 16 && infinite_zoom_commands.value()[16].y == 304);
    assert(infinite_zoom_commands.value()[16].rotation == 90);
    assert(infinite_zoom_commands.value()[17].x == 288 && infinite_zoom_commands.value()[17].rotation == 270);
    assert(infinite_zoom_commands.value()[20].x == 152 && infinite_zoom_commands.value()[20].y == 320);
    assert(infinite_zoom_commands.value()[21].y == 288 && infinite_zoom_commands.value()[21].rotation == 180);

    zoom_input.settings = (1U << 20U) | (1U << 16U) | (1U << 5U);
    zoom_bounds = nsmb::project::lua_actor_bounds(*zoom, zoom_input);
    zoom_commands = nsmb::project::render_lua_actor(*zoom, zoom_input);
    assert(zoom_bounds && zoom_bounds.value());
    assert(zoom_bounds.value()->x == 160 && zoom_bounds.value()->y == 320);
    assert(zoom_bounds.value()->width == 72 && zoom_bounds.value()->height == 28);
    assert(zoom_commands && zoom_commands.value().size() == 2);
    assert(zoom_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::FillRect);
    assert(zoom_commands.value()[1].kind == nsmb::project::LuaActorDrawKind::Text);

    // Chain Chomp draws the post first, then expands the selection to include
    // the Chomp layered in front when the count is one.
    auto chain_chomp_input = barrel_input;
    chain_chomp_input.settings = 1;
    const auto* chain_chomp = shipped_catalog.value().by_object_id(44);
    auto chain_chomp_bounds = nsmb::project::lua_actor_bounds(*chain_chomp, chain_chomp_input);
    auto chain_chomp_commands = nsmb::project::render_lua_actor(*chain_chomp, chain_chomp_input);
    assert(chain_chomp_bounds && chain_chomp_bounds.value());
    assert(chain_chomp_bounds.value()->x == 123 && chain_chomp_bounds.value()->y == 272);
    assert(chain_chomp_bounds.value()->width == 45 && chain_chomp_bounds.value()->height == 48);
    assert(chain_chomp_commands && chain_chomp_commands.value().size() == 2);
    assert(chain_chomp_commands.value()[0].x == 152 && chain_chomp_commands.value()[0].y == 272);
    assert(chain_chomp_commands.value()[0].width == 16 && chain_chomp_commands.value()[0].height == 48);
    assert(chain_chomp_commands.value()[1].x == 123 && chain_chomp_commands.value()[1].y == 292);
    assert(chain_chomp_commands.value()[1].width == 44 && chain_chomp_commands.value()[1].height == 28);

    // The three-bubble coin layout ignores the regular coin's shift flags.
    auto coin_input = barrel_input;
    coin_input.settings = 0x01100002;
    const auto* coin = shipped_catalog.value().by_object_id(66);
    auto coin_bounds = nsmb::project::lua_actor_bounds(*coin, coin_input);
    auto coin_commands = nsmb::project::render_lua_actor(*coin, coin_input);
    assert(coin_bounds && coin_bounds.value());
    assert(coin_bounds.value()->x == 156 && coin_bounds.value()->y == 300);
    assert(coin_bounds.value()->width == 72 && coin_bounds.value()->height == 40);
    assert(coin_commands && coin_commands.value().size() == 3);
    assert(coin_commands.value()[0].x == 156 && coin_commands.value()[0].y == 316);
    assert(coin_commands.value()[1].x == 180 && coin_commands.value()[1].y == 300);
    assert(coin_commands.value()[2].x == 204 && coin_commands.value()[2].y == 316);
    for (const auto& command : coin_commands.value()) {
        assert(command.width == 24 && command.height == 24);
    }

    // Unagi's three direction flags are separate nybbles with stationary,
    // left, then right precedence; no direction renders the fallback label.
    const auto* unagi = shipped_catalog.value().by_object_id(75);
    auto unagi_input = barrel_input;
    unagi_input.settings = 1;
    auto unagi_bounds = nsmb::project::lua_actor_bounds(*unagi, unagi_input);
    auto unagi_commands = nsmb::project::render_lua_actor(*unagi, unagi_input);
    assert(unagi_bounds && unagi_bounds.value());
    assert(unagi_bounds.value()->x == 163 && unagi_bounds.value()->y == 327);
    assert(unagi_bounds.value()->width == 25 && unagi_bounds.value()->height == 23);
    assert(unagi_commands && unagi_commands.value().size() == 1);
    assert(unagi_commands.value()[0].x == 163 && unagi_commands.value()[0].y == 327);

    unagi_input.settings = 0x10;
    unagi_bounds = nsmb::project::lua_actor_bounds(*unagi, unagi_input);
    unagi_commands = nsmb::project::render_lua_actor(*unagi, unagi_input);
    assert(unagi_bounds && unagi_bounds.value());
    assert(unagi_bounds.value()->x == 205 && unagi_bounds.value()->y == 323);
    assert(unagi_commands && unagi_commands.value().size() == 1);
    assert(unagi_commands.value()[0].x == 205 && unagi_commands.value()[0].flip_x);

    unagi_input.settings = 0x10000;
    unagi_bounds = nsmb::project::lua_actor_bounds(*unagi, unagi_input);
    unagi_commands = nsmb::project::render_lua_actor(*unagi, unagi_input);
    assert(unagi_bounds && unagi_bounds.value());
    assert(unagi_bounds.value()->x == 172 && unagi_bounds.value()->y == 323);
    assert(unagi_commands && unagi_commands.value().size() == 1);
    assert(unagi_commands.value()[0].x == 172 && !unagi_commands.value()[0].flip_x);

    unagi_input.settings = 0;
    unagi_commands = nsmb::project::render_lua_actor(*unagi, unagi_input);
    assert(unagi_commands && unagi_commands.value().size() == 2);
    assert(unagi_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::FillRect);
    assert(unagi_commands.value()[1].kind == nsmb::project::LuaActorDrawKind::Text);
    assert(unagi_commands.value()[1].text == "Eel");

    unagi_input.settings = 0x10011;
    unagi_commands = nsmb::project::render_lua_actor(*unagi, unagi_input);
    assert(unagi_commands && unagi_commands.value().size() == 1);
    assert(unagi_commands.value()[0].x == 163 && unagi_commands.value()[0].y == 327);

    // Pipe-generator direction wraps to the two low bits, matching the
    // original byte mask even for otherwise undocumented nibble values.
    auto pipe_generator_input = barrel_input;
    pipe_generator_input.settings = 0x1f;
    const auto* pipe_generator = shipped_catalog.value().by_object_id(64);
    auto pipe_generator_bounds = nsmb::project::lua_actor_bounds(*pipe_generator, pipe_generator_input);
    auto pipe_generator_commands = nsmb::project::render_lua_actor(*pipe_generator, pipe_generator_input);
    assert(pipe_generator_bounds && pipe_generator_bounds.value());
    assert(pipe_generator_bounds.value()->x == 160 && pipe_generator_bounds.value()->y == 320);
    assert(pipe_generator_bounds.value()->width == 32 && pipe_generator_bounds.value()->height == 32);
    assert(pipe_generator_commands && pipe_generator_commands.value().size() == 3);
    assert(pipe_generator_commands.value()[0].x == 167 && pipe_generator_commands.value()[0].y == 328);
    assert(pipe_generator_commands.value()[0].width == 16 && pipe_generator_commands.value()[0].height == 18);
    assert(pipe_generator_commands.value()[1].x == 160 && pipe_generator_commands.value()[1].y == 320);
    assert(pipe_generator_commands.value()[2].x == 192 && pipe_generator_commands.value()[2].y == 328);
    assert(pipe_generator_commands.value()[2].rotation == 270);

    // A two-sided fire bar walks through the center in layer order, then
    // overlays its direction marker using the declared rotation field.
    auto fire_bar_input = barrel_input;
    fire_bar_input.settings = 0x12;
    const auto* fire_bar = shipped_catalog.value().by_object_id(65);
    auto fire_bar_bounds = nsmb::project::lua_actor_bounds(*fire_bar, fire_bar_input);
    auto fire_bar_commands = nsmb::project::render_lua_actor(*fire_bar, fire_bar_input);
    assert(fire_bar_bounds && fire_bar_bounds.value());
    assert(fire_bar_bounds.value()->x == 140 && fire_bar_bounds.value()->y == 300);
    assert(fire_bar_bounds.value()->width == 52 && fire_bar_bounds.value()->height == 52);
    assert(fire_bar_commands && fire_bar_commands.value().size() == 6);
    assert(fire_bar_commands.value()[0].x == 140 && fire_bar_commands.value()[0].y == 340);
    assert(fire_bar_commands.value()[1].x == 150 && fire_bar_commands.value()[1].y == 330);
    assert(fire_bar_commands.value()[2].x == 164 && fire_bar_commands.value()[2].y == 324);
    assert(fire_bar_commands.value()[2].width == 8 && fire_bar_commands.value()[2].height == 8);
    assert(fire_bar_commands.value()[3].x == 170 && fire_bar_commands.value()[3].y == 310);
    assert(fire_bar_commands.value()[4].x == 180 && fire_bar_commands.value()[4].y == 300);
    assert(fire_bar_commands.value()[5].x == 152 && fire_bar_commands.value()[5].y == 312);
    assert(fire_bar_commands.value()[5].flip_x);

    // Down-facing Squiggler keeps its nonsquare image at the shifted origin
    // and rotates it in place.
    auto squiggler_input = barrel_input;
    squiggler_input.settings = 1;
    const auto* squiggler = shipped_catalog.value().by_object_id(126);
    auto squiggler_bounds = nsmb::project::lua_actor_bounds(*squiggler, squiggler_input);
    auto squiggler_commands = nsmb::project::render_lua_actor(*squiggler, squiggler_input);
    assert(squiggler_bounds && squiggler_bounds.value());
    assert(squiggler_bounds.value()->x == 170 && squiggler_bounds.value()->y == 336);
    assert(squiggler_bounds.value()->width == 13 && squiggler_bounds.value()->height == 15);
    assert(squiggler_commands && squiggler_commands.value().size() == 1);
    assert(squiggler_commands.value()[0].x == 170 && squiggler_commands.value()[0].y == 336);
    assert(squiggler_commands.value()[0].width == 13 && squiggler_commands.value()[0].height == 15);
    assert(squiggler_commands.value()[0].rotation == 180);

    // Scuttlebug's offset field already includes four blocks; the web ends at
    // the image origin while selection bounds exclude the web itself.
    auto scuttlebug_input = barrel_input;
    scuttlebug_input.settings = 0x20;
    const auto* scuttlebug = shipped_catalog.value().by_object_id(117);
    auto scuttlebug_bounds = nsmb::project::lua_actor_bounds(*scuttlebug, scuttlebug_input);
    auto scuttlebug_commands = nsmb::project::render_lua_actor(*scuttlebug, scuttlebug_input);
    assert(scuttlebug_bounds && scuttlebug_bounds.value());
    assert(scuttlebug_bounds.value()->x == 146 && scuttlebug_bounds.value()->y == 416);
    assert(scuttlebug_bounds.value()->width == 44 && scuttlebug_bounds.value()->height == 37);
    assert(scuttlebug_commands && scuttlebug_commands.value().size() == 3);
    assert(scuttlebug_commands.value()[0].kind == nsmb::project::LuaActorDrawKind::Line);
    assert(scuttlebug_commands.value()[0].x == 167 && scuttlebug_commands.value()[0].y == 320);
    assert(scuttlebug_commands.value()[0].x2 == 167 && scuttlebug_commands.value()[0].y2 == 416);
    assert(scuttlebug_commands.value()[1].x == 168 && scuttlebug_commands.value()[1].x2 == 168);
    assert(scuttlebug_commands.value()[2].x == 146 && scuttlebug_commands.value()[2].y == 416);

    // Bounce-bubble distance and direction are independent fields; ignored
    // low-byte bits do not change the preview offset.
    const auto* bounce_bubble = shipped_catalog.value().by_object_id(124);
    auto bounce_bubble_input = barrel_input;
    bounce_bubble_input.settings = 0x30;
    auto bounce_bubble_bounds = nsmb::project::lua_actor_bounds(*bounce_bubble, bounce_bubble_input);
    auto bounce_bubble_commands = nsmb::project::render_lua_actor(*bounce_bubble, bounce_bubble_input);
    assert(bounce_bubble_bounds && bounce_bubble_bounds.value());
    assert(bounce_bubble_bounds.value()->x == 140 && bounce_bubble_bounds.value()->y == 294);
    assert(bounce_bubble_commands && bounce_bubble_commands.value()[0].x == 140);

    bounce_bubble_input.settings = 0x31;
    bounce_bubble_bounds = nsmb::project::lua_actor_bounds(*bounce_bubble, bounce_bubble_input);
    assert(bounce_bubble_bounds && bounce_bubble_bounds.value()->x == 128);
    bounce_bubble_input.settings = 0x3f;
    bounce_bubble_bounds = nsmb::project::lua_actor_bounds(*bounce_bubble, bounce_bubble_input);
    assert(bounce_bubble_bounds && bounce_bubble_bounds.value()->x == 128);

    // Arrow signs alternate two base images and rotate each pair by 90°;
    // the unused high direction bit remains ignored.
    const auto* arrow_sign = shipped_catalog.value().by_object_id(125);
    std::shared_ptr<const nsmb::formats::RgbaImage> straight_arrow;
    std::shared_ptr<const nsmb::formats::RgbaImage> diagonal_arrow;
    for (std::uint32_t direction = 0; direction < 8; ++direction) {
        auto arrow_input = barrel_input;
        arrow_input.settings = direction << 24U;
        auto arrow_commands = nsmb::project::render_lua_actor(*arrow_sign, arrow_input);
        assert(arrow_commands && arrow_commands.value().size() == 1);
        const auto& command = arrow_commands.value()[0];
        assert(command.x == 160 && command.y == 320 && command.width == 32 && command.height == 32);
        assert(command.rotation == static_cast<double>((direction / 2U) * 90U));
        if (direction == 0) straight_arrow = command.image;
        if (direction == 1) diagonal_arrow = command.image;
        assert(command.image == (direction % 2U == 0 ? straight_arrow : diagonal_arrow));
    }
    auto arrow_high_bit_input = barrel_input;
    arrow_high_bit_input.settings = 0x08000000;
    auto arrow_high_bit_commands = nsmb::project::render_lua_actor(*arrow_sign, arrow_high_bit_input);
    assert(arrow_high_bit_commands && arrow_high_bit_commands.value()[0].image == straight_arrow);
    assert(arrow_high_bit_commands.value()[0].rotation == 0);

    // Bubble currents keep native image dimensions while transforms rotate
    // them into their visible selection boxes.
    const auto* down_current = shipped_catalog.value().by_object_id(51);
    auto down_bounds = nsmb::project::lua_actor_bounds(*down_current, barrel_input);
    auto down_commands = nsmb::project::render_lua_actor(*down_current, barrel_input);
    assert(down_bounds && down_bounds.value());
    assert(down_bounds.value()->x == 160 && down_bounds.value()->y == 352);
    assert(down_bounds.value()->width == 32 && down_bounds.value()->height == 74);
    assert(down_commands && down_commands.value().size() == 1);
    assert(down_commands.value()[0].x == 160 && down_commands.value()[0].y == 352);
    assert(down_commands.value()[0].width == 32 && down_commands.value()[0].height == 74);
    assert(down_commands.value()[0].rotation == 0 && down_commands.value()[0].flip_y);

    const auto* right_current = shipped_catalog.value().by_object_id(52);
    auto right_bounds = nsmb::project::lua_actor_bounds(*right_current, barrel_input);
    auto right_commands = nsmb::project::render_lua_actor(*right_current, barrel_input);
    assert(right_bounds && right_bounds.value());
    assert(right_bounds.value()->x == 192 && right_bounds.value()->y == 320);
    assert(right_bounds.value()->width == 74 && right_bounds.value()->height == 32);
    assert(right_commands && right_commands.value().size() == 1);
    assert(right_commands.value()[0].x == 213 && right_commands.value()[0].y == 299);
    assert(right_commands.value()[0].width == 32 && right_commands.value()[0].height == 74);
    assert(right_commands.value()[0].rotation == 90);

    const auto* left_current = shipped_catalog.value().by_object_id(53);
    auto left_bounds = nsmb::project::lua_actor_bounds(*left_current, barrel_input);
    auto left_commands = nsmb::project::render_lua_actor(*left_current, barrel_input);
    assert(left_bounds && left_bounds.value());
    assert(left_bounds.value()->x == 70 && left_bounds.value()->y == 320);
    assert(left_bounds.value()->width == 74 && left_bounds.value()->height == 32);
    assert(left_commands && left_commands.value().size() == 1);
    assert(left_commands.value()[0].x == 91 && left_commands.value()[0].y == 299);
    assert(left_commands.value()[0].width == 32 && left_commands.value()[0].height == 74);
    assert(left_commands.value()[0].rotation == 270);

    // A resource the project does not supply costs the actor its drawing, not
    // its metadata, and never the whole catalog.
    auto missing_resources = nsmb::project::load_lua_actor_layers(layers, [](auto, std::string_view) {
        return nsmb::core::Result<fs::path>(nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "missing"));
    }, decode_image);
    assert(missing_resources && missing_resources.value().actors.size() == 2);
    const auto* without = missing_resources.value().by_object_id(83);
    assert(without && without->sprite.name == "Goomba");
    auto missing_commands = nsmb::project::render_lua_actor(*without, input);
    assert(missing_commands && missing_commands.value().size() == 2);

    // init() failing leaves the metadata and drops the renderer.
    {
        fs::create_directories(root / "broken");
        std::ofstream file(root / "broken" / "broken.lua");
        file << R"(local function init(ctx) error("no") end
return {objectId=5, name="Broken", init=init, render=function() return true end})";
    }
    const std::vector<nsmb::project::LuaActorLayer> broken{{"broken", root / "broken"}};
    auto broken_catalog = nsmb::project::load_lua_actor_layers(broken, resolve, decode_image);
    assert(broken_catalog && broken_catalog.value().actors.size() == 1);
    const auto* broken_actor = broken_catalog.value().by_object_id(5);
    assert(broken_actor && broken_actor->sprite.name == "Broken");
    assert(!broken_actor->init_error.empty() && !nsmb::project::has_lua_renderer(*broken_actor));

    // A mistyped field name is an error, not a silent nil.
    {
        fs::create_directories(root / "typo");
        std::ofstream file(root / "typo" / "typo.lua");
        file << R"(return {objectId=6, name="Typo", render=function(actor) return actor.fields.nope end})";
    }
    const std::vector<nsmb::project::LuaActorLayer> typo{{"typo", root / "typo"}};
    auto typo_catalog = nsmb::project::load_lua_actor_layers(typo, resolve, decode_image);
    assert(typo_catalog);
    assert(!nsmb::project::render_lua_actor(*typo_catalog.value().by_object_id(6), input));

    // Stills may not climb out of their own actor directory.
    {
        fs::create_directories(root / "escape");
        std::ofstream file(root / "escape" / "escape.lua");
        file << R"(return {objectId=2, name="Escape", init=function(ctx) ctx.loadImage("../sign.png") end,
render=function() return true end})";
    }
    const std::vector<nsmb::project::LuaActorLayer> escape{{"escape", root / "escape"}};
    auto escaped = nsmb::project::load_lua_actor_layers(escape, resolve, decode_image);
    assert(escaped && !escaped.value().actors.empty());
    assert(!escaped.value().by_object_id(2)->init_error.empty());

    // Resource loading is unavailable once init() has returned.
    {
        fs::create_directories(root / "late");
        std::ofstream file(root / "late" / "late.lua");
        file << R"(local saved
local function init(ctx) saved = ctx end
return {objectId=7, name="Late", init=init, render=function() saved.loadImage("sign.png") return true end})";
    }
    const std::vector<nsmb::project::LuaActorLayer> late{{"late", root / "late"}};
    auto late_catalog = nsmb::project::load_lua_actor_layers(late, resolve, decode_image);
    assert(late_catalog);
    assert(!nsmb::project::render_lua_actor(*late_catalog.value().by_object_id(7), input));

    // A module actor overriding a vanilla class, ported from the NSMBe Signboard
    // plugin: the same settings must pick the same still and the same box.
    const std::vector<nsmb::project::LuaActorLayer> signboard{
        {"module signboard", fs::path(BSE_TEST_SOURCE_DIR) / "libs" / "nsmb-project" / "tests" / "fixtures" /
                                 "signboard-module" / "actors"},
        {"shipped", fs::path(BSE_TEST_SOURCE_DIR) / "data" / "actors"}};
    auto signboard_catalog = nsmb::project::load_lua_actor_layers(signboard, resolve, decode_image);
    if (!signboard_catalog) std::cerr << signboard_catalog.error().message << '\n';
    assert(signboard_catalog);
    const auto* sign = signboard_catalog.value().by_object_id(77);
    assert(sign && sign->sprite.name == "Arrow signboard" && sign->init_error.empty());
    assert(nsmb::project::has_lua_renderer(*sign));

    nsmb::project::LuaActorRenderInput sign_input;
    sign_input.object_id = 77;
    sign_input.pixel_x = 160;
    sign_input.pixel_y = 320;
    // Model type 2, no pipe mode, full size, unshifted: x -= 8, y -= 16.
    sign_input.settings = 2U << 24U;
    auto sign_bounds = nsmb::project::lua_actor_bounds(*sign, sign_input);
    assert(sign_bounds && sign_bounds.value());
    assert(sign_bounds.value()->x == 152 && sign_bounds.value()->y == 304);
    assert(sign_bounds.value()->width == 32 && sign_bounds.value()->height == 32);
    auto sign_commands = nsmb::project::render_lua_actor(*sign, sign_input);
    assert(sign_commands && sign_commands.value().size() == 1);
    assert(sign_commands.value().front().x == 152 && sign_commands.value().front().width == 32);

    // Smaller, shifted left and down: the half-block shift halves with the size.
    sign_input.settings = (1U << 16U) | (13U << 12U) | (14U << 8U);
    sign_bounds = nsmb::project::lua_actor_bounds(*sign, sign_input);
    assert(sign_bounds && sign_bounds.value());
    assert(sign_bounds.value()->x == 156 && sign_bounds.value()->y == 328);
    assert(sign_bounds.value()->width == 16 && sign_bounds.value()->height == 16);

    // Model type 10 with pipe mode off is the joint rather than an arrow.
    sign_input.settings = 10U << 24U;
    assert(nsmb::project::render_lua_actor(*sign, sign_input));

    // Every shipped program has to survive a spread of settings, not just zero.
    std::size_t with_renderer = 0;
    for (const auto& definition : shipped_catalog.value().actors) {
        if (!definition.init_error.empty()) {
            std::cerr << definition.source.filename().string() << ": " << definition.init_error << "\n";
            return 1;
        }
        if (nsmb::project::has_lua_renderer(definition)) ++with_renderer;
        for (const auto settings : {0x00000000U, 0xFFFFFFFFU, 0x12345678U, 0x88888888U, 0x01010101U}) {
            nsmb::project::LuaActorRenderInput probe;
            probe.x = 10;
            probe.y = 20;
            probe.pixel_x = 160.0;
            probe.pixel_y = 320.0;
            probe.object_id = definition.sprite.object_id;
            probe.settings = settings;
            probe.data = {static_cast<std::uint8_t>(settings), static_cast<std::uint8_t>(settings >> 8U),
                          static_cast<std::uint8_t>(settings >> 24U), static_cast<std::uint8_t>(settings >> 16U),
                          static_cast<std::uint8_t>(settings >> 8U), static_cast<std::uint8_t>(settings)};
            for (const bool render_box : {false, true}) {
                auto box = nsmb::project::lua_actor_bounds(definition, probe, render_box);
                if (!box) {
                    std::cerr << definition.source.filename().string() << " bounds: " << box.error().message << "\n";
                    return 1;
                }
            }
            auto drawn = nsmb::project::render_lua_actor(definition, probe);
            if (!drawn) {
                std::cerr << definition.source.filename().string() << " render: " << drawn.error().message << "\n";
                return 1;
            }
        }
    }
    assert(with_renderer > 190);


    fs::remove_all(root);
}
