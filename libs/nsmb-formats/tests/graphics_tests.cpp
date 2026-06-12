#include "nsmb/formats/graphics.hpp"
#include "nsmb/formats/identify.hpp"

#include <cassert>
#include <cstdint>
#include <vector>

int main() {
    using namespace nsmb::formats;

    std::vector<std::uint8_t> ncgr_header = {'R', 'G', 'C', 'N', 0xff, 0xfe, 0, 1, 16, 0, 0, 0, 16, 0, 1, 0};
    auto header = parse_nitro_header(ncgr_header);
    assert(header);
    assert(header.value().magic == "RGCN");
    assert(identify_graphics_format(ncgr_header, {}) == GraphicsFormatKind::NitroCharacterGraphics);
    assert(identify_format(ncgr_header, {}) == FormatKind::Graphics);

    std::vector<std::uint8_t> palette = {0x00, 0x00, 0x1f, 0x00, 0xe0, 0x03, 0x00, 0x7c};
    auto colors = decode_bgr555_palette(palette);
    assert(colors);
    assert(colors.value().size() == 4);
    assert(colors.value()[0].transparent);
    assert(colors.value()[1].red == 255);
    assert(colors.value()[2].green == 255);
    assert(colors.value()[3].blue == 255);

    assert(identify_graphics_format({}, "UI_O_1P_game_in_d_ncg.bin") == GraphicsFormatKind::RawCharacterGraphicsCandidate);
    assert(identify_graphics_format({}, "enemy/w_cannon.nsbmd") == GraphicsFormatKind::NitroModel);
    return 0;
}
