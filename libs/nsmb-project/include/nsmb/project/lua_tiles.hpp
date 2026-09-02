#pragma once

#include <cstdint>
#include <filesystem>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <unordered_map>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/formats/graphics.hpp"
#include "nsmb/project/lua_actors.hpp"

namespace nsmb::project {

// A Map16 block the game does not draw from the tileset.
//
// `StageLayout::renderTileRow` passes every block through `handle3DTile` before
// writing the tilemap, and for a good many of them what reaches the screen is a
// textured quad on the 3D engine instead: the pipes, fences, pillars, quicksand,
// tower and water surfaces. Their Map16 bytes are dead, which is why drawing
// them faithfully produces black squares nobody ever sees in game.
//
// See `docs/tileset-memory.md` for how the game does it and `docs/lua-tiles.md`
// for the scripts that describe it.
struct TileOverlay {
    // What the game draws in the block's place. Null when it draws nothing,
    // which is a real answer: a few blocks are blanked with no quad behind them.
    std::shared_ptr<const formats::RgbaImage> image;
    // Set when the game rewrites the block to another block rather than to a
    // texture, as it does for the thin fences.
    std::optional<std::uint16_t> remap;
    // False when the block's own Map16 still draws and the texture goes over
    // it, which is what the tower and water blocks do.
    bool replaces{true};
};

struct TileOverlayScript {
    std::string name;
    std::filesystem::path source;
    // Which tileset ids this script speaks for. Empty means every tileset.
    std::vector<std::uint16_t> tilesets;
    std::unordered_map<std::uint16_t, TileOverlay> blocks;
    // Files on disk behind the textures, for the watcher.
    std::vector<std::filesystem::path> resources;
    // Blocks whose texture would not resolve, named so the log can say which.
    // The script keeps the blocks that did resolve.
    std::vector<std::string> errors;
};

// Scripts in layer order; the first one that speaks for a tileset and claims a
// block wins, so a module overrides one block without restating the file.
struct TileOverlayCatalog {
    std::vector<TileOverlayScript> scripts;

    [[nodiscard]] const TileOverlay* find(std::uint16_t tileset_id, std::uint16_t block) const;
    [[nodiscard]] bool empty() const noexcept;
    // Distinct blocks claimed for this tileset, which is what a log line wants.
    [[nodiscard]] std::size_t block_count(std::uint16_t tileset_id) const;
};

struct LuaTileLayer {
    std::string name;
    std::filesystem::path directory;
};

// Tile scripts are data: they name a block, an NSBTX and a texture, and this
// resolves the texture through the same resolver the actor loader uses, so a
// module's replacement NSBTX is picked up the same way an actor's is.
core::Result<TileOverlayCatalog> load_lua_tile_layers(std::span<const LuaTileLayer> layers,
                                                      const ActorResourceResolver& resolve);

// `page_offset` is the tileset's first absolute block: 0 for jyotyu, 256 for
// the level tileset, 1024 for nohara.
//
// One block per image, which is what the level canvas draws from.
void overlay_map16_images(std::vector<formats::RgbaImage>& blocks,
                          std::uint16_t tileset_id,
                          std::uint16_t page_offset,
                          const TileOverlayCatalog& overlays);
// The same substitution on one sheet of `blocks_wide` blocks per row, which is
// what the tileset editor draws from.
void overlay_map16_sheet(formats::RgbaImage& sheet,
                         std::size_t blocks_wide,
                         std::size_t block_count,
                         std::uint16_t tileset_id,
                         std::uint16_t page_offset,
                         const TileOverlayCatalog& overlays);

} // namespace nsmb::project
