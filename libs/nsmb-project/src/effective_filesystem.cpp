#include "nsmb/project/effective_filesystem.hpp"

#include <algorithm>

#include "nsmb/core/file_io.hpp"

namespace nsmb::project {
namespace {

std::filesystem::path source_path(const std::filesystem::path& project_dir, std::string_view source) {
    if (source.empty()) return {};
    std::filesystem::path path(source);
    return path.is_absolute() ? path : project_dir / path;
}

// A source the plan names but the disk does not hold cannot be read, whatever
// the plan intends -- a `pre-build` hook has not run yet, or somebody deleted
// it. The ROM's copy is then the only answer there is.
std::filesystem::path readable(std::filesystem::path path) {
    std::error_code ec;
    return std::filesystem::is_regular_file(path, ec) ? std::move(path) : std::filesystem::path{};
}

} // namespace

RomAddress split_rom_address(std::string_view address) {
    const auto bang = address.find('!');
    if (bang == address.npos) return {std::string(address), {}};
    return {std::string(address.substr(0, bang)), std::string(address.substr(bang + 1))};
}

void EffectiveFilesystem::set_rom(std::optional<RomReader> rom) { rom_ = std::move(rom); }

void EffectiveFilesystem::set_plan(std::filesystem::path project_dir, std::optional<patching::FileManifest> plan) {
    project_dir_ = std::move(project_dir);
    plan_ = std::move(plan);
}

void EffectiveFilesystem::clear() {
    rom_.reset();
    plan_.reset();
    project_dir_.clear();
}

std::filesystem::path EffectiveFilesystem::source(std::string_view address) const {
    if (!plan_) return {};
    const auto split = split_rom_address(address);
    const auto file = std::ranges::find(plan_->files, split.container, &patching::RomFileEntry::path);
    if (file == plan_->files.end()) return {};

    if (!split.inside_archive()) return readable(source_path(project_dir_, file->source));

    const auto member = std::ranges::find(file->members, split.member, &patching::RomArchiveMember::path);
    if (member == file->members.end()) return {};
    return readable(source_path(project_dir_, member->source));
}

core::Result<std::vector<std::uint8_t>> EffectiveFilesystem::read(std::string_view address) {
    if (const auto path = source(address); !path.empty()) return core::read_file(path);
    if (!rom_) return core::make_error(core::ErrorCode::NotFound,
                                       "no built ROM is open, and nothing on disk supplies " + std::string(address));
    const auto split = split_rom_address(address);
    return split.inside_archive() ? rom_->read_member(split.container, split.member) : rom_->read(split.container);
}

core::Result<std::vector<std::uint8_t>> EffectiveFilesystem::read_prefix(std::string_view address, std::size_t maximum) {
    if (const auto path = source(address); !path.empty()) return core::read_file_prefix(path, maximum);
    const auto split = split_rom_address(address);
    if (rom_ && !split.inside_archive()) return rom_->read_prefix(split.container, maximum);
    auto data = read(address);
    if (!data) return data.error();
    if (data.value().size() > maximum) data.value().resize(maximum);
    return data;
}

std::optional<std::string> EffectiveFilesystem::path_for_id(std::uint16_t file_id) const {
    if (!rom_) return {};
    const auto& files = rom_->files();
    const auto found = std::lower_bound(files.begin(), files.end(), file_id,
                                        [](const auto& file, auto id) { return file.id < id; });
    if (found == files.end() || found->id != file_id) return {};
    return found->path.generic_string();
}

} // namespace nsmb::project
