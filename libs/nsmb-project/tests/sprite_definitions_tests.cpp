#include "nsmb/project/sprite_definitions.hpp"

#include <cassert>

int main() {
    nsmb::project::SpriteFieldDefinition field{"mode", "Mode", 4, 3, {}};
    assert(nsmb::project::read_sprite_field(0x70, field) == 7);
    assert(nsmb::project::write_sprite_field(0xffffffff, field, 2) == 0xffffffaf);

    nsmb::project::SpriteDefinition vanilla;
    vanilla.object_id = 83;
    nsmb::project::SpriteDefinition custom;
    custom.hash = 0x12345678;
    nsmb::project::SpriteDefinitionCatalog catalog{{vanilla, custom}};
    assert(catalog.by_object_id(83));
    assert(catalog.by_hash(0x12345678));
}
