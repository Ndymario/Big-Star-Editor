#include "nsmb/project/rom_reader.hpp"

#include <algorithm>

#include "nsmb/formats/graphics.hpp"

namespace nsmb::project {
namespace {

using core::ErrorCode;
using core::make_error;

// Short on purpose. An editor works within one level's resources at a time, and
// keeping more decompressed overlays or parsed archives than that spends memory
// answering questions nobody asked.
constexpr std::size_t cache_capacity = 4;

// Most-recently-used first, so the entry a repeat question wants is at the
// front and the one to drop is at the back.
template <typename Cache, typename Key>
auto* promote(Cache& cache, const Key& key) {
    const auto found = std::ranges::find(cache, key, [](const auto& entry) -> const Key& { return entry.first; });
    if (found == cache.end()) return decltype(&found->second){};
    std::rotate(cache.begin(), found, found + 1);
    return &cache.front().second;
}

template <typename Cache, typename Entry>
auto* remember(Cache& cache, Entry entry) {
    cache.insert(cache.begin(), std::move(entry));
    if (cache.size() > cache_capacity) cache.pop_back();
    return &cache.front().second;
}

} // namespace

core::Result<RomReader> RomReader::open(std::filesystem::path rom) {
    RomReader reader;
    reader.path_ = std::move(rom);

    std::error_code ec;
    reader.size_ = std::filesystem::file_size(reader.path_, ec);
    if (ec) return make_error(ErrorCode::NotFound, "could not measure the ROM " + reader.path_.string());

    reader.stream_.open(reader.path_, std::ios::binary);
    if (!reader.stream_) return make_error(ErrorCode::IoError, "could not open the ROM " + reader.path_.string());

    auto header_bytes = reader.read_region(0, nds::ds_cartridge_header_size, "ROM header");
    if (!header_bytes) return header_bytes.error();
    auto header = nds::parse_rom_header(header_bytes.value());
    if (!header) return header.error();
    reader.header_ = std::move(header.value());

    // The three tables are small and every question needs them, so they are the
    // one thing read up front rather than on demand.
    auto fat_bytes = reader.read_region(reader.header_.fat.rom_offset, reader.header_.fat.size, "FAT");
    if (!fat_bytes) return fat_bytes.error();
    auto fat = nds::parse_fat(fat_bytes.value());
    if (!fat) return fat.error();
    reader.fat_ = std::move(fat.value());

    auto fnt_bytes = reader.read_region(reader.header_.fnt.rom_offset, reader.header_.fnt.size, "FNT");
    if (!fnt_bytes) return fnt_bytes.error();
    auto files = nds::parse_fnt(fnt_bytes.value());
    if (!files) return files.error();
    reader.files_ = std::move(files.value());

    auto ovt_bytes = reader.read_region(reader.header_.arm9_overlay_table.rom_offset,
                                        reader.header_.arm9_overlay_table.size, "ARM9 overlay table");
    if (!ovt_bytes) return ovt_bytes.error();
    auto overlays = nds::parse_overlay_table(ovt_bytes.value());
    if (!overlays) return overlays.error();
    reader.overlays_ = std::move(overlays.value());

    return reader;
}

core::Result<std::vector<std::uint8_t>> RomReader::read_region(std::uint32_t offset, std::uint32_t size, std::string_view what) {
    if (offset > size_ || size > size_ - offset)
        return make_error(ErrorCode::OutOfBounds, std::string(what) + " extends past the end of " + path_.string());
    std::vector<std::uint8_t> data(size);
    if (size == 0) return data;
    stream_.clear();
    stream_.seekg(static_cast<std::streamoff>(offset));
    stream_.read(reinterpret_cast<char*>(data.data()), static_cast<std::streamsize>(size));
    if (stream_.gcount() != static_cast<std::streamsize>(size))
        return make_error(ErrorCode::IoError, "could not read " + std::string(what) + " from " + path_.string());
    return data;
}

std::optional<std::uint16_t> RomReader::file_id(std::string_view rom_path) const {
    const auto found = std::ranges::find_if(files_, [rom_path](const auto& file) { return file.path.generic_string() == rom_path; });
    return found == files_.end() ? std::optional<std::uint16_t>{} : found->id;
}

std::optional<std::uint32_t> RomReader::file_size(std::uint16_t id) const {
    if (id >= fat_.size()) return {};
    return fat_[id].used() ? fat_[id].size() : 0U;
}

core::Result<std::vector<std::uint8_t>> RomReader::read(std::uint16_t id) {
    if (id >= fat_.size()) return make_error(ErrorCode::NotFound, "no FAT entry for file id " + std::to_string(id));
    const auto& entry = fat_[id];
    if (!entry.used()) return std::vector<std::uint8_t>{};
    return read_region(entry.start_address, entry.size(), "NitroFS file");
}

core::Result<std::vector<std::uint8_t>> RomReader::read(std::string_view rom_path) {
    const auto id = file_id(rom_path);
    if (!id) return make_error(ErrorCode::NotFound, "the ROM does not hold " + std::string(rom_path));
    return read(*id);
}

core::Result<std::vector<std::uint8_t>> RomReader::read_prefix(std::string_view rom_path, std::size_t maximum) {
    const auto id = file_id(rom_path);
    if (!id) return make_error(ErrorCode::NotFound, "the ROM does not hold " + std::string(rom_path));
    if (*id >= fat_.size()) return make_error(ErrorCode::NotFound, "no FAT entry for " + std::string(rom_path));
    const auto& entry = fat_[*id];
    if (!entry.used()) return std::vector<std::uint8_t>{};
    return read_region(entry.start_address, static_cast<std::uint32_t>(std::min<std::uintmax_t>(entry.size(), maximum)),
                       "NitroFS file");
}

core::Result<std::vector<std::uint8_t>> RomReader::read_overlay(std::uint32_t overlay_id) {
    if (const auto* cached = promote(overlay_cache_, overlay_id)) return *cached;

    const auto found = std::ranges::find(overlays_, overlay_id, &nds::OverlayTableEntry::overlay_id);
    if (found == overlays_.end()) return make_error(ErrorCode::NotFound, "the ROM has no overlay " + std::to_string(overlay_id));
    if (found->file_id > 0xEFFF) return make_error(ErrorCode::ParseError, "overlay file id is outside the NitroFS range");

    auto stored = read(static_cast<std::uint16_t>(found->file_id));
    if (!stored) return stored.error();
    auto raw = formats::decompress_blz_overlay(stored.value());
    if (!raw) return raw.error();
    return *remember(overlay_cache_, std::pair{overlay_id, std::move(raw.value())});
}

core::Result<std::vector<std::uint8_t>> RomReader::read_arm9() {
    return read_region(header_.arm9.rom_offset, header_.arm9.size, "ARM9 binary");
}

core::Result<std::vector<std::uint8_t>> RomReader::read_banner() {
    if (header_.banner_rom_offset == 0 || header_.banner_rom_offset >= size_)
        return make_error(ErrorCode::NotFound, "the ROM declares no banner");

    // A banner does not declare its own length. Its version implies one, and
    // whatever region comes next bounds it; the implied length wins only when
    // it fits.
    const auto limit = nds::banner_region_limit(header_, fat_, static_cast<std::size_t>(size_));
    const auto available = limit - static_cast<std::size_t>(header_.banner_rom_offset);
    auto version = read_region(header_.banner_rom_offset, 2, "banner version");
    if (!version) return version.error();
    const auto implied = nds::banner_size_from_version(version.value());
    const auto size = implied != 0 && implied <= available ? implied : available;
    return read_region(header_.banner_rom_offset, static_cast<std::uint32_t>(size), "banner");
}

core::Result<const formats::NarcArchive*> RomReader::archive(std::string_view rom_path) {
    const std::string key(rom_path);
    if (const auto* cached = promote(archive_cache_, key)) return cached;

    auto data = read(rom_path);
    if (!data) return data.error();

    // A container may have arrived compressed -- Mario Kart DS names those
    // `.carc` -- and is an ordinary NARC underneath. What decides is what the
    // bytes decode to, never the extension.
    if (!formats::is_narc(data.value())) {
        auto raw = formats::decompress_nitro_lz(data.value());
        if (!raw) return make_error(ErrorCode::Unsupported, std::string(rom_path) + " is not a Nitro archive");
        data = std::move(raw.value());
    }
    auto parsed = formats::parse_narc(data.value());
    if (!parsed) return parsed.error();
    return remember(archive_cache_, std::pair{key, std::move(parsed.value())});
}

bool RomReader::is_archive(std::string_view rom_path) { return archive(rom_path).has_value(); }

core::Result<std::vector<ArchiveMember>> RomReader::list_archive(std::string_view rom_path) {
    auto opened = archive(rom_path);
    if (!opened) return opened.error();
    const auto& narc = *opened.value();

    auto names = nds::parse_fnt(narc.name_table);
    if (!names) return names.error();
    // An index the name table does not name keeps its row, empty, so that a
    // member's position in this list stays its index.
    std::vector<ArchiveMember> members(narc.files.size());
    for (std::size_t i = 0; i < narc.files.size(); ++i)
        members[i].size = static_cast<std::uint32_t>(narc.files[i].size());
    for (const auto& name : names.value())
        if (name.id < members.size()) members[name.id].path = name.path.generic_string();
    return members;
}

core::Result<std::vector<std::uint8_t>> RomReader::read_member(std::string_view rom_path, std::size_t index) {
    auto opened = archive(rom_path);
    if (!opened) return opened.error();
    if (index >= opened.value()->files.size())
        return make_error(ErrorCode::NotFound, std::string(rom_path) + " has no member " + std::to_string(index));
    return opened.value()->files[index];
}

core::Result<std::vector<std::uint8_t>> RomReader::read_member(std::string_view rom_path, std::string_view member) {
    auto opened = archive(rom_path);
    if (!opened) return opened.error();
    auto index = opened.value()->find(std::filesystem::path(member));
    if (!index) return index.error();
    if (index.value() >= opened.value()->files.size())
        return make_error(ErrorCode::NotFound, std::string(rom_path) + " names a member its FAT does not hold");
    return opened.value()->files[index.value()];
}

} // namespace nsmb::project
