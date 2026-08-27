#pragma once

#include <cstdint>
#include <filesystem>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::project {

struct SpriteBounds {
    std::int16_t x{-8}, y{-8};
    std::uint16_t width{16}, height{16};
};

enum class SpriteShape { Box, Ellipse, Diamond };

struct SpriteFieldChoice {
    std::uint32_t value{};
    std::string label;
};

struct SpriteFieldDefinition {
    std::string key, label;
    std::uint8_t bit_offset{}, bit_width{1};
    std::vector<SpriteFieldChoice> choices;
};

struct SpriteDefinition {
    std::uint32_t object_id{};
    std::string name, category{"Other"}, color{"#eeB24b"};
    SpriteBounds grab_bounds, render_bounds;
    SpriteShape shape{SpriteShape::Diamond};
    std::vector<SpriteFieldDefinition> fields;
};

struct SpriteDefinitionCatalog {
    std::vector<SpriteDefinition> definitions;

    [[nodiscard]] const SpriteDefinition* by_object_id(std::uint32_t object_id) const;
};

struct SpriteDefinitionLayer {
    std::string name;
    std::filesystem::path path;
};

core::Result<std::vector<SpriteDefinition>> parse_sprite_definitions(std::string_view json);
core::Result<SpriteDefinitionCatalog> load_sprite_definition_layers(std::span<const SpriteDefinitionLayer> layers);

[[nodiscard]] std::uint32_t read_sprite_field(std::uint32_t settings, const SpriteFieldDefinition& field);
[[nodiscard]] std::uint32_t write_sprite_field(std::uint32_t settings,
                                               const SpriteFieldDefinition& field,
                                               std::uint32_t value);

} // namespace nsmb::project
