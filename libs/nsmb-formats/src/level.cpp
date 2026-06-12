#include "nsmb/formats/level.hpp"

#include "nsmb/core/endian.hpp"

#include <algorithm>

namespace nsmb::formats {

namespace {

core::Result<std::span<const std::uint8_t>> checked_block(std::span<const std::uint8_t> data, const NsmbLevelBlock& block) {
    const auto offset = static_cast<std::size_t>(block.offset);
    const auto size = static_cast<std::size_t>(block.size);
    if (offset > data.size() || size > data.size() - offset) {
        return core::make_error(core::ErrorCode::OutOfBounds, "level block extends past the course file");
    }
    return std::span<const std::uint8_t>(data.data() + offset, size);
}

std::uint8_t byte_or_zero(std::span<const std::uint8_t> data, std::size_t offset) {
    return offset < data.size() ? data[offset] : 0;
}

std::uint16_t read_u16(std::span<const std::uint8_t> data, std::size_t offset) {
    return static_cast<std::uint16_t>(data[offset] | (data[offset + 1U] << 8U));
}

std::int16_t read_i16(std::span<const std::uint8_t> data, std::size_t offset) {
    return static_cast<std::int16_t>(read_u16(data, offset));
}

void parse_sprites(NsmbLevelCourse& course, std::span<const std::uint8_t> block) {
    if (block.size() < 2U) {
        return;
    }

    const auto count = (block.size() - 2U) / 12U;
    course.sprites.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 12U;
        NsmbLevelSprite sprite;
        sprite.type = read_u16(block, offset);
        sprite.x = read_u16(block, offset + 2U);
        sprite.y = read_u16(block, offset + 4U);
        std::copy_n(block.begin() + static_cast<std::ptrdiff_t>(offset + 6U), sprite.data.size(), sprite.data.begin());
        course.sprites.push_back(sprite);
    }
}

void parse_entrances(NsmbLevelCourse& course, std::span<const std::uint8_t> block) {
    const auto count = block.size() / 20U;
    course.entrances.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 20U;
        NsmbLevelEntrance entrance;
        entrance.x = read_u16(block, offset);
        entrance.y = read_u16(block, offset + 2U);
        entrance.camera_x = read_u16(block, offset + 4U);
        entrance.camera_y = read_u16(block, offset + 6U);
        entrance.number = block[offset + 8U];
        entrance.destination_area = block[offset + 9U];
        entrance.connected_pipe_id = block[offset + 10U];
        entrance.destination_entrance = block[offset + 12U];
        entrance.type = block[offset + 14U];
        entrance.settings = block[offset + 15U];
        entrance.unknown1 = block[offset + 16U];
        entrance.entry_view = block[offset + 18U];
        entrance.unknown2 = block[offset + 19U];
        course.entrances.push_back(entrance);
    }
}

void parse_views(NsmbLevelCourse& course, std::span<const std::uint8_t> block) {
    const auto count = block.size() / 16U;
    course.views.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 16U;
        NsmbLevelView view;
        view.x = read_u16(block, offset);
        view.y = read_u16(block, offset + 2U);
        view.width = read_u16(block, offset + 4U);
        view.height = read_u16(block, offset + 6U);
        view.number = block[offset + 8U];
        view.camera_id = block[offset + 9U];
        view.music = block[offset + 10U];
        view.unknown1 = block[offset + 11U];
        view.unknown2 = block[offset + 12U];
        view.unknown3 = block[offset + 13U];
        view.lighting = block[offset + 14U];
        view.flagpole_id = block[offset + 15U];
        course.views.push_back(view);
    }
}

void parse_zones(NsmbLevelCourse& course, std::span<const std::uint8_t> block) {
    const auto count = block.size() / 12U;
    course.zones.reserve(count);
    for (std::size_t i = 0; i < count; ++i) {
        const auto offset = i * 12U;
        NsmbLevelZone zone;
        zone.x = read_u16(block, offset);
        zone.y = read_u16(block, offset + 2U);
        zone.width = read_u16(block, offset + 4U);
        zone.height = read_u16(block, offset + 6U);
        zone.number = block[offset + 8U];
        course.zones.push_back(zone);
    }
}

} // namespace

core::Result<NsmbLevelCourse> parse_nsmb_level_course(std::span<const std::uint8_t> data) {
    constexpr std::size_t header_size = nsmb_level_block_count * 8U;
    if (data.size() < header_size) {
        return core::make_error(core::ErrorCode::ParseError, "NSMB course file is missing the block table");
    }

    core::LittleEndianReader reader(data);
    NsmbLevelCourse course;
    for (std::size_t i = 0; i < nsmb_level_block_count; ++i) {
        const auto entry_offset = i * 8U;
        auto block_offset = reader.u32(entry_offset);
        auto block_size = reader.u32(entry_offset + 4U);
        if (!block_offset || !block_size) {
            return core::make_error(core::ErrorCode::ParseError, "could not read NSMB course block table");
        }
        course.blocks[i] = NsmbLevelBlock{block_offset.value(), block_size.value()};
        auto block = checked_block(data, course.blocks[i]);
        if (!block) {
            return block.error();
        }
    }

    auto block0 = checked_block(data, course.blocks[0]);
    auto block2 = checked_block(data, course.blocks[2]);
    auto block5 = checked_block(data, course.blocks[5]);
    auto block6 = checked_block(data, course.blocks[6]);
    auto block7 = checked_block(data, course.blocks[7]);
    auto block8 = checked_block(data, course.blocks[8]);
    if (!block0 || !block2 || !block5 || !block6 || !block7 || !block8) {
        return core::make_error(core::ErrorCode::ParseError, "could not read NSMB course blocks");
    }

    course.background_graphics_id = byte_or_zero(block0.value(), 0x06);
    course.tileset_id = byte_or_zero(block0.value(), 0x0C);
    course.foreground_graphics_id = byte_or_zero(block0.value(), 0x12);
    course.background_screen_id = byte_or_zero(block2.value(), 0x02);

    parse_entrances(course, block5.value());
    parse_sprites(course, block6.value());
    parse_views(course, block7.value());
    parse_zones(course, block8.value());
    return course;
}

core::Result<std::vector<NsmbLevelObject>> parse_nsmb_bgdat_objects(std::span<const std::uint8_t> data) {
    if ((data.size() % nsmb_bgdat_object_size) != 0U) {
        return core::make_error(core::ErrorCode::ParseError, "NSMB BGDat object data must be made of 10-byte records");
    }

    std::vector<NsmbLevelObject> objects;
    objects.reserve(data.size() / nsmb_bgdat_object_size);
    for (std::size_t offset = 0; offset < data.size(); offset += nsmb_bgdat_object_size) {
        const auto packed_id = read_u16(data, offset);
        NsmbLevelObject object;
        object.object_id = static_cast<std::uint16_t>(packed_id & 0x0FFFU);
        object.tileset_slot = static_cast<std::uint8_t>((packed_id >> 12U) & 0x0FU);
        object.x = read_i16(data, offset + 2U);
        object.y = read_i16(data, offset + 4U);
        object.width = read_u16(data, offset + 6U);
        object.height = read_u16(data, offset + 8U);
        objects.push_back(object);
    }
    return objects;
}

} // namespace nsmb::formats
