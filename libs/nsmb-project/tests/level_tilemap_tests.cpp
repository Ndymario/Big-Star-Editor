#include "nsmb/project/level_tilemap.hpp"

#include <iostream>

namespace {
int failures{};
void check(bool ok, const char* text) { if (!ok) { std::cerr << "FAIL: " << text << '\n'; ++failures; } }

// One object, `width` by `height` cells, whose every cell holds `tile`.
nsmb::formats::ObjectLayout flat(std::int32_t tile) {
    nsmb::formats::ObjectLayout layout;
    layout.rows.push_back({{tile, 0}});
    return layout;
}

const nsmb::project::LevelTilemapCell& cell_of(const nsmb::project::LevelTilemap& tilemap,
                                               std::int32_t x,
                                               std::int32_t y) {
    static const nsmb::project::LevelTilemapCell nothing;
    const auto cells = static_cast<std::int32_t>(nsmb::project::level_chunk_cells);
    const auto floor_div = [cells](std::int32_t value) {
        const auto quotient = value / cells;
        return value % cells < 0 ? quotient - 1 : quotient;
    };
    for (const auto& chunk : tilemap.chunks) {
        if (chunk.x != floor_div(x) || chunk.y != floor_div(y)) continue;
        return chunk.cell(static_cast<std::size_t>(x - chunk.x * cells),
                          static_cast<std::size_t>(y - chunk.y * cells));
    }
    return nothing;
}
} // namespace

int main() {
    using nsmb::formats::NsmbLevelObject;
    nsmb::project::DecodedTileset jyotyu;
    jyotyu.objects.push_back(flat(40));                                        // 0: a plain block
    jyotyu.objects.push_back(flat(nsmb::formats::object_layout_empty_tile));    // 1: the empty block
    jyotyu.objects.push_back(flat(3));                                         // 2: a block the game skips
    nsmb::formats::ObjectLayout slope;
    slope.slope = true;
    slope.rows.push_back({{50, 0}});
    jyotyu.objects.push_back(slope);                                           // 3: a slope
    jyotyu.objects.push_back(flat(41));                                        // 4: another plain block
    nsmb::project::DecodedTileset level_tileset;
    level_tileset.objects.push_back(flat(3));                                  // block 259 once based
    const std::array<const nsmb::project::DecodedTileset*, 3> tilesets{&jyotyu, &level_tileset, nullptr};

    {
        // The object in front wins the cells it covers outright. Both are 2x2
        // and overlap on one cell.
        const std::vector<NsmbLevelObject> objects{{0, 0, 0, 0, 2, 2}, {4, 0, 1, 1, 2, 2}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(cell_of(tilemap, 0, 0).block == 40, "the object behind keeps the cells nothing covers");
        check(cell_of(tilemap, 1, 1).block == 41 && cell_of(tilemap, 1, 1).slot == 0,
              "the object in front owns the cell it shares");
        check(cell_of(tilemap, 2, 2).block == 41, "the object in front writes its own cells");
    }
    {
        // An empty block is a block. Object 1 is the eraser every tileset has,
        // and laying it over something leaves the cell holding nothing at all
        // rather than letting what is behind show through.
        const std::vector<NsmbLevelObject> objects{{0, 0, 0, 0, 2, 2}, {1, 0, 1, 1, 2, 2}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(cell_of(tilemap, 0, 0).draws(), "the cell the eraser misses still draws");
        check(cell_of(tilemap, 1, 1).written() && !cell_of(tilemap, 1, 1).draws(),
              "the eraser writes the cell and draws nothing in it");
    }
    {
        // Blocks 1-6 never reach the array, so a cell naming one keeps what an
        // earlier object put there.
        const std::vector<NsmbLevelObject> objects{{0, 0, 0, 0, 2, 2}, {2, 0, 1, 1, 2, 2}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(cell_of(tilemap, 1, 1).block == 40, "a skipped block leaves the block behind it standing");
        check(!cell_of(tilemap, 2, 2).written(), "a skipped block writes nothing of its own");
    }
    {
        // The same block number in the level tileset's slot is 259, which the
        // skip list does not reach.
        const std::vector<NsmbLevelObject> objects{{0, 0, 0, 0, 2, 2}, {0, 1, 1, 1, 2, 2}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(cell_of(tilemap, 1, 1).block == 3 && cell_of(tilemap, 1, 1).slot == 1,
              "the skip list is read in the space the tilesets share");
    }
    {
        // A slope fills one diagonal; the cells it never reaches are not its to
        // clear.
        const std::vector<NsmbLevelObject> objects{{0, 0, 0, 0, 3, 3}, {3, 0, 0, 0, 3, 3}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(cell_of(tilemap, 0, 2).block == 50, "the slope draws its own step");
        check(cell_of(tilemap, 2, 2).block == 40, "the empty half of a slope leaves what is under it");
    }
    {
        // Cells left of the origin belong to the chunk left of it, and an
        // object far out asks for its own cells rather than every cell between.
        const std::vector<NsmbLevelObject> objects{{0, 0, -2, -2, 2, 2}, {0, 0, 300, 300, 1, 1}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(cell_of(tilemap, -1, -1).block == 40, "a cell outside the origin is rasterised");
        check(tilemap.chunks.size() == 2, "only the chunks the objects touch are allocated");
    }
    {
        // What an object does to a cell, for the two kinds that draw nothing.
        using nsmb::project::LevelObjectEffect;
        check(nsmb::project::level_object_effect(jyotyu.objects[0], 0) == LevelObjectEffect::Draws,
              "an object that names a block draws");
        check(nsmb::project::level_object_effect(jyotyu.objects[1], 0) == LevelObjectEffect::Erases,
              "an object of empty blocks is the eraser");
        check(nsmb::project::level_object_effect(jyotyu.objects[2], 0) == LevelObjectEffect::Inert,
              "an object of blocks 1-6 does nothing");
        check(nsmb::project::level_object_effect(level_tileset.objects[0], 1) == LevelObjectEffect::Draws,
              "the same block number in another slot draws");
        nsmb::formats::ObjectLayout mixed;
        mixed.rows.push_back({{nsmb::formats::object_layout_empty_tile, 0}, {40, 0}});
        check(nsmb::project::level_object_effect(mixed, 0) == LevelObjectEffect::Draws,
              "an object that is empty in places still draws");
    }
    {
        // Nothing to draw with is not a crash.
        const std::vector<NsmbLevelObject> objects{{0, 2, 0, 0, 2, 2}, {99, 0, 4, 4, 2, 2}};
        const auto tilemap = nsmb::project::rasterize_level_objects(objects, tilesets);
        check(tilemap.chunks.empty(), "an object with no tileset or no definition writes nothing");
    }
    return failures ? 1 : 0;
}
