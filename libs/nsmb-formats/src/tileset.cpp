#include "nsmb/formats/tileset.hpp"

#include <algorithm>
#include <array>
#include <limits>

namespace nsmb::formats {
namespace {

struct Resource {
    std::vector<std::uint8_t> decoded;
    std::uint8_t compression{};
};

core::Result<Resource> unwrap(std::span<const std::uint8_t> bytes) {
    Resource result{{bytes.begin(), bytes.end()}, 0};
    if (!bytes.empty() && (bytes[0] == 0x10 || bytes[0] == 0x11)) {
        result.compression = bytes[0];
        auto decoded = decompress_nitro_lz(bytes);
        if (!decoded) return decoded.error();
        result.decoded = std::move(decoded.value());
    }
    return result;
}

core::Result<std::vector<std::uint8_t>> rewrap(std::span<const std::uint8_t> decoded,
                                                std::span<const std::uint8_t> original) {
    auto old = unwrap(original);
    if (!old) return old.error();
    if (std::ranges::equal(decoded, old.value().decoded)) return std::vector<std::uint8_t>{original.begin(), original.end()};
    if (old.value().compression == 0x10) return compress_nitro_lz10(decoded);
    if (old.value().compression == 0x11) return compress_nitro_lz11(decoded);
    return std::vector<std::uint8_t>{decoded.begin(), decoded.end()};
}

std::uint16_t u16(std::span<const std::uint8_t> bytes, std::size_t offset) {
    return static_cast<std::uint16_t>(bytes[offset] | (bytes[offset + 1U] << 8U));
}

std::uint32_t u32(std::span<const std::uint8_t> bytes, std::size_t offset) {
    return static_cast<std::uint32_t>(bytes[offset]) | (static_cast<std::uint32_t>(bytes[offset + 1U]) << 8U) |
           (static_cast<std::uint32_t>(bytes[offset + 2U]) << 16U) | (static_cast<std::uint32_t>(bytes[offset + 3U]) << 24U);
}

void put16(std::vector<std::uint8_t>& bytes, std::size_t offset, std::uint16_t value) {
    bytes[offset] = static_cast<std::uint8_t>(value); bytes[offset + 1U] = static_cast<std::uint8_t>(value >> 8U);
}

void put32(std::vector<std::uint8_t>& bytes, std::size_t offset, std::uint32_t value) {
    for (unsigned shift = 0; shift < 32U; shift += 8U) bytes[offset++] = static_cast<std::uint8_t>(value >> shift);
}

std::string_view named(std::uint8_t value, std::initializer_list<std::pair<std::uint8_t, std::string_view>> names) {
    const auto found = std::find_if(names.begin(), names.end(), [value](const auto& entry) { return entry.first == value; });
    return found == names.end() ? std::string_view{} : found->second;
}

core::Result<RgbaImage> render_tiles(std::span<const IndexedTile8Bpp> tiles,
                                     std::span<const Bgr555Color> palette,
                                     std::size_t width_tiles) {
    if (!width_tiles) return core::make_error(core::ErrorCode::InvalidArgument, "tile sheet width must be nonzero");
    std::vector<ScreenMapEntry> screen(tiles.size());
    for (std::size_t i = 0; i < screen.size(); ++i) { if (i > 0x3ffU) return core::make_error(core::ErrorCode::OutOfBounds, "tile sheet exceeds the DS tile index range"); screen[i].tile = static_cast<std::uint16_t>(i); }
    while (screen.size() % width_tiles) screen.push_back({});
    return render_screen_map(screen, width_tiles, tiles, palette);
}

core::Result<void> import_indexed(std::vector<IndexedTile8Bpp>& tiles,
                                  std::vector<Bgr555Color>& palette,
                                  const RgbaImage& image,
                                  std::vector<ScreenMapEntry>* screen,
                                  bool reserve_transparent) {
    if (!image.width || !image.height || image.width % 8U || image.height % 8U)
        return core::make_error(core::ErrorCode::InvalidArgument, "PNG dimensions must be nonzero multiples of eight");
    auto indexed = quantize_bgr555(image, reserve_transparent ? 255U : 256U); if (!indexed) return indexed.error();
    if (reserve_transparent && !indexed.value().palette.front().transparent) {
        indexed.value().palette.insert(indexed.value().palette.begin(), {0,0,0,true});
        for (auto& pixel : indexed.value().pixels) ++pixel;
    }
    const auto tiles_wide = image.width / 8U, tiles_high = image.height / 8U;
    if (tiles_wide * tiles_high > 1024U) return core::make_error(core::ErrorCode::OutOfBounds, "PNG needs more than 1024 DS tiles");
    tiles.clear(); if (screen) screen->clear(); palette = std::move(indexed.value().palette);
    for (std::size_t ty = 0; ty < tiles_high; ++ty) for (std::size_t tx = 0; tx < tiles_wide; ++tx) {
        IndexedTile8Bpp tile;
        for (std::size_t y = 0; y < 8U; ++y) for (std::size_t x = 0; x < 8U; ++x)
            tile.pixels[y * 8U + x] = indexed.value().pixels[(ty * 8U + y) * image.width + tx * 8U + x];
        if (screen) {
            const auto found = std::find(tiles.begin(), tiles.end(), tile);
            const auto index = found == tiles.end() ? tiles.size() : static_cast<std::size_t>(found - tiles.begin());
            if (found == tiles.end()) tiles.push_back(tile);
            screen->push_back({static_cast<std::uint16_t>(index), 0, false, false});
        } else tiles.push_back(tile);
    }
    return {};
}

} // namespace

core::Result<EditableTileset> parse_tileset(const TilesetResourceData& resources,
                                             std::uint16_t tile_offset,
                                             std::uint8_t palette_offset,
                                             std::uint8_t tile_page_offset) {
    auto graphics = unwrap(resources.graphics), palette = unwrap(resources.palette), map16 = unwrap(resources.map16);
    auto objects = unwrap(resources.objects), index = unwrap(resources.object_index), behaviors = unwrap(resources.behaviors);
    if (!graphics) return graphics.error(); if (!palette) return palette.error(); if (!map16) return map16.error();
    if (!objects) return objects.error(); if (!index) return index.error(); if (!behaviors) return behaviors.error();
    auto decoded_graphics = decode_8bpp_tiles(graphics.value().decoded);
    auto decoded_palette = decode_bgr555_palette(palette.value().decoded);
    auto decoded_map16 = decode_map16(map16.value().decoded, tile_offset, palette_offset);
    auto decoded_objects = decode_object_layouts(objects.value().decoded, index.value().decoded, tile_page_offset);
    if (!decoded_graphics) return decoded_graphics.error(); if (!decoded_palette) return decoded_palette.error();
    if (!decoded_map16) return decoded_map16.error(); if (!decoded_objects) return decoded_objects.error();
    if (behaviors.value().decoded.size() % 4U) return core::make_error(core::ErrorCode::ParseError, "tile behaviors must use four-byte records");
    EditableTileset result;
    result.tile_offset = tile_offset; result.palette_offset = palette_offset; result.tile_page_offset = tile_page_offset;
    result.graphics = std::move(decoded_graphics.value()); result.palette = std::move(decoded_palette.value());
    result.map16 = std::move(decoded_map16.value()); result.objects = std::move(decoded_objects.value());
    for (std::size_t offset = 0; offset < behaviors.value().decoded.size(); offset += 4U) result.behaviors.push_back(u32(behaviors.value().decoded, offset));
    if (!result.behaviors.empty() && result.behaviors.size() != result.map16.size())
        return core::make_error(core::ErrorCode::ParseError, "tile behavior count does not match Map16 tile count");
    result.original = resources; result.original_objects = result.objects;
    return result;
}

core::Result<TilesetResourceData> write_tileset(const EditableTileset& tileset) {
    auto graphics = encode_8bpp_tiles(tileset.graphics), palette = encode_bgr555_palette(tileset.palette);
    auto map16 = encode_map16(tileset.map16, tileset.tile_offset, tileset.palette_offset);
    if (!graphics) return graphics.error(); if (!palette) return palette.error(); if (!map16) return map16.error();
    std::vector<std::uint8_t> behavior_bytes(tileset.behaviors.size() * 4U);
    if (!tileset.behaviors.empty() && tileset.behaviors.size() != tileset.map16.size())
        return core::make_error(core::ErrorCode::InvalidArgument, "tile behavior count does not match Map16 tile count");
    for (std::size_t i = 0; i < tileset.behaviors.size(); ++i) put32(behavior_bytes, i * 4U, tileset.behaviors[i]);
    std::vector<std::uint8_t> object_bytes, index_bytes;
    if (tileset.objects == tileset.original_objects) {
        auto objects = unwrap(tileset.original.objects), index = unwrap(tileset.original.object_index);
        if (!objects) return objects.error(); if (!index) return index.error();
        object_bytes = std::move(objects.value().decoded); index_bytes = std::move(index.value().decoded);
    } else {
        auto old_index = unwrap(tileset.original.object_index); if (!old_index) return old_index.error();
        const auto index_template = old_index.value().decoded.size() == tileset.objects.size() * 4U ? std::span<const std::uint8_t>(old_index.value().decoded) : std::span<const std::uint8_t>{};
        auto encoded = encode_object_layouts(tileset.objects, tileset.tile_page_offset, index_template);
        if (!encoded) return encoded.error(); object_bytes = std::move(encoded.value().definitions); index_bytes = std::move(encoded.value().index);
    }
    TilesetResourceData output;
    auto wrap = [&](std::span<const std::uint8_t> bytes, const std::vector<std::uint8_t>& original, std::vector<std::uint8_t>& target) -> core::Result<void> { auto value = rewrap(bytes, original); if (!value) return value.error(); target = std::move(value.value()); return {}; };
    if (auto value = wrap(graphics.value(), tileset.original.graphics, output.graphics); !value) return value.error();
    if (auto value = wrap(palette.value(), tileset.original.palette, output.palette); !value) return value.error();
    if (auto value = wrap(map16.value(), tileset.original.map16, output.map16); !value) return value.error();
    if (auto value = wrap(object_bytes, tileset.original.objects, output.objects); !value) return value.error();
    if (auto value = wrap(index_bytes, tileset.original.object_index, output.object_index); !value) return value.error();
    if (auto value = wrap(behavior_bytes, tileset.original.behaviors, output.behaviors); !value) return value.error();
    return output;
}

core::Result<std::vector<std::uint8_t>> write_tileset_bundle(const TilesetResourceData& resources) {
    const std::array<std::span<const std::uint8_t>, 6> parts{resources.graphics, resources.palette, resources.map16, resources.objects, resources.object_index, resources.behaviors};
    constexpr std::size_t header_size = 8U + parts.size() * 12U;
    std::size_t total = header_size; for (const auto part : parts) { if (part.size() > std::numeric_limits<std::uint32_t>::max() || total > std::numeric_limits<std::uint32_t>::max() - part.size()) return core::make_error(core::ErrorCode::OutOfBounds, "tileset bundle exceeds 32-bit offsets"); total += part.size(); }
    std::vector<std::uint8_t> output(total); std::copy_n("BSTS", 4, output.begin()); put16(output, 4, 1); put16(output, 6, static_cast<std::uint16_t>(parts.size()));
    std::size_t cursor = header_size;
    for (std::size_t i = 0; i < parts.size(); ++i) { const auto entry = 8U + i * 12U; output[entry] = static_cast<std::uint8_t>(i); put32(output, entry + 4U, static_cast<std::uint32_t>(cursor)); put32(output, entry + 8U, static_cast<std::uint32_t>(parts[i].size())); std::copy(parts[i].begin(), parts[i].end(), output.begin() + static_cast<std::ptrdiff_t>(cursor)); cursor += parts[i].size(); }
    return output;
}

core::Result<TilesetResourceData> parse_tileset_bundle(std::span<const std::uint8_t> bytes) {
    constexpr std::size_t count = 6, header_size = 8U + count * 12U;
    if (bytes.size() < header_size || !std::equal(bytes.begin(), bytes.begin() + 4, "BSTS") || u16(bytes, 4) != 1 || u16(bytes, 6) != count)
        return core::make_error(core::ErrorCode::ParseError, "tileset bundle header is invalid");
    std::array<std::vector<std::uint8_t>, count> parts;
    std::array<bool, count> seen{};
    std::vector<std::pair<std::size_t,std::size_t>> ranges;
    for (std::size_t i = 0; i < count; ++i) { const auto entry = 8U + i * 12U; const auto kind = bytes[entry]; if (kind >= count || seen[kind]) return core::make_error(core::ErrorCode::ParseError, "tileset bundle resource kind is invalid or duplicated"); seen[kind] = true; const auto offset = u32(bytes, entry + 4U); const auto size = u32(bytes, entry + 8U); if (offset < header_size || offset > bytes.size() || size > bytes.size() - offset) return core::make_error(core::ErrorCode::OutOfBounds, "tileset bundle resource is outside the file"); if(size)ranges.emplace_back(offset,offset+size);parts[kind].assign(bytes.begin() + offset, bytes.begin() + offset + size); }
    std::ranges::sort(ranges);for(std::size_t i=1;i<ranges.size();++i)if(ranges[i].first<ranges[i-1U].second)return core::make_error(core::ErrorCode::ParseError,"tileset bundle resources overlap");
    return TilesetResourceData{std::move(parts[0]), std::move(parts[1]), std::move(parts[2]), std::move(parts[3]), std::move(parts[4]), std::move(parts[5])};
}

core::Result<EditableTilemap> parse_tilemap(const TilemapResourceData& resources, std::size_t width_tiles) {
    auto graphics = unwrap(resources.graphics), palette = unwrap(resources.palette), screen = unwrap(resources.screen);
    if (!graphics) return graphics.error(); if (!palette) return palette.error(); if (!screen) return screen.error();
    auto decoded_graphics = decode_8bpp_tiles(graphics.value().decoded);
    auto decoded_palette = decode_bgr555_palette(palette.value().decoded, false);
    auto decoded_screen = decode_screen_map(screen.value().decoded);
    if (!decoded_graphics) return decoded_graphics.error(); if (!decoded_palette) return decoded_palette.error(); if (!decoded_screen) return decoded_screen.error();
    if (!width_tiles) width_tiles = decoded_screen.value().size() % 64U == 0 ? 64U : 32U;
    if (!width_tiles || decoded_screen.value().size() % width_tiles) return core::make_error(core::ErrorCode::InvalidArgument, "tilemap width does not divide its screen entries");
    return EditableTilemap{width_tiles, std::move(decoded_graphics.value()), std::move(decoded_palette.value()), std::move(decoded_screen.value()), resources};
}

core::Result<TilemapResourceData> write_tilemap(const EditableTilemap& tilemap) {
    if (!tilemap.width_tiles || tilemap.screen.size() % tilemap.width_tiles) return core::make_error(core::ErrorCode::InvalidArgument, "tilemap width does not divide its screen entries");
    auto graphics = encode_8bpp_tiles(tilemap.graphics), palette = encode_bgr555_palette(tilemap.palette), screen = encode_screen_map(tilemap.screen);
    if (!graphics) return graphics.error(); if (!palette) return palette.error(); if (!screen) return screen.error();
    TilemapResourceData output;
    auto wrap = [&](std::span<const std::uint8_t> bytes, const std::vector<std::uint8_t>& original, std::vector<std::uint8_t>& target) -> core::Result<void> { auto value = rewrap(bytes, original); if (!value) return value.error(); target = std::move(value.value()); return {}; };
    if (auto value = wrap(graphics.value(), tilemap.original.graphics, output.graphics); !value) return value.error();
    if (auto value = wrap(palette.value(), tilemap.original.palette, output.palette); !value) return value.error();
    if (auto value = wrap(screen.value(), tilemap.original.screen, output.screen); !value) return value.error();
    return output;
}

core::Result<RgbaImage> render_tilemap(const EditableTilemap& tilemap) {
    return render_screen_map(tilemap.screen, tilemap.width_tiles, tilemap.graphics, tilemap.palette);
}

core::Result<void> import_tilemap_image(EditableTilemap& tilemap, const RgbaImage& image) {
    auto result = import_indexed(tilemap.graphics, tilemap.palette, image, &tilemap.screen, false);
    if (result) tilemap.width_tiles = image.width / 8U;
    return result;
}

core::Result<RgbaImage> render_tileset_graphics(const EditableTileset& tileset, std::size_t width_tiles) {
    width_tiles = std::min(width_tiles, std::max<std::size_t>(1, tileset.graphics.size()));
    return render_tiles(tileset.graphics, tileset.palette, width_tiles);
}

core::Result<RgbaImage> render_map16_sheet(const EditableTileset& tileset, std::size_t width_tiles) {
    if (!width_tiles) return core::make_error(core::ErrorCode::InvalidArgument, "Map16 sheet width must be nonzero");
    const auto map16_rows = (tileset.map16.size() + width_tiles - 1U) / width_tiles;
    std::vector<ScreenMapEntry> arranged(map16_rows * width_tiles * 4U);
    for (std::size_t i = 0; i < tileset.map16.size(); ++i) for (std::size_t q = 0; q < 4U; ++q) {
        const auto x = (i % width_tiles) * 2U + q % 2U, y = (i / width_tiles) * 2U + q / 2U;
        arranged[y * width_tiles * 2U + x] = tileset.map16[i].tiles[q];
    }
    return render_screen_map(arranged, width_tiles * 2U, tileset.graphics, tileset.palette);
}

core::Result<void> import_tileset_graphics(EditableTileset& tileset, const RgbaImage& image) {
    return import_indexed(tileset.graphics, tileset.palette, image, nullptr, true);
}

TileBehaviorParts decode_tile_behavior(std::uint32_t value) {
    return {((value >> 16U) & 0xffffU) | ((value & 0xf00U) << 8U), static_cast<std::uint8_t>((value >> 12U) & 15U), static_cast<std::uint8_t>(value)};
}

std::uint32_t encode_tile_behavior(TileBehaviorParts value) {
    return ((value.flags & 0xffffU) << 16U) | ((value.flags & 0xf0000U) >> 8U) |
           (static_cast<std::uint32_t>(value.subtype & 15U) << 12U) | value.parameter;
}

TileBehaviorParameterKind tile_behavior_parameter_kind(TileBehaviorParts value) {
    std::array<TileBehaviorParameterKind, 11> kinds{}; std::size_t count = 0;
    auto add = [&](std::uint32_t flag, TileBehaviorParameterKind kind) { if (value.flags & flag) kinds[count++] = kind; };
    add(2, TileBehaviorParameterKind::Coin); add(4, TileBehaviorParameterKind::Question); add(8, TileBehaviorParameterKind::Explodable);
    add(0x10, TileBehaviorParameterKind::Brick); add(0x20 | 0x40, TileBehaviorParameterKind::Slope); add(0x100, TileBehaviorParameterKind::PipeDoor);
    add(0x400, TileBehaviorParameterKind::Climbable); add(0x800, TileBehaviorParameterKind::PartialSolid); add(0x1000, TileBehaviorParameterKind::Harmful);
    add(0x20000, TileBehaviorParameterKind::Donut); if (value.subtype == 4 || value.subtype == 5) kinds[count++] = TileBehaviorParameterKind::Conveyor;
    if (!count) return TileBehaviorParameterKind::Generic;
    return count == 1 ? kinds[0] : TileBehaviorParameterKind::Conflict;
}

std::string_view tile_behavior_parameter_name(TileBehaviorParameterKind kind, std::uint8_t value) {
    switch (kind) {
    case TileBehaviorParameterKind::Coin: return named(value, {{0,"Normal"},{2,"Unknown 2"},{4,"Unknown 4"}});
    case TileBehaviorParameterKind::Question: return named(value, {{0,"Mushroom or flower"},{1,"Starman"},{2,"Continuous Starman"},{3,"1-Up Mushroom"},{4,"Springboard"},{5,"Mini Mushroom"},{6,"Five floating coins"},{7,"Coin"},{8,"Vine"},{9,"Mushroom (parameter 9)"},{10,"Mushroom (parameter 10)"},{11,"Unknown 11"},{12,"Boo"},{13,"Mega Mushroom"},{14,"Unknown 14"},{15,"Unknown 15"},{0x10,"Thrown power-up"},{0x11,"Thrown coin"},{0x30,"Invisible power-up"},{0x31,"Invisible Starman"},{0x32,"Invisible 1-Up"},{0x33,"Invisible coin"},{0x34,"Invisible vine"},{0x35,"Invisible mushroom"},{0x36,"Invisible unknown 0x36"},{0x37,"Invisible unknown 0x37"}});
    case TileBehaviorParameterKind::Brick: return named(value, {{0,"Empty"},{1,"Mushroom or flower"},{2,"Starman"},{3,"1-Up Mushroom"},{4,"Coin"},{5,"Multiple coins"},{6,"Vine"},{7,"Empty (parameter 7)"},{8,"Empty (parameter 8)"},{0x34,"Flower"},{0x35,"Blue shell"},{0x45,"Two coins"},{0x95,"Mini Mushroom"},{0x97,"Mega Mushroom"}});
    case TileBehaviorParameterKind::Explodable: return named(value, {{0,"Used block shards"},{1,"Stone block shards"},{2,"Wooden block shards"},{3,"Red block shards"}});
    case TileBehaviorParameterKind::Harmful: return named(value, {{0,"Spikes left"},{1,"Spikes right"},{2,"Spikes up"},{3,"Spikes down"},{4,"Lava"},{5,"Instant death"},{7,"Harmful on all sides"}});
    case TileBehaviorParameterKind::Conveyor: return named(value, {{0,"Slow"},{1,"Fast"}});
    case TileBehaviorParameterKind::Donut: return named(value, {{0,"Donut lift"},{1,"Beach platform left"},{2,"Beach platform right"}});
    case TileBehaviorParameterKind::Climbable: return named(value, {{0,"Vine"},{2,"Fence top-left"},{3,"Fence top"},{4,"Fence top-right"},{5,"Fence left"},{6,"Fence centre"},{7,"Fence right"},{8,"Fence bottom-left"},{9,"Fence bottom"},{10,"Fence bottom-right"},{11,"Flip fence top-left"},{12,"Flip fence top"},{13,"Flip fence top-right"},{14,"Flip fence left"},{15,"Flip fence centre"},{16,"Flip fence right"},{17,"Flip fence bottom-left"},{18,"Flip fence bottom"},{19,"Flip fence bottom-right"}});
    case TileBehaviorParameterKind::Slope: return named(value, {{0,"1x1 up-right"},{1,"1x1 down-right"},{2,"2x1 up-right left"},{3,"2x1 up-right right"},{4,"2x1 down-right left"},{5,"2x1 down-right right"},{6,"1x2 up-right bottom"},{7,"1x2 up-right top"},{8,"1x2 down-right bottom"},{9,"1x2 down-right top"},{10,"Slope edge"},{11,"4x1 up-right part 1"},{12,"4x1 up-right part 2"},{13,"4x1 up-right part 3"},{14,"4x1 up-right part 4"},{15,"4x1 down-right part 1"},{16,"4x1 down-right part 2"},{17,"4x1 down-right part 3"},{18,"4x1 down-right part 4"}});
    case TileBehaviorParameterKind::PipeDoor: { static constexpr std::array<std::string_view, 32> names{"Up left / door bottom-left","Up right / door bottom-right","Down left / mini door","Down right / boss door left","Vertical left / boss door centre","Vertical right / boss door right","Vertical-over-horizontal left","Vertical-over-horizontal right","Left-facing top","Left-facing bottom","Right-facing top","Right-facing bottom","Horizontal top","Horizontal bottom","Horizontal-over-vertical top","Horizontal-over-vertical bottom","Mini pipe top","Cracked vertical top-left","Mini pipe bottom","Cracked vertical top-right","Unknown 20","Cracked vertical bottom-left","Mini vertical-over-horizontal","Cracked vertical bottom-right","Mini pipe left","Cracked mini pipe bottom","Mini pipe right","Cracked mini pipe top","Mini vertical pipe","Mini horizontal pipe","Mini horizontal-over-vertical","Pipe joint"}; return names[value & 31U]; }
    case TileBehaviorParameterKind::PartialSolid: return "Quadrant mask";
    case TileBehaviorParameterKind::Generic: return named(value, {{0,"Normal"},{1,"Path bottom-left to top-right"},{2,"Path top-left to bottom-right"},{3,"Path corner top-left"},{4,"Path corner bottom-right"},{5,"Horizontal path"},{6,"Vertical path"},{8,"2x1 up-right left"},{9,"2x1 up-right right"},{0xa,"2x1 down-right left"},{0xb,"2x1 down-right right"},{0xc,"1x2 up-right top"},{0xd,"1x2 up-right bottom"},{0xe,"1x2 down-right bottom"},{0xf,"1x2 down-right top"},{0x10,"1x1 circle path"},{0x11,"2x2 circle top-left"},{0x12,"2x2 circle top-right"},{0x13,"2x2 circle bottom-left"},{0x14,"2x2 circle bottom-right"},{0x15,"4x4 circle top-left"},{0x16,"4x4 circle top-middle left"},{0x17,"4x4 circle top-middle right"},{0x18,"4x4 circle top-right"},{0x19,"4x4 circle left-middle top"},{0x1a,"4x4 circle right-middle top"},{0x1b,"4x4 circle left-middle bottom"},{0x1c,"4x4 circle right-middle bottom"},{0x1d,"4x4 circle bottom-left"},{0x1e,"4x4 circle bottom-middle left"},{0x1f,"4x4 circle bottom-middle right"},{0x20,"4x4 circle bottom-right"},{0x22,"Path stopper"},{0x28,"Coin outline 1"},{0x29,"Coin outline 2"}});
    case TileBehaviorParameterKind::Conflict: return "Conflicting parameter flags";
    }
    return {};
}

} // namespace nsmb::formats
