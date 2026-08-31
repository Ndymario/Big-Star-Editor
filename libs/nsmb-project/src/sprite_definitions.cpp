#include "nsmb/project/sprite_definitions.hpp"

#include <algorithm>
#include <limits>

namespace nsmb::project {
namespace {

std::uint32_t value_mask(const SpriteFieldDefinition& field) {
    return field.bit_width == 32 ? std::numeric_limits<std::uint32_t>::max()
                                 : (std::uint32_t{1} << field.bit_width) - 1U;
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

std::uint32_t read_sprite_field(std::uint32_t settings, const SpriteFieldDefinition& field) {
    return (settings >> field.bit_offset) & value_mask(field);
}

std::uint32_t write_sprite_field(std::uint32_t settings, const SpriteFieldDefinition& field, std::uint32_t value) {
    const auto mask = value_mask(field);
    return (settings & ~(mask << field.bit_offset)) | ((value & mask) << field.bit_offset);
}

} // namespace nsmb::project
