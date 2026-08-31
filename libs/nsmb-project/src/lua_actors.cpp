#include "nsmb/project/lua_actors.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <limits>
#include <memory>
#include <unordered_map>
#include <unordered_set>

extern "C" {
#include <lua.h>
#include <lauxlib.h>
}

#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/g2d.hpp"
#include "nsmb/formats/tex0.hpp"

namespace nsmb::project {

struct LuaMemory {
    std::size_t used{};
};

struct LuaBitmap {
    std::size_t width{}, height{};
    std::vector<std::uint8_t> pixels;
};

struct LuaPalette {
    std::vector<formats::Bgr555Color> colors;
};

enum class LuaResourceType { LegacyGraphics, LegacyPalette, LegacyModel, Bitmap, Palette, Texture };

struct ResourceRef {
    LuaResourceType type{};
    std::filesystem::path path;
    std::shared_ptr<const LuaBitmap> bitmap;
    std::shared_ptr<const LuaPalette> palette;
    std::shared_ptr<const formats::RgbaImage> image;
};

struct LuaAssetCache {
    std::unordered_map<std::string, std::weak_ptr<const LuaBitmap>> bitmaps;
    std::unordered_map<std::string, std::weak_ptr<const LuaPalette>> palettes;
    std::unordered_map<std::string, std::weak_ptr<const formats::RgbaImage>> textures;
};

struct LuaContext {
    const ActorResourceResolver* resolve{};
    LuaAssetCache* cache{};
    std::vector<ResourceRef> resources;
    std::unordered_map<std::uint64_t, std::shared_ptr<const formats::RgbaImage>> rasterized;
    std::vector<LuaActorDrawCommand>* commands{};
    const LuaActorRenderInput* input{};
    std::size_t instructions{};
};

struct LuaActorProgram {
    LuaMemory memory;
    LuaContext context;
    lua_State* state{};
    int on_render{LUA_NOREF};
    int grab_bounds{LUA_NOREF};
    int render_bounds{LUA_NOREF};
    ~LuaActorProgram() { if (state) lua_close(state); }
};

namespace {

constexpr std::size_t max_script_bytes = 1024U * 1024U;
constexpr std::size_t max_lua_bytes = 8U * 1024U * 1024U;
constexpr std::size_t max_instructions = 200000U;
constexpr std::size_t max_draw_commands = 256U;

void* allocate(void* opaque, void* pointer, std::size_t old_size, std::size_t new_size) {
    auto& memory = *static_cast<LuaMemory*>(opaque);
    if (!pointer) old_size = 0;
    if (!new_size) {
        std::free(pointer);
        memory.used -= std::min(memory.used, old_size);
        return nullptr;
    }
    if (new_size > old_size && new_size - old_size > max_lua_bytes - std::min(memory.used, max_lua_bytes)) return nullptr;
    auto* replacement = std::realloc(pointer, new_size);
    if (!replacement) return nullptr;
    memory.used = memory.used - std::min(memory.used, old_size) + new_size;
    return replacement;
}

LuaContext& context(lua_State* state) {
    return **static_cast<LuaContext**>(lua_getextraspace(state));
}

void instruction_hook(lua_State* state, lua_Debug*) {
    auto& value = context(state);
    value.instructions += 1000U;
    if (value.instructions > max_instructions) luaL_error(state, "actor script exceeded its instruction limit");
}

int read_resource(lua_State* state) {
    auto& value = context(state);
    const auto kind = static_cast<ActorResourceKind>(lua_tointeger(state, lua_upvalueindex(1)));
    if (!value.resolve) return luaL_error(state, "actor resources must be loaded before onRender");
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "resource filename must be a string");
    const auto* name = luaL_checkstring(state, 1);
    auto resolved = (*value.resolve)(kind, name);
    if (!resolved && resolved.error().code != core::ErrorCode::NotFound)
        return luaL_error(state, "%s", resolved.error().message.c_str());
    auto* resource = static_cast<std::size_t*>(lua_newuserdatauv(state, sizeof(std::size_t), 0));
    *resource = value.resources.size();
    const auto type = kind == ActorResourceKind::Graphics ? LuaResourceType::LegacyGraphics
                    : kind == ActorResourceKind::Palette ? LuaResourceType::LegacyPalette
                                                         : LuaResourceType::LegacyModel;
    value.resources.push_back({type, resolved ? std::move(resolved.value()) : std::filesystem::path{}});
    return 1;
}

void expose_reader(lua_State* state, const char* name, ActorResourceKind kind) {
    lua_pushinteger(state, static_cast<lua_Integer>(kind));
    lua_pushcclosure(state, read_resource, 1);
    lua_setglobal(state, name);
}

lua_Integer option_integer(lua_State* state,
                           int table,
                           const char* key,
                           lua_Integer fallback,
                           lua_Integer minimum,
                           lua_Integer maximum) {
    table = lua_absindex(state, table);
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    if (!lua_isinteger(state, -1)) luaL_error(state, "%s must be an integer", key);
    const auto value = lua_tointeger(state, -1);
    lua_pop(state, 1);
    if (value < minimum || value > maximum) luaL_error(state, "%s is outside its supported range", key);
    return value;
}

double option_number(lua_State* state, int table, const char* key, double fallback, double minimum, double maximum) {
    table = lua_absindex(state, table);
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    if (!lua_isnumber(state, -1)) luaL_error(state, "%s must be a number", key);
    const auto value = static_cast<double>(lua_tonumber(state, -1));
    lua_pop(state, 1);
    if (!std::isfinite(value) || value < minimum || value > maximum) luaL_error(state, "%s is outside its supported range", key);
    return value;
}

bool option_boolean(lua_State* state, int table, const char* key, bool fallback) {
    table = lua_absindex(state, table);
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    if (!lua_isboolean(state, -1)) luaL_error(state, "%s must be a boolean", key);
    const bool value = lua_toboolean(state, -1);
    lua_pop(state, 1);
    return value;
}

core::Result<std::vector<std::uint8_t>> actor_resource_bytes(const std::filesystem::path& path) {
    auto bytes = core::read_file(path);
    if (!bytes) return bytes.error();
    if (!bytes.value().empty() && (bytes.value().front() == 0x10 || bytes.value().front() == 0x11))
        return formats::decompress_nitro_lz(bytes.value());
    return bytes;
}

std::optional<std::filesystem::path> resolve_resource(lua_State* state, ActorResourceKind kind, const char* name) {
    auto& value = context(state);
    if (!value.resolve) luaL_error(state, "actor resources must be loaded before onRender");
    auto resolved = (*value.resolve)(kind, name);
    if (resolved) return std::move(resolved.value());
    if (resolved.error().code == core::ErrorCode::NotFound) return std::nullopt;
    luaL_error(state, "%s", resolved.error().message.c_str());
    return std::nullopt;
}

template <typename T>
std::shared_ptr<const T> cached(std::unordered_map<std::string, std::weak_ptr<const T>>& cache,
                                const std::string& key) {
    const auto found = cache.find(key);
    return found == cache.end() ? nullptr : found->second.lock();
}

core::Result<std::shared_ptr<const LuaBitmap>> decode_bitmap(const std::filesystem::path& path,
                                                             int bpp,
                                                             std::size_t width,
                                                             std::size_t height,
                                                             std::size_t tile_offset) {
    auto bytes = actor_resource_bytes(path);
    if (!bytes) return bytes.error();
    std::span<const std::uint8_t> characters(bytes.value());
    std::optional<formats::Ncgr> ncgr;
    if (formats::identify_graphics_format(bytes.value(), path.string()) == formats::GraphicsFormatKind::NitroCharacterGraphics) {
        auto parsed = formats::parse_ncgr(bytes.value());
        if (!parsed) return parsed.error();
        ncgr = std::move(parsed.value());
        characters = ncgr->characters;
        if (!width && ncgr->width != 0xffffU) width = static_cast<std::size_t>(ncgr->width) * 8U;
        if (!height && ncgr->height != 0xffffU) height = static_cast<std::size_t>(ncgr->height) * 8U;
    }
    if (!width || width % 8U || width > 4096U)
        return core::make_error(core::ErrorCode::InvalidArgument, "loadBitmap needs a width divisible by 8");
    std::vector<std::array<std::uint8_t, 64>> tiles;
    if (bpp == 4) {
        auto decoded = formats::decode_4bpp_tiles(characters);
        if (!decoded) return decoded.error();
        tiles.reserve(decoded.value().size());
        for (const auto& tile : decoded.value()) tiles.push_back(tile.pixels);
    } else {
        auto decoded = formats::decode_8bpp_tiles(characters);
        if (!decoded) return decoded.error();
        tiles.reserve(decoded.value().size());
        for (const auto& tile : decoded.value()) tiles.push_back(tile.pixels);
    }
    if (tile_offset > tiles.size()) return core::make_error(core::ErrorCode::OutOfBounds, "loadBitmap tileOffset is outside the resource");
    const auto tiles_wide = width / 8U;
    const auto available = tiles.size() - tile_offset;
    if (!height) height = ((available + tiles_wide - 1U) / tiles_wide) * 8U;
    if (!height || height % 8U || height > 4096U || width > (4096U * 4096U) / height)
        return core::make_error(core::ErrorCode::InvalidArgument, "loadBitmap height is invalid");
    auto result = std::make_shared<LuaBitmap>();
    result->width = width;
    result->height = height;
    result->pixels.assign(width * height, 0);
    const auto wanted = tiles_wide * (height / 8U);
    for (std::size_t tile = 0; tile < std::min(wanted, available); ++tile) {
        const auto tile_x = tile % tiles_wide, tile_y = tile / tiles_wide;
        for (std::size_t y = 0; y < 8U; ++y)
            std::copy_n(tiles[tile_offset + tile].begin() + static_cast<std::ptrdiff_t>(y * 8U), 8,
                        result->pixels.begin() + static_cast<std::ptrdiff_t>((tile_y * 8U + y) * width + tile_x * 8U));
    }
    return std::shared_ptr<const LuaBitmap>(std::move(result));
}

core::Result<std::shared_ptr<const LuaPalette>> decode_palette(const std::filesystem::path& path,
                                                               std::size_t bank,
                                                               std::size_t color_count,
                                                               bool transparent) {
    auto bytes = actor_resource_bytes(path);
    if (!bytes) return bytes.error();
    std::span<const std::uint8_t> raw(bytes.value());
    std::optional<formats::Nclr> nclr;
    if (formats::identify_graphics_format(bytes.value(), path.string()) == formats::GraphicsFormatKind::NitroPalette) {
        auto parsed = formats::parse_nclr(bytes.value());
        if (!parsed) return parsed.error();
        nclr = std::move(parsed.value());
        raw = nclr->palette;
    }
    auto colors = formats::decode_bgr555_palette(raw, false);
    if (!colors) return colors.error();
    if (!color_count) color_count = colors.value().size();
    const auto start = bank * color_count;
    if (!color_count || start > colors.value().size() || color_count > colors.value().size() - start)
        return core::make_error(core::ErrorCode::OutOfBounds, "loadPalette bank is outside the resource");
    auto result = std::make_shared<LuaPalette>();
    result->colors.assign(colors.value().begin() + static_cast<std::ptrdiff_t>(start),
                          colors.value().begin() + static_cast<std::ptrdiff_t>(start + color_count));
    if (transparent && !result->colors.empty()) result->colors.front().transparent = true;
    return std::shared_ptr<const LuaPalette>(std::move(result));
}

core::Result<std::shared_ptr<const formats::RgbaImage>> decode_texture(const std::filesystem::path& path,
                                                                       std::size_t texture,
                                                                       std::size_t palette) {
    auto bytes = actor_resource_bytes(path);
    if (!bytes) return bytes.error();
    auto tex0 = formats::parse_tex0(bytes.value());
    if (!tex0) return tex0.error();
    if (texture >= tex0.value().textures.size() || palette >= tex0.value().palettes.size())
        return core::make_error(core::ErrorCode::OutOfBounds, "loadTexture index is outside the resource");
    auto image = formats::render_tex0_texture(tex0.value().textures[texture], tex0.value().palettes[palette]);
    if (!image) return image.error();
    return std::make_shared<const formats::RgbaImage>(std::move(image.value()));
}

void push_resource(lua_State* state, ResourceRef resource) {
    auto& value = context(state);
    auto* handle = static_cast<std::size_t*>(lua_newuserdatauv(state, sizeof(std::size_t), 0));
    *handle = value.resources.size();
    value.resources.push_back(std::move(resource));
}

int load_bitmap(lua_State* state) {
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadBitmap filename must be a string");
    luaL_checktype(state, 2, LUA_TTABLE);
    const auto* name = lua_tostring(state, 1);
    const auto bpp = static_cast<int>(option_integer(state, 2, "bpp", 0, 4, 8));
    if (bpp != 4 && bpp != 8) return luaL_error(state, "loadBitmap bpp must be 4 or 8");
    const auto width = static_cast<std::size_t>(option_integer(state, 2, "width", 0, 0, 4096));
    const auto height = static_cast<std::size_t>(option_integer(state, 2, "height", 0, 0, 4096));
    const auto offset = static_cast<std::size_t>(option_integer(state, 2, "tileOffset", 0, 0, 1 << 24));
    auto path = resolve_resource(state, ActorResourceKind::Graphics, name);
    ResourceRef resource{LuaResourceType::Bitmap, path.value_or(std::filesystem::path{})};
    if (path) {
        const auto key = path->string() + ":" + std::to_string(bpp) + ":" + std::to_string(width) + ":" +
                         std::to_string(height) + ":" + std::to_string(offset);
        resource.bitmap = cached(context(state).cache->bitmaps, key);
        if (!resource.bitmap) {
            auto decoded = decode_bitmap(*path, bpp, width, height, offset);
            if (decoded) context(state).cache->bitmaps[key] = resource.bitmap = std::move(decoded.value());
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

int load_palette(lua_State* state) {
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadPalette filename must be a string");
    if (!lua_isnoneornil(state, 2)) luaL_checktype(state, 2, LUA_TTABLE);
    const auto table = lua_istable(state, 2) ? 2 : 0;
    const auto bank = table ? static_cast<std::size_t>(option_integer(state, table, "bank", 0, 0, 65535)) : 0U;
    const auto colors = table ? static_cast<std::size_t>(option_integer(state, table, "colors", 0, 0, 65536)) : 0U;
    const bool transparent = table ? option_boolean(state, table, "transparent", true) : true;
    const auto* name = lua_tostring(state, 1);
    auto path = resolve_resource(state, ActorResourceKind::Palette, name);
    ResourceRef resource{LuaResourceType::Palette, path.value_or(std::filesystem::path{})};
    if (path) {
        const auto key = path->string() + ":" + std::to_string(bank) + ":" + std::to_string(colors) + ":" +
                         (transparent ? "1" : "0");
        resource.palette = cached(context(state).cache->palettes, key);
        if (!resource.palette) {
            auto decoded = decode_palette(*path, bank, colors, transparent);
            if (decoded) context(state).cache->palettes[key] = resource.palette = std::move(decoded.value());
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

int load_texture(lua_State* state) {
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadTexture filename must be a string");
    if (!lua_isnoneornil(state, 2)) luaL_checktype(state, 2, LUA_TTABLE);
    const auto table = lua_istable(state, 2) ? 2 : 0;
    const auto texture = table ? static_cast<std::size_t>(option_integer(state, table, "texture", 0, 0, 65535)) : 0U;
    const auto palette = table ? static_cast<std::size_t>(option_integer(state, table, "palette", 0, 0, 65535)) : 0U;
    const auto* name = lua_tostring(state, 1);
    auto path = resolve_resource(state, ActorResourceKind::Model, name);
    ResourceRef resource{LuaResourceType::Texture, path.value_or(std::filesystem::path{})};
    if (path) {
        const auto key = path->string() + ":" + std::to_string(texture) + ":" + std::to_string(palette);
        resource.image = cached(context(state).cache->textures, key);
        if (!resource.image) {
            auto decoded = decode_texture(*path, texture, palette);
            if (decoded) context(state).cache->textures[key] = resource.image = std::move(decoded.value());
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

void expose_render_loaders(lua_State* state) {
    lua_pushcfunction(state, load_bitmap); lua_setglobal(state, "loadBitmap");
    lua_pushcfunction(state, load_palette); lua_setglobal(state, "loadPalette");
    lua_pushcfunction(state, load_texture); lua_setglobal(state, "loadTexture");
}

const ResourceRef* resource_at(lua_State* state, int index) {
    if (!lua_isuserdata(state, index)) luaL_error(state, "renderer resource must be a loadBitmap/loadPalette/loadTexture handle");
    const auto handle = *static_cast<const std::size_t*>(lua_touserdata(state, index));
    const auto& resources = context(state).resources;
    if (handle >= resources.size()) luaL_error(state, "renderer resource handle is invalid");
    return &resources[handle];
}

std::shared_ptr<const formats::RgbaImage> rasterize(lua_State* state,
                                                    std::size_t bitmap_index,
                                                    std::size_t palette_index) {
    auto& value = context(state);
    const auto key = (static_cast<std::uint64_t>(bitmap_index) << 32U) | palette_index;
    if (const auto found = value.rasterized.find(key); found != value.rasterized.end()) return found->second;
    if (bitmap_index >= value.resources.size() || palette_index >= value.resources.size()) return {};
    const auto& bitmap = value.resources[bitmap_index];
    const auto& palette = value.resources[palette_index];
    if (bitmap.type != LuaResourceType::Bitmap || palette.type != LuaResourceType::Palette || !bitmap.bitmap || !palette.palette) return {};
    auto image = std::make_shared<formats::RgbaImage>();
    image->width = bitmap.bitmap->width;
    image->height = bitmap.bitmap->height;
    image->pixels.assign(image->width * image->height * 4U, 0);
    for (std::size_t pixel = 0; pixel < bitmap.bitmap->pixels.size(); ++pixel) {
        const auto color_index = bitmap.bitmap->pixels[pixel];
        if (color_index >= palette.palette->colors.size()) continue;
        const auto& color = palette.palette->colors[color_index];
        const auto target = pixel * 4U;
        image->pixels[target] = color.red;
        image->pixels[target + 1U] = color.green;
        image->pixels[target + 2U] = color.blue;
        image->pixels[target + 3U] = color.transparent ? 0 : 255;
    }
    value.rasterized[key] = image;
    return image;
}

bool emit_draw(lua_State* state,
               std::shared_ptr<const formats::RgbaImage> image,
               int options,
               std::int32_t source_x,
               std::int32_t source_y,
               std::uint32_t source_width,
               std::uint32_t source_height) {
    auto& value = context(state);
    if (!value.commands || !value.input) luaL_error(state, "renderer methods are only available inside onRender");
    if (!image || !image->width || !image->height) return false;
    if (!lua_isnoneornil(state, options)) luaL_checktype(state, options, LUA_TTABLE);
    const auto table = lua_istable(state, options) ? options : 0;
    if (table) {
        source_x = static_cast<std::int32_t>(option_integer(state, table, "sourceX", source_x, 0, 1 << 20));
        source_y = static_cast<std::int32_t>(option_integer(state, table, "sourceY", source_y, 0, 1 << 20));
        source_width = static_cast<std::uint32_t>(option_integer(state, table, "sourceWidth", source_width, 1, 1 << 20));
        source_height = static_cast<std::uint32_t>(option_integer(state, table, "sourceHeight", source_height, 1, 1 << 20));
    }
    if (source_x < 0 || source_y < 0 || static_cast<std::size_t>(source_x) + source_width > image->width ||
        static_cast<std::size_t>(source_y) + source_height > image->height)
        luaL_error(state, "draw source rectangle is outside the resource");
    if (value.commands->size() >= max_draw_commands) luaL_error(state, "actor exceeded 256 draw commands");
    LuaActorDrawCommand command;
    command.image = std::move(image);
    command.source_x = source_x;
    command.source_y = source_y;
    command.source_width = source_width;
    command.source_height = source_height;
    command.x = table ? option_number(state, table, "x", value.input->pixel_x, -1048576.0, 1048576.0) : value.input->pixel_x;
    command.y = table ? option_number(state, table, "y", value.input->pixel_y, -1048576.0, 1048576.0) : value.input->pixel_y;
    command.width = table ? option_number(state, table, "width", source_width, 0.001, 1048576.0) : source_width;
    command.height = table ? option_number(state, table, "height", source_height, 0.001, 1048576.0) : source_height;
    command.rotation = table ? option_number(state, table, "rotation", 0.0, -36000.0, 36000.0) : 0.0;
    command.opacity = table ? option_number(state, table, "opacity", 1.0, 0.0, 1.0) : 1.0;
    command.flip_x = table && option_boolean(state, table, "flipX", false);
    command.flip_y = table && option_boolean(state, table, "flipY", false);
    value.commands->push_back(std::move(command));
    return true;
}

int renderer_draw(lua_State* state) {
    const auto* resource = resource_at(state, 2);
    std::shared_ptr<const formats::RgbaImage> image;
    int options = 3;
    if (resource->type == LuaResourceType::Texture) image = resource->image;
    else if (resource->type == LuaResourceType::Bitmap) {
        const auto* palette = resource_at(state, 3);
        if (palette->type != LuaResourceType::Palette) return luaL_error(state, "bitmap draw needs a palette handle");
        const auto bitmap_index = *static_cast<const std::size_t*>(lua_touserdata(state, 2));
        const auto palette_index = *static_cast<const std::size_t*>(lua_touserdata(state, 3));
        image = rasterize(state, bitmap_index, palette_index);
        options = 4;
    } else return luaL_error(state, "renderer:draw needs a bitmap or texture handle");
    const bool drawn = emit_draw(state, image, options, 0, 0,
                                 image ? static_cast<std::uint32_t>(image->width) : 1U,
                                 image ? static_cast<std::uint32_t>(image->height) : 1U);
    lua_pushboolean(state, drawn);
    return 1;
}

int renderer_draw_tile(lua_State* state) {
    const auto* bitmap = resource_at(state, 2);
    const auto* palette = resource_at(state, 3);
    if (bitmap->type != LuaResourceType::Bitmap || palette->type != LuaResourceType::Palette)
        return luaL_error(state, "renderer:drawTile needs bitmap and palette handles");
    const auto tile = luaL_checkinteger(state, 4);
    if (tile < 0) return luaL_error(state, "tile index cannot be negative");
    const auto bitmap_index = *static_cast<const std::size_t*>(lua_touserdata(state, 2));
    const auto palette_index = *static_cast<const std::size_t*>(lua_touserdata(state, 3));
    auto image = rasterize(state, bitmap_index, palette_index);
    if (!image) { lua_pushboolean(state, false); return 1; }
    const auto tiles_wide = image->width / 8U;
    const auto source_x = static_cast<std::uint64_t>(tile) % tiles_wide * 8U;
    const auto source_y = static_cast<std::uint64_t>(tile) / tiles_wide * 8U;
    const bool drawn = source_y < image->height && emit_draw(state, image, 5, static_cast<std::int32_t>(source_x),
                                                              static_cast<std::int32_t>(source_y), 8, 8);
    lua_pushboolean(state, drawn);
    return 1;
}

int renderer_draw_frame(lua_State* state) {
    const auto* bitmap = resource_at(state, 2);
    const auto* palette = resource_at(state, 3);
    if (bitmap->type != LuaResourceType::Bitmap || palette->type != LuaResourceType::Palette)
        return luaL_error(state, "renderer:drawFrame needs bitmap and palette handles");
    const auto frame = luaL_checkinteger(state, 4);
    const auto width = luaL_checkinteger(state, 5);
    const auto height = luaL_checkinteger(state, 6);
    if (frame < 0 || width <= 0 || height <= 0 || width > 4096 || height > 4096)
        return luaL_error(state, "drawFrame arguments are invalid");
    const auto bitmap_index = *static_cast<const std::size_t*>(lua_touserdata(state, 2));
    const auto palette_index = *static_cast<const std::size_t*>(lua_touserdata(state, 3));
    auto image = rasterize(state, bitmap_index, palette_index);
    if (!image) { lua_pushboolean(state, false); return 1; }
    const auto frames_wide = image->width / static_cast<std::size_t>(width);
    if (!frames_wide) return luaL_error(state, "drawFrame width exceeds the bitmap");
    const auto source_x = static_cast<std::uint64_t>(frame) % frames_wide * static_cast<std::uint64_t>(width);
    const auto source_y = static_cast<std::uint64_t>(frame) / frames_wide * static_cast<std::uint64_t>(height);
    const bool drawn = source_y + static_cast<std::uint64_t>(height) <= image->height &&
                       emit_draw(state, image, 7, static_cast<std::int32_t>(source_x), static_cast<std::int32_t>(source_y),
                                 static_cast<std::uint32_t>(width), static_cast<std::uint32_t>(height));
    lua_pushboolean(state, drawn);
    return 1;
}

void push_renderer(lua_State* state) {
    lua_newtable(state);
    lua_pushcfunction(state, renderer_draw); lua_setfield(state, -2, "draw");
    lua_pushcfunction(state, renderer_draw_tile); lua_setfield(state, -2, "drawTile");
    lua_pushcfunction(state, renderer_draw_frame); lua_setfield(state, -2, "drawFrame");
}

void push_actor(lua_State* state, const LuaActorRenderInput& input) {
    lua_newtable(state);
    const auto integer = [&](const char* key, lua_Integer value) { lua_pushinteger(state, value); lua_setfield(state, -2, key); };
    const auto number = [&](const char* key, lua_Number value) { lua_pushnumber(state, value); lua_setfield(state, -2, key); };
    integer("x", input.x); integer("y", input.y);
    number("pixelX", input.pixel_x); number("pixelY", input.pixel_y);
    integer("objectId", input.object_id); integer("type", input.type); integer("settings", input.settings);
    if (input.hash) integer("hash", *input.hash);
    lua_pushboolean(state, input.valid); lua_setfield(state, -2, "valid");
    lua_newtable(state);
    for (std::size_t i = 0; i < input.data.size(); ++i) {
        lua_pushinteger(state, input.data[i]);
        lua_rawseti(state, -2, static_cast<lua_Integer>(i));
    }
    lua_setfield(state, -2, "data");
}

core::Result<lua_Integer> integer_field(lua_State* state,
                                        int table,
                                        const char* key,
                                        lua_Integer minimum,
                                        lua_Integer maximum,
                                        std::string_view actor,
                                        bool required = true,
                                        lua_Integer fallback = 0) {
    lua_getfield(state, table, key);
    const bool missing = lua_isnil(state, -1);
    if (missing && !required) { lua_pop(state, 1); return fallback; }
    if (!lua_isinteger(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    }
    const auto value = lua_tointeger(state, -1);
    lua_pop(state, 1);
    if (value < minimum || value > maximum)
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    return value;
}

core::Result<std::string> string_field(lua_State* state,
                                       int table,
                                       const char* key,
                                       std::string_view actor,
                                       bool required = true) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1) && !required) { lua_pop(state, 1); return std::string{}; }
    if (lua_type(state, -1) != LUA_TSTRING) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    }
    std::size_t size{};
    const auto* value = lua_tolstring(state, -1, &size);
    std::string result(value, size);
    lua_pop(state, 1);
    if (required && result.empty())
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an empty " + key);
    return result;
}

core::Result<SpriteBounds> bounds_field(lua_State* state,
                                        int table,
                                        const char* key,
                                        SpriteBounds fallback,
                                        std::string_view actor) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    if (!lua_istable(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    }
    const auto bounds_table = lua_gettop(state);
    auto x = integer_field(state, bounds_table, "x", -32768, 32767, actor);
    auto y = integer_field(state, bounds_table, "y", -32768, 32767, actor);
    auto width = integer_field(state, bounds_table, "width", 1, 4096, actor);
    auto height = integer_field(state, bounds_table, "height", 1, 4096, actor);
    lua_pop(state, 1);
    if (!x) return x.error(); if (!y) return y.error(); if (!width) return width.error(); if (!height) return height.error();
    return SpriteBounds{static_cast<std::int16_t>(x.value()), static_cast<std::int16_t>(y.value()),
                        static_cast<std::uint16_t>(width.value()), static_cast<std::uint16_t>(height.value())};
}

core::Result<std::filesystem::path> resource_field(lua_State* state,
                                                   int table,
                                                   const char* key,
                                                   ActorResourceKind kind,
                                                   const LuaContext& context,
                                                   std::string_view actor) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return std::filesystem::path{}; }
    if (!lua_isuserdata(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError,
                                std::string(actor) + " must set " + key + " with read" +
                                    (kind == ActorResourceKind::Graphics ? "Graphics" : kind == ActorResourceKind::Palette ? "Palette" : "Model"));
    }
    const auto index = *static_cast<const std::size_t*>(lua_touserdata(state, -1));
    lua_pop(state, 1);
    const auto expected = kind == ActorResourceKind::Graphics ? LuaResourceType::LegacyGraphics
                          : kind == ActorResourceKind::Palette ? LuaResourceType::LegacyPalette
                                                               : LuaResourceType::LegacyModel;
    if (index >= context.resources.size() || context.resources[index].type != expected)
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has the wrong resource type in " + key);
    return context.resources[index].path;
}

core::Result<bool> boolean_field(lua_State* state,
                                 int table,
                                 const char* key,
                                 std::string_view actor,
                                 bool fallback = false) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    if (!lua_isboolean(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    }
    const bool value = lua_toboolean(state, -1);
    lua_pop(state, 1);
    return value;
}

std::uint64_t field_mask(const SpriteFieldDefinition& field) {
    return (std::uint64_t{1} << field.bit_width) - 1U;
}

core::Result<void> read_fields(lua_State* state, int table, LuaActorDefinition& actor) {
    lua_getfield(state, table, "fields");
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return {}; }
    if (!lua_istable(state, -1)) { lua_pop(state, 1); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has invalid fields"); }
    const auto fields_table = lua_gettop(state);
    std::unordered_set<std::string> keys;
    const auto count = lua_rawlen(state, fields_table);
    for (std::size_t i = 1; i <= count; ++i) {
        lua_rawgeti(state, fields_table, static_cast<lua_Integer>(i));
        if (!lua_istable(state, -1)) { lua_pop(state, 2); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has an invalid field"); }
        const auto field_table = lua_gettop(state);
        SpriteFieldDefinition field;
        auto key = string_field(state, field_table, "key", actor.sprite.name); if (!key) { lua_pop(state, 2); return key.error(); } field.key = std::move(key.value());
        auto label = string_field(state, field_table, "label", actor.sprite.name); if (!label) { lua_pop(state, 2); return label.error(); } field.label = std::move(label.value());
        auto notes = string_field(state, field_table, "notes", actor.sprite.name, false); if (!notes) { lua_pop(state, 2); return notes.error(); } field.notes = std::move(notes.value());
        if (!keys.insert(field.key).second) { lua_pop(state, 2); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has a duplicate field key"); }
        auto offset = integer_field(state, field_table, "bitOffset", 0, 47, field.key); if (!offset) { lua_pop(state, 2); return offset.error(); }
        auto width = integer_field(state, field_table, "bitWidth", 1, std::min<lua_Integer>(31, 48 - offset.value()), field.key); if (!width) { lua_pop(state, 2); return width.error(); }
        field.bit_offset = static_cast<std::uint8_t>(offset.value()); field.bit_width = static_cast<std::uint8_t>(width.value());
        auto value_offset = integer_field(state, field_table, "valueOffset", std::numeric_limits<std::int32_t>::min(),
                                          std::numeric_limits<std::int32_t>::max(), field.key, false, 0);
        if (!value_offset) { lua_pop(state, 2); return value_offset.error(); }
        field.value_offset = static_cast<std::int32_t>(value_offset.value());
        auto checkbox = integer_field(state, field_table, "checkboxValue", 0, field_mask(field), field.key, false);
        if (!checkbox) { lua_pop(state, 2); return checkbox.error(); }
        field.checkbox_value = static_cast<std::uint32_t>(checkbox.value());
        auto signed_value = boolean_field(state, field_table, "signed", field.key);
        if (!signed_value) { lua_pop(state, 2); return signed_value.error(); }
        field.signed_value = signed_value.value();
        lua_getfield(state, field_table, "choices");
        if (!lua_isnil(state, -1)) {
            if (!lua_istable(state, -1)) { lua_pop(state, 3); return core::make_error(core::ErrorCode::ParseError, field.key + " has invalid choices"); }
            const auto choices_table = lua_gettop(state);
            for (std::size_t choice = 1; choice <= lua_rawlen(state, choices_table); ++choice) {
                lua_rawgeti(state, choices_table, static_cast<lua_Integer>(choice));
                if (!lua_istable(state, -1)) { lua_pop(state, 4); return core::make_error(core::ErrorCode::ParseError, field.key + " has an invalid choice"); }
                const auto choice_table = lua_gettop(state);
                auto value = integer_field(state, choice_table, "value", 0, std::numeric_limits<std::uint32_t>::max(), field.key); if (!value) { lua_pop(state, 4); return value.error(); }
                auto choice_label = string_field(state, choice_table, "label", field.key); if (!choice_label) { lua_pop(state, 4); return choice_label.error(); }
                field.choices.push_back({static_cast<std::uint32_t>(value.value()), std::move(choice_label.value())});
                lua_pop(state, 1);
            }
        }
        lua_pop(state, 1);
        actor.sprite.fields.push_back(std::move(field));
        lua_pop(state, 1);
    }
    lua_pop(state, 1);
    return {};
}

core::Result<void> read_source_files(lua_State* state, int table, LuaActorDefinition& actor) {
    lua_getfield(state, table, "sourceFiles");
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return {}; }
    if (!lua_istable(state, -1)) { lua_pop(state, 1); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has invalid sourceFiles"); }
    const auto files_table = lua_gettop(state);
    for (std::size_t i = 1; i <= lua_rawlen(state, files_table); ++i) {
        lua_rawgeti(state, files_table, static_cast<lua_Integer>(i));
        if (lua_type(state, -1) != LUA_TSTRING) { lua_pop(state, 2); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has an invalid source file"); }
        std::size_t size{};
        const auto* value = lua_tolstring(state, -1, &size);
        actor.sprite.source_files.emplace_back(value, size);
        lua_pop(state, 1);
    }
    lua_pop(state, 1);
    return {};
}

core::Result<int> function_field(lua_State* state, int table, const char* key, std::string_view actor) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return LUA_NOREF; }
    if (!lua_isfunction(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    }
    return luaL_ref(state, LUA_REGISTRYINDEX);
}

core::Result<LuaActorDefinition> load_actor(const std::filesystem::path& path,
                                            const ActorResourceResolver& resolve,
                                            LuaAssetCache& cache) {
    auto bytes = core::read_file(path);
    if (!bytes) return bytes.error();
    if (bytes.value().size() > max_script_bytes)
        return core::make_error(core::ErrorCode::OutOfBounds, "actor script is larger than 1 MiB");
    auto program = std::make_shared<LuaActorProgram>();
#if LUA_VERSION_NUM >= 505
    lua_State* state = lua_newstate(allocate, &program->memory, 0);
#else
    lua_State* state = lua_newstate(allocate, &program->memory);
#endif
    if (!state) return core::make_error(core::ErrorCode::IoError, "could not create the Lua actor state");
    program->state = state;
    program->context.resolve = &resolve;
    program->context.cache = &cache;
    *static_cast<LuaContext**>(lua_getextraspace(state)) = &program->context;
    expose_reader(state, "readGraphics", ActorResourceKind::Graphics);
    expose_reader(state, "readPalette", ActorResourceKind::Palette);
    expose_reader(state, "readModel", ActorResourceKind::Model);
    expose_render_loaders(state);
    lua_sethook(state, instruction_hook, LUA_MASKCOUNT, 1000);
    const std::string chunk = "@" + path.string();
    if (luaL_loadbufferx(state, reinterpret_cast<const char*>(bytes.value().data()), bytes.value().size(), chunk.c_str(), "t") != LUA_OK ||
        lua_pcall(state, 0, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(state, -1);
        return core::make_error(core::ErrorCode::ParseError, message ? message : "actor script failed");
    }
    if (!lua_istable(state, -1)) return core::make_error(core::ErrorCode::ParseError, "actor script must return a table");
    const auto table = lua_gettop(state);
    auto schema = integer_field(state, table, "schema", 1, 1, path.filename().string()); if (!schema) return schema.error();
    (void)schema;
    LuaActorDefinition actor;
    actor.source = path;
    lua_getfield(state, table, "objectId");
    if (!lua_isnil(state, -1)) {
        if (!lua_isinteger(state, -1) || lua_tointeger(state, -1) < 0 || static_cast<lua_Unsigned>(lua_tointeger(state, -1)) > std::numeric_limits<std::uint32_t>::max()) {
            lua_pop(state, 1); return core::make_error(core::ErrorCode::ParseError, "actor has an invalid objectId");
        }
        actor.object_id = static_cast<std::uint32_t>(lua_tointeger(state, -1));
    }
    lua_pop(state, 1);
    lua_getfield(state, table, "hash");
    if (!lua_isnil(state, -1)) {
        if (!lua_isinteger(state, -1) || lua_tointeger(state, -1) < 0 || static_cast<lua_Unsigned>(lua_tointeger(state, -1)) > std::numeric_limits<std::uint32_t>::max()) {
            lua_pop(state, 1); return core::make_error(core::ErrorCode::ParseError, "actor has an invalid hash");
        }
        actor.hash = static_cast<std::uint32_t>(lua_tointeger(state, -1));
    }
    lua_pop(state, 1);
    if (actor.object_id.has_value() == actor.hash.has_value())
        return core::make_error(core::ErrorCode::ParseError, "actor must define exactly one of objectId or hash");
    auto name = string_field(state, table, "name", path.filename().string()); if (!name) return name.error(); actor.sprite.name = std::move(name.value());
    actor.sprite.object_id = actor.object_id.value_or(0);
    actor.sprite.hash = actor.hash;
    auto category = string_field(state, table, "category", actor.sprite.name, false); if (!category) return category.error(); if (!category.value().empty()) actor.sprite.category = std::move(category.value());
    auto notes = string_field(state, table, "notes", actor.sprite.name, false); if (!notes) return notes.error(); actor.sprite.notes = std::move(notes.value());
    auto known = boolean_field(state, table, "known", actor.sprite.name); if (!known) return known.error(); actor.sprite.known = known.value();
    auto complete = boolean_field(state, table, "complete", actor.sprite.name); if (!complete) return complete.error(); actor.sprite.complete = complete.value();
    auto color = string_field(state, table, "color", actor.sprite.name, false); if (!color) return color.error(); if (!color.value().empty()) actor.sprite.color = std::move(color.value());
    if (actor.sprite.color.size() != 7 || actor.sprite.color.front() != '#' ||
        std::ranges::any_of(std::string_view(actor.sprite.color).substr(1), [](char c) { return !std::isxdigit(static_cast<unsigned char>(c)); }))
        return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has an invalid #RRGGBB color");
    auto shape = string_field(state, table, "shape", actor.sprite.name, false); if (!shape) return shape.error();
    if (!shape.value().empty()) {
        if (shape.value() == "box") actor.sprite.shape = SpriteShape::Box;
        else if (shape.value() == "ellipse") actor.sprite.shape = SpriteShape::Ellipse;
        else if (shape.value() == "diamond") actor.sprite.shape = SpriteShape::Diamond;
        else return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has an invalid shape");
    }
    auto grab = bounds_field(state, table, "grabBounds", actor.sprite.grab_bounds, actor.sprite.name); if (!grab) return grab.error(); actor.sprite.grab_bounds = grab.value();
    auto render = bounds_field(state, table, "renderBounds", actor.sprite.render_bounds, actor.sprite.name); if (!render) return render.error(); actor.sprite.render_bounds = render.value();
    if (auto files = read_source_files(state, table, actor); !files) return files.error();
    if (auto fields = read_fields(state, table, actor); !fields) return fields.error();
    auto graphics = resource_field(state, table, "graphics", ActorResourceKind::Graphics, program->context, actor.sprite.name); if (!graphics) return graphics.error(); actor.graphics = std::move(graphics.value());
    auto palette = resource_field(state, table, "palette", ActorResourceKind::Palette, program->context, actor.sprite.name); if (!palette) return palette.error(); actor.palette = std::move(palette.value());
    auto model = resource_field(state, table, "model", ActorResourceKind::Model, program->context, actor.sprite.name); if (!model) return model.error(); actor.model = std::move(model.value());
    if (actor.graphics.empty() != actor.palette.empty()) {
        actor.graphics.clear();
        actor.palette.clear();
    }
    if (!actor.model.empty() && !actor.graphics.empty())
        return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " cannot define both a model and 2D graphics");
    auto on_render = function_field(state, table, "onRender", actor.sprite.name); if (!on_render) return on_render.error(); program->on_render = on_render.value();
    auto grab_bounds = function_field(state, table, "getGrabBounds", actor.sprite.name); if (!grab_bounds) return grab_bounds.error(); program->grab_bounds = grab_bounds.value();
    auto render_bounds = function_field(state, table, "getRenderBounds", actor.sprite.name); if (!render_bounds) return render_bounds.error(); program->render_bounds = render_bounds.value();
    for (const auto& resource : program->context.resources)
        if (!resource.path.empty() && resource.type >= LuaResourceType::Bitmap) actor.render_resources.push_back(resource.path);
    std::ranges::sort(actor.render_resources);
    actor.render_resources.erase(std::unique(actor.render_resources.begin(), actor.render_resources.end()), actor.render_resources.end());
    program->context.resolve = nullptr;
    program->context.cache = nullptr;
    if (program->on_render != LUA_NOREF || program->grab_bounds != LUA_NOREF || program->render_bounds != LUA_NOREF)
        actor.program = std::move(program);
    return actor;
}

} // namespace

bool has_lua_renderer(const LuaActorDefinition& actor) {
    return actor.program && actor.program->on_render != LUA_NOREF;
}

core::Result<std::vector<LuaActorDrawCommand>> render_lua_actor(const LuaActorDefinition& actor,
                                                                const LuaActorRenderInput& input) {
    std::vector<LuaActorDrawCommand> commands;
    if (!has_lua_renderer(actor)) return commands;
    auto& program = *actor.program;
    auto& value = program.context;
    value.instructions = 0;
    value.commands = &commands;
    value.input = &input;
    lua_settop(program.state, 0);
    lua_rawgeti(program.state, LUA_REGISTRYINDEX, program.on_render);
    push_actor(program.state, input);
    push_renderer(program.state);
    if (lua_pcall(program.state, 2, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(program.state, -1);
        const std::string error = message ? message : "actor onRender failed";
        value.commands = nullptr;
        value.input = nullptr;
        lua_settop(program.state, 0);
        return core::make_error(core::ErrorCode::ParseError, error);
    }
    if (lua_isboolean(program.state, -1) && !lua_toboolean(program.state, -1)) commands.clear();
    value.commands = nullptr;
    value.input = nullptr;
    lua_settop(program.state, 0);
    return commands;
}

core::Result<std::optional<SpriteBounds>> lua_actor_bounds(const LuaActorDefinition& actor,
                                                           const LuaActorRenderInput& input,
                                                           bool render_bounds) {
    if (!actor.program) return std::optional<SpriteBounds>{};
    auto& program = *actor.program;
    const auto function = render_bounds && program.render_bounds != LUA_NOREF ? program.render_bounds : program.grab_bounds;
    if (function == LUA_NOREF) return std::optional<SpriteBounds>{};
    program.context.instructions = 0;
    lua_settop(program.state, 0);
    lua_rawgeti(program.state, LUA_REGISTRYINDEX, function);
    push_actor(program.state, input);
    if (lua_pcall(program.state, 1, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(program.state, -1);
        const std::string error = message ? message : "actor bounds function failed";
        lua_settop(program.state, 0);
        return core::make_error(core::ErrorCode::ParseError, error);
    }
    if (lua_isnil(program.state, -1)) { lua_settop(program.state, 0); return std::optional<SpriteBounds>{}; }
    if (!lua_istable(program.state, -1)) {
        lua_settop(program.state, 0);
        return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " bounds function must return a table or nil");
    }
    const auto table = lua_gettop(program.state);
    auto x = integer_field(program.state, table, "x", -32768, 32767, actor.sprite.name);
    auto y = integer_field(program.state, table, "y", -32768, 32767, actor.sprite.name);
    auto width = integer_field(program.state, table, "width", 1, 4096, actor.sprite.name);
    auto height = integer_field(program.state, table, "height", 1, 4096, actor.sprite.name);
    if (!x || !y || !width || !height) {
        const auto error = !x ? x.error() : !y ? y.error() : !width ? width.error() : height.error();
        lua_settop(program.state, 0);
        return error;
    }
    const SpriteBounds bounds{static_cast<std::int16_t>(x.value()), static_cast<std::int16_t>(y.value()),
                              static_cast<std::uint16_t>(width.value()), static_cast<std::uint16_t>(height.value())};
    lua_settop(program.state, 0);
    return std::optional<SpriteBounds>(bounds);
}

const LuaActorDefinition* LuaActorCatalog::by_object_id(std::uint32_t object_id) const {
    const auto found = std::ranges::find(actors, std::optional(object_id), &LuaActorDefinition::object_id);
    return found == actors.end() ? nullptr : &*found;
}

const LuaActorDefinition* LuaActorCatalog::by_hash(std::uint32_t hash) const {
    const auto found = std::ranges::find(actors, std::optional(hash), &LuaActorDefinition::hash);
    return found == actors.end() ? nullptr : &*found;
}

SpriteDefinitionCatalog LuaActorCatalog::sprite_definitions() const {
    SpriteDefinitionCatalog result;
    result.definitions.reserve(actors.size());
    for (const auto& actor : actors) result.definitions.push_back(actor.sprite);
    return result;
}

core::Result<LuaActorCatalog> load_lua_actor_layers(std::span<const LuaActorLayer> layers,
                                                    const ActorResourceResolver& resolve) {
    LuaActorCatalog result;
    LuaAssetCache cache;
    std::unordered_set<std::uint32_t> object_ids, hashes;
    for (const auto& layer : layers) {
        std::error_code ec;
        if (!std::filesystem::exists(layer.directory, ec)) {
            if (ec) return core::make_error(core::ErrorCode::IoError, "could not inspect actor layer " + layer.name);
            continue;
        }
        if (!std::filesystem::is_directory(layer.directory, ec) || ec)
            return core::make_error(core::ErrorCode::InvalidArgument, "actor layer is not a directory: " + layer.name);
        std::vector<std::filesystem::path> scripts;
        for (const auto& entry : std::filesystem::directory_iterator(layer.directory, ec))
            if (entry.is_regular_file(ec) && entry.path().extension() == ".lua") scripts.push_back(entry.path());
        if (ec) return core::make_error(core::ErrorCode::IoError, "could not read actor layer " + layer.name);
        std::ranges::sort(scripts);
        for (const auto& script : scripts) {
            auto actor = load_actor(script, resolve, cache);
            if (!actor) return core::make_error(actor.error().code, layer.name + " / " + script.filename().string() + ": " + actor.error().message);
            const bool claimed = actor.value().object_id ? !object_ids.insert(*actor.value().object_id).second
                                                         : !hashes.insert(*actor.value().hash).second;
            if (!claimed) result.actors.push_back(std::move(actor.value()));
        }
    }
    return result;
}

} // namespace nsmb::project
