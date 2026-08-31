#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <vector>

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
    std::string notes;
    std::int32_t value_offset{};
    std::uint32_t checkbox_value{};
    bool signed_value{};
};

struct SpriteDefinition {
    std::uint32_t object_id{};
    std::optional<std::uint32_t> hash;
    std::string name, category{"Other"}, color{"#eeB24b"}, notes;
    bool known{}, complete{};
    std::vector<std::string> source_files;
    SpriteBounds grab_bounds, render_bounds;
    SpriteShape shape{SpriteShape::Diamond};
    std::vector<SpriteFieldDefinition> fields;
};

struct SpriteDefinitionCatalog {
    std::vector<SpriteDefinition> definitions;

    [[nodiscard]] const SpriteDefinition* by_object_id(std::uint32_t object_id) const;
    [[nodiscard]] const SpriteDefinition* by_hash(std::uint32_t hash) const;
};

[[nodiscard]] std::int32_t sprite_field_min(const SpriteFieldDefinition& field);
[[nodiscard]] std::int32_t sprite_field_max(const SpriteFieldDefinition& field);
[[nodiscard]] std::int32_t read_sprite_field(std::uint64_t data, const SpriteFieldDefinition& field);
[[nodiscard]] std::uint64_t write_sprite_field(std::uint64_t data,
                                               const SpriteFieldDefinition& field,
                                               std::int32_t value);

} // namespace nsmb::project
