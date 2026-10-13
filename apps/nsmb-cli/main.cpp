#include "nsmb/formats/graphics.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/nds/rom.hpp"
#include "nsmb/patching/ncpatcher.hpp"
#include "nsmb/project/manifest.hpp"
#include "nsmb/formats/g2d.hpp"
#include "nsmb/formats/message.hpp"
#include "nsmb/formats/narc.hpp"
#include "nsmb/formats/identify.hpp"
#include "nsmb/formats/tex0.hpp"
#include "nsmb/formats/ui_resources.hpp"
#include "nsmb/project/editable_level.hpp"
#include "nsmb/project/effective_filesystem.hpp"
#include "nsmb/project/graphics_resources.hpp"
#include "nsmb/project/level_tilemap.hpp"
#include "nsmb/project/lua_actors.hpp"
#include "nsmb/project/lua_tiles.hpp"
#include "nsmb/project/rom_reader.hpp"

#include <algorithm>
#include <array>
#include <cstdio>
#include <span>

#include <filesystem>
#include <iostream>

namespace fs = std::filesystem;

namespace {

void usage() {
    std::cout << "nsmb-cli " BSE_VERSION "\n"
              << "Commands:\n"
              << "  --version\n"
              << "  inspect-rom <path>\n"
              << "  import-rom <rom> <project-dir>\n"
              << "  run-patcher <project-dir> [--variant <name>]\n"
              << "  patcher-info <project-dir>\n"
              << "  oracle <project-dir> <data-dir>\n";
}

std::size_t count_graphics_candidates(const nsmb::project::Project& project) {
    std::size_t count = 0;
    for (const auto& file : project.game_files()) {
        if (nsmb::formats::identify_graphics_format({}, file.path.generic_string()) != nsmb::formats::GraphicsFormatKind::Unknown) {
            ++count;
        }
    }
    return count;
}


// ---------------------------------------------------------------------------
// oracle <project> <data-dir>
//
// One line per fact, printed identically by `bigstar-host oracle`, so `cargo
// xtask oracle` can diff the two implementations on a real project: every ROM
// file's format and the hash of what parsing and writing it back produces,
// every course's entities and decoded graphics, and every shipped actor's
// bounds and draw list at default settings. Error messages differ between the
// two by design, so only error codes are printed.
// ---------------------------------------------------------------------------

class Sha256 {
public:
    void update(std::span<const std::uint8_t> data) {
        for (const auto byte : data) {
            block_[used_++] = byte;
            ++length_;
            if (used_ == 64) { compress(); used_ = 0; }
        }
    }
    void update(std::string_view text) {
        update(std::span(reinterpret_cast<const std::uint8_t*>(text.data()), text.size()));
    }
    void update_u32(std::uint32_t value) {
        const std::array<std::uint8_t, 4> bytes{static_cast<std::uint8_t>(value), static_cast<std::uint8_t>(value >> 8U),
                                                static_cast<std::uint8_t>(value >> 16U), static_cast<std::uint8_t>(value >> 24U)};
        update(bytes);
    }
    std::string hex() {
        const auto bits = length_ * 8U;
        const std::uint8_t pad = 0x80;
        update(std::span(&pad, 1));
        const std::uint8_t zero = 0;
        while (used_ != 56) update(std::span(&zero, 1));
        for (int shift = 56; shift >= 0; shift -= 8) block_[used_++] = static_cast<std::uint8_t>(bits >> shift);
        compress();
        static constexpr char digits[] = "0123456789abcdef";
        std::string out;
        for (const auto word : state_)
            for (int shift = 28; shift >= 0; shift -= 4) out.push_back(digits[(word >> shift) & 0xfU]);
        return out;
    }

private:
    void compress() {
        static constexpr std::array<std::uint32_t, 64> k{
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01,
            0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
            0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
            0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
            0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08,
            0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
            0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2};
        const auto rotr = [](std::uint32_t x, int n) { return (x >> n) | (x << (32 - n)); };
        std::array<std::uint32_t, 64> w{};
        for (int i = 0; i < 16; ++i)
            w[i] = static_cast<std::uint32_t>(block_[i * 4]) << 24U | static_cast<std::uint32_t>(block_[i * 4 + 1]) << 16U |
                   static_cast<std::uint32_t>(block_[i * 4 + 2]) << 8U | block_[i * 4 + 3];
        for (int i = 16; i < 64; ++i) {
            const auto s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3U);
            const auto s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10U);
            w[i] = w[i - 16] + s0 + w[i - 7] + s1;
        }
        auto [a, b, c, d, e, f, g, h] = state_;
        for (int i = 0; i < 64; ++i) {
            const auto t1 = h + (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ (~e & g)) + k[i] + w[i];
            const auto t2 = (rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c));
            h = g; g = f; f = e; e = d + t1; d = c; c = b; b = a; a = t1 + t2;
        }
        const std::array<std::uint32_t, 8> add{a, b, c, d, e, f, g, h};
        for (int i = 0; i < 8; ++i) state_[i] += add[i];
    }

    std::array<std::uint32_t, 8> state_{0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    std::array<std::uint8_t, 64> block_{};
    std::size_t used_{};
    std::uint64_t length_{};
};

std::string sha(std::span<const std::uint8_t> data) { Sha256 hash; hash.update(data); return hash.hex(); }

std::string sha_image(const nsmb::formats::RgbaImage& image) {
    Sha256 hash;
    hash.update_u32(static_cast<std::uint32_t>(image.width));
    hash.update_u32(static_cast<std::uint32_t>(image.height));
    hash.update(image.pixels);
    return hash.hex();
}

std::string_view code_name(nsmb::core::ErrorCode code) {
    switch (code) {
    case nsmb::core::ErrorCode::InvalidArgument: return "InvalidArgument";
    case nsmb::core::ErrorCode::OutOfBounds: return "OutOfBounds";
    case nsmb::core::ErrorCode::IoError: return "IoError";
    case nsmb::core::ErrorCode::ParseError: return "ParseError";
    case nsmb::core::ErrorCode::NotFound: return "NotFound";
    case nsmb::core::ErrorCode::ExternalToolFailed: return "ExternalToolFailed";
    case nsmb::core::ErrorCode::Unsupported: return "Unsupported";
    default: return "Unknown";
    }
}

std::string failed(const nsmb::core::Error& error) { return "E:" + std::string(code_name(error.code)); }

template <typename T>
std::string hashed(const nsmb::core::Result<T>& result) { return result ? sha(result.value()) : failed(result.error()); }

std::string number(double value) {
    char buffer[64];
    std::snprintf(buffer, sizeof buffer, "%.4f", value);
    return buffer;
}

bool starts_with(std::span<const std::uint8_t> data, std::string_view magic) {
    return data.size() >= magic.size() && std::equal(magic.begin(), magic.end(), data.begin(),
                                                     [](char a, std::uint8_t b) { return static_cast<std::uint8_t>(a) == b; });
}

// What parsing and writing back produces, for the formats that have both.
std::string rewrite(std::span<const std::uint8_t> data, std::string_view path) {
    namespace f = nsmb::formats;
    if (starts_with(data, "RLCN")) { auto v = f::parse_nclr(data); return " nclr " + (v ? hashed(f::write_nclr(v.value())) : failed(v.error())); }
    if (starts_with(data, "RGCN")) { auto v = f::parse_ncgr(data); return " ncgr " + (v ? hashed(f::write_ncgr(v.value())) : failed(v.error())); }
    if (starts_with(data, "RCSN")) { auto v = f::parse_nscr(data); return " nscr " + (v ? hashed(f::write_nscr(v.value())) : failed(v.error())); }
    if (starts_with(data, "RECN")) { auto v = f::parse_ncer(data); return " ncer " + (v ? hashed(f::write_ncer(v.value())) : failed(v.error())); }
    if (starts_with(data, "RNAN")) { auto v = f::parse_nanr(data); return " nanr " + (v ? hashed(f::write_nanr(v.value())) : failed(v.error())); }
    if (starts_with(data, "BTX0") || starts_with(data, "BMD0") || starts_with(data, "TEX0")) {
        auto v = f::parse_tex0(data);
        if (!v) return " tex0 " + failed(v.error());
        Sha256 rendered;
        for (const auto& texture : v.value().textures) {
            const auto palette = v.value().palettes.empty() ? f::Tex0Palette{} : v.value().palettes.front();
            auto image = f::render_tex0_texture(texture, palette);
            rendered.update(image ? sha_image(image.value()) : failed(image.error()));
        }
        return " tex0 " + hashed(f::write_tex0(v.value())) + " textures=" + std::to_string(v.value().textures.size()) + " render=" + rendered.hex();
    }
    if (f::is_bmg(data, path)) {
        auto v = f::parse_bmg(data);
        if (!v) return " bmg " + failed(v.error());
        return " bmg " + hashed(f::write_bmg(v.value())) + " messages=" + std::to_string(v.value().messages.size());
    }
    switch (f::identify_ui_resource(data, path)) {
    case f::UiResourceKind::Bnbl: { auto v = f::parse_bnbl(data); return " bnbl " + (v ? hashed(f::write_bnbl(v.value())) : failed(v.error())); }
    case f::UiResourceKind::Bncl: { auto v = f::parse_bncl(data); return " bncl " + (v ? hashed(f::write_bncl(v.value())) : failed(v.error())); }
    case f::UiResourceKind::Bncd: { auto v = f::parse_bncd(data); return " bncd " + (v ? hashed(f::write_bncd(v.value())) : failed(v.error())); }
    case f::UiResourceKind::Enpg: { auto v = f::parse_enpg(data); return " enpg " + (v ? hashed(f::write_enpg(v.value())) : failed(v.error())); }
    default: return {};
    }
}

void report_file(const std::string& indent, const std::string& label, const std::string& path,
                 std::span<const std::uint8_t> data, bool members) {
    namespace f = nsmb::formats;
    std::string line = indent + label + " " + std::string(f::format_name(f::identify_format(data, path))) + " size=" + std::to_string(data.size());
    std::vector<std::uint8_t> raw(data.begin(), data.end());
    if (!data.empty() && (data[0] == 0x10 || data[0] == 0x11)) {
        if (auto unpacked = f::decompress_nitro_lz(data)) {
            const auto repacked = data[0] == 0x10 ? f::compress_nitro_lz10(unpacked.value()) : f::compress_nitro_lz11(unpacked.value());
            line += std::string(data[0] == 0x10 ? " lz10" : " lz11") + " raw=" + sha(unpacked.value()) + " repack=" + hashed(repacked);
            raw = std::move(unpacked.value());
        }
    }
    if (f::is_narc(raw)) {
        auto narc = f::parse_narc(raw);
        if (!narc) { std::cout << line << " narc " << failed(narc.error()) << '\n'; return; }
        std::cout << line << " narc " << hashed(f::write_narc(narc.value())) << " members=" << narc.value().files.size() << '\n';
        if (!members) return;
        for (std::size_t i = 0; i < narc.value().files.size(); ++i)
            report_file(indent + "  ", "member " + std::to_string(i), path + "!" + std::to_string(i), narc.value().files[i], false);
        return;
    }
    std::cout << line << rewrite(raw, path) << '\n';
}

void report_level(nsmb::project::EffectiveFilesystem& files, const std::string& course_path, const std::string& bgdat_path,
                  const nsmb::project::TileOverlayCatalog& overlays) {
    std::cout << "level " << course_path << '\n';
    auto course = files.read(course_path);
    auto bgdat = files.read(bgdat_path);
    if (!course || !bgdat) { std::cout << "  read " << failed(!course ? course.error() : bgdat.error()) << '\n'; return; }
    auto loaded = nsmb::project::EditableLevel::load(course.value(), bgdat.value());
    if (!loaded) { std::cout << "  load " << failed(loaded.error()) << '\n'; return; }
    const auto& level = loaded.value();
    const auto& c = level.course();
    const auto& h = c.header;
    std::cout << "  header " << +h.entrances[0] << ' ' << +h.entrances[1] << ' ' << h.settings << ' ' << h.timer;
    for (const auto& themes : {h.background_theme, h.tileset_theme, h.foreground_theme})
        for (const auto theme : themes) std::cout << ' ' << theme;
    std::cout << ' ' << h.events << ' ' << h.sound_group << ' ' << h.sprite_priority << ' ' << h.mega_drop_chance << '\n';
    for (const auto& camera : c.cameras)
        std::cout << "  camera " << camera.top_offset << ' ' << camera.bottom_offset << ' ' << camera.top_offset_special << ' '
                  << camera.bottom_offset_special << ' ' << camera.id << ' ' << camera.scroll_height << ' ' << camera.unknown << '\n';
    const auto screens = [](std::string_view name, const auto& list) {
        for (const auto& s : list)
            std::cout << "  " << name << ' ' << s.id << ' ' << s.screen_id << ' ' << s.palette_id << ' ' << s.parallax_x << ' '
                      << s.parallax_y << ' ' << s.scroll_x << ' ' << s.scroll_y << ' ' << s.vertical_offset << ' ' << s.unused_10 << ' '
                      << s.unused_12 << '\n';
    };
    screens("background", c.backgrounds);
    screens("tileset", c.tilesets);
    screens("foreground", c.foregrounds);
    for (const auto& o : level.objects())
        std::cout << "  object " << o.object_id << ' ' << +o.tileset_slot << ' ' << o.x << ' ' << o.y << ' ' << o.width << ' ' << o.height << '\n';
    for (const auto& s : c.sprites)
        std::cout << "  sprite " << s.id << ' ' << s.x << ' ' << s.y << ' ' << +s.events[0] << ' ' << +s.events[1] << ' ' << s.settings << '\n';
    for (const auto& e : c.entrances)
        std::cout << "  entrance " << e.x << ' ' << e.y << ' ' << e.camera_x << ' ' << e.camera_y << ' ' << +e.id << ' ' << +e.target_stage_area
                  << ' ' << +e.target_stage_index << ' ' << +e.target_stage_group << ' ' << +e.target_entrance << ' ' << +e.camera_zoom << ' '
                  << +e.type << ' ' << +e.flags << ' ' << +e.unknown_10 << ' ' << +e.unknown_11 << ' ' << +e.view_id << ' ' << +e.target_worldmap_node << '\n';
    for (const auto& v : c.views)
        std::cout << "  view " << v.x << ' ' << v.y << ' ' << v.width << ' ' << v.height << ' ' << +v.id << ' ' << +v.camera_settings_id << ' '
                  << +v.bgm_id << ' ' << +v.background_id << ' ' << +v.tileset_id << ' ' << +v.foreground_id << ' ' << +v.light_type << ' '
                  << +v.progress_path_id << '\n';
    for (const auto& z : c.zones)
        std::cout << "  zone " << z.x << ' ' << z.y << ' ' << z.width << ' ' << z.height << ' ' << +z.id << ' ' << +z.unknown_9 << ' '
                  << +z.unknown_a << ' ' << +z.unknown_b << '\n';
    const auto paths = [](std::string_view name, const auto& list) {
        for (const auto& p : list) {
            std::cout << "  " << name << ' ' << +p.id << ' ' << p.unknown << " nodes=" << p.nodes.size() << '\n';
            for (const auto& n : p.nodes)
                std::cout << "    node " << n.x << ' ' << n.y << ' ' << n.speed.raw() << ' ' << n.acceleration.raw() << ' ' << n.delay << ' '
                          << n.flags << '\n';
        }
    };
    paths("path", c.paths);
    paths("progress", c.progress_paths);
    std::cout << "  banks";
    for (const auto bank : c.object_banks) std::cout << ' ' << +bank;
    std::cout << '\n';
    std::cout << "  rewrite course=" << hashed(level.serialize_course()) << " bgdat=" << hashed(level.serialize_bgdat()) << '\n';

    const auto references = level.tileset_references();
    auto resources = nsmb::project::resolve_level_graphics(files, references);
    if (!resources) { std::cout << "  graphics " << failed(resources.error()) << '\n'; return; }
    const auto tilesets = nsmb::project::load_level_tilesets(resources.value().tilesets, references.tileset_id, &overlays);
    for (std::size_t slot = 0; slot < 3; ++slot) {
        Sha256 pixels;
        for (const auto& block : tilesets.tilesets[slot].map16_tiles) pixels.update(sha_image(block));
        std::cout << "  tileset " << slot << " blocks=" << tilesets.tilesets[slot].map16_tiles.size() << " objects="
                  << tilesets.tilesets[slot].objects.size() << " pixels=" << pixels.hex() << (tilesets.errors[slot].empty() ? "" : " error") << '\n';
    }
    const std::array<const nsmb::project::DecodedTileset*, 3> decoded{&tilesets.tilesets[0], &tilesets.tilesets[1], &tilesets.tilesets[2]};
    const auto tilemap = nsmb::project::rasterize_level_objects(level.objects(), decoded);
    Sha256 cells;
    for (const auto& chunk : tilemap.chunks) {
        cells.update_u32(static_cast<std::uint32_t>(chunk.x));
        cells.update_u32(static_cast<std::uint32_t>(chunk.y));
        for (const auto& cell : chunk.cells) {
            cells.update_u32(static_cast<std::uint32_t>(cell.block));
            const std::uint8_t slot = cell.slot;
            cells.update(std::span(&slot, 1));
        }
    }
    std::cout << "  tilemap chunks=" << tilemap.chunks.size() << " cells=" << cells.hex() << '\n';
    const auto background = nsmb::project::load_background_graphics(resources.value().background);
    const auto foreground = nsmb::project::load_background_graphics(resources.value().foreground);
    std::cout << "  background " << (background ? sha_image(background.value()) : failed(background.error())) << " foreground "
              << (foreground ? sha_image(foreground.value()) : failed(foreground.error())) << '\n';
}

std::string draw_kind(nsmb::project::LuaActorDrawKind kind) {
    switch (kind) {
    case nsmb::project::LuaActorDrawKind::Image: return "image";
    case nsmb::project::LuaActorDrawKind::Text: return "text";
    case nsmb::project::LuaActorDrawKind::FillRect: return "fillRect";
    case nsmb::project::LuaActorDrawKind::StrokeRect: return "strokeRect";
    case nsmb::project::LuaActorDrawKind::FillEllipse: return "fillEllipse";
    case nsmb::project::LuaActorDrawKind::Line: return "line";
    }
    return "?";
}

std::string box(const nsmb::core::Result<std::optional<nsmb::project::SpriteBounds>>& bounds) {
    if (!bounds) return failed(bounds.error());
    if (!bounds.value()) return "none";
    const auto& b = *bounds.value();
    return std::to_string(b.x) + "," + std::to_string(b.y) + "," + std::to_string(b.width) + "," + std::to_string(b.height);
}

// The stills are opaque at the size their header declares: the oracle compares
// geometry, and a PNG codec of its own would only add a third implementation.
nsmb::core::Result<nsmb::formats::RgbaImage> oracle_png(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 24 || bytes[1] != 'P' || bytes[2] != 'N' || bytes[3] != 'G')
        return nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "not a PNG");
    const auto read = [&](std::size_t at) {
        return static_cast<std::size_t>(bytes[at]) << 24U | static_cast<std::size_t>(bytes[at + 1]) << 16U |
               static_cast<std::size_t>(bytes[at + 2]) << 8U | static_cast<std::size_t>(bytes[at + 3]);
    };
    nsmb::formats::RgbaImage image{read(16), read(20), {}};
    if (!image.width || !image.height || image.width > 8192U || image.height > 8192U)
        return nsmb::core::make_error(nsmb::core::ErrorCode::ParseError, "bad PNG size");
    image.pixels.assign(image.width * image.height * 4U, 0xff);
    return image;
}

int oracle(const fs::path& project_dir, const fs::path& data_dir) {
    auto manifest = nsmb::project::load_manifest(project_dir / nsmb::project::manifest_file_name);
    if (!manifest || !manifest.value().rom()) {
        std::cerr << (manifest ? std::string("the project has no ROM") : manifest.error().message) << '\n';
        return 2;
    }
    const auto rom_path = project_dir / manifest.value().rom()->rom_path;
    auto opened = nsmb::project::RomReader::open(rom_path);
    if (!opened) { std::cerr << opened.error().message << '\n'; return 2; }
    nsmb::project::EffectiveFilesystem files;
    files.set_rom(std::move(opened.value()));
    auto& rom = *files.rom();

    std::cout << "rom " << rom.header().game_code << " files=" << rom.files().size() << '\n';
    for (const auto& file : rom.files()) {
        const auto path = file.path.generic_string();
        auto data = rom.read(file.id);
        if (!data) { std::cout << "file " << file.id << ' ' << path << ' ' << failed(data.error()) << '\n'; continue; }
        report_file("", "file " + std::to_string(file.id) + " " + path, path, data.value(), true);
    }
    for (std::uint32_t id = 0;; ++id) {
        auto overlay = rom.read_overlay(id);
        if (!overlay) break;
        std::cout << "overlay9 " << id << " raw=" << sha(overlay.value()) << " repack=" << hashed(nsmb::formats::compress_blz_overlay(overlay.value())) << '\n';
    }
    if (auto banner = rom.read_banner()) {
        auto parsed = nsmb::formats::parse_ds_banner(banner.value());
        std::cout << "banner " << (parsed ? hashed(nsmb::formats::write_ds_banner(parsed.value())) : failed(parsed.error())) << '\n';
    }

    const auto resolve = [&](nsmb::project::ActorResourceKind kind, std::string_view name) -> nsmb::core::Result<nsmb::project::ActorResource> {
        if (kind == nsmb::project::ActorResourceKind::Image) {
            auto bytes = nsmb::core::read_file(data_dir / "actors" / name);
            if (!bytes) return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, "missing");
            return nsmb::project::ActorResource{std::string(name), data_dir / "actors" / name, std::move(bytes.value())};
        }
        auto bytes = files.read(name);
        if (!bytes) return nsmb::core::make_error(nsmb::core::ErrorCode::NotFound, bytes.error().message);
        return nsmb::project::ActorResource{std::string(name), {}, std::move(bytes.value())};
    };
    const std::vector<nsmb::project::LuaTileLayer> tile_layers{{"shipped", data_dir / "tiles"}};
    auto overlays = nsmb::project::load_lua_tile_layers(tile_layers, resolve);
    if (!overlays) { std::cerr << overlays.error().message << '\n'; return 3; }
    for (const auto& script : overlays.value().scripts) {
        std::size_t textured = 0;
        for (const auto& [block, overlay] : script.blocks) textured += overlay.image ? 1U : 0U;
        std::cout << "tiles " << script.name << " blocks=" << script.blocks.size() << " textured=" << textured << " errors=" << script.errors.size() << '\n';
    }

    std::vector<std::string> paths;
    for (const auto& file : rom.files()) paths.push_back(file.path.generic_string());
    for (const auto& path : paths) {
        if (!path.starts_with("course/") || !path.ends_with(".bin") || path.ends_with("_bgdat.bin")) continue;
        const auto bgdat = path.substr(0, path.size() - 4) + "_bgdat.bin";
        if (std::ranges::find(paths, bgdat) == paths.end()) continue;
        report_level(files, path, bgdat, overlays.value());
    }

    const std::vector<nsmb::project::LuaActorLayer> actor_layers{{"shipped", data_dir / "actors"}};
    auto actors = nsmb::project::load_lua_actor_layers(actor_layers, resolve, oracle_png);
    if (!actors) { std::cerr << actors.error().message << '\n'; return 3; }
    for (const auto& actor : actors.value().actors) {
        std::cout << "actor " << actor.source.filename().string() << " object=" << (actor.object_id ? std::to_string(*actor.object_id) : "-")
                  << " fields=" << actor.sprite.fields.size() << " init=" << (actor.init_error.empty() ? "ok" : "error")
                  << " renderer=" << (nsmb::project::has_lua_renderer(actor) ? 1 : 0) << '\n';
        nsmb::project::LuaActorRenderInput input;
        input.object_id = actor.sprite.object_id;
        input.hash = actor.hash;
        std::cout << "  bounds grab=" << box(nsmb::project::lua_actor_bounds(actor, input, false))
                  << " render=" << box(nsmb::project::lua_actor_bounds(actor, input, true)) << '\n';
        bool custom = false;
        auto commands = nsmb::project::render_lua_actor(actor, input, &custom);
        if (!commands) { std::cout << "  draw " << failed(commands.error()) << '\n'; continue; }
        std::cout << "  draw custom=" << (custom ? 1 : 0) << " commands=" << commands.value().size() << '\n';
        for (const auto& c : commands.value()) {
            char color[16];
            std::snprintf(color, sizeof color, "%08x", c.color);
            std::cout << "    " << draw_kind(c.kind) << " src=" << c.source_x << ',' << c.source_y << ',' << c.source_width << ','
                      << c.source_height << " dst=" << number(c.x) << ',' << number(c.y) << ',' << number(c.width) << ',' << number(c.height)
                      << " rot=" << number(c.rotation) << " op=" << number(c.opacity) << " end=" << number(c.x2) << ',' << number(c.y2)
                      << " stroke=" << number(c.stroke_width) << " color=" << color << " flip=" << (c.flip_x ? 1 : 0) << (c.flip_y ? 1 : 0)
                      << " text=" << c.text << " image=" << (c.image ? std::to_string(c.image->width) + "x" + std::to_string(c.image->height) : "-") << '\n';
        }
    }
    return 0;
}

} // namespace

int main(int argc, char** argv) {
    if (argc < 2) {
        usage();
        return 1;
    }

    const std::string command = argv[1];
    if (command == "--version") {
        std::cout << "nsmb-cli " BSE_VERSION "\n";
        return 0;
    }

    if (command == "inspect-rom" && argc == 3) {
        auto data = nsmb::core::read_file_prefix(argv[2], 0x4000);
        if (!data) {
            std::cerr << data.error().message << '\n';
            return 2;
        }
        auto header = nsmb::nds::parse_rom_header(data.value());
        if (!header) {
            std::cerr << header.error().message << '\n';
            return 3;
        }
        std::cout << "Title: " << header.value().game_title << "\nGame code: " << header.value().game_code << "\n";
        return 0;
    }

    if (command == "import-rom" && argc == 4) {
        const fs::path rom_path = argv[2];
        const fs::path project_dir = argv[3];

        std::error_code ec;
        fs::create_directories(project_dir, ec);
        if (ec) {
            std::cerr << "Could not create project directory: " << ec.message() << '\n';
            return 4;
        }
        const auto manifest_exists = fs::exists(project_dir / nsmb::project::manifest_file_name, ec);
        if (ec) {
            std::cerr << "Could not inspect project directory: " << ec.message() << '\n';
            return 4;
        }
        if (manifest_exists) {
            std::cerr << "Project already exists: " << (project_dir / nsmb::project::manifest_file_name) << '\n';
            return 4;
        }

        auto data = nsmb::core::read_file_prefix(rom_path, 0x4000);
        if (!data) {
            std::cerr << data.error().message << '\n';
            return 5;
        }
        auto header = nsmb::nds::parse_rom_header(data.value());
        if (!header) {
            std::cerr << header.error().message << '\n';
            return 6;
        }

        // The ROM is read where it lies rather than unpacked, so importing one
        // is copying it in and reading its table -- and a manifest path has to
        // stay inside the project, so a ROM from elsewhere is copied rather
        // than pointed at.
        const auto imported = project_dir / nsmb::project::input_rom_file_name;
        if (fs::weakly_canonical(rom_path, ec) != fs::weakly_canonical(imported, ec)) {
            fs::copy_file(rom_path, imported, fs::copy_options::overwrite_existing, ec);
            if (ec) {
                std::cerr << "Could not copy the ROM into the project: " << ec.message() << '\n';
                return 7;
            }
        }

        auto project_name = project_dir.filename().string();
        if (project_name.empty() || project_name == ".") {
            project_name = header.value().game_title.empty() ? "Imported ROM" : header.value().game_title;
        }
        auto project = nsmb::project::create_empty_project(project_name, project_dir).value();
        project.attach_rom(nsmb::project::input_rom_file_name, header.value());
        auto indexed = nsmb::project::index_rom_files(project);
        if (!indexed) {
            std::cerr << indexed.error().message << '\n';
            return 8;
        }

        auto saved = nsmb::project::save_manifest(project, project_dir / nsmb::project::manifest_file_name);
        if (!saved) {
            std::cerr << saved.error().message << '\n';
            return 8;
        }

        std::cout << "Imported ROM into " << project_dir << "\n"
                  << "Files indexed: " << project.game_files().size() << "\n"
                  << "Graphics candidates: " << count_graphics_candidates(project) << "\n";
        return 0;
    }

    if (command == "run-patcher" && (argc == 3 || argc == 5)) {
        nsmb::patching::BuildRequest request;
        request.project_dir = argv[2];
        // `run-patcher <project> --variant <name>`. A project that configures
        // variants requires an explicit one, so NCPatcher cannot silently build
        // the wrong language.
        if (argc == 5) {
            if (std::string(argv[3]) != "--variant") {
                usage();
                return 2;
            }
            request.variant = argv[4];
        }

        nsmb::core::CallbackLogger logger([](nsmb::core::LogLevel level, std::string_view message) {
            std::cerr << '[' << nsmb::core::log_level_name(level) << "] " << message << '\n';
        });
        nsmb::patching::NCPatcherClient client("ncpatcher", logger);

        nsmb::patching::BuildObserver observer;
        observer.on_progress = [](const nsmb::patching::BuildProgress& progress) {
            std::cerr << progress.phase << ' ' << progress.current << '/' << progress.total
                      << "  " << progress.item << '\n';
        };
        observer.on_diagnostic = [](const nsmb::patching::Diagnostic& diagnostic) {
            std::cerr << diagnostic.code << ": " << diagnostic.message << '\n';
        };
        observer.on_log = [](std::string_view line) { std::cerr << line << '\n'; };

        auto result = client.build(request, observer);
        if (!result) {
            std::cerr << result.error().message << '\n';
            return 10;
        }
        const auto& summary = result.value();
        if (!summary.succeeded()) {
            std::cerr << "NCPatcher: " << describe_stage(summary.stage) << " ("
                      << summary.errors << " error(s), " << summary.warnings << " warning(s))\n";
        }
        return summary.exit_code;
    }

    if (command == "patcher-info" && argc == 3) {
        // What the project says about itself, without building it.
        nsmb::patching::NCPatcherClient client;
        const fs::path project_dir = argv[2];

        auto version = client.version();
        if (!version) {
            std::cerr << version.error().message << '\n';
            return 10;
        }
        std::cout << version.value() << '\n';

        auto config = client.dump_config(project_dir);
        if (!config) {
            std::cerr << config.error().message << '\n';
            return 10;
        }
        std::cout << "Config: " << config.value().config_file << '\n'
                  << "ROM: " << config.value().rom_file << '\n'
                  << "Toolchain: " << config.value().toolchain << '\n';
        std::cout << "Variants: ";
        if (config.value().variants.empty()) {
            std::cout << "(this project configures none)";
        } else {
            for (const auto& variant : config.value().variants) {
                std::cout << variant.name << ' ';
            }
        }
        std::cout << '\n';

        auto graph = client.dump_modules(project_dir);
        if (graph) {
            std::cout << "Modules (" << graph.value().modules.size() << "):\n";
            for (const auto& module : graph.value().modules) {
                std::cout << "  " << module.key << (module.enabled ? "" : " (disabled)");
                if (module.nitrofs) {
                    std::cout << " nitrofs=" << module.nitrofs->dir
                              << (module.nitrofs->layered ? " [layered]" : "");
                }
                std::cout << '\n';
            }
        }
        return 0;
    }

    if (command == "oracle" && argc == 4) {
        return oracle(argv[2], argv[3]);
    }

    usage();
    return 1;
}
