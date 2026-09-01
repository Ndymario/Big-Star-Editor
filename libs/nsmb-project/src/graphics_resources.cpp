#include "nsmb/project/graphics_resources.hpp"

#include <algorithm>
#include <array>

#include "nsmb/core/file_io.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/formats/g2d.hpp"
#include "nsmb/formats/tex0.hpp"

namespace nsmb::project {

core::Result<ObjectBankOverlayTable> find_object_bank_overlays(std::span<const std::uint8_t> arm9) {
    constexpr std::array<std::array<std::uint32_t,10>,16> expected{{
        {{22,23,24,25,26,27,28,29,30,31}},{{32,33,34,35,36,37,38,39,40,41}},{{56,57,58,59,60,61,62,63,64,65}},{{66,67,68,69,70,71,72,73,74,75}},
        {{42,43,44,45,46,47,48,49,50,51}},{{76,77,78,79,80,81,82,83,84,85}},{{86,87,88,89,90,91,92,93,94,95}},{{96,97,98,99,100,101,102,103,104,105}},
        {{106,107,108,109,110,111,112,113,114,115}},{{116,117,118,119,120,121,122,123,124,125}},{{0,0,0,0,0,0,0,0,0,0}},{{0,0,0,0,0,0,0,0,0,0}},
        {{0,0,0,0,0,0,0,0,0,0}},{{0,0,0,0,0,0,0,0,0,0}},{{0,0,0,0,0,0,0,0,0,0}},{{12,13,14,15,16,17,18,19,20,21}}
    }};
    constexpr auto bytes=sizeof(expected);
    for(std::size_t offset=0;offset+bytes<=arm9.size();offset+=4){
        ObjectBankOverlayTable found{};bool match=true;
        for(std::size_t slot=0;slot<16&&match;++slot)for(std::size_t bank=0;bank<10;++bank){const auto o=offset+(slot*10+bank)*4;const auto v=static_cast<std::uint32_t>(arm9[o])|(static_cast<std::uint32_t>(arm9[o+1])<<8)|(static_cast<std::uint32_t>(arm9[o+2])<<16)|(static_cast<std::uint32_t>(arm9[o+3])<<24);found[slot][bank]=v;if(v!=expected[slot][bank]){match=false;break;}}
        if(match)return found;
    }
    return core::make_error(core::ErrorCode::NotFound,"could not locate the object-bank overlay table in ARM9");
}

namespace {

constexpr std::array profiles{
    NsmbGraphicsProfile{"A2DE", 131, 0x2F8E4, 0x2FA14, 0x2FB44, 0x30EA4, 0x31494, 0x316F4, 0x30D74, 0x31364, 0x31234, 0x30FD4, 0x315C4, 0x31104, 0x29BD8},
    NsmbGraphicsProfile{"A2DP", 135, 0x2F0F8, 0x2F228, 0x2F358, 0x306B8, 0x30CA8, 0x30F08, 0x30588, 0x30B78, 0x30A48, 0x307E8, 0x30DD8, 0x30918, 0x290C4},
    NsmbGraphicsProfile{"A2DJ", 131, 0x2ECE4, 0x2EE14, 0x2EF44, 0x302A4, 0x30894, 0x30AF4, 0x30174, 0x30764, 0x30634, 0x303D4, 0x309C4, 0x30504, 0x29008},
    NsmbGraphicsProfile{"A2DK", 131, 0x2EDA4, 0x2EED4, 0x2F004, 0x30364, 0x30954, 0x30BB4, 0x30234, 0x30824, 0x306F4, 0x30494, 0x30A84, 0x305C4, 0x290E0},
    NsmbGraphicsProfile{"A2DC", 131, 0x2EDA4, 0x2EED4, 0x2F004, 0x30364, 0x30954, 0x30BB4, 0x30234, 0x30824, 0x306F4, 0x30494, 0x30A84, 0x305C4, 0x290DC},
};

// A graphics resource may be stored LZ-wrapped whether it comes from the ROM or
// from a module tree, so the unwrap belongs to the bytes rather than to where
// they were read.
core::Result<std::vector<std::uint8_t>> unwrap_resource(const std::vector<std::uint8_t>& data) {
    if (!data.empty() && (data[0] == 0x10 || data[0] == 0x11)) return formats::decompress_nitro_lz(data);
    return data;
}

core::Result<std::uint16_t> table_file_id(std::span<const std::uint8_t> overlay,
                                          std::size_t table,
                                          std::uint8_t id,
                                          std::uint16_t base) {
    const auto offset = table + static_cast<std::size_t>(id) * 4U;
    if (offset + 1U >= overlay.size()) {
        return core::make_error(core::ErrorCode::OutOfBounds, "graphics resource table is outside Overlay 0");
    }
    return static_cast<std::uint16_t>(overlay[offset] | (overlay[offset + 1U] << 8U)) + base;
}

std::string find_named_file(std::span<const nds::NitroFileEntry> files, std::string_view name) {
    const auto found = std::find_if(files.begin(), files.end(), [name](const auto& file) { return file.path.filename() == name; });
    return found == files.end() ? std::string{} : found->path.generic_string();
}

void copy_image(const formats::RgbaImage& source,
                formats::RgbaImage& destination,
                std::size_t destination_x,
                std::size_t destination_y) {
    for (std::size_t y = 0; y < source.height; ++y) {
        for (std::size_t x = 0; x < source.width; ++x) {
            const auto source_pixel = (y * source.width + x) * 4U;
            const auto destination_pixel = ((destination_y + y) * destination.width + destination_x + x) * 4U;
            std::copy_n(source.pixels.begin() + static_cast<std::ptrdiff_t>(source_pixel),
                        4U,
                        destination.pixels.begin() + static_cast<std::ptrdiff_t>(destination_pixel));
        }
    }
}

formats::RgbaImage trim_image(const formats::RgbaImage& source) {
    if (!source.width || !source.height) return source;
    std::size_t left=source.width,top=source.height,right=0,bottom=0;bool found=false;
    for(std::size_t y=0;y<source.height;++y)for(std::size_t x=0;x<source.width;++x)if(source.pixels[(y*source.width+x)*4U+3]){found=true;left=std::min(left,x);top=std::min(top,y);right=std::max(right,x);bottom=std::max(bottom,y);}
    if(!found)return source;formats::RgbaImage result{right-left+1U,bottom-top+1U,{}};result.pixels.resize(result.width*result.height*4U);
    for(std::size_t y=0;y<result.height;++y)for(std::size_t x=0;x<result.width;++x)std::copy_n(source.pixels.begin()+static_cast<std::ptrdiff_t>(((top+y)*source.width+left+x)*4U),4,result.pixels.begin()+static_cast<std::ptrdiff_t>((y*result.width+x)*4U));return result;
}

} // namespace

const NsmbGraphicsProfile* nsmb_graphics_profile(std::string_view game_code) {
    const auto found = std::find_if(profiles.begin(), profiles.end(), [game_code](const auto& profile) { return profile.game_code == game_code; });
    return found == profiles.end() ? nullptr : &*found;
}

core::Result<LevelGraphicsResources> resolve_level_graphics(EffectiveFilesystem& files,
                                                            LevelTilesetReferences references) {
    auto* rom = files.rom();
    if (!rom) return core::make_error(core::ErrorCode::InvalidArgument, "the project has no built ROM to read graphics from");
    const auto* profile = nsmb_graphics_profile(rom->header().game_code);
    if (!profile) return core::make_error(core::ErrorCode::Unsupported, "ROM game code does not have a graphics resource profile");
    auto overlay = rom->read_overlay(0);
    if (!overlay) return overlay.error();

    // Where each resource is addressed from. The game's tables hold file ids,
    // and two of the three tilesets are named outright rather than indexed.
    auto table = [&](std::size_t offset, std::uint8_t id) -> core::Result<std::string> {
        auto file_id = table_file_id(overlay.value(), offset, id, profile->file_id_offset);
        if (!file_id) return file_id.error();
        auto path = files.path_for_id(file_id.value());
        if (!path) return core::make_error(core::ErrorCode::NotFound,
                                           "graphics resource file ID " + std::to_string(file_id.value()) + " is not in the ROM");
        return std::move(path.value());
    };
    auto named = [&](std::string_view name) -> core::Result<std::string> {
        auto path = find_named_file(rom->files(), name);
        if (path.empty()) return core::make_error(core::ErrorCode::NotFound,
                                                  "required ROM graphics file is missing: " + std::string(name));
        return path;
    };

    LevelGraphicsResources result;
    // The ROM is watched whole, because a build replaces it whole and nothing
    // inside it changes on its own.
    result.watched_files.push_back(rom->path());
    auto fill = [&](ResourceFile& target, core::Result<std::string> address) -> core::Result<void> {
        if (!address) return address.error();
        auto data = files.read(address.value());
        if (!data) return data.error();
        // A module supplying this resource is the file to watch; when the ROM
        // supplies it there is nothing to add, since the ROM is already there.
        if (auto source = files.source(address.value()); !source.empty()) result.watched_files.push_back(std::move(source));
        target = ResourceFile{std::move(address.value()), std::move(data.value())};
        return {};
    };
    auto fill_tileset = [&](TilesetResourceFiles& set,
                            core::Result<std::string> graphics,
                            core::Result<std::string> palette,
                            core::Result<std::string> map16,
                            core::Result<std::string> objects,
                            core::Result<std::string> index,
                            core::Result<std::string> behaviors = std::string{}) -> core::Result<void> {
        if (auto value = fill(set.graphics, std::move(graphics)); !value) return value;
        if (auto value = fill(set.palette, std::move(palette)); !value) return value;
        if (auto value = fill(set.map16, std::move(map16)); !value) return value;
        if (auto value = fill(set.objects, std::move(objects)); !value) return value;
        if (auto value = fill(set.object_index, std::move(index)); !value) return value;
        if (behaviors && !behaviors.value().empty()) return fill(set.behaviors, std::move(behaviors));
        return {};
    };
    if (auto value = fill_tileset(result.tilesets[0], named("d_2d_A_J_jyotyu_ncg.bin"), named("d_2d_A_J_jyotyu_ncl.bin"),
                                  named("d_2d_PA_A_J_jyotyu.bin"), named("A_J_jyotyu.bin"), named("A_J_jyotyu_hd.bin"));
        !value) return value.error();
    result.tilesets[1].tile_offset = 192;
    result.tilesets[1].palette_offset = 2;
    result.tilesets[1].tile_page_offset = 1;
    if (auto value = fill_tileset(result.tilesets[1], table(profile->tileset_graphics, references.tileset_id),
                                  table(profile->tileset_palette, references.tileset_id), table(profile->tileset_map16, references.tileset_id),
                                  table(profile->tileset_objects, references.tileset_id), table(profile->tileset_index, references.tileset_id),
                                  table(profile->tileset_behaviors, references.tileset_id));
        !value) return value.error();
    result.tilesets[2].tile_offset = 640;
    result.tilesets[2].palette_offset = 6;
    result.tilesets[2].tile_page_offset = 4;
    if (auto value = fill_tileset(result.tilesets[2], named("d_2d_I_S_tikei_nohara_ncg.bin"), named("d_2d_I_S_tikei_nohara_ncl.bin"),
                                  named("d_2d_PA_I_S_nohara.bin"), named("I_S_nohara.bin"), named("I_S_nohara_hd.bin"),
                                  named("NoHaRaSubUnitChangeData.bin"));
        !value) return value.error();

    auto fill_background = [&](BackgroundResourceFiles& background,
                               std::size_t graphics_table,
                               std::size_t palette_table,
                               std::size_t screen_table,
                               std::uint8_t graphics_id,
                               std::uint8_t screen_id) -> core::Result<void> {
        if (auto value = fill(background.graphics, table(graphics_table, graphics_id)); !value) return value;
        if (auto value = fill(background.palette, table(palette_table, graphics_id)); !value) return value;
        return fill(background.screen, table(screen_table, screen_id));
    };
    if (auto value = fill_background(result.background, profile->background_graphics, profile->background_palette,
                                     profile->background_screen, references.background_graphics_id, references.background_screen_id);
        !value) return value.error();
    if (auto value = fill_background(result.foreground, profile->foreground_graphics, profile->foreground_palette,
                                     profile->foreground_screen, references.foreground_graphics_id, references.foreground_screen_id);
        !value) return value.error();

    constexpr std::uint16_t actor_type_count = 326;
    for (std::uint16_t type = 0; type < actor_type_count; ++type) {
        const auto class_offset = profile->sprite_object_ids + static_cast<std::size_t>(type) * 2U;
        if (class_offset + 1U >= overlay.value().size()) continue;
        const auto class_id = static_cast<std::uint16_t>(overlay.value()[class_offset] | (overlay.value()[class_offset + 1U] << 8U));
        ActorResourceFiles actor;
        actor.type = type;
        actor.object_id = class_id;
        actor.name = "Actor " + std::to_string(type) + " (class " + std::to_string(class_id) + ")";
        result.actors.push_back(std::move(actor));
    }
    std::ranges::sort(result.watched_files);
    result.watched_files.erase(std::unique(result.watched_files.begin(), result.watched_files.end()), result.watched_files.end());
    return result;
}

core::Result<DecodedTileset> load_tileset_graphics(const TilesetResourceFiles& files) {
    auto graphics = unwrap_resource(files.graphics.data);
    auto palette_data = unwrap_resource(files.palette.data);
    if (!graphics) return graphics.error();
    if (!palette_data) return palette_data.error();
    auto tiles = formats::decode_8bpp_tiles(graphics.value());
    auto palette = formats::decode_bgr555_palette(palette_data.value());
    auto map16 = formats::decode_map16(files.map16.data, files.tile_offset, files.palette_offset);
    auto objects = formats::decode_object_layouts(files.objects.data, files.object_index.data, files.tile_page_offset);
    if (!tiles) return tiles.error();
    if (!palette) return palette.error();
    if (!map16) return map16.error();
    if (!objects) return objects.error();
    if (palette.value().size() > 256) palette.value()[256].transparent = true;
    DecodedTileset result;
    result.objects = std::move(objects.value());
    result.map16_tiles.reserve(map16.value().size());
    for (const auto& tile : map16.value()) {
        auto image = formats::render_screen_map(tile.tiles, 2, tiles.value(), palette.value());
        if (!image) return image.error();
        result.map16_tiles.push_back(std::move(image.value()));
    }
    return result;
}

core::Result<formats::RgbaImage> render_tileset_object(const DecodedTileset& tileset,
                                                       std::uint16_t object_id,
                                                       std::size_t width,
                                                       std::size_t height) {
    if (object_id >= tileset.objects.size()) return core::make_error(core::ErrorCode::NotFound, "tileset object ID is not defined");
    auto layout = formats::render_object_layout(tileset.objects[object_id], width, height);
    if (!layout) return layout.error();
    formats::RgbaImage image{width * 16U, height * 16U, {}};
    image.pixels.assign(image.width * image.height * 4U, 0);
    for (std::size_t y = 0; y < height; ++y) {
        for (std::size_t x = 0; x < width; ++x) {
            const auto tile = layout.value()[y * width + x];
            if (tile >= 0 && static_cast<std::size_t>(tile) < tileset.map16_tiles.size()) {
                copy_image(tileset.map16_tiles[static_cast<std::size_t>(tile)], image, x * 16U, y * 16U);
            }
        }
    }
    return image;
}

core::Result<formats::RgbaImage> load_background_graphics(const BackgroundResourceFiles& files) {
    auto graphics = unwrap_resource(files.graphics.data);
    auto palette_data = unwrap_resource(files.palette.data);
    auto screen_data = unwrap_resource(files.screen.data);
    if (!graphics) return graphics.error();
    if (!palette_data) return palette_data.error();
    if (!screen_data) return screen_data.error();
    auto tiles = formats::decode_8bpp_tiles(graphics.value());
    auto palette = formats::decode_bgr555_palette(palette_data.value(), false);
    auto screen = formats::decode_screen_map(screen_data.value());
    if (!tiles) return tiles.error();
    if (!palette) return palette.error();
    if (!screen) return screen.error();
    const auto width = screen.value().size() % 64U == 0 ? 64U : 32U;
    return formats::render_screen_map(screen.value(), width, tiles.value(), palette.value());
}

} // namespace nsmb::project
