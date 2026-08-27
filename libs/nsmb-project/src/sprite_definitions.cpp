#include "nsmb/project/sprite_definitions.hpp"

#include "nsmb/core/file_io.hpp"
#include "nsmb/core/json.hpp"

#include <algorithm>
#include <initializer_list>
#include <limits>
#include <unordered_set>

namespace nsmb::project {
namespace {

core::Result<void> only(const core::JsonValue& node,
                        std::initializer_list<std::string_view> allowed,
                        std::string_view context) {
    for (const auto& [key, value] : node.members()) {
        (void)value;
        if (std::ranges::find(allowed, key) == allowed.end())
            return core::make_error(core::ErrorCode::ParseError,
                                    std::string(context) + " contains unknown property " + key);
    }
    return {};
}

core::Result<std::string> string_value(const core::JsonValue& node,
                                       std::string_view key,
                                       std::string_view context,
                                       bool required = true) {
    if (!node[key].is_string() || (required && node[key].as_string().empty()))
        return core::make_error(core::ErrorCode::ParseError,
                                std::string(context) + " has an invalid " + std::string(key));
    return node[key].as_string();
}

core::Result<std::int64_t> integer(const core::JsonValue& node,
                                   std::string_view key,
                                   std::int64_t minimum,
                                   std::int64_t maximum,
                                   std::string_view context) {
    const auto value = node[key].as_int64(minimum - 1);
    if (!node[key].is_number() || value < minimum || value > maximum)
        return core::make_error(core::ErrorCode::ParseError,
                                std::string(context) + " has an invalid " + std::string(key));
    return value;
}

core::Result<SpriteBounds> bounds(const core::JsonValue& node, std::string_view context) {
    if (!node.is_object())
        return core::make_error(core::ErrorCode::ParseError, std::string(context) + " is not an object");
    if (auto ok = only(node, {"x", "y", "width", "height"}, context); !ok) return ok.error();
    SpriteBounds result;
    auto x = integer(node, "x", -32768, 32767, context); if (!x) return x.error();
    auto y = integer(node, "y", -32768, 32767, context); if (!y) return y.error();
    auto width = integer(node, "width", 1, 4096, context); if (!width) return width.error();
    auto height = integer(node, "height", 1, 4096, context); if (!height) return height.error();
    result.x = static_cast<std::int16_t>(x.value());
    result.y = static_cast<std::int16_t>(y.value());
    result.width = static_cast<std::uint16_t>(width.value());
    result.height = static_cast<std::uint16_t>(height.value());
    return result;
}

std::uint32_t value_mask(const SpriteFieldDefinition& field) {
    return field.bit_width == 32 ? std::numeric_limits<std::uint32_t>::max()
                                 : (std::uint32_t{1} << field.bit_width) - 1U;
}

} // namespace

core::Result<std::vector<SpriteDefinition>> parse_sprite_definitions(std::string_view json) {
    auto parsed = core::parse_json(json);
    if (!parsed) return parsed.error();
    const auto& root = parsed.value();
    if (!root.is_object()) return core::make_error(core::ErrorCode::ParseError, "sprite definitions root is not an object");
    if (auto ok = only(root, {"schema", "sprites"}, "sprite definitions"); !ok) return ok.error();
    if (!root["schema"].is_string() || root["schema"].as_string() != "bigstar.sprite-definitions/1")
        return core::make_error(core::ErrorCode::Unsupported, "sprite definitions have an unsupported schema");
    if (!root["sprites"].is_array())
        return core::make_error(core::ErrorCode::ParseError, "sprite definitions have no sprites array");

    std::vector<SpriteDefinition> result;
    std::unordered_set<std::uint32_t> ids;
    for (const auto& node : root["sprites"].elements()) {
        if (!node.is_object()) return core::make_error(core::ErrorCode::ParseError, "sprite definition is not an object");
        if (auto ok = only(node, {"objectId", "name", "category", "color", "shape", "grabBounds", "renderBounds", "fields"},
                           "sprite definition"); !ok) return ok.error();
        SpriteDefinition definition;
        auto id = integer(node, "objectId", 0, std::numeric_limits<std::uint32_t>::max(), "sprite definition");
        if (!id) return id.error();
        definition.object_id = static_cast<std::uint32_t>(id.value());
        if (!ids.insert(definition.object_id).second)
            return core::make_error(core::ErrorCode::ParseError, "sprite layer contains a duplicate objectId");
        auto name = string_value(node, "name", "sprite definition"); if (!name) return name.error(); definition.name = std::move(name.value());
        if (node["category"].is_string() && !node["category"].as_string().empty()) definition.category = node["category"].as_string();
        if (node["color"].is_string()) definition.color = node["color"].as_string();
        if (definition.color.size() != 7 || definition.color.front() != '#' ||
            std::ranges::any_of(std::string_view(definition.color).substr(1), [](char c) {
                return !(c >= '0' && c <= '9') && !(c >= 'a' && c <= 'f') && !(c >= 'A' && c <= 'F');
            }))
            return core::make_error(core::ErrorCode::ParseError, definition.name + " has an invalid #RRGGBB color");
        if (node["shape"].is_string()) {
            const auto& shape = node["shape"].as_string();
            if (shape == "box") definition.shape = SpriteShape::Box;
            else if (shape == "ellipse") definition.shape = SpriteShape::Ellipse;
            else if (shape == "diamond") definition.shape = SpriteShape::Diamond;
            else return core::make_error(core::ErrorCode::ParseError, definition.name + " has an invalid shape");
        }
        if (node["grabBounds"].is_object()) { auto value = bounds(node["grabBounds"], definition.name + " grabBounds"); if (!value) return value.error(); definition.grab_bounds = value.value(); }
        if (node["renderBounds"].is_object()) { auto value = bounds(node["renderBounds"], definition.name + " renderBounds"); if (!value) return value.error(); definition.render_bounds = value.value(); }
        if (!node["fields"].is_array())
            return core::make_error(core::ErrorCode::ParseError, definition.name + " has no fields array");
        std::uint32_t used_bits{};
        std::unordered_set<std::string> keys;
        for (const auto& field_node : node["fields"].elements()) {
            if (!field_node.is_object()) return core::make_error(core::ErrorCode::ParseError, definition.name + " field is not an object");
            if (auto ok = only(field_node, {"key", "label", "bitOffset", "bitWidth", "choices"}, definition.name + " field"); !ok) return ok.error();
            SpriteFieldDefinition field;
            auto key = string_value(field_node, "key", definition.name + " field"); if (!key) return key.error(); field.key = std::move(key.value());
            auto label = string_value(field_node, "label", definition.name + " field"); if (!label) return label.error(); field.label = std::move(label.value());
            if (!keys.insert(field.key).second) return core::make_error(core::ErrorCode::ParseError, definition.name + " has a duplicate field key");
            auto offset = integer(field_node, "bitOffset", 0, 31, field.key); if (!offset) return offset.error();
            auto width = integer(field_node, "bitWidth", 1, std::min<std::int64_t>(16, 32 - offset.value()), field.key); if (!width) return width.error();
            field.bit_offset = static_cast<std::uint8_t>(offset.value());
            field.bit_width = static_cast<std::uint8_t>(width.value());
            const auto mask = value_mask(field) << field.bit_offset;
            if ((used_bits & mask) != 0) return core::make_error(core::ErrorCode::ParseError, definition.name + " has overlapping fields");
            used_bits |= mask;
            if (field_node["choices"].is_array()) {
                for (const auto& choice_node : field_node["choices"].elements()) {
                    if (!choice_node.is_object()) return core::make_error(core::ErrorCode::ParseError, field.key + " choice is not an object");
                    if (auto ok = only(choice_node, {"value", "label"}, field.key + " choice"); !ok) return ok.error();
                    auto value = integer(choice_node, "value", 0, value_mask(field), field.key + " choice"); if (!value) return value.error();
                    auto choice_label = string_value(choice_node, "label", field.key + " choice"); if (!choice_label) return choice_label.error();
                    field.choices.push_back({static_cast<std::uint32_t>(value.value()), std::move(choice_label.value())});
                }
            }
            definition.fields.push_back(std::move(field));
        }
        result.push_back(std::move(definition));
    }
    return result;
}

core::Result<SpriteDefinitionCatalog> load_sprite_definition_layers(std::span<const SpriteDefinitionLayer> layers) {
    SpriteDefinitionCatalog catalog;
    std::unordered_set<std::uint32_t> claimed;
    for (const auto& layer : layers) {
        std::error_code ec;
        if (!std::filesystem::exists(layer.path, ec)) {
            if (ec) return core::make_error(core::ErrorCode::IoError, "could not inspect sprite-definition layer " + layer.name);
            continue;
        }
        auto bytes = core::read_file(layer.path);
        if (!bytes) return bytes.error();
        auto parsed = parse_sprite_definitions(std::string_view(reinterpret_cast<const char*>(bytes.value().data()), bytes.value().size()));
        if (!parsed) return core::make_error(parsed.error().code, layer.name + ": " + parsed.error().message);
        for (auto& definition : parsed.value())
            if (claimed.insert(definition.object_id).second) catalog.definitions.push_back(std::move(definition));
    }
    return catalog;
}

const SpriteDefinition* SpriteDefinitionCatalog::by_object_id(std::uint32_t object_id) const {
    const auto found = std::ranges::find(definitions, object_id, &SpriteDefinition::object_id);
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
