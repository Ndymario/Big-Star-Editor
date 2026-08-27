#include "nsmb/project/level_exchange.hpp"

#include "nsmb/formats/graphics.hpp"

#include <algorithm>
#include <array>
#include <filesystem>
#include <limits>
#include <unordered_set>

namespace nsmb::project {
namespace {

constexpr std::size_t max_blob_size = 128U * 1024U * 1024U;
constexpr std::string_view nml_header = "NSMBe5 Exported Level";
constexpr std::string_view old_nml_header = "NSMBe4 Exported Level";
constexpr std::array<std::uint8_t, 16> patch_header{'B','I','G','S','T','A','R','-','P','A','T','C','H',0,0,1};
constexpr std::array<std::uint8_t, 16> recovery_header{'B','I','G','S','T','A','R','-','R','E','C','O','V','E','R',1};

void u16(std::vector<std::uint8_t>& out, std::uint16_t value) {
    out.push_back(static_cast<std::uint8_t>(value)); out.push_back(static_cast<std::uint8_t>(value >> 8U));
}
void u32(std::vector<std::uint8_t>& out, std::uint32_t value) {
    for (int shift = 0; shift < 32; shift += 8) out.push_back(static_cast<std::uint8_t>(value >> shift));
}

void string(std::vector<std::uint8_t>& out, std::string_view value) {
    u32(out, static_cast<std::uint32_t>(value.size()));
    out.insert(out.end(), value.begin(), value.end());
}

void dotnet_string(std::vector<std::uint8_t>& out, std::string_view value) {
    auto size = static_cast<std::uint32_t>(value.size());
    do { auto byte = static_cast<std::uint8_t>(size & 0x7fU); size >>= 7U; if (size) byte |= 0x80U; out.push_back(byte); } while (size);
    out.insert(out.end(), value.begin(), value.end());
}

class Reader {
public:
    explicit Reader(std::span<const std::uint8_t> data) : data_(data) {}
    core::Result<std::uint16_t> u16() { auto value = take(2); if (!value) return value.error(); return static_cast<std::uint16_t>(value.value()[0] | value.value()[1] << 8U); }
    core::Result<std::uint32_t> u32() { auto value = take(4); if (!value) return value.error(); return static_cast<std::uint32_t>(value.value()[0]) | static_cast<std::uint32_t>(value.value()[1]) << 8U | static_cast<std::uint32_t>(value.value()[2]) << 16U | static_cast<std::uint32_t>(value.value()[3]) << 24U; }
    core::Result<std::span<const std::uint8_t>> take(std::size_t size) {
        if (size > data_.size() - offset_) return core::make_error(core::ErrorCode::OutOfBounds, "exchange file is truncated");
        const auto result = data_.subspan(offset_, size); offset_ += size; return result;
    }
    core::Result<std::string> string() { auto size = u32(); if (!size) return size.error(); return text(size.value()); }
    core::Result<std::string> dotnet_string() {
        std::uint32_t size{}; int shift{};
        for (; shift < 35; shift += 7) { auto byte = take(1); if (!byte) return byte.error(); size |= static_cast<std::uint32_t>(byte.value()[0] & 0x7fU) << shift; if (!(byte.value()[0] & 0x80U)) return text(size); }
        return core::make_error(core::ErrorCode::ParseError, "exchange string length is invalid");
    }
    [[nodiscard]] bool done() const { return offset_ == data_.size(); }
private:
    core::Result<std::string> text(std::size_t size) {
        if (size > max_blob_size) return core::make_error(core::ErrorCode::ParseError, "exchange string is too large");
        auto value = take(size); if (!value) return value.error(); return std::string(value.value().begin(), value.value().end());
    }
    std::span<const std::uint8_t> data_; std::size_t offset_{};
};

core::Result<void> validate_package(const LevelPackage& package) {
    if (package.course.empty()) return core::make_error(core::ErrorCode::InvalidArgument, "level package has no course data");
    if (package.course.size() > max_blob_size || package.bgdat.size() > max_blob_size)
        return core::make_error(core::ErrorCode::InvalidArgument, "level package is too large");
    return {};
}

bool safe_patch_path(std::string_view text) {
    const std::filesystem::path path(text);
    if (text.empty() || path.is_absolute()) return false;
    return std::ranges::none_of(path, [](const auto& component) { return component == ".." || component == "."; });
}

core::Result<LevelPackage> read_package_body(Reader& reader) {
    LevelPackage result;
    auto course_id = reader.u16(); if (!course_id) return course_id.error(); result.course_file_id = course_id.value();
    auto bgdat_id = reader.u16(); if (!bgdat_id) return bgdat_id.error(); result.bgdat_file_id = bgdat_id.value();
    auto course_size = reader.u32(); if (!course_size) return course_size.error();
    if (course_size.value() > max_blob_size) return core::make_error(core::ErrorCode::ParseError, "course data is too large");
    auto course = reader.take(course_size.value()); if (!course) return course.error(); result.course.assign(course.value().begin(), course.value().end());
    auto bgdat_size = reader.u32(); if (!bgdat_size) return bgdat_size.error();
    if (bgdat_size.value() > max_blob_size) return core::make_error(core::ErrorCode::ParseError, "BGDat data is too large");
    auto bgdat = reader.take(bgdat_size.value()); if (!bgdat) return bgdat.error(); result.bgdat.assign(bgdat.value().begin(), bgdat.value().end());
    if (auto valid = validate_package(result); !valid) return valid.error();
    return result;
}

void write_package_body(std::vector<std::uint8_t>& out, const LevelPackage& package) {
    u16(out, package.course_file_id); u16(out, package.bgdat_file_id);
    u32(out, static_cast<std::uint32_t>(package.course.size())); out.insert(out.end(), package.course.begin(), package.course.end());
    u32(out, static_cast<std::uint32_t>(package.bgdat.size())); out.insert(out.end(), package.bgdat.begin(), package.bgdat.end());
}

} // namespace

core::Result<std::vector<std::uint8_t>> encode_nml(const LevelPackage& package) {
    if (auto valid = validate_package(package); !valid) return valid.error();
    std::vector<std::uint8_t> result; result.reserve(package.course.size() + package.bgdat.size() + 64);
    dotnet_string(result, nml_header); u16(result, 1); write_package_body(result, package); return result;
}

core::Result<LevelPackage> decode_nml(std::span<const std::uint8_t> bytes) {
    Reader reader(bytes); auto header = reader.dotnet_string(); if (!header) return header.error();
    if (header.value() != nml_header && header.value() != old_nml_header)
        return core::make_error(core::ErrorCode::Unsupported, "file is not an NSMB exported level");
    auto version = reader.u16(); if (!version) return version.error();
    if (version.value() > 1) return core::make_error(core::ErrorCode::Unsupported, "exported level version is newer than Big Star supports");
    auto result = read_package_body(reader); if (!result) return result.error();
    if (!reader.done()) return core::make_error(core::ErrorCode::ParseError, "exported level has trailing data");
    return result;
}

core::Result<std::vector<std::uint8_t>> encode_nsmbpatch(std::span<const NsmbPatchEntry> entries) {
    if (entries.size() > 65535) return core::make_error(core::ErrorCode::InvalidArgument, "patch has too many files");
    std::unordered_set<std::string> paths; std::vector<std::uint8_t> result(patch_header.begin(), patch_header.end()); u32(result, static_cast<std::uint32_t>(entries.size()));
    for (const auto& entry : entries) {
        if (!safe_patch_path(entry.path) || !paths.insert(entry.path).second) return core::make_error(core::ErrorCode::InvalidArgument, "patch contains an unsafe or duplicate path");
        if (entry.contents.size() > 0xffffffU) return core::make_error(core::ErrorCode::InvalidArgument, entry.path + " is too large for LZ10");
        auto compressed = formats::compress_nitro_lz10(entry.contents); if (!compressed) return compressed.error();
        string(result, entry.path); u32(result, static_cast<std::uint32_t>(entry.expected_file_id));
        u32(result, static_cast<std::uint32_t>(compressed.value().size())); result.insert(result.end(), compressed.value().begin(), compressed.value().end());
    }
    return result;
}

core::Result<std::vector<NsmbPatchEntry>> decode_nsmbpatch(std::span<const std::uint8_t> bytes) {
    Reader reader(bytes); auto header = reader.take(patch_header.size()); if (!header) return header.error();
    if (!std::ranges::equal(header.value(), patch_header)) return core::make_error(core::ErrorCode::Unsupported, "file is not a Big Star patch");
    auto count = reader.u32(); if (!count) return count.error(); if (count.value() > 65535) return core::make_error(core::ErrorCode::ParseError, "patch has too many files");
    std::vector<NsmbPatchEntry> result; result.reserve(count.value()); std::unordered_set<std::string> paths;
    for (std::uint32_t i = 0; i < count.value(); ++i) {
        NsmbPatchEntry entry; auto path = reader.string(); if (!path) return path.error(); entry.path = std::move(path.value());
        if (!safe_patch_path(entry.path) || !paths.insert(entry.path).second) return core::make_error(core::ErrorCode::ParseError, "patch contains an unsafe or duplicate path");
        auto id = reader.u32(); if (!id) return id.error(); entry.expected_file_id = static_cast<std::int32_t>(id.value());
        auto size = reader.u32(); if (!size) return size.error(); if (size.value() > max_blob_size) return core::make_error(core::ErrorCode::ParseError, "compressed patch entry is too large");
        auto compressed = reader.take(size.value()); if (!compressed) return compressed.error();
        auto contents = formats::decompress_nitro_lz(compressed.value()); if (!contents) return contents.error(); entry.contents = std::move(contents.value());
        result.push_back(std::move(entry));
    }
    if (!reader.done()) return core::make_error(core::ErrorCode::ParseError, "patch has trailing data");
    return result;
}

core::Result<std::vector<std::uint8_t>> encode_level_recovery(const LevelRecoverySnapshot& snapshot) {
    if (!safe_patch_path(snapshot.course_path) || !safe_patch_path(snapshot.bgdat_path)) return core::make_error(core::ErrorCode::InvalidArgument, "recovery snapshot has unsafe level paths");
    if (auto valid = validate_package(snapshot.package); !valid) return valid.error();
    std::vector<std::uint8_t> result(recovery_header.begin(), recovery_header.end());
    string(result, snapshot.course_path); string(result, snapshot.bgdat_path); write_package_body(result, snapshot.package); return result;
}

core::Result<LevelRecoverySnapshot> decode_level_recovery(std::span<const std::uint8_t> bytes) {
    Reader reader(bytes); auto header = reader.take(recovery_header.size()); if (!header) return header.error();
    if (!std::ranges::equal(header.value(), recovery_header)) return core::make_error(core::ErrorCode::Unsupported, "file is not a Big Star recovery snapshot");
    LevelRecoverySnapshot result; auto course = reader.string(); if (!course) return course.error(); result.course_path = std::move(course.value());
    auto bgdat = reader.string(); if (!bgdat) return bgdat.error(); result.bgdat_path = std::move(bgdat.value());
    if (!safe_patch_path(result.course_path) || !safe_patch_path(result.bgdat_path)) return core::make_error(core::ErrorCode::ParseError, "recovery snapshot has unsafe level paths");
    auto package = read_package_body(reader); if (!package) return package.error(); result.package = std::move(package.value());
    if (!reader.done()) return core::make_error(core::ErrorCode::ParseError, "recovery snapshot has trailing data"); return result;
}

} // namespace nsmb::project
