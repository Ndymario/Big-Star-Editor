#include "nsmb/project/lua_actors.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <cstring>
#include <limits>
#include <memory>
#include <string_view>
#include <unordered_map>
#include <unordered_set>

extern "C" {
#include <lua.h>
#include <lauxlib.h>
#include <lualib.h>
}

#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/g2d.hpp"
#include "nsmb/formats/tex0.hpp"

namespace nsmb::project {

struct LuaMemory {
    std::size_t used{};
};

struct LuaPalette {
    std::vector<formats::Bgr555Color> colors;
};

// Everything the renderer can draw is a finished RGBA image. Indexed graphics
// are combined with their palette at load time, so painting never re-rasterizes.
struct LuaDrawable {
    std::shared_ptr<const formats::RgbaImage> image;
};

enum class LuaResourceType { Palette, Drawable };

struct ResourceRef {
    LuaResourceType type{};
    // What the asset cache is keyed on: the file on disk when a module supplies
    // this resource, and the ROM address otherwise. Two variants name the same
    // address and hand back different bytes, so the address alone would serve a
    // stale image after a variant switch.
    std::string key;
    // The file to watch, empty when the ROM supplied it -- the ROM changes only
    // when a build replaces it, and is watched once rather than per resource.
    std::filesystem::path watch;
    std::shared_ptr<const LuaPalette> palette;
    std::shared_ptr<const formats::RgbaImage> image;
};

struct LuaAssetCache {
    std::unordered_map<std::string, std::weak_ptr<const LuaPalette>> palettes;
    std::unordered_map<std::string, std::weak_ptr<const formats::RgbaImage>> images;
};

struct LuaContext {
    const ActorResourceResolver* resolve{};
    const ActorImageDecoder* decode_image{};
    LuaAssetCache* cache{};
    std::filesystem::path layer;
    std::vector<ResourceRef> resources;
    std::vector<LuaActorDrawCommand>* commands{};
    const LuaActorRenderInput* input{};
    const std::vector<SpriteFieldDefinition>* fields{};
    std::size_t instructions{};
};

// A render is pure in its input: the same program given the same
// LuaActorRenderInput always draws the same commands. A repaint re-asks for
// every visible sprite, and scrolling or zooming changes none of their inputs,
// so the answer is kept rather than recomputed. Position is part of the key
// because commands carry absolute scene coordinates -- a sprite being dragged
// misses until it is dropped, while the rest of the level keeps hitting.
struct CallCacheKey {
    std::int32_t x{}, y{};
    double pixel_x{}, pixel_y{};
    std::uint32_t object_id{};
    std::uint16_t type{};
    std::uint32_t hash{};
    std::array<std::uint8_t, 6> data{};
    std::uint32_t settings{};
    bool has_hash{};
    bool valid{};
    bool operator==(const CallCacheKey&) const = default;
};

struct CallCacheKeyHash {
    std::size_t operator()(const CallCacheKey& key) const noexcept {
        std::size_t seed = 0;
        const auto mix = [&seed](std::size_t value) {
            seed ^= value + 0x9E3779B97F4A7C15ULL + (seed << 6U) + (seed >> 2U);
        };
        mix(static_cast<std::uint32_t>(key.x)); mix(static_cast<std::uint32_t>(key.y));
        mix(std::hash<double>{}(key.pixel_x)); mix(std::hash<double>{}(key.pixel_y));
        mix(key.object_id); mix(key.type); mix(key.settings);
        for (const auto byte : key.data) mix(byte);
        mix(key.has_hash ? key.hash : 0U);
        mix(static_cast<std::size_t>(key.valid) | (static_cast<std::size_t>(key.has_hash) << 1U));
        return seed;
    }
};

// Bounds are asked per sprite for the grab box and again for the render box, so
// which function ran is part of the key. It is keyed on the resolved reference
// rather than on the caller's flag: an actor that declares no renderBounds
// answers both questions from grabBounds, and should not pay twice for it.
struct BoundsCacheKey {
    CallCacheKey input;
    int function{};
    bool operator==(const BoundsCacheKey&) const = default;
};

struct BoundsCacheKeyHash {
    std::size_t operator()(const BoundsCacheKey& key) const noexcept {
        return CallCacheKeyHash{}(key.input) * 31U + static_cast<std::size_t>(key.function);
    }
};

// A script that raises is cached too: an actor whose render() is broken is the
// worst case for repaint cost otherwise, since it pays a full pcall and a
// message build on every frame to arrive at the same failure.
struct RenderCacheEntry {
    std::vector<LuaActorDrawCommand> commands;
    std::string error;
    bool custom_rendered{};
};

// An absent box is a real answer, and the common one, so nullopt is cached as
// readily as a box.
struct BoundsCacheEntry {
    std::optional<SpriteBounds> bounds;
    std::string error;
};

struct LuaActorProgram {
    LuaMemory memory;
    LuaContext context;
    lua_State* state{};
    int render{LUA_NOREF};
    int grab_bounds{LUA_NOREF};
    int render_bounds{LUA_NOREF};
    // Keyed only on the call input: everything else a script reads -- the
    // resource resolver, the layer, the field definitions -- is fixed for the
    // program's lifetime, so the cache lives here and dies with the program it
    // answers for. Reloading a layer builds new programs, which is what makes
    // an edited script take effect without an invalidation step of its own.
    std::unordered_map<CallCacheKey, RenderCacheEntry, CallCacheKeyHash> render_cache;
    std::unordered_map<BoundsCacheKey, BoundsCacheEntry, BoundsCacheKeyHash> bounds_cache;
    ~LuaActorProgram() { if (state) lua_close(state); }
};

namespace {

constexpr std::size_t max_script_bytes = 1024U * 1024U;
constexpr std::size_t max_lua_bytes = 8U * 1024U * 1024U;
constexpr std::size_t max_instructions = 200000U;
// Enough for a screen-wide run of 16px tiles. Past it a draw is dropped
// rather than raised, so an actor given nonsense settings still draws the
// part that fits instead of vanishing.
constexpr std::size_t max_draw_commands = 4096U;
// Sized past the sprite count a course reaches, so a level's steady state fits
// and only a drag -- which mints a key per mouse position -- can fill it.
constexpr std::size_t max_call_cache_entries = 1024U;

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

core::Result<std::vector<std::uint8_t>> actor_resource_bytes(const std::vector<std::uint8_t>& bytes) {
    if (!bytes.empty() && (bytes.front() == 0x10 || bytes.front() == 0x11))
        return formats::decompress_nitro_lz(bytes);
    return bytes;
}

// The address when the ROM supplies this, the disk path when a module does.
std::string resource_key(const ActorResource& resource) {
    return resource.source.empty() ? resource.rom_path : resource.source.string();
}

std::optional<ActorResource> resolve_resource(lua_State* state, ActorResourceKind kind, const char* name) {
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

core::Result<std::shared_ptr<const formats::RgbaImage>> decode_bitmap(const ActorResource& resource,
                                                                             int bpp,
                                                                             std::size_t width,
                                                                             std::size_t height,
                                                                             std::size_t tile_offset,
                                                                             const LuaPalette& palette) {
    auto bytes = actor_resource_bytes(resource.data);
    if (!bytes) return bytes.error();
    std::span<const std::uint8_t> characters(bytes.value());
    std::optional<formats::Ncgr> ncgr;
    if (formats::identify_graphics_format(bytes.value(), resource.rom_path) == formats::GraphicsFormatKind::NitroCharacterGraphics) {
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
    auto result = std::make_shared<formats::RgbaImage>();
    result->width = width;
    result->height = height;
    result->pixels.assign(width * height * 4U, 0);
    const auto wanted = tiles_wide * (height / 8U);
    for (std::size_t tile = 0; tile < std::min(wanted, available); ++tile) {
        const auto tile_x = tile % tiles_wide, tile_y = tile / tiles_wide;
        for (std::size_t y = 0; y < 8U; ++y) {
            for (std::size_t x = 0; x < 8U; ++x) {
                const auto index = tiles[tile_offset + tile][y * 8U + x];
                if (index >= palette.colors.size()) continue;
                const auto& color = palette.colors[index];
                const auto target = ((tile_y * 8U + y) * width + tile_x * 8U + x) * 4U;
                result->pixels[target] = color.red;
                result->pixels[target + 1U] = color.green;
                result->pixels[target + 2U] = color.blue;
                result->pixels[target + 3U] = color.transparent ? 0 : 255;
            }
        }
    }
    return std::shared_ptr<const formats::RgbaImage>(std::move(result));
}

core::Result<std::shared_ptr<const LuaPalette>> decode_palette(const ActorResource& resource,
                                                               std::size_t bank,
                                                               std::size_t color_count,
                                                               bool transparent) {
    auto bytes = actor_resource_bytes(resource.data);
    if (!bytes) return bytes.error();
    std::span<const std::uint8_t> raw(bytes.value());
    std::optional<formats::Nclr> nclr;
    if (formats::identify_graphics_format(bytes.value(), resource.rom_path) == formats::GraphicsFormatKind::NitroPalette) {
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

core::Result<std::shared_ptr<const formats::RgbaImage>> decode_texture(const ActorResource& resource,
                                                                       std::size_t texture,
                                                                       std::size_t palette) {
    auto bytes = actor_resource_bytes(resource.data);
    if (!bytes) return bytes.error();
    auto tex0 = formats::parse_tex0(bytes.value());
    if (!tex0) return tex0.error();
    if (texture >= tex0.value().textures.size() || palette >= tex0.value().palettes.size())
        return core::make_error(core::ErrorCode::OutOfBounds, "loadTexture index is outside the resource");
    auto image = formats::render_tex0_texture(tex0.value().textures[texture], tex0.value().palettes[palette]);
    if (!image) return image.error();
    return std::make_shared<const formats::RgbaImage>(std::move(image.value()));
}

// Ports need the pixel size of what they loaded: the stock renderer places a
// rotated still by its rotated top-left, which only converts to a centred
// destination rectangle once the script knows the source dimensions.
int resource_index(lua_State* state) {
    const auto handle = *static_cast<const std::size_t*>(lua_touserdata(state, 1));
    const auto& resources = context(state).resources;
    const auto* key = luaL_checkstring(state, 2);
    const auto& image = handle < resources.size() ? resources[handle].image : nullptr;
    if (std::string_view(key) == "width") lua_pushinteger(state, image ? static_cast<lua_Integer>(image->width) : 0);
    else if (std::string_view(key) == "height") lua_pushinteger(state, image ? static_cast<lua_Integer>(image->height) : 0);
    else return luaL_error(state, "a resource has no '%s'", key);
    return 1;
}

void push_resource(lua_State* state, ResourceRef resource) {
    auto& value = context(state);
    auto* handle = static_cast<std::size_t*>(lua_newuserdatauv(state, sizeof(std::size_t), 0));
    *handle = value.resources.size();
    value.resources.push_back(std::move(resource));
    lua_newtable(state);
    lua_pushcfunction(state, resource_index);
    lua_setfield(state, -2, "__index");
    lua_setmetatable(state, -2);
}

const ResourceRef* resource_handle(lua_State* state, int index, LuaResourceType expected, const char* what) {
    if (!lua_isuserdata(state, index)) luaL_error(state, "%s", what);
    const auto handle = *static_cast<const std::size_t*>(lua_touserdata(state, index));
    const auto& resources = context(state).resources;
    if (handle >= resources.size() || resources[handle].type != expected) luaL_error(state, "%s", what);
    return &resources[handle];
}

// Stills live beside the actor script rather than in the ROM, so they resolve
// against the layer directory and may not climb out of it.
int load_image(lua_State* state) {
    auto& value = context(state);
    if (!value.cache) return luaL_error(state, "resources can only be loaded from init");
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadImage filename must be a string");
    const std::filesystem::path name = lua_tostring(state, 1);
    if (name.is_absolute()) return luaL_error(state, "loadImage filename must be relative to the actor directory");
    for (const auto& part : name)
        if (part == "..") return luaL_error(state, "loadImage filename may not leave the actor directory");
    ResourceRef resource{LuaResourceType::Drawable, {}, {}};
    const auto path = value.layer / name;
    std::error_code ec;
    if (std::filesystem::exists(path, ec) && !ec) {
        resource.watch = path;
        const auto key = resource.key = path.string();
        resource.image = cached(value.cache->images, key);
        if (!resource.image && value.decode_image && *value.decode_image) {
            auto bytes = core::read_file(path);
            if (bytes) {
                auto decoded = (*value.decode_image)(bytes.value());
                if (decoded)
                    value.cache->images[key] = resource.image =
                        std::make_shared<const formats::RgbaImage>(std::move(decoded.value()));
            }
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

int load_palette(lua_State* state) {
    if (!context(state).cache) return luaL_error(state, "resources can only be loaded from init");
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadPalette filename must be a string");
    if (!lua_isnoneornil(state, 2)) luaL_checktype(state, 2, LUA_TTABLE);
    const auto table = lua_istable(state, 2) ? 2 : 0;
    const auto bank = table ? static_cast<std::size_t>(option_integer(state, table, "bank", 0, 0, 65535)) : 0U;
    const auto colors = table ? static_cast<std::size_t>(option_integer(state, table, "colors", 0, 0, 65536)) : 0U;
    const bool transparent = table ? option_boolean(state, table, "transparent", true) : true;
    const auto* name = lua_tostring(state, 1);
    auto found = resolve_resource(state, ActorResourceKind::Palette, name);
    ResourceRef resource{LuaResourceType::Palette, found ? resource_key(*found) : std::string{},
                         found ? found->source : std::filesystem::path{}};
    if (found) {
        const auto key = resource.key + ":" + std::to_string(bank) + ":" + std::to_string(colors) + ":" +
                         (transparent ? "1" : "0");
        resource.palette = cached(context(state).cache->palettes, key);
        if (!resource.palette) {
            auto decoded = decode_palette(*found, bank, colors, transparent);
            if (decoded) context(state).cache->palettes[key] = resource.palette = std::move(decoded.value());
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

// The palette is bound here, not at draw time: an actor that wants two palettes
// of the same characters loads the bitmap twice, and painting stays a blit.
int load_bitmap(lua_State* state) {
    if (!context(state).cache) return luaL_error(state, "resources can only be loaded from init");
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadBitmap filename must be a string");
    luaL_checktype(state, 2, LUA_TTABLE);
    const auto* name = lua_tostring(state, 1);
    const auto bpp = static_cast<int>(option_integer(state, 2, "bpp", 0, 4, 8));
    if (bpp != 4 && bpp != 8) return luaL_error(state, "loadBitmap bpp must be 4 or 8");
    const auto width = static_cast<std::size_t>(option_integer(state, 2, "width", 0, 0, 4096));
    const auto height = static_cast<std::size_t>(option_integer(state, 2, "height", 0, 0, 4096));
    const auto offset = static_cast<std::size_t>(option_integer(state, 2, "tileOffset", 0, 0, 1 << 24));
    lua_getfield(state, 2, "palette");
    if (lua_isnil(state, -1)) return luaL_error(state, "loadBitmap needs a palette from loadPalette");
    const auto* palette = resource_handle(state, lua_gettop(state), LuaResourceType::Palette,
                                          "loadBitmap palette must come from loadPalette");
    auto palette_colors = palette->palette;
    const auto palette_key = palette->key;
    lua_pop(state, 1);
    auto found = resolve_resource(state, ActorResourceKind::Graphics, name);
    ResourceRef resource{LuaResourceType::Drawable, found ? resource_key(*found) : std::string{},
                         found ? found->source : std::filesystem::path{}};
    if (found && palette_colors) {
        const auto key = resource.key + ":" + std::to_string(bpp) + ":" + std::to_string(width) + ":" +
                         std::to_string(height) + ":" + std::to_string(offset) + ":" + palette_key;
        resource.image = cached(context(state).cache->images, key);
        if (!resource.image) {
            auto decoded = decode_bitmap(*found, bpp, width, height, offset, *palette_colors);
            if (decoded) context(state).cache->images[key] = resource.image = std::move(decoded.value());
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

int load_texture(lua_State* state) {
    if (!context(state).cache) return luaL_error(state, "resources can only be loaded from init");
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "loadTexture filename must be a string");
    if (!lua_isnoneornil(state, 2)) luaL_checktype(state, 2, LUA_TTABLE);
    const auto table = lua_istable(state, 2) ? 2 : 0;
    const auto texture = table ? static_cast<std::size_t>(option_integer(state, table, "texture", 0, 0, 65535)) : 0U;
    const auto palette = table ? static_cast<std::size_t>(option_integer(state, table, "palette", 0, 0, 65535)) : 0U;
    const auto* name = lua_tostring(state, 1);
    auto found = resolve_resource(state, ActorResourceKind::Model, name);
    ResourceRef resource{LuaResourceType::Drawable, found ? resource_key(*found) : std::string{},
                         found ? found->source : std::filesystem::path{}};
    if (found) {
        const auto key = resource.key + ":tex:" + std::to_string(texture) + ":" + std::to_string(palette);
        resource.image = cached(context(state).cache->images, key);
        if (!resource.image) {
            auto decoded = decode_texture(*found, texture, palette);
            if (decoded) context(state).cache->images[key] = resource.image = std::move(decoded.value());
        }
    }
    push_resource(state, std::move(resource));
    return 1;
}

std::shared_ptr<const formats::RgbaImage> drawable_at(lua_State* state, int index) {
    return resource_handle(state, index, LuaResourceType::Drawable,
                           "renderer methods need a handle from loadImage, loadBitmap or loadTexture")->image;
}

bool emit_draw(lua_State* state,
               std::shared_ptr<const formats::RgbaImage> image,
               std::string text,
               int options,
               std::int32_t source_x,
               std::int32_t source_y,
               std::uint32_t source_width,
               std::uint32_t source_height) {
    auto& value = context(state);
    if (!value.commands || !value.input) luaL_error(state, "renderer methods are only available inside render");
    const bool is_text = !text.empty();
    if (!is_text && (!image || !image->width || !image->height)) return false;
    if (!lua_isnoneornil(state, options)) luaL_checktype(state, options, LUA_TTABLE);
    const auto table = lua_istable(state, options) ? options : 0;
    if (table && !is_text) {
        source_x = static_cast<std::int32_t>(option_integer(state, table, "sourceX", source_x, 0, 1 << 20));
        source_y = static_cast<std::int32_t>(option_integer(state, table, "sourceY", source_y, 0, 1 << 20));
        source_width = static_cast<std::uint32_t>(option_integer(state, table, "sourceWidth", source_width, 1, 1 << 20));
        source_height = static_cast<std::uint32_t>(option_integer(state, table, "sourceHeight", source_height, 1, 1 << 20));
    }
    if (!is_text && (source_x < 0 || source_y < 0 || static_cast<std::size_t>(source_x) + source_width > image->width ||
                     static_cast<std::size_t>(source_y) + source_height > image->height))
        luaL_error(state, "draw source rectangle is outside the resource");
    if (value.commands->size() >= max_draw_commands) return false;
    LuaActorDrawCommand command;
    command.kind = is_text ? LuaActorDrawKind::Text : LuaActorDrawKind::Image;
    command.image = std::move(image);
    command.text = std::move(text);
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
    auto image = drawable_at(state, 2);
    const auto width = image ? static_cast<std::uint32_t>(image->width) : 1U;
    const auto height = image ? static_cast<std::uint32_t>(image->height) : 1U;
    lua_pushboolean(state, emit_draw(state, std::move(image), {}, 3, 0, 0, width, height));
    return 1;
}

int renderer_draw_tile(lua_State* state) {
    auto image = drawable_at(state, 2);
    const auto tile = luaL_checkinteger(state, 3);
    if (tile < 0) return luaL_error(state, "tile index cannot be negative");
    if (!image) { lua_pushboolean(state, false); return 1; }
    const auto tiles_wide = image->width / 8U;
    if (!tiles_wide) return luaL_error(state, "drawTile needs a resource at least one tile wide");
    const auto source_x = static_cast<std::uint64_t>(tile) % tiles_wide * 8U;
    const auto source_y = static_cast<std::uint64_t>(tile) / tiles_wide * 8U;
    const bool drawn = source_y < image->height &&
                       emit_draw(state, std::move(image), {}, 4, static_cast<std::int32_t>(source_x),
                                 static_cast<std::int32_t>(source_y), 8, 8);
    lua_pushboolean(state, drawn);
    return 1;
}

int renderer_draw_frame(lua_State* state) {
    auto image = drawable_at(state, 2);
    const auto frame = luaL_checkinteger(state, 3);
    const auto width = luaL_checkinteger(state, 4);
    const auto height = luaL_checkinteger(state, 5);
    if (frame < 0 || width <= 0 || height <= 0 || width > 4096 || height > 4096)
        return luaL_error(state, "drawFrame arguments are invalid");
    if (!image) { lua_pushboolean(state, false); return 1; }
    const auto frames_wide = image->width / static_cast<std::size_t>(width);
    if (!frames_wide) return luaL_error(state, "drawFrame width exceeds the resource");
    const auto source_x = static_cast<std::uint64_t>(frame) % frames_wide * static_cast<std::uint64_t>(width);
    const auto source_y = static_cast<std::uint64_t>(frame) / frames_wide * static_cast<std::uint64_t>(height);
    const bool drawn = source_y + static_cast<std::uint64_t>(height) <= image->height &&
                       emit_draw(state, std::move(image), {}, 6, static_cast<std::int32_t>(source_x),
                                 static_cast<std::int32_t>(source_y), static_cast<std::uint32_t>(width),
                                 static_cast<std::uint32_t>(height));
    lua_pushboolean(state, drawn);
    return 1;
}

// The stock renderer prints numbers over a few actors, so text is a draw command
// rather than something scripts have to fake with graphics.
int renderer_draw_text(lua_State* state) {
    if (lua_type(state, 2) != LUA_TSTRING) return luaL_error(state, "renderer:drawText needs a string");
    std::size_t size{};
    const auto* text = lua_tolstring(state, 2, &size);
    if (!size) { lua_pushboolean(state, false); return 1; }
    lua_pushboolean(state, emit_draw(state, {}, std::string(text, size), 3, 0, 0, 0, 0));
    return 1;
}

// "#rgb", "#rrggbb" and "#rrggbbaa" all land as 0xRRGGBBAA; an integer is taken
// as that packed value directly so scripts can compute a colour.
core::Result<std::uint32_t> parse_color(std::string_view text) {
    if (!text.empty() && text.front() == '#') text.remove_prefix(1);
    if (text.size() != 3 && text.size() != 6 && text.size() != 8)
        return core::make_error(core::ErrorCode::ParseError, "a colour must be #rgb, #rrggbb or #rrggbbaa");
    std::uint32_t value = 0;
    for (const auto digit : text) {
        const auto nibble = digit >= '0' && digit <= '9'   ? digit - '0'
                            : digit >= 'a' && digit <= 'f' ? digit - 'a' + 10
                            : digit >= 'A' && digit <= 'F' ? digit - 'A' + 10
                                                           : -1;
        if (nibble < 0) return core::make_error(core::ErrorCode::ParseError, "a colour has a non-hex digit");
        value = value << 4U | static_cast<std::uint32_t>(nibble);
        if (text.size() == 3) value = value << 4U | static_cast<std::uint32_t>(nibble);
    }
    if (text.size() != 8) value = value << 8U | 0xFFU;
    return value;
}

std::uint32_t option_color(lua_State* state, int table, std::uint32_t fallback) {
    if (!table) return fallback;
    table = lua_absindex(state, table);
    lua_getfield(state, table, "color");
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    std::uint32_t result = fallback;
    if (lua_isinteger(state, -1)) {
        result = static_cast<std::uint32_t>(lua_tointeger(state, -1));
    } else if (lua_type(state, -1) == LUA_TSTRING) {
        auto parsed = parse_color(lua_tostring(state, -1));
        if (!parsed) { lua_pop(state, 1); luaL_error(state, "%s", parsed.error().message.c_str()); }
        result = parsed.value();
    } else {
        lua_pop(state, 1);
        luaL_error(state, "color must be a string or a packed integer");
    }
    lua_pop(state, 1);
    return result;
}

int emit_shape(lua_State* state, LuaActorDrawKind kind) {
    auto& value = context(state);
    if (!value.commands || !value.input) return luaL_error(state, "renderer methods are only available inside render");
    luaL_checktype(state, 2, LUA_TTABLE);
    if (value.commands->size() >= max_draw_commands) { lua_pushboolean(state, false); return 1; }
    LuaActorDrawCommand command;
    command.kind = kind;
    command.x = option_number(state, 2, "x", value.input->pixel_x, -1048576.0, 1048576.0);
    command.y = option_number(state, 2, "y", value.input->pixel_y, -1048576.0, 1048576.0);
    if (kind == LuaActorDrawKind::Line) {
        command.x2 = option_number(state, 2, "x2", command.x, -1048576.0, 1048576.0);
        command.y2 = option_number(state, 2, "y2", command.y, -1048576.0, 1048576.0);
    } else {
        // A degenerate or inverted box draws nothing, the way GDI+ treats one,
        // rather than failing the actor's whole render.
        command.width = option_number(state, 2, "width", 16.0, -1048576.0, 1048576.0);
        command.height = option_number(state, 2, "height", 16.0, -1048576.0, 1048576.0);
        if (command.width <= 0.0 || command.height <= 0.0) { lua_pushboolean(state, false); return 1; }
    }
    command.rotation = option_number(state, 2, "rotation", 0.0, -36000.0, 36000.0);
    command.opacity = option_number(state, 2, "opacity", 1.0, 0.0, 1.0);
    command.stroke_width = option_number(state, 2, "strokeWidth", 1.0, 0.0, 4096.0);
    command.color = option_color(state, 2, 0xFFFFFFFFU);
    value.commands->push_back(std::move(command));
    lua_pushboolean(state, true);
    return 1;
}

int renderer_fill_rect(lua_State* state) { return emit_shape(state, LuaActorDrawKind::FillRect); }
int renderer_stroke_rect(lua_State* state) { return emit_shape(state, LuaActorDrawKind::StrokeRect); }
int renderer_fill_ellipse(lua_State* state) { return emit_shape(state, LuaActorDrawKind::FillEllipse); }
int renderer_draw_line(lua_State* state) { return emit_shape(state, LuaActorDrawKind::Line); }

void push_renderer(lua_State* state) {
    lua_newtable(state);
    lua_pushcfunction(state, renderer_draw); lua_setfield(state, -2, "draw");
    lua_pushcfunction(state, renderer_draw_tile); lua_setfield(state, -2, "drawTile");
    lua_pushcfunction(state, renderer_draw_frame); lua_setfield(state, -2, "drawFrame");
    lua_pushcfunction(state, renderer_draw_text); lua_setfield(state, -2, "drawText");
    lua_pushcfunction(state, renderer_fill_rect); lua_setfield(state, -2, "fillRect");
    lua_pushcfunction(state, renderer_stroke_rect); lua_setfield(state, -2, "strokeRect");
    lua_pushcfunction(state, renderer_fill_ellipse); lua_setfield(state, -2, "fillEllipse");
    lua_pushcfunction(state, renderer_draw_line); lua_setfield(state, -2, "drawLine");
}

std::uint64_t actor_data_value(const LuaActorRenderInput& input) {
    return input.settings | (static_cast<std::uint64_t>(input.data[1]) << 32U) |
           (static_cast<std::uint64_t>(input.data[0]) << 40U);
}

// Scripts read settings through the fields they already declare, so the canvas
// and the properties panel decode the same bits the same way. A checkbox field
// yields a boolean because 0 is truthy in Lua.
void push_field_value(lua_State* state, const SpriteFieldDefinition& field, const LuaActorRenderInput& input) {
    const auto value = read_sprite_field(actor_data_value(input), field);
    if (field.checkbox_value) lua_pushboolean(state, value != 0);
    else lua_pushinteger(state, value);
}

int actor_field_index(lua_State* state) {
    auto& value = context(state);
    const auto* key = luaL_checkstring(state, 2);
    if (!value.fields || !value.input) return luaL_error(state, "actor fields are unavailable here");
    for (const auto& field : *value.fields)
        if (field.key == key) { push_field_value(state, field, *value.input); return 1; }
    // Returning nil would let a mistyped key silently render the wrong thing.
    return luaL_error(state, "actor has no declared field '%s'", key);
}

int actor_field_call(lua_State* state) {
    lua_remove(state, 1); // the actor table; the key is what matters
    return actor_field_index(state);
}

// Actors format numbers into labels and resource names, so the pure parts of the
// standard library are available. Anything that reaches the filesystem, the
// process, or the loader stays out.
void expose_safe_library(lua_State* state) {
    for (const auto& [name, opener] : std::initializer_list<std::pair<const char*, lua_CFunction>>{
             {LUA_GNAME, luaopen_base}, {LUA_STRLIBNAME, luaopen_string},
             {LUA_MATHLIBNAME, luaopen_math}, {LUA_TABLIBNAME, luaopen_table}}) {
        luaL_requiref(state, name, opener, 1);
        lua_pop(state, 1);
    }
    for (const auto* name : {"dofile", "loadfile", "load", "require", "collectgarbage", "print", "loadstring"}) {
        lua_pushnil(state);
        lua_setglobal(state, name);
    }
}

// Two things come up in nearly every actor: an image that has to be drawn with a
// transform decided somewhere other than the draw call, and a destination given
// as the top-left of the *drawn* result rather than as a rectangle to rotate in
// place. `renderer:draw` is the low-level primitive for the latter; `gfx` is the
// small Lua layer over it that actor scripts share, so neither idea has to be
// restated in three hundred files. Scripts may ignore it and call the renderer
// directly.
constexpr const char* actor_prelude = R"LUA(
gfx = {}

-- Pairs an image with a transform to apply wherever it is eventually drawn.
function gfx.transform(image, rotation, flipX, flipY)
    if type(image) == "table" then image = image.image end
    return { image = image, rotation = rotation, flipX = flipX, flipY = flipY }
end

-- The pixel size of an image or of a transformed image.
function gfx.width(image)
    if type(image) == "table" then image = image.image end
    return image and image.width or 0
end

function gfx.height(image)
    if type(image) == "table" then image = image.image end
    return image and image.height or 0
end

-- Gives a rectangle a colour so it can go straight to a shape method.
function gfx.colored(rect, color)
    rect.color = color
    return rect
end

-- Draws an image so that the visible result's top-left corner lands on x, y.
-- renderer:draw rotates a destination rectangle about its own centre, so a
-- quarter-turn needs the rectangle offset by half the difference of its sides.
function gfx.blit(renderer, image, x, y, w, h, source)
    local rotation, flipX, flipY = 0, false, false
    if type(image) == "table" then
        rotation, flipX, flipY, image = image.rotation, image.flipX, image.flipY, image.image
    end
    if not image then return false end
    local opts = { rotation = rotation, flipX = flipX, flipY = flipY }
    if source then
        opts.sourceX, opts.sourceY = source.x, source.y
        opts.sourceWidth, opts.sourceHeight = source.width, source.height
    end
    opts.width = w or (source and source.width) or image.width
    opts.height = h or (source and source.height) or image.height
    if rotation % 180 ~= 0 then
        x, y = x + (opts.height - opts.width) / 2, y + (opts.width - opts.height) / 2
    end
    opts.x, opts.y = x, y
    return renderer:draw(image, opts)
end
)LUA";

core::Result<void> run_prelude(lua_State* state) {
    if (luaL_loadbuffer(state, actor_prelude, std::strlen(actor_prelude), "=gfx") != LUA_OK ||
        lua_pcall(state, 0, 0, 0) != LUA_OK) {
        const auto* message = lua_tostring(state, -1);
        return core::make_error(core::ErrorCode::ParseError,
                                std::string("the gfx prelude failed: ") + (message ? message : "?"));
    }
    return {};
}

void push_loader_context(lua_State* state) {
    lua_newtable(state);
    lua_pushcfunction(state, load_image); lua_setfield(state, -2, "loadImage");
    lua_pushcfunction(state, load_palette); lua_setfield(state, -2, "loadPalette");
    lua_pushcfunction(state, load_bitmap); lua_setfield(state, -2, "loadBitmap");
    lua_pushcfunction(state, load_texture); lua_setfield(state, -2, "loadTexture");
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
    lua_pushcfunction(state, actor_field_call); lua_setfield(state, -2, "field");
    lua_newtable(state);                  // actor.fields, empty and read through
    lua_newtable(state);                  // its metatable
    lua_pushcfunction(state, actor_field_index); lua_setfield(state, -2, "__index");
    lua_setmetatable(state, -2);
    lua_setfield(state, -2, "fields");
}

// Bounds arrive in scene pixels derived from pixelX/pixelY, which carry a drag
// offset and so are not necessarily integers.
core::Result<lua_Integer> bounds_number(lua_State* state,
                                        int table,
                                        const char* key,
                                        lua_Integer minimum,
                                        lua_Integer maximum,
                                        std::string_view actor) {
    lua_getfield(state, table, key);
    if (!lua_isnumber(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    }
    const auto value = static_cast<lua_Integer>(std::llround(lua_tonumber(state, -1)));
    lua_pop(state, 1);
    if (value < minimum || value > maximum)
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has an invalid " + key);
    return value;
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
                                            const std::filesystem::path& layer,
                                            const ActorResourceResolver& resolve,
                                            const ActorImageDecoder& decode_image,
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
    program->context.decode_image = &decode_image;
    program->context.cache = &cache;
    program->context.layer = layer;
    *static_cast<LuaContext**>(lua_getextraspace(state)) = &program->context;
    expose_safe_library(state);
    if (auto ready = run_prelude(state); !ready) return ready.error();
    lua_sethook(state, instruction_hook, LUA_MASKCOUNT, 1000);
    const std::string chunk = "@" + path.string();
    if (luaL_loadbufferx(state, reinterpret_cast<const char*>(bytes.value().data()), bytes.value().size(), chunk.c_str(), "t") != LUA_OK ||
        lua_pcall(state, 0, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(state, -1);
        return core::make_error(core::ErrorCode::ParseError, message ? message : "actor script failed");
    }
    if (!lua_istable(state, -1)) return core::make_error(core::ErrorCode::ParseError, "actor script must return a table");
    const auto table = lua_gettop(state);
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
    if (auto fields = read_fields(state, table, actor); !fields) return fields.error();
    auto render = function_field(state, table, "render", actor.sprite.name); if (!render) return render.error(); program->render = render.value();
    auto grab_bounds = function_field(state, table, "grabBounds", actor.sprite.name); if (!grab_bounds) return grab_bounds.error(); program->grab_bounds = grab_bounds.value();
    auto render_bounds = function_field(state, table, "renderBounds", actor.sprite.name); if (!render_bounds) return render_bounds.error(); program->render_bounds = render_bounds.value();
    auto init = function_field(state, table, "init", actor.sprite.name); if (!init) return init.error();

    // init() is the only place resources load, so painting never touches disk.
    // A failure here costs the actor its renderer, not the whole catalog.
    if (init.value() != LUA_NOREF) {
        lua_rawgeti(state, LUA_REGISTRYINDEX, init.value());
        push_loader_context(state);
        program->context.instructions = 0;
        if (lua_pcall(state, 1, 0, 0) != LUA_OK) {
            const auto* message = lua_tostring(state, -1);
            actor.init_error = message ? message : "actor init failed";
            lua_pop(state, 1);
            program->render = LUA_NOREF;
        }
        luaL_unref(state, LUA_REGISTRYINDEX, init.value());
    }
    for (const auto& resource : program->context.resources)
        if (!resource.watch.empty()) actor.render_resources.push_back(resource.watch);
    std::ranges::sort(actor.render_resources);
    actor.render_resources.erase(std::unique(actor.render_resources.begin(), actor.render_resources.end()), actor.render_resources.end());
    // Loaders exist only for init(); clearing them here makes a later call fail
    // loudly instead of reading from disk mid-paint.
    program->context.resolve = nullptr;
    program->context.decode_image = nullptr;
    program->context.cache = nullptr;
    if (program->render != LUA_NOREF || program->grab_bounds != LUA_NOREF || program->render_bounds != LUA_NOREF)
        actor.program = std::move(program);

    // Evaluate the bounds once at zero settings and origin, so the absolute rect
    // that comes back doubles as the offset used for hit testing and thumbnails.
    if (actor.program) {
        LuaActorRenderInput probe;
        probe.object_id = actor.object_id.value_or(0);
        probe.hash = actor.hash;
        if (auto grab = lua_actor_bounds(actor, probe, false); grab && grab.value()) actor.sprite.grab_bounds = *grab.value();
        if (auto render_box = lua_actor_bounds(actor, probe, true); render_box && render_box.value())
            actor.sprite.render_bounds = *render_box.value();
        else actor.sprite.render_bounds = actor.sprite.grab_bounds;
    }
    return actor;
}

CallCacheKey call_cache_key(const LuaActorRenderInput& input) {
    return {input.x, input.y, input.pixel_x, input.pixel_y, input.object_id, input.type,
            input.hash.value_or(0U), input.data, input.settings, input.hash.has_value(), input.valid};
}

// Dragging mints a fresh key every mouse position, so a map is capped and
// dropped whole once it fills rather than carrying per-entry age. Refilling
// costs one pass over the visible sprites, once every few hundred drag frames,
// which is what the paint path used to pay on every frame.
template <typename Map, typename Key>
void remember(Map& map, const Key& key, typename Map::mapped_type entry) {
    if (map.size() >= max_call_cache_entries) map.clear();
    map.emplace(key, std::move(entry));
}

} // namespace

bool has_lua_renderer(const LuaActorDefinition& actor) {
    return actor.program && actor.program->render != LUA_NOREF;
}

core::Result<std::vector<LuaActorDrawCommand>> render_lua_actor(const LuaActorDefinition& actor,
                                                                const LuaActorRenderInput& input,
                                                                bool* custom_rendered) {
    std::vector<LuaActorDrawCommand> commands;
    if (custom_rendered) *custom_rendered = false;
    if (!has_lua_renderer(actor)) return commands;
    auto& program = *actor.program;
    const auto key = call_cache_key(input);
    if (const auto cached = program.render_cache.find(key); cached != program.render_cache.end()) {
        if (!cached->second.error.empty())
            return core::make_error(core::ErrorCode::ParseError, cached->second.error);
        if (custom_rendered) *custom_rendered = cached->second.custom_rendered;
        return cached->second.commands;
    }
    auto& value = program.context;
    value.instructions = 0;
    value.commands = &commands;
    value.input = &input;
    value.fields = &actor.sprite.fields;
    lua_settop(program.state, 0);
    lua_rawgeti(program.state, LUA_REGISTRYINDEX, program.render);
    push_actor(program.state, input);
    push_renderer(program.state);
    if (lua_pcall(program.state, 2, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(program.state, -1);
        const std::string error = message ? message : "actor render failed";
        value.commands = nullptr;
        value.input = nullptr;
        lua_settop(program.state, 0);
        remember(program.render_cache, key, {{}, error, false});
        return core::make_error(core::ErrorCode::ParseError, error);
    }
    // A falsy return is NSMBe's `customRendered = false`: whatever the script
    // drew stands, and the stock numbered marker is drawn over it as well.
    const bool custom = lua_toboolean(program.state, -1) != 0;
    if (custom_rendered) *custom_rendered = custom;
    value.commands = nullptr;
    value.input = nullptr;
    lua_settop(program.state, 0);
    remember(program.render_cache, key, {commands, {}, custom});
    return commands;
}

core::Result<std::optional<SpriteBounds>> lua_actor_bounds(const LuaActorDefinition& actor,
                                                           const LuaActorRenderInput& input,
                                                           bool render_bounds) {
    if (!actor.program) return std::optional<SpriteBounds>{};
    auto& program = *actor.program;
    const auto function = render_bounds && program.render_bounds != LUA_NOREF ? program.render_bounds : program.grab_bounds;
    if (function == LUA_NOREF) return std::optional<SpriteBounds>{};
    const BoundsCacheKey key{call_cache_key(input), function};
    if (const auto cached = program.bounds_cache.find(key); cached != program.bounds_cache.end()) {
        if (!cached->second.error.empty())
            return core::make_error(core::ErrorCode::ParseError, cached->second.error);
        return cached->second.bounds;
    }
    program.context.instructions = 0;
    program.context.input = &input;
    program.context.fields = &actor.sprite.fields;
    lua_settop(program.state, 0);
    lua_rawgeti(program.state, LUA_REGISTRYINDEX, function);
    push_actor(program.state, input);
    if (lua_pcall(program.state, 1, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(program.state, -1);
        const std::string error = message ? message : "actor bounds function failed";
        program.context.input = nullptr;
        lua_settop(program.state, 0);
        remember(program.bounds_cache, key, {std::nullopt, error});
        return core::make_error(core::ErrorCode::ParseError, error);
    }
    if (lua_isnil(program.state, -1)) {
        program.context.input = nullptr;
        lua_settop(program.state, 0);
        remember(program.bounds_cache, key, {std::nullopt, {}});
        return std::optional<SpriteBounds>{};
    }
    if (!lua_istable(program.state, -1)) {
        program.context.input = nullptr;
        lua_settop(program.state, 0);
        const auto error = actor.sprite.name + " bounds function must return a table or nil";
        remember(program.bounds_cache, key, {std::nullopt, error});
        return core::make_error(core::ErrorCode::ParseError, error);
    }
    const auto table = lua_gettop(program.state);
    auto x = bounds_number(program.state, table, "x", -32768, 32767, actor.sprite.name);
    auto y = bounds_number(program.state, table, "y", -32768, 32767, actor.sprite.name);
    auto width = bounds_number(program.state, table, "width", 1, 65536, actor.sprite.name);
    auto height = bounds_number(program.state, table, "height", 1, 65536, actor.sprite.name);
    if (!x || !y || !width || !height) {
        const auto error = !x ? x.error() : !y ? y.error() : !width ? width.error() : height.error();
        program.context.input = nullptr;
        lua_settop(program.state, 0);
        remember(program.bounds_cache, key, {std::nullopt, error.message});
        return error;
    }
    const SpriteBounds bounds{static_cast<std::int16_t>(x.value()), static_cast<std::int16_t>(y.value()),
                              static_cast<std::uint16_t>(width.value()), static_cast<std::uint16_t>(height.value())};
    program.context.input = nullptr;
    lua_settop(program.state, 0);
    remember(program.bounds_cache, key, {bounds, {}});
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
                                                    const ActorResourceResolver& resolve,
                                                    const ActorImageDecoder& decode_image) {
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
            auto actor = load_actor(script, layer.directory, resolve, decode_image, cache);
            if (!actor) return core::make_error(actor.error().code, layer.name + " / " + script.filename().string() + ": " + actor.error().message);
            const bool claimed = actor.value().object_id ? !object_ids.insert(*actor.value().object_id).second
                                                         : !hashes.insert(*actor.value().hash).second;
            if (!claimed) result.actors.push_back(std::move(actor.value()));
        }
    }
    return result;
}

} // namespace nsmb::project
