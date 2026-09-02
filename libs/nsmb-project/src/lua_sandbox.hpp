#pragma once

// The parts of a sandboxed Lua state that have nothing to do with what the
// script is for: the bounded allocator, the safe standard library, and the
// typed table readers every loader needs to report a bad field by name.
//
// Two loaders share this -- actors, which run code on every paint, and tiles,
// which are pure data -- so it holds no notion of a renderer, a resource
// context or a draw command.

#include <cstddef>
#include <string>
#include <string_view>

extern "C" {
#include <lua.h>
}

#include "nsmb/core/result.hpp"

namespace nsmb::project::lua_sandbox {

inline constexpr std::size_t max_script_bytes = 1024U * 1024U;
inline constexpr std::size_t max_lua_bytes = 8U * 1024U * 1024U;
inline constexpr std::size_t max_instructions = 200000U;

struct Memory {
    std::size_t used{};
};

void* allocate(void* opaque, void* pointer, std::size_t old_size, std::size_t new_size);

// Scripts format numbers into labels and resource names, so the pure parts of
// the standard library are available. Anything that reaches the filesystem, the
// process, or the loader stays out.
void expose_safe_library(lua_State* state);

// `what` names the thing being read in the error message -- an actor's name, a
// tile script's filename -- so a bad field says which file to open.
core::Result<lua_Integer> integer_field(lua_State* state,
                                        int table,
                                        const char* key,
                                        lua_Integer minimum,
                                        lua_Integer maximum,
                                        std::string_view what,
                                        bool required = true,
                                        lua_Integer fallback = 0);
core::Result<std::string> string_field(lua_State* state,
                                       int table,
                                       const char* key,
                                       std::string_view what,
                                       bool required = true);
core::Result<bool> boolean_field(lua_State* state,
                                 int table,
                                 const char* key,
                                 std::string_view what,
                                 bool fallback = false);

} // namespace nsmb::project::lua_sandbox
