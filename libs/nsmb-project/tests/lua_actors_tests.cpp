#include "nsmb/project/lua_actors.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>

int main() {
    namespace fs = std::filesystem;
    const auto root = fs::temp_directory_path() / "big-star-lua-actor-tests";
    fs::remove_all(root);
    fs::create_directories(root / "first");
    fs::create_directories(root / "last");
    {
        std::ofstream file(root / "first" / "goomba.lua");
        file << R"(return {schema=1, objectId=83, name="Goomba", category="Enemies / Ground", shape="ellipse", color="#b5651d", grabBounds={x=-8,y=-8,width=16,height=16}, renderBounds={x=-8,y=-8,width=16,height=16}, fields={{key="size",label="Size",bitOffset=0,bitWidth=2,choices={{value=0,label="Small"},{value=1,label="Big"}}}}, graphics=readGraphics("goomba.bin"), palette=readPalette("enemy.bin")})";
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
    const auto* custom = catalog.value().by_hash(0x12345678);
    assert(custom && custom->model.filename() == "actor.nsbmd");
    assert(catalog.value().sprite_definitions().by_hash(0x12345678));

    const std::vector<nsmb::project::LuaActorLayer> shipped{{"shipped", fs::path(BSE_TEST_SOURCE_DIR) / "data" / "actors"}};
    auto shipped_catalog = nsmb::project::load_lua_actor_layers(shipped, [&](auto, std::string_view name) {
        return nsmb::core::Result<fs::path>(root / "rom" / name);
    });
    assert(shipped_catalog && shipped_catalog.value().actors.size() == 25);

    { std::ofstream file(root / "bad.lua"); file << R"(return {schema=1, objectId=1, name="Bad", graphics="escape.bin"})"; }
    const std::vector<nsmb::project::LuaActorLayer> bad{{"bad", root}};
    assert(!nsmb::project::load_lua_actor_layers(bad, [&](auto, std::string_view name) {
        return nsmb::core::Result<fs::path>(root / name);
    }));
    fs::remove_all(root);
}
