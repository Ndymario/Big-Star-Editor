#pragma once

#include <array>
#include <filesystem>
#include <functional>
#include <memory>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"
#include "nsmb/formats/graphics.hpp"
#include "nsmb/project/sprite_definitions.hpp"

namespace nsmb::project {

enum class ActorResourceKind { Graphics, Palette, Model };

struct LuaActorProgram;

struct LuaActorRenderInput {
    std::int32_t x{}, y{};
    double pixel_x{}, pixel_y{};
    std::uint32_t object_id{};
    std::uint16_t type{};
    std::optional<std::uint32_t> hash;
    std::array<std::uint8_t, 6> data{};
    std::uint32_t settings{};
    bool valid{true};
};

struct LuaActorDrawCommand {
    std::shared_ptr<const formats::RgbaImage> image;
    std::int32_t source_x{}, source_y{};
    std::uint32_t source_width{}, source_height{};
    double x{}, y{}, width{}, height{}, rotation{}, opacity{1.0};
    bool flip_x{}, flip_y{};
};

struct LuaActorDefinition {
    std::optional<std::uint32_t> object_id;
    std::optional<std::uint32_t> hash;
    SpriteDefinition sprite;
    std::filesystem::path graphics, palette, model, source;
    std::vector<std::filesystem::path> render_resources;
    std::shared_ptr<LuaActorProgram> program;
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
[[nodiscard]] bool has_lua_renderer(const LuaActorDefinition& actor);
core::Result<std::vector<LuaActorDrawCommand>> render_lua_actor(const LuaActorDefinition& actor,
                                                                const LuaActorRenderInput& input);
core::Result<std::optional<SpriteBounds>> lua_actor_bounds(const LuaActorDefinition& actor,
                                                           const LuaActorRenderInput& input,
                                                           bool render_bounds = false);

} // namespace nsmb::project
