#pragma once

#include <filesystem>
#include <functional>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/project/sprite_definitions.hpp"

namespace nsmb::project {

enum class ActorResourceKind { Graphics, Palette, Model };

struct LuaActorDefinition {
    std::optional<std::uint32_t> object_id;
    std::optional<std::uint32_t> hash;
    SpriteDefinition sprite;
    std::filesystem::path graphics, palette, model, source;
};

struct LuaActorCatalog {
    std::vector<LuaActorDefinition> actors;

    [[nodiscard]] const LuaActorDefinition* by_object_id(std::uint32_t object_id) const;
    [[nodiscard]] const LuaActorDefinition* by_hash(std::uint32_t hash) const;
    [[nodiscard]] SpriteDefinitionCatalog sprite_definitions() const;
};

struct LuaActorLayer {
    std::string name;
    std::filesystem::path directory;
};

using ActorResourceResolver = std::function<core::Result<std::filesystem::path>(ActorResourceKind,
                                                                               std::string_view)>;

core::Result<LuaActorCatalog> load_lua_actor_layers(std::span<const LuaActorLayer> layers,
                                                     const ActorResourceResolver& resolve);

} // namespace nsmb::project
