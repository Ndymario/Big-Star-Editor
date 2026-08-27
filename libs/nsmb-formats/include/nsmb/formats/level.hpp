#pragma once

#include <array>
#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include "nsmb/core/fx.hpp"
#include "nsmb/core/result.hpp"

namespace nsmb::formats {

inline constexpr std::size_t nsmb_level_block_count = 14;
inline constexpr std::size_t nsmb_bgdat_object_size = 10;
inline constexpr std::size_t nsmb_object_bank_slot_count = 16;

struct NsmbLevelBlock { std::uint32_t offset{}, size{}; };
struct NsmbLevelHeader {
    std::array<std::uint8_t, 2> entrances{};
    std::uint16_t settings{}, timer{};
    std::array<std::uint16_t, 3> background_theme{}, tileset_theme{}, foreground_theme{};
    std::uint16_t events{}, sound_group{}, sprite_priority{}, mega_drop_chance{};
};
struct NsmbLevelCamera {
    std::int32_t top_offset{}, bottom_offset{}, top_offset_special{}, bottom_offset_special{};
    std::uint16_t id{}, scroll_height{};
    std::uint32_t unknown{};
};
struct NsmbLevelScreen {
    std::uint16_t id{}, screen_id{}, palette_id{}, parallax_x{}, parallax_y{};
    std::uint16_t scroll_x{}, scroll_y{}, vertical_offset{}, unused_10{}, unused_12{};
};
struct NsmbLevelObject {
    std::uint16_t object_id{};
    std::uint8_t tileset_slot{};
    std::int16_t x{}, y{};
    std::uint16_t width{}, height{};
};
struct NsmbLevelStageObject {
    std::uint16_t id{}, x{}, y{};
    std::array<std::uint8_t, 2> events{};
    std::uint32_t settings{};
};
using NsmbLevelSprite = NsmbLevelStageObject;
enum class NsmbEntranceType : std::uint8_t {
    Normal, OverriddenNormal, Door, PipeUp, PipeDown, PipeLeft, PipeRight,
    Falling, GroundPounding, Sliding, Swimming, TransitNormal, TransitDoor,
    MarioVsLuigiPipe, InvisibleDoorNoSound, InvisibleDoor, MiniPipeUp,
    MiniPipeDown, MiniPipeLeft, MiniPipeRight, Jumping, Vine
};
enum class NsmbEntranceFlags : std::uint8_t {
    SubScreen=0x01, LockedPlayer1=0x02, LockedPlayer0=0x04, ConnectedPipe=0x08,
    WipeFade=0x10, AlignedX=0x40, ExitOnly=0x80
};
struct NsmbLevelEntrance {
    std::uint16_t x{}, y{}, camera_x{}, camera_y{};
    std::uint8_t id{}, target_stage_area{}, target_stage_index{}, target_stage_group{};
    std::uint8_t target_entrance{}, camera_zoom{}, type{}, flags{}, unknown_10{}, unknown_11{};
    std::uint8_t view_id{}, target_worldmap_node{};
};
struct NsmbLevelView {
    std::uint16_t x{}, y{}, width{}, height{};
    std::uint8_t id{}, camera_settings_id{}, bgm_id{}, background_id{};
    std::uint8_t tileset_id{}, foreground_id{}, light_type{}, progress_path_id{};
};
struct NsmbLevelZone {
    std::uint16_t x{}, y{}, width{}, height{};
    std::uint8_t id{}, unknown_9{}, unknown_a{}, unknown_b{};
};
struct NsmbLevelPathNode {
    std::int16_t x{}, y{};
    core::Fx32 speed{}, acceleration{};
    std::uint16_t delay{}, flags{};
};
struct NsmbLevelPath {
    std::uint8_t id{};
    std::vector<NsmbLevelPathNode> nodes;
    std::uint16_t unknown{};
};
struct NsmbLevelCourse {
    std::array<NsmbLevelBlock, nsmb_level_block_count> blocks{};
    NsmbLevelHeader header;
    std::vector<NsmbLevelCamera> cameras;
    std::vector<NsmbLevelScreen> backgrounds, tilesets, foregrounds;
    std::vector<NsmbLevelStageObject> sprites;
    std::vector<NsmbLevelEntrance> entrances;
    std::vector<NsmbLevelView> views;
    std::vector<NsmbLevelZone> zones;
    std::vector<NsmbLevelPath> paths, progress_paths;
    std::array<std::uint8_t, nsmb_object_bank_slot_count> object_banks{};
};

static_assert(sizeof(NsmbLevelHeader) == 0x20);
static_assert(sizeof(NsmbLevelCamera) == 0x18);
static_assert(sizeof(NsmbLevelScreen) == 0x14);
static_assert(sizeof(NsmbLevelStageObject) == 0x0c);
static_assert(sizeof(NsmbLevelEntrance) == 0x14);
static_assert(sizeof(NsmbLevelView) == 0x10);
static_assert(sizeof(NsmbLevelZone) == 0x0c);
static_assert(sizeof(NsmbLevelPathNode) == 0x10);

core::Result<NsmbLevelCourse> parse_nsmb_level_course(std::span<const std::uint8_t> data);
core::Result<std::vector<NsmbLevelObject>> parse_nsmb_bgdat_objects(std::span<const std::uint8_t> data);

} // namespace nsmb::formats
