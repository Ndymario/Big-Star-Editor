#include "lua_sandbox.hpp"

#include <algorithm>
#include <cstdlib>

extern "C" {
#include <lauxlib.h>
#include <lualib.h>
}

namespace nsmb::project::lua_sandbox {

void* allocate(void* opaque, void* pointer, std::size_t old_size, std::size_t new_size) {
    auto& memory = *static_cast<Memory*>(opaque);
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

core::Result<lua_Integer> integer_field(lua_State* state,
                                        int table,
                                        const char* key,
                                        lua_Integer minimum,
                                        lua_Integer maximum,
                                        std::string_view what,
                                        bool required,
                                        lua_Integer fallback) {
    lua_getfield(state, table, key);
    const bool missing = lua_isnil(state, -1);
    if (missing && !required) { lua_pop(state, 1); return fallback; }
    if (!lua_isinteger(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an invalid " + key);
    }
    const auto value = lua_tointeger(state, -1);
    lua_pop(state, 1);
    if (value < minimum || value > maximum)
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an invalid " + key);
    return value;
}

core::Result<std::string> string_field(lua_State* state,
                                       int table,
                                       const char* key,
                                       std::string_view what,
                                       bool required) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1) && !required) { lua_pop(state, 1); return std::string{}; }
    if (lua_type(state, -1) != LUA_TSTRING) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an invalid " + key);
    }
    std::size_t size{};
    const auto* value = lua_tolstring(state, -1, &size);
    std::string result(value, size);
    lua_pop(state, 1);
    if (required && result.empty())
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an empty " + key);
    return result;
}

core::Result<bool> boolean_field(lua_State* state,
                                 int table,
                                 const char* key,
                                 std::string_view what,
                                 bool fallback) {
    lua_getfield(state, table, key);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return fallback; }
    if (!lua_isboolean(state, -1)) {
        lua_pop(state, 1);
        return core::make_error(core::ErrorCode::ParseError, std::string(what) + " has an invalid " + key);
    }
    const bool value = lua_toboolean(state, -1);
    lua_pop(state, 1);
    return value;
}

} // namespace nsmb::project::lua_sandbox
