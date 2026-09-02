#include "nsmb/project/lua_tiles.hpp"

#include "nsmb/project/graphics_resources.hpp"

#include <cassert>
#include <filesystem>
#include <fstream>
#include <iostream>

namespace {

namespace fs = std::filesystem;

void write(const fs::path& path, std::string_view text) {
    fs::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary);
    out << text;
}

// No script in these tests resolves a texture: building a valid TEX0 by hand
// would test the container parser rather than the loader, and the loader's job
// is to report the failure per block and keep going. The end-to-end decode is
// checked against a real ROM, which no test may assume is present.
nsmb::core::Result<nsmb::project::ActorResource> no_resources(nsmb::project::ActorResourceKind,
                                                              std::string_view name) {
    return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "missing " + std::string(name));
}

nsmb::formats::RgbaImage solid(std::size_t width, std::size_t height, std::uint8_t value) {
    nsmb::formats::RgbaImage image{width, height, {}};
    image.pixels.assign(width * height * 4U, value);
    return image;
}

} // namespace

int main() {
    using namespace nsmb::project;
    const auto root = fs::temp_directory_path() / "big-star-lua-tile-tests";
    fs::remove_all(root);

    // A script is data. Every shape it can take is here.
    write(root / "base" / "pipes.lua", R"LUA(
return {
    name = "test.pipes",
    blocks = {
        [171] = { "polygon_unit/I_dokan.nsbtx", "I_do_yoko_lu" },
        [172] = { "polygon_unit/I_dokan.nsbtx", "I_do_yoko_u", palette = 256 },
        [800] = {},
        [785] = { remap = 781 },
    },
}
)LUA");
    write(root / "base" / "tower.lua", R"LUA(
return {
    name = "test.tower",
    replaces = false,
    tilesets = { 1, 0x2f },
    blocks = { [1456] = { "polygon_unit/I_toride_1.nsbtx", "I_toride_1" } },
}
)LUA");

    const std::vector<LuaTileLayer> layers{{"base", root / "base"}};
    auto catalog = load_lua_tile_layers(layers, no_resources);
    assert(catalog);
    assert(catalog.value().scripts.size() == 2);

    // Alphabetical within a layer, the way the actor loader orders scripts.
    assert(catalog.value().scripts[0].name == "test.pipes");
    assert(catalog.value().scripts[1].name == "test.tower");

    // Every entry shape survived, and the two that named a texture recorded the
    // failure against their own block rather than losing the script.
    const auto& pipes = catalog.value().scripts[0];
    assert(pipes.blocks.size() == 4);
    assert(pipes.errors.size() == 2);
    assert(pipes.errors[0].find("test.pipes block 171") != std::string::npos);

    const auto* replaced = catalog.value().find(0, 171);
    assert(replaced && replaced->replaces && !replaced->image && !replaced->remap);
    const auto* blanked = catalog.value().find(0, 800);
    assert(blanked && blanked->replaces && !blanked->image && !blanked->remap);
    const auto* remapped = catalog.value().find(0, 785);
    assert(remapped && remapped->remap && *remapped->remap == 781);
    assert(!catalog.value().find(0, 999));

    // A tilesets list is a filter, not decoration.
    assert(catalog.value().find(1, 1456) && !catalog.value().find(1, 1456)->replaces);
    assert(catalog.value().find(0x2f, 1456));
    assert(!catalog.value().find(2, 1456));
    assert(catalog.value().block_count(0) == 4 && catalog.value().block_count(1) == 5);

    // A later layer overrides one block and inherits the rest.
    write(root / "module" / "override.lua", R"LUA(
return {
    name = "module.pipe",
    blocks = { [171] = { remap = 42 } },
}
)LUA");
    const std::vector<LuaTileLayer> layered{{"module", root / "module"}, {"base", root / "base"}};
    auto stacked = load_lua_tile_layers(layered, no_resources);
    assert(stacked);
    const auto* overridden = stacked.value().find(0, 171);
    assert(overridden && overridden->remap && *overridden->remap == 42);
    assert(stacked.value().find(0, 172));

    // Bad scripts name themselves and take the load down, because a tile script
    // that cannot be read is a mistake in the file rather than a missing asset.
    for (const auto* bad : {R"LUA(return { name = "x", blocks = { [2000] = {} } })LUA",
                            R"LUA(return { name = "x" })LUA",
                            R"LUA(return { name = "x", tilesets = {}, blocks = {} })LUA",
                            R"LUA(return { name = "x", blocks = { [1] = 5 } })LUA",
                            R"LUA(return 7)LUA"}) {
        fs::remove_all(root / "bad");
        write(root / "bad" / "bad.lua", bad);
        const std::vector<LuaTileLayer> broken{{"bad", root / "bad"}};
        auto failed = load_lua_tile_layers(broken, no_resources);
        assert(!failed);
        assert(failed.error().message.find("bad.lua") != std::string::npos);
    }

    // A missing layer is not an error: most projects ship no tile scripts.
    const std::vector<LuaTileLayer> absent{{"absent", root / "nothing-here"}};
    auto empty = load_lua_tile_layers(absent, no_resources);
    assert(empty && empty.value().empty());

    // The sandbox is the actors' sandbox.
    write(root / "escape" / "escape.lua", R"LUA(return { name = "x", blocks = {}, danger = require("os") })LUA");
    const std::vector<LuaTileLayer> escape{{"escape", root / "escape"}};
    assert(!load_lua_tile_layers(escape, no_resources));

    // What the catalog does to a Map16 sheet. Built by hand rather than loaded,
    // so the four outcomes are checked without a TEX0 anywhere.
    TileOverlayCatalog applied;
    TileOverlayScript script;
    script.name = "test.apply";
    const auto texture = std::make_shared<const nsmb::formats::RgbaImage>(solid(16, 16, 0x40));
    script.blocks[112] = TileOverlay{texture, std::nullopt, true};
    script.blocks[113] = TileOverlay{nullptr, std::nullopt, true};
    script.blocks[114] = TileOverlay{texture, std::nullopt, false};
    script.blocks[115] = TileOverlay{nullptr, std::uint16_t{116}, true};
    applied.scripts.push_back(std::move(script));
    assert(applied.find(0, 112) && applied.block_count(0) == 4);

    std::cerr << "lua tiles: ok\n";
    fs::remove_all(root);
    return 0;
}
