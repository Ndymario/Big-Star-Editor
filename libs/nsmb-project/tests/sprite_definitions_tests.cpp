#include "nsmb/project/sprite_definitions.hpp"

#include <filesystem>
#include <fstream>
#include <iostream>
#include <iterator>

namespace {
int failures{};
void check(bool condition, const char* message) { if (!condition) { std::cerr << "FAIL: " << message << '\n'; ++failures; } }
}

int main() {
    constexpr auto valid = R"({"schema":"bigstar.sprite-definitions/1","sprites":[{"objectId":7,"name":"Switch","category":"Logic","color":"#336699","shape":"box","grabBounds":{"x":-4,"y":-8,"width":8,"height":16},"renderBounds":{"x":-8,"y":-8,"width":16,"height":16},"fields":[{"key":"mode","label":"Mode","bitOffset":0,"bitWidth":2,"choices":[{"value":0,"label":"Off"},{"value":2,"label":"On"}]},{"key":"speed","label":"Speed","bitOffset":4,"bitWidth":3}]}]})";
    auto parsed = nsmb::project::parse_sprite_definitions(valid);
    check(parsed && parsed.value().size() == 1, "valid schema parses");
    if (parsed) {
        const auto& field = parsed.value()[0].fields[1];
        check(nsmb::project::read_sprite_field(0x70, field) == 7, "field extraction uses zero-based bits");
        check(nsmb::project::write_sprite_field(0xffffffff, field, 2) == 0xffffffaf, "field update preserves unrelated bits");
    }
    check(!nsmb::project::parse_sprite_definitions(R"({"schema":"bigstar.sprite-definitions/1","sprites":[{"objectId":1,"name":"Bad","fields":[{"key":"a","label":"A","bitOffset":1,"bitWidth":2},{"key":"b","label":"B","bitOffset":2,"bitWidth":2}]}]})"),
          "overlapping fields are rejected");

    const auto root = std::filesystem::temp_directory_path() / "big-star-sprite-definition-tests";
    std::filesystem::create_directories(root);
    { std::ofstream file(root / "first.json"); file << valid; }
    { std::ofstream file(root / "last.json"); file << R"({"schema":"bigstar.sprite-definitions/1","sprites":[{"objectId":7,"name":"Loses","fields":[]},{"objectId":8,"name":"Added","fields":[]}]})"; }
    const std::vector layers{nsmb::project::SpriteDefinitionLayer{"plugin", root / "first.json"},
                             nsmb::project::SpriteDefinitionLayer{"project", root / "last.json"}};
    auto catalog = nsmb::project::load_sprite_definition_layers(layers);
    check(catalog && catalog.value().definitions.size() == 2, "layers merge unique object IDs");
    check(catalog && catalog.value().by_object_id(7)->name == "Switch", "first definition wins");
    std::filesystem::remove_all(root);

    std::ifstream shipped(std::filesystem::path(BSE_TEST_SOURCE_DIR) / "data" / "sprite-definitions.json");
    const std::string shipped_json{std::istreambuf_iterator<char>(shipped), {}};
    const auto shipped_catalog = nsmb::project::parse_sprite_definitions(shipped_json);
    check(shipped_catalog && !shipped_catalog.value().empty(), "shipped clean-room catalog parses");
    return failures == 0 ? 0 : 1;
}
