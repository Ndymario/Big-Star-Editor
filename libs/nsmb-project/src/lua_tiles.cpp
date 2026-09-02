#include "nsmb/project/lua_tiles.hpp"

#include <algorithm>
#include <limits>
#include <string_view>
#include <unordered_set>

extern "C" {
#include <lauxlib.h>
#include <lua.h>
}

#include "lua_sandbox.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/formats/tex0.hpp"

namespace nsmb::project {

namespace {

using namespace lua_sandbox;

// Map16 block numbers are absolute across the three tilesets a level loads:
// jyotyu 0-255, the level tileset 256-1023, nohara 1024-1535.
constexpr lua_Integer max_block = 1535;

// Keyed on the resolved resource plus everything that changes the pixels, so
// the three pipe colours -- the same texture at three palette offsets -- decode
// once each and the whole of `I_dokan.nsbtx` decodes once.
struct TextureKey {
    std::string resource, texture;
    std::size_t palette{}, palette_offset{};

    bool operator==(const TextureKey&) const = default;
};

struct TextureKeyHash {
    std::size_t operator()(const TextureKey& key) const noexcept {
        std::size_t seed = std::hash<std::string>{}(key.resource);
        for (const auto value : {std::hash<std::string>{}(key.texture), key.palette, key.palette_offset})
            seed ^= value + 0x9e3779b97f4a7c15ULL + (seed << 6U) + (seed >> 2U);
        return seed;
    }
};

struct TextureCache {
    std::unordered_map<TextureKey, std::shared_ptr<const formats::RgbaImage>, TextureKeyHash> images;
    std::unordered_map<std::string, std::shared_ptr<const formats::Tex0>> containers;
};

core::Result<std::vector<std::uint8_t>> tile_resource_bytes(const std::vector<std::uint8_t>& bytes) {
    if (!bytes.empty() && (bytes.front() == 0x10 || bytes.front() == 0x11))
        return formats::decompress_nitro_lz(bytes);
    return bytes;
}

// What one `blocks` entry asks for, before the texture is resolved.
struct BlockRequest {
    std::string file, texture;
    std::size_t texture_index{};
    bool texture_by_name{};
    std::size_t palette{}, palette_offset{};
    std::optional<std::uint16_t> remap;
    bool draws{};  // false when the entry is empty: claimed, drawn as nothing
};

core::Result<std::shared_ptr<const formats::RgbaImage>> resolve_texture(const ActorResourceResolver& resolve,
                                                                        const BlockRequest& request,
                                                                        TextureCache& cache,
                                                                        std::vector<std::filesystem::path>& watched) {
    auto found = resolve(ActorResourceKind::Model, request.file);
    if (!found) return found.error();
    // The address when the ROM supplies this, the disk path when a module does.
    const auto resource_key = found.value().source.empty() ? found.value().rom_path
                                                           : found.value().source.string();
    if (!found.value().source.empty()) watched.push_back(found.value().source);

    auto container = cache.containers.find(resource_key);
    if (container == cache.containers.end()) {
        auto bytes = tile_resource_bytes(found.value().data);
        if (!bytes) return bytes.error();
        auto parsed = formats::parse_tex0(bytes.value());
        if (!parsed) return parsed.error();
        container = cache.containers.emplace(resource_key, std::make_shared<const formats::Tex0>(std::move(parsed.value()))).first;
    }
    const auto& tex0 = *container->second;

    std::size_t index = request.texture_index;
    if (request.texture_by_name) {
        const auto match = std::ranges::find(tex0.textures, request.texture, &formats::Tex0Texture::name);
        if (match == tex0.textures.end())
            return core::make_error(core::ErrorCode::NotFound,
                                    request.file + " has no texture named " + request.texture);
        index = static_cast<std::size_t>(match - tex0.textures.begin());
    }
    if (index >= tex0.textures.size() || request.palette >= tex0.palettes.size())
        return core::make_error(core::ErrorCode::OutOfBounds, request.file + " has no such texture or palette");

    const TextureKey key{resource_key, tex0.textures[index].name, request.palette, request.palette_offset};
    if (const auto cached = cache.images.find(key); cached != cache.images.end()) return cached->second;

    // A palette offset selects a band inside one palette rather than another
    // palette: the green, red and yellow pipes are the same texels read 0, 256
    // and 512 colours into `I_dokan`'s single 768-colour palette.
    auto palette = tex0.palettes[request.palette];
    const auto skip = request.palette_offset * 2U;
    if (skip >= palette.colors.size())
        return core::make_error(core::ErrorCode::OutOfBounds,
                                request.file + " palette offset is past the end of the palette");
    palette.colors.erase(palette.colors.begin(), palette.colors.begin() + static_cast<std::ptrdiff_t>(skip));

    auto image = formats::render_tex0_texture(tex0.textures[index], palette);
    if (!image) return image.error();
    auto shared = std::make_shared<const formats::RgbaImage>(std::move(image.value()));
    cache.images.emplace(key, shared);
    return shared;
}

core::Result<BlockRequest> read_block_request(lua_State* state, int entry, std::string_view what) {
    BlockRequest request;
    if (auto remap = integer_field(state, entry, "remap", 0, max_block, what, false, -1); !remap) return remap.error();
    else if (remap.value() >= 0) { request.remap = static_cast<std::uint16_t>(remap.value()); return request; }

    lua_rawgeti(state, entry, 1);
    const bool has_file = !lua_isnil(state, -1);
    lua_pop(state, 1);
    // An empty entry claims the block and draws nothing, which is what the game
    // does for the thin fences the boss tilesets blank outright.
    if (!has_file) return request;

    lua_rawgeti(state, entry, 1);
    if (lua_type(state, -1) != LUA_TSTRING) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " needs a file name");
    }
    request.file = lua_tostring(state, -1);
    lua_pop(state, 1);

    lua_rawgeti(state, entry, 2);
    if (lua_type(state, -1) == LUA_TSTRING) { request.texture = lua_tostring(state, -1); request.texture_by_name = true; }
    else if (lua_isinteger(state, -1)) {
        const auto index = lua_tointeger(state, -1);
        if (index < 0 || index > 65535) { lua_pop(state, 1); return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an invalid texture"); }
        request.texture_index = static_cast<std::size_t>(index);
    } else if (!lua_isnil(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an invalid texture");
    }
    lua_pop(state, 1);

    auto palette = integer_field(state, entry, "paletteIndex", 0, 65535, what, false, 0);
    if (!palette) return palette.error();
    request.palette = static_cast<std::size_t>(palette.value());
    auto offset = integer_field(state, entry, "palette", 0, 65535, what, false, 0);
    if (!offset) return offset.error();
    request.palette_offset = static_cast<std::size_t>(offset.value());
    request.draws = true;
    return request;
}

core::Result<TileOverlayScript> load_script(const std::filesystem::path& path,
                                           const ActorResourceResolver& resolve,
                                           TextureCache& cache) {
    auto bytes = core::read_file(path);
    if (!bytes) return bytes.error();
    if (bytes.value().size() > max_script_bytes)
        return core::make_error(core::ErrorCode::OutOfBounds, "tile script is larger than 1 MiB");

    Memory memory;
#if LUA_VERSION_NUM >= 505
    lua_State* state = lua_newstate(allocate, &memory, 0);
#else
    lua_State* state = lua_newstate(allocate, &memory);
#endif
    if (!state) return core::make_error(core::ErrorCode::IoError, "could not create the Lua tile state");
    // A tile script is data, so the state dies with this call and nothing has to
    // be kept alive for painting.
    struct Closer { lua_State* state; ~Closer() { lua_close(state); } } closer{state};
    expose_safe_library(state);

    const std::string chunk = "@" + path.string();
    if (luaL_loadbufferx(state, reinterpret_cast<const char*>(bytes.value().data()), bytes.value().size(), chunk.c_str(), "t") != LUA_OK ||
        lua_pcall(state, 0, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(state, -1);
        return core::make_error(core::ErrorCode::ParseError, message ? message : "tile script failed");
    }
    if (!lua_istable(state, -1)) return core::make_error(core::ErrorCode::ParseError, "tile script must return a table");
    const auto table = lua_gettop(state);

    TileOverlayScript script;
    script.source = path;
    auto name = string_field(state, table, "name", path.filename().string());
    if (!name) return name.error();
    script.name = std::move(name.value());
    auto replaces = boolean_field(state, table, "replaces", script.name, true);
    if (!replaces) return replaces.error();

    lua_getfield(state, table, "tilesets");
    if (!lua_isnil(state, -1)) {
        if (!lua_istable(state, -1)) return core::make_error(core::ErrorCode::ParseError, script.name + " has an invalid tilesets");
        const auto count = lua_rawlen(state, -1);
        for (lua_Unsigned i = 1; i <= count; ++i) {
            lua_rawgeti(state, -1, static_cast<lua_Integer>(i));
            if (!lua_isinteger(state, -1) || lua_tointeger(state, -1) < 0 || lua_tointeger(state, -1) > 65535)
                return core::make_error(core::ErrorCode::ParseError, script.name + " has an invalid tilesets entry");
            script.tilesets.push_back(static_cast<std::uint16_t>(lua_tointeger(state, -1)));
            lua_pop(state, 1);
        }
        // An empty list would otherwise read as "every tileset", which is the
        // opposite of what writing one means.
        if (script.tilesets.empty())
            return core::make_error(core::ErrorCode::ParseError, script.name + " lists no tilesets; omit the field to mean every tileset");
    }
    lua_pop(state, 1);

    lua_getfield(state, table, "blocks");
    if (!lua_istable(state, -1)) return core::make_error(core::ErrorCode::ParseError, script.name + " needs a blocks table");
    const auto blocks = lua_gettop(state);
    lua_pushnil(state);
    while (lua_next(state, blocks)) {
        if (!lua_isinteger(state, -2) || lua_tointeger(state, -2) < 0 || lua_tointeger(state, -2) > max_block)
            return core::make_error(core::ErrorCode::ParseError, script.name + " has a block number outside 0-1535");
        const auto block = static_cast<std::uint16_t>(lua_tointeger(state, -2));
        if (!lua_istable(state, -1))
            return core::make_error(core::ErrorCode::ParseError, script.name + " block " + std::to_string(block) + " must be a table");
        const auto what = script.name + " block " + std::to_string(block);
        auto request = read_block_request(state, lua_gettop(state), what);
        if (!request) return request.error();

        TileOverlay overlay;
        overlay.replaces = replaces.value();
        overlay.remap = request.value().remap;
        if (request.value().draws) {
            auto image = resolve_texture(resolve, request.value(), cache, script.resources);
            // A texture the ROM does not hold costs that block and nothing else,
            // so a module missing one asset still contributes the rest.
            if (!image) script.errors.push_back(what + ": " + image.error().message);
            else overlay.image = std::move(image.value());
        }
        script.blocks.emplace(block, std::move(overlay));
        lua_pop(state, 1);
    }
    lua_pop(state, 1);

    std::ranges::sort(script.resources);
    script.resources.erase(std::unique(script.resources.begin(), script.resources.end()), script.resources.end());
    return script;
}

// Nearest-neighbour, because a 3D tile texture is 16x16 and so is the block it
// stands in: this is a copy in every case the ROM presents, and stays correct
// for a module that ships something else.
void blit_over(formats::RgbaImage& target,
               std::size_t left,
               std::size_t top,
               std::size_t width,
               std::size_t height,
               const formats::RgbaImage& source) {
    if (!width || !height || !source.width || !source.height) return;
    for (std::size_t y = 0; y < height; ++y) {
        const auto sy = y * source.height / height;
        for (std::size_t x = 0; x < width; ++x) {
            const auto sx = x * source.width / width;
            const auto* from = &source.pixels[(sy * source.width + sx) * 4U];
            if (!from[3]) continue;
            std::copy_n(from, 4U, target.pixels.begin() + static_cast<std::ptrdiff_t>(((top + y) * target.width + left + x) * 4U));
        }
    }
}

void clear_rect(formats::RgbaImage& target, std::size_t left, std::size_t top, std::size_t width, std::size_t height) {
    for (std::size_t y = 0; y < height; ++y)
        std::fill_n(target.pixels.begin() + static_cast<std::ptrdiff_t>(((top + y) * target.width + left) * 4U), width * 4U, std::uint8_t{0});
}

} // namespace

const TileOverlay* TileOverlayCatalog::find(std::uint16_t tileset_id, std::uint16_t block) const {
    for (const auto& script : scripts) {
        if (!script.tilesets.empty() && std::ranges::find(script.tilesets, tileset_id) == script.tilesets.end()) continue;
        if (const auto found = script.blocks.find(block); found != script.blocks.end()) return &found->second;
    }
    return nullptr;
}

bool TileOverlayCatalog::empty() const noexcept {
    return std::ranges::all_of(scripts, [](const auto& script) { return script.blocks.empty(); });
}

std::size_t TileOverlayCatalog::block_count(std::uint16_t tileset_id) const {
    std::unordered_set<std::uint16_t> seen;
    for (const auto& script : scripts) {
        if (!script.tilesets.empty() && std::ranges::find(script.tilesets, tileset_id) == script.tilesets.end()) continue;
        for (const auto& [block, overlay] : script.blocks) { (void)overlay; seen.insert(block); }
    }
    return seen.size();
}

core::Result<TileOverlayCatalog> load_lua_tile_layers(std::span<const LuaTileLayer> layers,
                                                      const ActorResourceResolver& resolve) {
    TileOverlayCatalog result;
    TextureCache cache;
    for (const auto& layer : layers) {
        std::error_code ec;
        if (!std::filesystem::exists(layer.directory, ec)) {
            if (ec) return core::make_error(core::ErrorCode::IoError, "could not inspect tile layer " + layer.name);
            continue;
        }
        if (!std::filesystem::is_directory(layer.directory, ec) || ec)
            return core::make_error(core::ErrorCode::InvalidArgument, "tile layer is not a directory: " + layer.name);
        std::vector<std::filesystem::path> scripts;
        for (const auto& entry : std::filesystem::directory_iterator(layer.directory, ec))
            if (entry.is_regular_file(ec) && entry.path().extension() == ".lua") scripts.push_back(entry.path());
        if (ec) return core::make_error(core::ErrorCode::IoError, "could not read tile layer " + layer.name);
        std::ranges::sort(scripts);
        for (const auto& script : scripts) {
            auto loaded = load_script(script, resolve, cache);
            if (!loaded) return core::make_error(loaded.error().code,
                                                 layer.name + " / " + script.filename().string() + ": " + loaded.error().message);
            result.scripts.push_back(std::move(loaded.value()));
        }
    }
    return result;
}

void overlay_map16_images(std::vector<formats::RgbaImage>& blocks,
                          std::uint16_t tileset_id,
                          std::uint16_t page_offset,
                          const TileOverlayCatalog& overlays) {
    // Resolved against the Map16 as the files describe it, then applied in one
    // pass, so a block the game rewrites to another block picks up that block's
    // own graphics rather than whatever replaced them.
    std::vector<std::pair<std::size_t, formats::RgbaImage>> replacements;
    for (std::size_t i = 0; i < blocks.size(); ++i) {
        const auto* overlay = overlays.find(tileset_id, static_cast<std::uint16_t>(page_offset + i));
        if (!overlay) continue;
        auto& block = blocks[i];
        if (overlay->remap) {
            const auto from = static_cast<std::size_t>(*overlay->remap);
            formats::RgbaImage image{block.width, block.height, {}};
            image.pixels.assign(block.width * block.height * 4U, 0);
            if (from >= page_offset && from - page_offset < blocks.size()) image = blocks[from - page_offset];
            replacements.emplace_back(i, std::move(image));
        } else if (overlay->replaces) {
            formats::RgbaImage image{block.width, block.height, {}};
            image.pixels.assign(block.width * block.height * 4U, 0);
            if (overlay->image) blit_over(image, 0, 0, image.width, image.height, *overlay->image);
            replacements.emplace_back(i, std::move(image));
        } else if (overlay->image) {
            auto image = block;
            blit_over(image, 0, 0, image.width, image.height, *overlay->image);
            replacements.emplace_back(i, std::move(image));
        }
    }
    for (auto& [index, image] : replacements) blocks[index] = std::move(image);
}

void overlay_map16_sheet(formats::RgbaImage& sheet,
                         std::size_t blocks_wide,
                         std::size_t block_count,
                         std::uint16_t tileset_id,
                         std::uint16_t page_offset,
                         const TileOverlayCatalog& overlays) {
    if (!blocks_wide || !sheet.width || !sheet.height) return;
    const auto cell = sheet.width / blocks_wide;
    if (!cell) return;
    // Remaps read the sheet before anything is drawn on it, for the same reason
    // as above.
    const auto original = sheet;
    for (std::size_t i = 0; i < block_count; ++i) {
        const auto* overlay = overlays.find(tileset_id, static_cast<std::uint16_t>(page_offset + i));
        if (!overlay) continue;
        const auto left = (i % blocks_wide) * cell, top = (i / blocks_wide) * cell;
        if (top + cell > sheet.height) break;
        if (overlay->remap) {
            clear_rect(sheet, left, top, cell, cell);
            const auto from = static_cast<std::size_t>(*overlay->remap);
            if (from < page_offset || from - page_offset >= block_count) continue;
            const auto index = from - page_offset;
            const auto source_left = (index % blocks_wide) * cell, source_top = (index / blocks_wide) * cell;
            if (source_top + cell > original.height) continue;
            for (std::size_t y = 0; y < cell; ++y)
                std::copy_n(original.pixels.begin() + static_cast<std::ptrdiff_t>(((source_top + y) * original.width + source_left) * 4U),
                            cell * 4U,
                            sheet.pixels.begin() + static_cast<std::ptrdiff_t>(((top + y) * sheet.width + left) * 4U));
        } else if (overlay->replaces) {
            clear_rect(sheet, left, top, cell, cell);
            if (overlay->image) blit_over(sheet, left, top, cell, cell, *overlay->image);
        } else if (overlay->image) {
            blit_over(sheet, left, top, cell, cell, *overlay->image);
        }
    }
}

} // namespace nsmb::project
