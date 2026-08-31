#include "nsmb/project/lua_actors.hpp"

#include <algorithm>
#include <cctype>
#include <cstdlib>
#include <limits>
#include <unordered_set>

extern "C" {
#include <lua.h>
#include <lauxlib.h>
}

#include "nsmb/core/file_io.hpp"

namespace nsmb::project {
namespace {

constexpr std::size_t max_script_bytes = 1024U * 1024U;
constexpr std::size_t max_lua_bytes = 8U * 1024U * 1024U;
constexpr std::size_t max_instructions = 200000U;

struct LuaMemory {
    std::size_t used{};
};

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

struct ResourceRef {
    ActorResourceKind kind{};
    std::filesystem::path path;
};

struct LuaContext {
    const ActorResourceResolver* resolve{};
    std::vector<ResourceRef> resources;
    std::size_t instructions{};
    std::string resource_error;
};

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
    if (lua_type(state, 1) != LUA_TSTRING) return luaL_error(state, "resource filename must be a string");
    const auto* name = luaL_checkstring(state, 1);
    auto resolved = (*value.resolve)(kind, name);
    if (!resolved) {
        value.resource_error = resolved.error().message;
        lua_pushnil(state);
        return 1;
    }
    auto* resource = static_cast<std::size_t*>(lua_newuserdatauv(state, sizeof(std::size_t), 0));
    *resource = value.resources.size();
    value.resources.push_back({kind, std::move(resolved.value())});
    return 1;
}

void expose_reader(lua_State* state, const char* name, ActorResourceKind kind) {
    lua_pushinteger(state, static_cast<lua_Integer>(kind));
    lua_pushcclosure(state, read_resource, 1);
    lua_setglobal(state, name);
}

core::Result<lua_Integer> integer_field(lua_State* state,
                                        int table,
                                        const char* key,
                                        lua_Integer minimum,
                                        lua_Integer maximum,
                                        std::string_view actor,
                                        bool required = true) {
    lua_getfield(state, table, key);
    const bool missing = lua_isnil(state, -1);
    if (missing && !required) { lua_pop(state, 1); return minimum; }
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
    if (index >= context.resources.size() || context.resources[index].kind != kind)
        return core::make_error(core::ErrorCode::ParseError, std::string(actor) + " has the wrong resource type in " + key);
    return context.resources[index].path;
}

std::uint32_t field_mask(const SpriteFieldDefinition& field) {
    return field.bit_width == 32 ? std::numeric_limits<std::uint32_t>::max() : (std::uint32_t{1} << field.bit_width) - 1U;
}

core::Result<void> read_fields(lua_State* state, int table, LuaActorDefinition& actor) {
    lua_getfield(state, table, "fields");
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return {}; }
    if (!lua_istable(state, -1)) { lua_pop(state, 1); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has invalid fields"); }
    const auto fields_table = lua_gettop(state);
    std::uint32_t used_bits{};
    std::unordered_set<std::string> keys;
    const auto count = lua_rawlen(state, fields_table);
    for (std::size_t i = 1; i <= count; ++i) {
        lua_rawgeti(state, fields_table, static_cast<lua_Integer>(i));
        if (!lua_istable(state, -1)) { lua_pop(state, 2); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has an invalid field"); }
        const auto field_table = lua_gettop(state);
        SpriteFieldDefinition field;
        auto key = string_field(state, field_table, "key", actor.sprite.name); if (!key) { lua_pop(state, 2); return key.error(); } field.key = std::move(key.value());
        auto label = string_field(state, field_table, "label", actor.sprite.name); if (!label) { lua_pop(state, 2); return label.error(); } field.label = std::move(label.value());
        if (!keys.insert(field.key).second) { lua_pop(state, 2); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has a duplicate field key"); }
        auto offset = integer_field(state, field_table, "bitOffset", 0, 31, field.key); if (!offset) { lua_pop(state, 2); return offset.error(); }
        auto width = integer_field(state, field_table, "bitWidth", 1, std::min<lua_Integer>(16, 32 - offset.value()), field.key); if (!width) { lua_pop(state, 2); return width.error(); }
        field.bit_offset = static_cast<std::uint8_t>(offset.value()); field.bit_width = static_cast<std::uint8_t>(width.value());
        const auto mask = field_mask(field) << field.bit_offset;
        if (used_bits & mask) { lua_pop(state, 2); return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " has overlapping fields"); }
        used_bits |= mask;
        lua_getfield(state, field_table, "choices");
        if (!lua_isnil(state, -1)) {
            if (!lua_istable(state, -1)) { lua_pop(state, 3); return core::make_error(core::ErrorCode::ParseError, field.key + " has invalid choices"); }
            const auto choices_table = lua_gettop(state);
            for (std::size_t choice = 1; choice <= lua_rawlen(state, choices_table); ++choice) {
                lua_rawgeti(state, choices_table, static_cast<lua_Integer>(choice));
                if (!lua_istable(state, -1)) { lua_pop(state, 4); return core::make_error(core::ErrorCode::ParseError, field.key + " has an invalid choice"); }
                const auto choice_table = lua_gettop(state);
                auto value = integer_field(state, choice_table, "value", 0, field_mask(field), field.key); if (!value) { lua_pop(state, 4); return value.error(); }
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

core::Result<LuaActorDefinition> load_actor(const std::filesystem::path& path,
                                            const ActorResourceResolver& resolve) {
    auto bytes = core::read_file(path);
    if (!bytes) return bytes.error();
    if (bytes.value().size() > max_script_bytes)
        return core::make_error(core::ErrorCode::OutOfBounds, "actor script is larger than 1 MiB");
    LuaMemory memory;
#if LUA_VERSION_NUM >= 505
    lua_State* state = lua_newstate(allocate, &memory, 0);
#else
    lua_State* state = lua_newstate(allocate, &memory);
#endif
    if (!state) return core::make_error(core::ErrorCode::IoError, "could not create the Lua actor state");
    struct Close { lua_State* state; ~Close() { lua_close(state); } } close{state};
    LuaContext value{&resolve};
    *static_cast<LuaContext**>(lua_getextraspace(state)) = &value;
    expose_reader(state, "readGraphics", ActorResourceKind::Graphics);
    expose_reader(state, "readPalette", ActorResourceKind::Palette);
    expose_reader(state, "readModel", ActorResourceKind::Model);
    lua_sethook(state, instruction_hook, LUA_MASKCOUNT, 1000);
    const std::string chunk = "@" + path.string();
    if (luaL_loadbufferx(state, reinterpret_cast<const char*>(bytes.value().data()), bytes.value().size(), chunk.c_str(), "t") != LUA_OK ||
        lua_pcall(state, 0, 1, 0) != LUA_OK) {
        const auto* message = lua_tostring(state, -1);
        return core::make_error(core::ErrorCode::ParseError, message ? message : "actor script failed");
    }
    if (!value.resource_error.empty()) return core::make_error(core::ErrorCode::NotFound, value.resource_error);
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
    if (auto fields = read_fields(state, table, actor); !fields) return fields.error();
    auto graphics = resource_field(state, table, "graphics", ActorResourceKind::Graphics, value, actor.sprite.name); if (!graphics) return graphics.error(); actor.graphics = std::move(graphics.value());
    auto palette = resource_field(state, table, "palette", ActorResourceKind::Palette, value, actor.sprite.name); if (!palette) return palette.error(); actor.palette = std::move(palette.value());
    auto model = resource_field(state, table, "model", ActorResourceKind::Model, value, actor.sprite.name); if (!model) return model.error(); actor.model = std::move(model.value());
    if (actor.graphics.empty() != actor.palette.empty())
        return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " must define graphics and palette together");
    if (!actor.model.empty() && !actor.graphics.empty())
        return core::make_error(core::ErrorCode::ParseError, actor.sprite.name + " cannot define both a model and 2D graphics");
    return actor;
}

} // namespace

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
            auto actor = load_actor(script, resolve);
            if (!actor) return core::make_error(actor.error().code, layer.name + " / " + script.filename().string() + ": " + actor.error().message);
            const bool claimed = actor.value().object_id ? !object_ids.insert(*actor.value().object_id).second
                                                         : !hashes.insert(*actor.value().hash).second;
            if (!claimed) result.actors.push_back(std::move(actor.value()));
        }
    }
    return result;
}

} // namespace nsmb::project
