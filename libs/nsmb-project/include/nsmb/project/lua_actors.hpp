#pragma once

#include <array>
#include <cstdint>
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

// Graphics and Palette name indexed ROM resources, Model a TEX0 container, and
// Image a still that ships beside the actor script rather than in the ROM.
enum class ActorResourceKind { Graphics, Palette, Model, Image };

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

// The stock editor draws more than stills: a few actors outline their reach with
// bars, rings and lines, so shapes are commands too rather than something a
// script has to fake with a one-pixel image.
enum class LuaActorDrawKind { Image, Text, FillRect, StrokeRect, FillEllipse, Line };

// Every command shares the destination rectangle so the painter treats them
// uniformly; `image`, `text`, `color` and the line endpoint are the parts only
// some kinds use.
struct LuaActorDrawCommand {
    LuaActorDrawKind kind{LuaActorDrawKind::Image};
    std::shared_ptr<const formats::RgbaImage> image;
    std::string text;
    std::int32_t source_x{}, source_y{};
    std::uint32_t source_width{}, source_height{};
    double x{}, y{}, width{}, height{}, rotation{}, opacity{1.0};
    // Line endpoint, in scene pixels, for LuaActorDrawKind::Line.
    double x2{}, y2{};
    // Stroke thickness for StrokeRect and Line.
    double stroke_width{1.0};
    // Non-premultiplied 0xRRGGBBAA.
    std::uint32_t color{0xFFFFFFFFU};
    bool flip_x{}, flip_y{};
};

struct LuaActorDefinition {
    std::optional<std::uint32_t> object_id;
    std::optional<std::uint32_t> hash;
    SpriteDefinition sprite;
    std::filesystem::path source;
    std::vector<std::filesystem::path> render_resources;
    // Set when init() failed: the actor keeps its metadata and loses its
    // renderer, rather than taking the whole catalog down with it.
    std::string init_error;
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

// What the host found for a resource name. The address is what identifies the
// format and keys the cache; `source` is the file on disk it came from, empty
// when the ROM supplied it and therefore nothing for a watcher to follow.
struct ActorResource {
    std::string rom_path;
    std::filesystem::path source;
    std::vector<std::uint8_t> data;
};

using ActorResourceResolver = std::function<core::Result<ActorResource>(ActorResourceKind,
                                                                       std::string_view)>;
// nsmb-project stays free of an image codec, so the host injects one. The Qt
// app supplies a QImage-backed decoder; headless callers may supply none, in
// which case loadImage reports every still as unavailable.
using ActorImageDecoder = std::function<core::Result<formats::RgbaImage>(std::span<const std::uint8_t>)>;

core::Result<LuaActorCatalog> load_lua_actor_layers(std::span<const LuaActorLayer> layers,
                                                    const ActorResourceResolver& resolve,
                                                    const ActorImageDecoder& decode_image = {});
[[nodiscard]] bool has_lua_renderer(const LuaActorDefinition& actor);
// `custom_rendered` receives what render() returned: false means the script drew
// something but still wants the stock numbered marker, the way NSMBe's
// `customRendered = false` cases do.
core::Result<std::vector<LuaActorDrawCommand>> render_lua_actor(const LuaActorDefinition& actor,
                                                                const LuaActorRenderInput& input,
                                                                bool* custom_rendered = nullptr);
// Returns the actor's box in scene pixels, or nothing when the actor declares no
// bounds function.
core::Result<std::optional<SpriteBounds>> lua_actor_bounds(const LuaActorDefinition& actor,
                                                           const LuaActorRenderInput& input,
                                                           bool render_bounds = false);

} // namespace nsmb::project
