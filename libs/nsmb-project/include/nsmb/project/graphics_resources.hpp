#pragma once

#include <array>
#include <filesystem>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/formats/graphics.hpp"
#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/project.hpp"

namespace nsmb::project {

struct NsmbGraphicsProfile {
    std::string_view game_code;
    std::uint16_t file_id_offset{};
    std::size_t tileset_index{}, tileset_objects{}, tileset_behaviors{};
    std::size_t tileset_graphics{}, tileset_palette{}, tileset_map16{};
    std::size_t background_graphics{}, background_palette{}, background_screen{};
    std::size_t foreground_graphics{}, foreground_palette{}, foreground_screen{};
    std::size_t sprite_object_ids{};
};

struct TilesetResourceFiles {
    std::filesystem::path graphics, palette, map16, objects, object_index, behaviors;
    std::uint16_t tile_offset{};
    std::uint8_t palette_offset{}, tile_page_offset{};
};

struct BackgroundResourceFiles {
    std::filesystem::path graphics, palette, screen;
};

struct ActorResourceFiles {
    std::uint16_t type{};
    std::uint32_t object_id{};
    std::string name;
    std::filesystem::path graphics, palette, model;
};

struct LevelGraphicsResources {
    std::array<TilesetResourceFiles, 3> tilesets;
    BackgroundResourceFiles background, foreground;
    std::vector<ActorResourceFiles> actors;
    std::vector<std::filesystem::path> watched_files;
};

// Visits every path this struct holds, so a caller can redirect the whole set
// in one pass. `resolve_level_graphics` reads the ROM's own file table and so
// only ever names files in the extracted tree; a frontend that knows about
// module layers uses this to point each one at the layer actually supplying it.
// Empty paths are visited too -- the callback decides what to do with them.
template <typename F>
void for_each_resource_path(LevelGraphicsResources& resources, F&& visit) {
    for (auto& tileset : resources.tilesets) {
        for (auto* path : {&tileset.graphics, &tileset.palette, &tileset.map16,
                           &tileset.objects, &tileset.object_index, &tileset.behaviors}) {
            visit(*path);
        }
    }
    for (auto* background : {&resources.background, &resources.foreground}) {
        for (auto* path : {&background->graphics, &background->palette, &background->screen}) visit(*path);
    }
    for (auto& actor : resources.actors) {
        for (auto* path : {&actor.graphics, &actor.palette, &actor.model}) visit(*path);
    }
    for (auto& path : resources.watched_files) visit(path);
}

using ObjectBankOverlayTable = std::array<std::array<std::uint32_t, 10>, 16>;
core::Result<ObjectBankOverlayTable> find_object_bank_overlays(std::span<const std::uint8_t> arm9);

struct DecodedTileset {
    std::vector<formats::RgbaImage> map16_tiles;
    std::vector<formats::ObjectLayout> objects;
};

[[nodiscard]] const NsmbGraphicsProfile* nsmb_graphics_profile(std::string_view game_code);
core::Result<std::filesystem::path> resolve_nitro_file_id(const std::filesystem::path& extracted_root,
                                                          std::span<const std::uint8_t> fnt,
                                                          std::uint16_t file_id);
core::Result<LevelGraphicsResources> resolve_level_graphics(const Project& project,
                                                            LevelTilesetReferences references);
core::Result<DecodedTileset> load_tileset_graphics(const TilesetResourceFiles& files);
core::Result<formats::RgbaImage> render_tileset_object(const DecodedTileset& tileset,
                                                       std::uint16_t object_id,
                                                       std::size_t width,
                                                       std::size_t height);
core::Result<formats::RgbaImage> load_background_graphics(const BackgroundResourceFiles& files);
core::Result<formats::RgbaImage> load_actor_graphics(const ActorResourceFiles& files);

} // namespace nsmb::project
