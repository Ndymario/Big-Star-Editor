#include "nsmb/formats/ui_resources.hpp"

#include "nsmb/nds/rom.hpp"

#include <algorithm>
#include <iostream>

namespace {

int failures{};
void check(bool condition, const char* message) { if (!condition) { std::cerr << "FAIL: " << message << '\n'; ++failures; } }
void put16(std::vector<std::uint8_t>& bytes, std::size_t offset, std::uint16_t value) { bytes[offset] = static_cast<std::uint8_t>(value); bytes[offset + 1] = static_cast<std::uint8_t>(value >> 8U); }
void put32(std::vector<std::uint8_t>& bytes, std::size_t offset, std::uint32_t value) { for (int shift = 0; shift < 32; shift += 8) bytes[offset + shift / 8] = static_cast<std::uint8_t>(value >> shift); }

} // namespace

int main() {
    std::vector<std::uint8_t> bnbl{'J','N','B','L',0x34,0x12,1,0xaa,0x23,0xd1,0x56,0x72,8,9,0xee};
    auto touch = nsmb::formats::parse_bnbl(bnbl); check(touch && touch.value().entries[0].x == 0x123 && touch.value().entries[0].x_alignment == 1 && touch.value().entries[0].x_reserved == 3, "BNBL packed position parses");
    auto touch_out = touch ? nsmb::formats::write_bnbl(touch.value()) : nsmb::core::Result<std::vector<std::uint8_t>>(touch.error()); check(touch_out && touch_out.value() == bnbl, "BNBL untouched round trip is byte-identical");
    if (touch) { touch.value().entries.push_back({1,2,0,0,0,0,3,4}); auto changed = nsmb::formats::write_bnbl(touch.value()); check(changed && changed.value()[6] == 2 && changed.value().back() == 0xee, "BNBL add preserves trailing bytes"); }
    check(!nsmb::formats::parse_bnbl(std::vector<std::uint8_t>{'J','N','B','L',0,0,1,0}), "BNBL refuses a truncated entry table");

    std::vector<std::uint8_t> bncl{'J','N','C','L',0,0,1,0x99,1,0x20,2,0x10,7,8,9,10,0xfe};
    auto layout = nsmb::formats::parse_bncl(bncl); auto layout_out = layout ? nsmb::formats::write_bncl(layout.value()) : nsmb::core::Result<std::vector<std::uint8_t>>(layout.error()); check(layout_out && layout_out.value() == bncl, "BNCL untouched round trip is byte-identical");
    check(!nsmb::formats::parse_bncl(std::vector<std::uint8_t>{'J','N','B','L',0,0,0,0}), "BNCL refuses the wrong magic");

    std::vector<std::uint8_t> bncd(0x40, 0); std::copy_n("JNCD", 4, bncd.begin()); put16(bncd,4,0x101);put16(bncd,6,1);put32(bncd,8,0x1c);put32(bncd,12,0x24);put32(bncd,16,0x30);put32(bncd,20,0x10);bncd[0x1c]=16;bncd[0x1d]=8;put16(bncd,0x20,1);put16(bncd,0x24,0x4000);put16(bncd,0x26,0x8000);put32(bncd,0x28,0x11223344);put16(bncd,0x2c,2);put16(bncd,0x2e,1);
    auto cells = nsmb::formats::parse_bncd(bncd); auto cells_out = cells ? nsmb::formats::write_bncd(cells.value()) : nsmb::core::Result<std::vector<std::uint8_t>>(cells.error()); check(cells_out && cells_out.value() == bncd && cells.value().subentries.size() == 1, "BNCD tables and texture round trip");
    if (cells) { cells.value().entries[0].subentry_count = 2; check(!nsmb::formats::write_bncd(cells.value()), "BNCD rejects references outside subentry table"); }
    if (cells) { cells.value().texture_offset = 0xffffffffU; check(!nsmb::formats::write_bncd(cells.value()), "BNCD refuses overflowing mutable offsets"); }

    std::vector<std::uint8_t> enpg(0x10203); for (std::size_t i=0;i<enpg.size();++i) enpg[i]=static_cast<std::uint8_t>(i);
    auto image = nsmb::formats::parse_enpg(enpg); auto image_out = image ? nsmb::formats::write_enpg(image.value()) : nsmb::core::Result<std::vector<std::uint8_t>>(image.error()); check(image_out && image_out.value() == enpg, "ENPG pixels, palette, and trailing data round trip");
    check(!nsmb::formats::parse_enpg(std::vector<std::uint8_t>(0x101ff)), "ENPG refuses a truncated palette");

    std::vector<std::uint8_t> banner(0xa00,0);put16(banner,0,1);banner[0x20]=0x21;put16(banner,0x220,0x7fff);put16(banner,0x240,'T');put16(banner,0x242,'e');put16(banner,0x244,'s');put16(banner,0x246,'t');put16(banner,2,nsmb::nds::crc16(std::span<const std::uint8_t>(banner).subspan(0x20,0x820)));
    auto parsed_banner=nsmb::formats::parse_ds_banner(banner);check(parsed_banner&&nsmb::formats::banner_title(parsed_banner.value(),0)==u"Test","banner title parses");if(parsed_banner){check(static_cast<bool>(nsmb::formats::set_banner_title(parsed_banner.value(),1,u"English")),"banner title edits");auto output=nsmb::formats::write_ds_banner(parsed_banner.value());check(output&&output.value().size()==banner.size()&&output.value()[0x840]==0,"banner preserves padded size");check(output&&static_cast<std::uint16_t>(output.value()[2]|output.value()[3]<<8U)==nsmb::nds::crc16(std::span<const std::uint8_t>(output.value()).subspan(0x20,0x820)),"banner CRC updates");}
    check(!nsmb::formats::parse_ds_banner(std::vector<std::uint8_t>{9,0}), "banner refuses unsupported versions");

    const std::string metadata="[Music]\r\n0=Overworld\r\n[65535]\r\n1=Breakable block\r\n";auto user=nsmb::formats::parse_rom_user_metadata(std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t*>(metadata.data()),metadata.size()));auto user_out=user?nsmb::formats::write_rom_user_metadata(user.value()):nsmb::core::Result<std::vector<std::uint8_t>>(user.error());check(user_out&&std::string(user_out.value().begin(),user_out.value().end())==metadata,"metadata untouched round trip preserves line endings");if(user){user.value().sections[0].entries[0].value="Ground";auto changed=nsmb::formats::write_rom_user_metadata(user.value());check(changed&&std::string(changed.value().begin(),changed.value().end()).find("0=Ground")!=std::string::npos,"metadata edit writes canonical text");}
    const std::string duplicate="[A]\n0=x\n0=y\n";check(!nsmb::formats::parse_rom_user_metadata(std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t*>(duplicate.data()),duplicate.size())),"metadata duplicate key rejected");
    const std::string before_section="0=x\n";check(!nsmb::formats::parse_rom_user_metadata(std::span<const std::uint8_t>(reinterpret_cast<const std::uint8_t*>(before_section.data()),before_section.size())),"metadata refuses entries before a section");

    return failures ? 1 : 0;
}
