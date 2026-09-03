#pragma once

#include <array>
#include <cstdint>
#include <span>
#include <vector>

#include "nsmb/formats/level.hpp"
#include "nsmb/project/graphics_resources.hpp"

namespace nsmb::project {

// A level is not a stack of objects. `InitBGDat @ 020b3924` walks the object
// list once and rasterises every object into one array of Map16 blocks -- one
// block per 16x16 cell -- and that array is all the hardware ever sees. So two
// objects cannot both show through a cell: the one written later wins outright,
// transparent pixels and all, and the block behind it is gone rather than
// showing through. See `docs/tileset-memory.md`.
//
// The array is allocated in chunks of 16x16 cells (`processX_allocChunk @
// 020b597c`), and so is this, which keeps an object dropped far outside the
// level from asking for the rectangle between it and the origin.
inline constexpr std::size_t level_chunk_cells = 16;

struct LevelTilemapCell {
    // The slot no object writes: what an untouched cell holds.
    static constexpr std::uint8_t no_slot = 0xff;

    // Index into that slot's Map16, or -1 for the empty block -- written like
    // any other block, and drawing nothing.
    std::int32_t block{-1};
    std::uint8_t slot{no_slot};

    [[nodiscard]] bool written() const noexcept { return slot != no_slot; }
    [[nodiscard]] bool draws() const noexcept { return written() && block >= 0; }
};

struct LevelTilemapChunk {
    std::int32_t x{}, y{};  // In chunks, so cell (x * 16, y * 16) is the corner.
    std::array<LevelTilemapCell, level_chunk_cells * level_chunk_cells> cells;

    [[nodiscard]] const LevelTilemapCell& cell(std::size_t x, std::size_t y) const noexcept {
        return cells[y * level_chunk_cells + x];
    }
};

struct LevelTilemap {
    std::vector<LevelTilemapChunk> chunks;
};

// Blocks 1 through 6 are never written. `processX_InsertIntoChunk @ 020b54f4`
// drops them before it reaches the array, so an object that names one leaves
// the cell -- and whatever an earlier object put in it -- alone. That is how an
// object gets a hole in it that something behind can show through.
inline constexpr std::int32_t level_tilemap_first_skipped_block = 1;
inline constexpr std::int32_t level_tilemap_last_skipped_block = 6;

// Where each tileset slot's Map16 starts in the one address space the three
// share, in blocks: jyotyu at 0, the level tileset at 256, nohara at 1024.
inline constexpr std::array<std::int32_t, 3> level_tileset_block_bases{0, 256, 1024};

// What placing an object does to the cells it covers. Two of these draw
// nothing, so a picker that previews them draws seven identical blanks unless it
// says which is which.
enum class LevelObjectEffect {
    Draws,   // The blocks it names, the ordinary case.
    Erases,  // Every cell is the empty block: it clears what it is laid over.
    Inert,   // Every cell names a block 1-6, so it does nothing whatsoever.
};

LevelObjectEffect level_object_effect(const formats::ObjectLayout& object, std::uint8_t tileset_slot);

// The tile array the game would build from these objects, in the order they are
// listed -- which is the order the editor draws and reorders them in.
LevelTilemap rasterize_level_objects(std::span<const formats::NsmbLevelObject> objects,
                                     const std::array<const DecodedTileset*, 3>& tilesets);

} // namespace nsmb::project
