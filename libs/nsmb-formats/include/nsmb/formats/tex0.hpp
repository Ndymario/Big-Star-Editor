#pragma once

#include <cstdint>
#include <span>
#include <string>
#include <vector>

#include "nsmb/formats/graphics.hpp"

namespace nsmb::formats {

enum class TextureFormat : std::uint8_t {
    None, A3I5, Palette4, Palette16, Palette256, Compressed4x4, A5I3, Direct
};

struct Tex0Texture {
    std::string name;
    TextureFormat format{TextureFormat::None};
    std::size_t width{}, height{}, original_width{}, original_height{};
    bool color_zero_transparent{};
    std::vector<std::uint8_t> texels;
    std::vector<std::uint8_t> palette_indices;
};

struct Tex0Palette {
    std::string name;
    std::uint16_t flags{};
    std::vector<std::uint8_t> colors;
};

struct Tex0 {
    // The complete NSBTX/NSBMD, or a standalone TEX0 block. Writers preserve
    // every non-texture byte and require the same texture/palette layout.
    std::vector<std::uint8_t> original;
    std::vector<Tex0Texture> textures;
    std::vector<Tex0Palette> palettes;
};

struct Tex4x4Encoded {
    std::vector<std::uint8_t> texels;
    std::vector<std::uint8_t> palette_indices;
    std::vector<std::uint8_t> palette;
};

core::Result<Tex0> parse_tex0(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_tex0(const Tex0& value);
core::Result<RgbaImage> render_tex0_texture(const Tex0Texture& texture, const Tex0Palette& palette);
core::Result<Tex4x4Encoded> compress_tex4x4(const RgbaImage& image, std::size_t maximum_palette_groups = 8192);

} // namespace nsmb::formats
