#pragma once

#include <array>
#include <cstdint>
#include <span>
#include <string_view>
#include <vector>

#include "nsmb/formats/graphics.hpp"

namespace nsmb::formats {

struct TilesetResourceData {
    std::vector<std::uint8_t> graphics;
    std::vector<std::uint8_t> palette;
    std::vector<std::uint8_t> map16;
    std::vector<std::uint8_t> objects;
    std::vector<std::uint8_t> object_index;
    std::vector<std::uint8_t> behaviors;
};

struct TilemapResourceData {
    std::vector<std::uint8_t> graphics;
    std::vector<std::uint8_t> palette;
    std::vector<std::uint8_t> screen;
};

struct EditableTileset {
    std::uint16_t tile_offset{};
    std::uint8_t palette_offset{};
    std::uint8_t tile_page_offset{};
    std::vector<IndexedTile8Bpp> graphics;
    std::vector<Bgr555Color> palette;
    std::vector<Map16Tile> map16;
    std::vector<ObjectLayout> objects;
    std::vector<std::uint32_t> behaviors;

    // Writer state used to keep untouched resources byte-identical, including
    // their original LZ stream and any object-index metadata bytes.
    TilesetResourceData original;
    std::vector<ObjectLayout> original_objects;
};

struct EditableTilemap {
    std::size_t width_tiles{};
    std::vector<IndexedTile8Bpp> graphics;
    std::vector<Bgr555Color> palette;
    std::vector<ScreenMapEntry> screen;
    TilemapResourceData original;
};

enum class TileBehaviorParameterKind {
    Generic, Coin, Question, Explodable, Brick, Slope, PipeDoor,
    Climbable, PartialSolid, Harmful, Donut, Conveyor, Conflict,
};

struct TileBehaviorParts {
    std::uint32_t flags{};
    std::uint8_t subtype{};
    std::uint8_t parameter{};
};

inline constexpr std::array<std::string_view, 20> tile_behavior_flag_names{
    "Solid", "Coin", "Question block", "Explodable block", "Brick block",
    "Slope", "Flipped slope", "Unknown 0x80", "Door or pipe", "Water",
    "Climbable", "Partial solid", "Harmful", "Invisible block", "Push player down",
    "Solid on top", "Unknown 0x10000", "Donut lift", "Unknown 0x40000", "Unknown 0x80000",
};

inline constexpr std::array<std::string_view, 16> tile_behavior_subtype_names{
    "Normal", "Ice", "Snow", "Quicksand", "Conveyor right", "Conveyor left",
    "Horizontal rope", "Sideways spikes / collapsible stairs", "Unknown 8", "Ledge",
    "Vertical pole", "No sliding on slopes", "Unknown 12", "Slow movement", "Sandy", "Unknown 15",
};

inline constexpr std::array<std::string_view, 8> tile_behavior_pipe_main_names{
    "Door / breakable green pipe", "Unbreakable green pipe", "Breakable red pipe",
    "Unbreakable red pipe", "Breakable yellow pipe", "Unbreakable yellow pipe",
    "Unknown pipe type 6", "Unknown pipe type 7",
};

core::Result<EditableTileset> parse_tileset(const TilesetResourceData& resources,
                                             std::uint16_t tile_offset = 0,
                                             std::uint8_t palette_offset = 0,
                                             std::uint8_t tile_page_offset = 0);
core::Result<TilesetResourceData> write_tileset(const EditableTileset& tileset);
core::Result<std::vector<std::uint8_t>> write_tileset_bundle(const TilesetResourceData& resources);
core::Result<TilesetResourceData> parse_tileset_bundle(std::span<const std::uint8_t> bytes);
core::Result<EditableTilemap> parse_tilemap(const TilemapResourceData& resources, std::size_t width_tiles = 0);
core::Result<TilemapResourceData> write_tilemap(const EditableTilemap& tilemap);
core::Result<RgbaImage> render_tilemap(const EditableTilemap& tilemap);
core::Result<void> import_tilemap_image(EditableTilemap& tilemap, const RgbaImage& image);
core::Result<RgbaImage> render_tileset_graphics(const EditableTileset& tileset, std::size_t width_tiles = 32);
core::Result<RgbaImage> render_map16_sheet(const EditableTileset& tileset, std::size_t width_tiles = 16);

// The character and palette memory a level actually has, with every tileset laid
// out at its own offsets in one space -- what `loadJyotyuAndSubNohara` and
// `loadCharacterGraphics` build in VRAM. A Map16 entry naming a tile below its
// own tileset's offsets is drawable against this, because the tileset that
// supplies that tile is present too. See `docs/tileset-memory.md`.
struct ComposedTilesetMemory {
    std::vector<IndexedTile8Bpp> graphics;  // indexed by absolute tile number
    std::vector<Bgr555Color> palette;       // indexed by absolute colour index
};

ComposedTilesetMemory compose_tileset_memory(std::span<const EditableTileset> tilesets);

// The same sheet as `render_map16_sheet`, drawn against the composed space, so
// entries this tileset cannot resolve alone come out as the tiles the game shows.
core::Result<RgbaImage> render_composed_map16_sheet(const EditableTileset& tileset,
                                                    const ComposedTilesetMemory& memory,
                                                    std::size_t width_tiles = 16);
core::Result<void> import_tileset_graphics(EditableTileset& tileset, const RgbaImage& image);

TileBehaviorParts decode_tile_behavior(std::uint32_t value);
std::uint32_t encode_tile_behavior(TileBehaviorParts value);
TileBehaviorParameterKind tile_behavior_parameter_kind(TileBehaviorParts value);
std::string_view tile_behavior_parameter_name(TileBehaviorParameterKind kind, std::uint8_t value);

} // namespace nsmb::formats
