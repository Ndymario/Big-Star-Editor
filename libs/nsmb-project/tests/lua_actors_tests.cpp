#include "nsmb/project/lua_actors.hpp"

#include <algorithm>
#include <array>
#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>

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
        std::ofstream file(root / "first" / "goomba.lua");
        file << R"(local bitmap=loadBitmap("goomba.bin",{bpp=4,width=16,height=8})
local palette=loadPalette("enemy.bin",{colors=16})
local function bounds(actor) return {x=-8,y=-8,width=16+(actor.data[5]&1)*8,height=16} end
local function render(actor,renderer)
 renderer:drawTile(bitmap,palette,actor.data[5]&1,{x=actor.pixelX+2,y=actor.pixelY-3,width=16,height=12,rotation=90,opacity=0.5,flipX=true})
 renderer:drawFrame(bitmap,palette,0,8,8,{x=actor.pixelX-8,y=actor.pixelY})
end
return {schema=1, objectId=83, name="Goomba", category="Enemies / Ground", notes="Walks.", known=true, complete=true, sourceFiles={"/obj/goomba.bin"}, shape="ellipse", color="#b5651d", grabBounds={x=-8,y=-8,width=16,height=16}, renderBounds={x=-8,y=-8,width=16,height=16}, fields={{key="size",label="Size",notes="A field note.",bitOffset=40,bitWidth=8,valueOffset=1,signed=true,choices={{value=0,label="Small"},{value=1,label="Big"}}}}, graphics=readGraphics("goomba.bin"), palette=readPalette("enemy.bin"), getGrabBounds=bounds, getRenderBounds=bounds, onRender=render})";
    }
    {
        std::ofstream file(root / "first" / "custom.lua");
        file << R"(return {schema=1, hash=0x12345678, name="Module Actor", model=readModel("actor.nsbmd")})";
    }
    {
        std::ofstream file(root / "last" / "duplicate.lua");
        file << R"(return {schema=1, objectId=83, name="Loses"})";
    }
    const std::vector<nsmb::project::LuaActorLayer> layers{{"first", root / "first"}, {"last", root / "last"}};
    auto catalog = nsmb::project::load_lua_actor_layers(layers, [&](auto, std::string_view name) {
        return nsmb::core::Result<fs::path>(root / "rom" / name);
    });
    assert(catalog);
    assert(catalog.value().actors.size() == 2);
    const auto* goomba = catalog.value().by_object_id(83);
    assert(goomba && goomba->sprite.name == "Goomba");
    assert(goomba->graphics.filename() == "goomba.bin" && goomba->palette.filename() == "enemy.bin");
    assert(goomba->sprite.fields.size() == 1 && goomba->sprite.fields.front().choices.size() == 2);
    assert(goomba->sprite.notes == "Walks." && goomba->sprite.known && goomba->sprite.complete);
    assert(goomba->sprite.source_files.size() == 1 && goomba->sprite.fields.front().notes == "A field note.");
    assert(goomba->sprite.fields.front().bit_offset == 40 && goomba->sprite.fields.front().signed_value);
    assert(nsmb::project::has_lua_renderer(*goomba));
    assert(goomba->render_resources.size() == 2);
    nsmb::project::LuaActorRenderInput input;
    input.x = 10;
    input.y = 20;
    input.pixel_x = 160;
    input.pixel_y = 320;
    input.object_id = 83;
    input.type = 7;
    input.settings = 1;
    input.data[5] = 1;
    auto commands = nsmb::project::render_lua_actor(*goomba, input);
    if (!commands) std::cerr << commands.error().message << '\n';
    else if (commands.value().size() != 2) std::cerr << "expected 2 Lua draw commands, got " << commands.value().size() << '\n';
    assert(commands && commands.value().size() == 2);
    assert(commands.value()[0].source_x == 8 && commands.value()[0].source_y == 0);
    assert(commands.value()[0].x == 162 && commands.value()[0].y == 317);
    assert(commands.value()[0].width == 16 && commands.value()[0].height == 12);
    assert(commands.value()[0].rotation == 90 && commands.value()[0].opacity == 0.5 && commands.value()[0].flip_x);
    assert(commands.value()[0].image == commands.value()[1].image);
    auto bounds = nsmb::project::lua_actor_bounds(*goomba, input);
    assert(bounds && bounds.value() && bounds.value()->x == -8 && bounds.value()->width == 24);
    auto commands_again = nsmb::project::render_lua_actor(*goomba, input);
    assert(commands_again && commands_again.value().front().image == commands.value().front().image);
    const auto* custom = catalog.value().by_hash(0x12345678);
    assert(custom && custom->model.filename() == "actor.nsbmd");
    assert(catalog.value().sprite_definitions().by_hash(0x12345678));

    const std::vector<nsmb::project::LuaActorLayer> shipped{{"shipped", fs::path(BSE_TEST_SOURCE_DIR) / "data" / "actors"}};
    auto shipped_catalog = nsmb::project::load_lua_actor_layers(shipped, [&](auto, std::string_view name) {
        return nsmb::core::Result<fs::path>(root / "rom" / name);
    });
    if (!shipped_catalog) std::cerr << shipped_catalog.error().message << '\n';
    assert(shipped_catalog && shipped_catalog.value().actors.size() == 305);
    const auto* pokey = shipped_catalog.value().by_object_id(98);
    assert(pokey && nsmb::project::has_lua_renderer(*pokey));
    input.object_id = 98;
    input.data[5] = 7;
    auto pokey_bounds = nsmb::project::lua_actor_bounds(*pokey, input, true);
    assert(pokey_bounds && pokey_bounds.value() && pokey_bounds.value()->y == -137 && pokey_bounds.value()->height == 153);

    auto missing_resources = nsmb::project::load_lua_actor_layers(layers, [](auto, std::string_view) {
        return nsmb::core::Result<fs::path>(nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "missing"));
    });
    assert(missing_resources && missing_resources.value().actors.size() == 2);
    assert(missing_resources.value().by_object_id(83)->graphics.empty());
    auto missing_commands = nsmb::project::render_lua_actor(*missing_resources.value().by_object_id(83), input);
    assert(missing_commands && missing_commands.value().empty());

    { std::ofstream file(root / "escape.lua"); file << R"(return {schema=1, objectId=2, name="Escape", model=readModel("../escape.nsbmd")})"; }
    const std::vector<nsmb::project::LuaActorLayer> escape{{"escape", root}};
    assert(!nsmb::project::load_lua_actor_layers(escape, [](auto, std::string_view) {
        return nsmb::core::Result<fs::path>(nsmb::core::make_error(nsmb::core::ErrorCode::InvalidArgument, "file names only"));
    }));

    { std::ofstream file(root / "bad.lua"); file << R"(return {schema=1, objectId=1, name="Bad", graphics="escape.bin"})"; }
    const std::vector<nsmb::project::LuaActorLayer> bad{{"bad", root}};
    assert(!nsmb::project::load_lua_actor_layers(bad, [&](auto, std::string_view name) {
        return nsmb::core::Result<fs::path>(root / name);
    }));
    fs::remove_all(root);
}
