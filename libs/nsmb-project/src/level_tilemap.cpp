#include "nsmb/project/level_tilemap.hpp"

#include <algorithm>
#include <unordered_map>

namespace nsmb::project {
namespace {

std::uint64_t chunk_key(std::int32_t x, std::int32_t y) {
    return (static_cast<std::uint64_t>(static_cast<std::uint32_t>(x)) << 32U) |
           static_cast<std::uint32_t>(y);
}

// Division that rounds towards minus infinity, because cells left of the origin
// belong to the chunk left of it: -1 / 16 is chunk -1, cell 15.
std::int32_t floor_div(std::int32_t value, std::int32_t divisor) noexcept {
    const auto quotient = value / divisor;
    return value % divisor < 0 ? quotient - 1 : quotient;
}

// The largest object the editor will rasterise. A level's objects are smaller
// than this by orders of magnitude; the cap is here so that a width read out of
// a damaged file cannot ask for a billion cells.
constexpr std::size_t maximum_object_cells = 1U << 20U;

} // namespace

LevelObjectEffect level_object_effect(const formats::ObjectLayout& object, std::uint8_t tileset_slot) {
    if (tileset_slot >= level_tileset_block_bases.size()) return LevelObjectEffect::Draws;
    const auto base = level_tileset_block_bases[tileset_slot];
    bool any = false, all_empty = true, all_skipped = true;
    for (const auto& row : object.rows) {
        for (const auto& tile : row) {
            any = true;
            const auto block = tile.tile == formats::object_layout_empty_tile ? 0 : tile.tile + base;
            if (block != 0) all_empty = false;
            if (block < level_tilemap_first_skipped_block || block > level_tilemap_last_skipped_block) all_skipped = false;
        }
    }
    if (!any) return LevelObjectEffect::Draws;
    if (all_empty) return LevelObjectEffect::Erases;
    if (all_skipped) return LevelObjectEffect::Inert;
    return LevelObjectEffect::Draws;
}

LevelTilemap rasterize_level_objects(std::span<const formats::NsmbLevelObject> objects,
                                     const std::array<const DecodedTileset*, 3>& tilesets) {
    LevelTilemap tilemap;
    std::unordered_map<std::uint64_t, std::size_t> chunks;
    const auto cell_at = [&](std::int32_t x, std::int32_t y) -> LevelTilemapCell& {
        const auto chunk_x = floor_div(x, static_cast<std::int32_t>(level_chunk_cells));
        const auto chunk_y = floor_div(y, static_cast<std::int32_t>(level_chunk_cells));
        const auto key = chunk_key(chunk_x, chunk_y);
        auto found = chunks.find(key);
        if (found == chunks.end()) {
            found = chunks.emplace(key, tilemap.chunks.size()).first;
            tilemap.chunks.push_back(LevelTilemapChunk{chunk_x, chunk_y, {}});
        }
        auto& chunk = tilemap.chunks[found->second];
        const auto offset_x = static_cast<std::size_t>(x - chunk_x * static_cast<std::int32_t>(level_chunk_cells));
        const auto offset_y = static_cast<std::size_t>(y - chunk_y * static_cast<std::int32_t>(level_chunk_cells));
        return chunk.cells[offset_y * level_chunk_cells + offset_x];
    };

    for (const auto& object : objects) {
        if (object.tileset_slot >= tilesets.size()) continue;
        const auto* tileset = tilesets[object.tileset_slot];
        if (!tileset || object.object_id >= tileset->objects.size()) continue;
        const std::size_t width = std::max<std::uint16_t>(1, object.width);
        const std::size_t height = std::max<std::uint16_t>(1, object.height);
        if (width > maximum_object_cells / height) continue;
        auto layout = formats::render_object_layout(tileset->objects[object.object_id], width, height);
        if (!layout) continue;
        const auto base = level_tileset_block_bases[object.tileset_slot];
        for (std::size_t y = 0; y < height; ++y) {
            for (std::size_t x = 0; x < width; ++x) {
                const auto value = layout.value()[y * width + x];
                if (value == formats::object_layout_unwritten) continue;
                // The block the game would put in the cell, in the space the
                // three tilesets share -- which is what its skip list is in.
                const auto block = value == formats::object_layout_empty_tile ? 0 : value + base;
                if (block >= level_tilemap_first_skipped_block && block <= level_tilemap_last_skipped_block) continue;
                cell_at(static_cast<std::int32_t>(object.x) + static_cast<std::int32_t>(x),
                        static_cast<std::int32_t>(object.y) + static_cast<std::int32_t>(y)) =
                    LevelTilemapCell{value >= 0 ? value : -1, object.tileset_slot};
            }
        }
    }
    return tilemap;
}

} // namespace nsmb::project
