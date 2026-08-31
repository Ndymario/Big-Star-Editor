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
