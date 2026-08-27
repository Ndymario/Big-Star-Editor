#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <vector>

#include "nsmb/formats/graphics.hpp"

namespace nsmb::formats {

using FourCc = std::array<char, 4>;

struct Nitro2dBlock {
    FourCc magic{};
    std::vector<std::uint8_t> data;
};

struct Nitro2dFile {
    FourCc magic{};
    std::uint16_t byte_order{0xfeff};
    std::uint16_t version{0x0100};
    std::vector<std::uint8_t> header_extra;
    std::vector<Nitro2dBlock> blocks;
};

struct Nclr {
    Nitro2dFile file;
    std::uint32_t format{};
    bool extended_palette{};
    std::vector<std::uint8_t> palette;
    std::optional<std::vector<std::uint16_t>> compressed_palette_indices;
};

struct NcgrPosition { std::uint16_t x{}, y{}, width{}, height{}; };

struct Ncgr {
    Nitro2dFile file;
    std::uint16_t height{}, width{};
    std::uint32_t pixel_format{}, mapping_type{}, character_format{};
    std::vector<std::uint8_t> characters;
    std::optional<NcgrPosition> position;
};

struct Nscr {
    Nitro2dFile file;
    std::uint16_t width{}, height{}, color_mode{}, screen_format{};
    std::vector<std::uint8_t> screen;
};

struct NcerOam { std::uint16_t attr0{}, attr1{}, attr2{}; };
struct NcerBounds { std::int16_t max_x{}, max_y{}, min_x{}, min_y{}; };
struct NcerCell {
    std::uint16_t attributes{};
    std::vector<NcerOam> oam;
    std::optional<NcerBounds> bounds;
};
struct NcerTransfer { std::uint32_t source_offset{}, size{}; };
struct Ncer {
    Nitro2dFile file;
    std::uint16_t bank_attributes{};
    std::uint32_t mapping_mode{};
    std::vector<NcerCell> cells;
    std::uint32_t maximum_transfer_size{};
    std::vector<NcerTransfer> transfers;
};

struct NanrFrame {
    std::uint16_t duration{}, padding{0xbeef};
    std::vector<std::uint8_t> content;
};
struct NanrSequence {
    std::uint16_t loop_start{};
    std::uint32_t animation_type{}, play_mode{};
    std::vector<NanrFrame> frames;
};
struct Nanr {
    Nitro2dFile file;
    std::vector<NanrSequence> sequences;
};

core::Result<Nitro2dFile> parse_nitro_2d_file(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_nitro_2d_file(const Nitro2dFile& file);
core::Result<Nclr> parse_nclr(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_nclr(const Nclr& value);
core::Result<Ncgr> parse_ncgr(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_ncgr(const Ncgr& value);
core::Result<Nscr> parse_nscr(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_nscr(const Nscr& value);
core::Result<Ncer> parse_ncer(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_ncer(const Ncer& value);
core::Result<Nanr> parse_nanr(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_nanr(const Nanr& value);
core::Result<RgbaImage> render_ncer_cell(const Ncer& cells,
                                         std::size_t cell_index,
                                         std::span<const std::uint8_t> characters,
                                         std::span<const Bgr555Color> palette);

} // namespace nsmb::formats
