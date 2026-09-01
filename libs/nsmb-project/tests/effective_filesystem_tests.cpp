#include "nsmb/project/effective_filesystem.hpp"

#include "synthetic_rom.hpp"

#include <cassert>
#include <fstream>
#include <string>

namespace {

namespace fs = std::filesystem;
using synthetic::Bytes;

void write_text(const fs::path& path, const std::string& text) {
    fs::create_directories(path.parent_path());
    std::ofstream out(path, std::ios::binary);
    out << text;
}

Bytes bytes_of(const std::string& text) { return Bytes(text.begin(), text.end()); }

nsmb::patching::RomFileEntry entry(std::string path, std::string source) {
    nsmb::patching::RomFileEntry file;
    file.path = std::move(path);
    file.source = std::move(source);
    file.module = "coop";
    file.action = nsmb::patching::FileAction::Modified;
    return file;
}

} // namespace

int main() {
    const auto dir = fs::temp_directory_path() / "big-star-effective-filesystem-tests";
    fs::remove_all(dir);
    fs::create_directories(dir);

    const Bytes vs_bmg{'M', 'E', 'S', 'G', 1, 2, 3, 4};
    const Bytes hd_bin{'H', 'D', 0, 0};
    const Bytes fresh{'F', 'R', 'E', 'S', 'H'};
    const auto plain = synthetic::make_narc({{"vs.bmg", vs_bmg}, {"menu/hd.bin", hd_bin}});
    const auto packed_raw = synthetic::make_narc({{"kart.bin", Bytes{'K', 'A', 'R', 'T'}}});
    auto packed = nsmb::formats::compress_nitro_lz10(packed_raw);
    assert(packed);
    Bytes overlay_raw(0x400, 0);
    for (std::size_t i = 0; i < overlay_raw.size(); ++i) overlay_raw[i] = static_cast<std::uint8_t>(i % 5);
    auto overlay = nsmb::formats::compress_blz_overlay(overlay_raw);
    assert(overlay);

    const auto rom_path = dir / "big-star-build.nds";
    synthetic::write_bytes(rom_path, synthetic::make_rom({{"plain.narc", plain}, {"packed.carc", packed.value()},
                                                          {"z_new/fresh.bin", fresh}}, overlay.value()));

    auto opened = nsmb::project::RomReader::open(rom_path);
    assert(opened);

    nsmb::project::EffectiveFilesystem files;
    files.set_rom(std::move(opened.value()));

    // Split first, because everything below turns on it: an address with a `!`
    // names something inside a container, and one without names the container.
    assert(nsmb::project::split_rom_address("plain.narc").container == "plain.narc");
    assert(!nsmb::project::split_rom_address("plain.narc").inside_archive());
    assert(nsmb::project::split_rom_address("plain.narc!menu/hd.bin").member == "menu/hd.bin");

    // With no plan, everything is the ROM's answer.
    assert(files.read("z_new/fresh.bin").value() == fresh);
    assert(files.read("plain.narc!vs.bmg").value() == vs_bmg);
    assert(files.source("z_new/fresh.bin").empty());
    assert(files.path_for_id(0) == "plain.narc");
    assert(!files.path_for_id(9));

    write_text(dir / "modules/coop/nitrofs/z_new/fresh.bin", "EDITED");
    write_text(dir / "modules/message/nitrofs/plain_narc/vs.bmg", "PENDING");

    nsmb::patching::FileManifest plan;
    plan.planned = true;
    plan.files.push_back(entry("z_new/fresh.bin", "modules/coop/nitrofs/z_new/fresh.bin"));
    // A source the plan names but the disk does not hold -- a `pre-build` hook
    // writes this one -- has no bytes to offer, so the ROM still answers.
    plan.files.push_back(entry("packed.carc", "build/generated/packed.carc"));

    auto archive = entry("plain.narc", {});
    nsmb::patching::RomArchiveMember member;
    member.index = 0;
    member.path = "vs.bmg";
    member.source = "modules/message/nitrofs/plain_narc/vs.bmg";
    member.module = "message";
    archive.members.push_back(member);
    plan.files.push_back(std::move(archive));
    files.set_plan(dir, plan);

    // A module edit that has not been built yet is what the editor has to show,
    // or the edit is invisible until a rebuild.
    assert(files.read("z_new/fresh.bin").value() == bytes_of("EDITED"));
    assert(files.source("z_new/fresh.bin") == dir / "modules/coop/nitrofs/z_new/fresh.bin");

    // One edited member does not take its siblings out of the ROM.
    assert(files.read("plain.narc!vs.bmg").value() == bytes_of("PENDING"));
    assert(files.read("plain.narc!menu/hd.bin").value() == hd_bin);
    assert(files.source("plain.narc!menu/hd.bin").empty());

    // The container's own bytes are still the ROM's: sweeping members back into
    // it is a build's job, not a reader's.
    assert(files.read("plain.narc").value() == plain);

    // A source the plan names and the disk lacks falls back rather than failing.
    assert(files.source("packed.carc").empty());
    assert(files.read("packed.carc").value() == packed.value());

    // Nothing supplies this at all.
    assert(!files.read("nowhere/at/all.bin"));

    // Without a ROM only what a module supplies can be read, which is the state
    // a project is in before its first build.
    nsmb::project::EffectiveFilesystem unbuilt;
    unbuilt.set_plan(dir, plan);
    assert(unbuilt.read("z_new/fresh.bin").value() == bytes_of("EDITED"));
    assert(!unbuilt.read("plain.narc!menu/hd.bin"));
    assert(!unbuilt.path_for_id(0));

    fs::remove_all(dir);
    return 0;
}
