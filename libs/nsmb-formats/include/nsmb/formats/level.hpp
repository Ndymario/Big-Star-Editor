#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::formats {

inline constexpr std::size_t nsmb_level_block_count = 14;
inline constexpr std::size_t nsmb_bgdat_object_size = 10;

struct NsmbLevelBlock {
    std::uint32_t offset{};
    std::uint32_t size{};
};

struct NsmbLevelObject {
    std::uint16_t object_id{};
    std::uint8_t tileset_slot{};
    std::int16_t x{};
    std::int16_t y{};
    std::uint16_t width{};
    std::uint16_t height{};
};

struct NsmbLevelSprite {
    std::uint16_t type{};
    std::uint16_t x{};
    std::uint16_t y{};
    std::array<std::uint8_t, 6> data{};
};

struct NsmbLevelEntrance {
    std::uint16_t x{};
    std::uint16_t y{};
    std::uint16_t camera_x{};
    std::uint16_t camera_y{};
    std::uint8_t number{};
    std::uint8_t destination_area{};
    std::uint8_t connected_pipe_id{};
    std::uint8_t destination_entrance{};
    std::uint8_t type{};
    std::uint8_t settings{};
    std::uint8_t unknown1{};
    std::uint8_t entry_view{};
    std::uint8_t unknown2{};
    std::array<std::uint8_t, 20> raw{};
};

struct NsmbLevelView {
    std::uint16_t x{};
    std::uint16_t y{};
    std::uint16_t width{};
    std::uint16_t height{};
    std::uint8_t number{};
    std::uint8_t camera_id{};
    std::uint8_t music{};
    std::uint8_t unknown1{};
    std::uint8_t unknown2{};
    std::uint8_t unknown3{};
    std::uint8_t lighting{};
    std::uint8_t flagpole_id{};
};

struct NsmbLevelZone {
    std::uint16_t x{};
    std::uint16_t y{};
    std::uint16_t width{};
    std::uint16_t height{};
    std::uint8_t number{};
    std::array<std::uint8_t, 3> trailing{};
};

struct NsmbLevelPathNode {
    std::uint16_t x{};
    std::uint16_t y{};
    std::array<std::uint16_t, 6> values{};
};

struct NsmbLevelPath {
    std::uint16_t id{};
    std::vector<NsmbLevelPathNode> nodes;
    std::uint16_t unknown{};
};

struct NsmbLevelCourse {
    std::array<NsmbLevelBlock, nsmb_level_block_count> blocks{};
    std::uint8_t background_graphics_id{};
    std::uint8_t tileset_id{};
    std::uint8_t foreground_graphics_id{};
    std::uint8_t background_screen_id{};
    std::vector<NsmbLevelSprite> sprites;
    std::vector<NsmbLevelEntrance> entrances;
    std::vector<NsmbLevelView> views;
    std::vector<NsmbLevelZone> zones;
    std::vector<NsmbLevelPath> paths;
    std::vector<NsmbLevelPath> progress_paths;
};

core::Result<NsmbLevelCourse> parse_nsmb_level_course(std::span<const std::uint8_t> data);
core::Result<std::vector<NsmbLevelObject>> parse_nsmb_bgdat_objects(std::span<const std::uint8_t> data);

} // namespace nsmb::formats
