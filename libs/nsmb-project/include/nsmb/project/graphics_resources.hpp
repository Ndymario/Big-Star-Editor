#pragma once

#include <array>
#include <filesystem>
#include <span>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/formats/graphics.hpp"
#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/effective_filesystem.hpp"
#include "nsmb/project/lua_tiles.hpp"
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

// One resource, named the way the game names it and carrying what the editor
// would read there now -- which is the module's copy when a build has not caught
// up with an edit yet, and the ROM's otherwise.
struct ResourceFile {
    std::string rom_path;
    std::vector<std::uint8_t> data;

    [[nodiscard]] bool empty() const noexcept { return rom_path.empty(); }
};

struct TilesetResourceFiles {
    ResourceFile graphics, palette, map16, objects, object_index, behaviors;
    std::uint16_t tile_offset{};
    std::uint8_t palette_offset{}, tile_page_offset{};
};

struct BackgroundResourceFiles {
    ResourceFile graphics, palette, screen;
};

struct ActorResourceFiles {
    std::uint16_t type{};
    std::uint32_t object_id{};
    std::optional<std::uint32_t> hash;
    std::string name;
};

struct LevelGraphicsResources {
    std::array<TilesetResourceFiles, 3> tilesets;
    BackgroundResourceFiles background, foreground;
    std::vector<ActorResourceFiles> actors;

    // Files on disk that, if they change, change these resources: the module
    // sources supplying them, and the ROM everything else comes from. There is
    // nothing to watch inside the ROM, so it is watched whole.
    std::vector<std::filesystem::path> watched_files;
};

using ObjectBankOverlayTable = std::array<std::array<std::uint32_t, 10>, 16>;
core::Result<ObjectBankOverlayTable> find_object_bank_overlays(std::span<const std::uint8_t> arm9);

struct DecodedTileset {
    std::vector<formats::RgbaImage> map16_tiles;
    std::vector<formats::ObjectLayout> objects;
};

[[nodiscard]] const NsmbGraphicsProfile* nsmb_graphics_profile(std::string_view game_code);

// What the level's graphics are, read through `files` rather than off a tree
// somebody unpacked earlier. The game's own resource tables address files by
// id, so this walks the ROM's file table to turn each id into an address, and
// then asks `files` what that address holds -- which is how a module's pending
// edit reaches the canvas without a rebuild.
core::Result<LevelGraphicsResources> resolve_level_graphics(EffectiveFilesystem& files,
                                                            LevelTilesetReferences references);
// Every tileset the level loads, decoded together. A Map16 block naming another
// tileset's graphics is drawn rather than left blank, because the three are laid
// out in one address space first -- which is what the game does. A slot that
// fails to decode leaves its entry empty and its reason in `errors`; the others
// still render. See `docs/tileset-memory.md`.
struct LevelTilesets {
    std::array<DecodedTileset, 3> tilesets;
    std::array<std::string, 3> errors;
};

// `tileset_id` and `overlays` are what turns a block the game never draws into
// the texture it actually shows: a few hundred blocks are handed to the 3D
// engine, and which ones depends on the tileset. Pass no catalog and the Map16
// is drawn exactly as the files describe it. See `docs/lua-tiles.md`.
LevelTilesets load_level_tilesets(const std::array<TilesetResourceFiles, 3>& files,
                                  std::uint16_t tileset_id = 0,
                                  const TileOverlayCatalog* overlays = nullptr);
core::Result<formats::RgbaImage> render_tileset_object(const DecodedTileset& tileset,
                                                       std::uint16_t object_id,
                                                       std::size_t width,
                                                       std::size_t height);
core::Result<formats::RgbaImage> load_background_graphics(const BackgroundResourceFiles& files);
} // namespace nsmb::project
