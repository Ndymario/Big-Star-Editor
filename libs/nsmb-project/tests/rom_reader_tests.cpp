#include "nsmb/project/rom_reader.hpp"

#include "synthetic_rom.hpp"

namespace {
using synthetic::Bytes;
namespace fs = std::filesystem;
using synthetic::make_narc;
using synthetic::make_rom;
using synthetic::write_bytes;
using synthetic::banner_size;
} // namespace


int main() {
    const auto dir = fs::temp_directory_path() / "big-star-rom-reader-tests";
    fs::remove_all(dir);
    fs::create_directories(dir);

    const Bytes vs_bmg{'M', 'E', 'S', 'G', 1, 2, 3, 4};
    const Bytes hd_bin{'H', 'D', 0, 0};
    const auto plain = make_narc({{"vs.bmg", vs_bmg}, {"menu/hd.bin", hd_bin}});
    const Bytes kart{'K', 'A', 'R', 'T', 0, 0, 0, 0};
    const auto packed_raw = make_narc({{"kart.bin", kart}});
    auto packed = nsmb::formats::compress_nitro_lz10(packed_raw);
    assert(packed);
    const Bytes fresh{'F', 'R', 'E', 'S', 'H'};

    // Compressible on purpose: BLZ declines to grow a file, and an overlay it
    // declined is stored raw, which would not exercise the decompression path.
    Bytes overlay_raw(0x800, 0);
    for (std::size_t i = 0; i < overlay_raw.size(); ++i) overlay_raw[i] = static_cast<std::uint8_t>(i % 7);
    auto overlay = nsmb::formats::compress_blz_overlay(overlay_raw);
    assert(overlay && !overlay.value().empty());

    const auto rom_path = dir / "build.nds";
    write_bytes(rom_path, make_rom({{"plain.narc", plain}, {"packed.carc", packed.value()}, {"z_new/fresh.bin", fresh}},
                                   overlay.value()));

    auto opened = nsmb::project::RomReader::open(rom_path);
    assert(opened);
    auto& reader = opened.value();

    assert(reader.header().game_code == "A2DE");
    assert(reader.files().size() == 3);

    // By path, and by the id that path resolves to -- the two have to agree, or
    // a resource table that names an id and a dock that names a path disagree.
    assert(reader.file_id("z_new/fresh.bin") == 2);
    assert(!reader.file_id("z_new/missing.bin"));
    assert(reader.read("z_new/fresh.bin").value() == fresh);
    assert(reader.read(std::uint16_t{2}).value() == fresh);
    assert(!reader.read("nothing/here.bin"));

    assert(reader.read_arm9().value().front() == 0x90);
    assert(reader.read_banner().value().size() == banner_size);
    assert(reader.read_banner().value()[2] == 0xAB);

    // The ROM stores the overlay compressed; the reader hands back what the
    // game would run.
    assert(reader.read_overlay(0).value() == overlay_raw);
    assert(!reader.read_overlay(7));

    // A plain container and an LZ-wrapped one are the same thing to a caller.
    assert(reader.is_archive("plain.narc"));
    assert(reader.is_archive("packed.carc"));
    assert(!reader.is_archive("z_new/fresh.bin"));
    const auto listed = reader.list_archive("plain.narc");
    assert(listed && listed.value().size() == 2);
    // Every member carries the file id its container's name table gave it --
    // which is what game code loads it by -- and the size comes from the
    // container rather than from a second read of every member.
    assert(listed.value()[0].id == 0 && listed.value()[0].path == "vs.bmg" && listed.value()[0].size == vs_bmg.size());
    assert(listed.value()[1].id == 1 && listed.value()[1].path == "menu/hd.bin" && listed.value()[1].size == hd_bin.size());
    // An id is a position in this list, so reading a member by the id the
    // listing reported reaches the member the listing named.
    assert(reader.read_member("plain.narc", std::size_t{listed.value()[1].id}).value() == hd_bin);
    // A compressed container numbers its members no differently.
    assert(reader.list_archive("packed.carc").value()[0].id == 0);
    assert(reader.list_archive("packed.carc").value()[0].path == "kart.bin");
    assert(reader.read_member("plain.narc", "vs.bmg").value() == vs_bmg);
    assert(reader.read_member("plain.narc", std::size_t{1}).value() == hd_bin);
    assert(reader.read_member("packed.carc", "kart.bin").value() == kart);
    assert(!reader.read_member("plain.narc", "menu/nope.bin"));
    assert(!reader.read_member("plain.narc", std::size_t{9}));

    // Repeat questions are what the cache exists for, and have to answer the
    // same thing as the first one.
    assert(reader.read_overlay(0).value() == overlay_raw);
    assert(reader.read_member("plain.narc", "vs.bmg").value() == vs_bmg);

    assert(!nsmb::project::RomReader::open(dir / "absent.nds"));

    fs::remove_all(dir);
    return 0;
}
