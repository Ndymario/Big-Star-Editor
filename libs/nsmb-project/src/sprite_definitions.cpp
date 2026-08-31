#include "nsmb/project/sprite_definitions.hpp"

#include <algorithm>
#include <limits>

namespace nsmb::project {
namespace {

std::uint64_t value_mask(const SpriteFieldDefinition& field) {
    return (std::uint64_t{1} << field.bit_width) - 1U;
}

} // namespace

const SpriteDefinition* SpriteDefinitionCatalog::by_object_id(std::uint32_t object_id) const {
    const auto found = std::ranges::find(definitions, object_id, &SpriteDefinition::object_id);
    return found == definitions.end() ? nullptr : &*found;
}

const SpriteDefinition* SpriteDefinitionCatalog::by_hash(std::uint32_t hash) const {
    const auto found = std::ranges::find(definitions, std::optional(hash), &SpriteDefinition::hash);
    return found == definitions.end() ? nullptr : &*found;
}

std::int32_t sprite_field_min(const SpriteFieldDefinition& field) {
    if (field.checkbox_value) return 0;
    const auto minimum = field.signed_value ? -(std::int64_t{1} << (field.bit_width - 1U)) : 0;
    return static_cast<std::int32_t>(minimum + field.value_offset);
}

std::int32_t sprite_field_max(const SpriteFieldDefinition& field) {
    if (field.checkbox_value) return 1;
    const auto maximum = field.signed_value ? (std::int64_t{1} << (field.bit_width - 1U)) - 1
                                            : static_cast<std::int64_t>(value_mask(field));
    return static_cast<std::int32_t>(maximum + field.value_offset);
}

std::int32_t read_sprite_field(std::uint64_t data, const SpriteFieldDefinition& field) {
    const auto raw = (data >> field.bit_offset) & value_mask(field);
    if (field.checkbox_value) return static_cast<std::int32_t>(raw / field.checkbox_value);
    std::int64_t value = static_cast<std::int64_t>(raw);
    if (field.signed_value && (raw & (std::uint64_t{1} << (field.bit_width - 1U))))
        value -= std::int64_t{1} << field.bit_width;
    return static_cast<std::int32_t>(value + field.value_offset);
}

std::uint64_t write_sprite_field(std::uint64_t data, const SpriteFieldDefinition& field, std::int32_t value) {
    const auto mask = value_mask(field);
    const auto raw = field.checkbox_value ? static_cast<std::int64_t>(value) * field.checkbox_value
                                          : static_cast<std::int64_t>(value) - field.value_offset;
    return (data & ~(mask << field.bit_offset)) | ((static_cast<std::uint64_t>(raw) & mask) << field.bit_offset);
}

} // namespace nsmb::project
