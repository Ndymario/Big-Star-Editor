#include "nsmb/project/sprite_definitions.hpp"

#include <cassert>

int main() {
    nsmb::project::SpriteFieldDefinition field;
    field.key = "mode";
    field.label = "Mode";
    field.bit_offset = 4;
    field.bit_width = 3;
    assert(nsmb::project::read_sprite_field(0x70, field) == 7);
    assert(nsmb::project::write_sprite_field(0xffffffff, field, 2) == 0xffffffaf);

    nsmb::project::SpriteFieldDefinition event;
    event.key = "event";
    event.label = "Event";
    event.bit_offset = 40;
    event.bit_width = 8;
    const std::uint64_t sprite_data = 0x12'34'89abcdefULL;
    assert(nsmb::project::read_sprite_field(sprite_data, event) == 0x12);
    assert(nsmb::project::write_sprite_field(sprite_data, event, 0x56) == 0x56'34'89abcdefULL);

    nsmb::project::SpriteFieldDefinition signed_field;
    signed_field.bit_width = 4;
    signed_field.signed_value = true;
    signed_field.value_offset = 2;
    assert(nsmb::project::sprite_field_min(signed_field) == -6);
    assert(nsmb::project::sprite_field_max(signed_field) == 9);
    assert(nsmb::project::read_sprite_field(0xf, signed_field) == 1);
    assert(nsmb::project::write_sprite_field(0, signed_field, 1) == 0xf);

    nsmb::project::SpriteFieldDefinition checkbox;
    checkbox.bit_width = 4;
    checkbox.checkbox_value = 8;
    assert(nsmb::project::sprite_field_max(checkbox) == 1);
    assert(nsmb::project::read_sprite_field(8, checkbox) == 1);
    assert(nsmb::project::write_sprite_field(0, checkbox, 1) == 8);

    nsmb::project::SpriteDefinition vanilla;
    vanilla.object_id = 83;
    nsmb::project::SpriteDefinition custom;
    custom.hash = 0x12345678;
    nsmb::project::SpriteDefinitionCatalog catalog{{vanilla, custom}};
    assert(catalog.by_object_id(83));
    assert(catalog.by_hash(0x12345678));
}
