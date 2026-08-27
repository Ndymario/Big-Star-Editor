#pragma once

#include <cstdint>
#include <span>
#include <string>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::project {

struct LevelPackage {
    std::uint16_t course_file_id{}, bgdat_file_id{};
    std::vector<std::uint8_t> course, bgdat;
};

core::Result<std::vector<std::uint8_t>> encode_nml(const LevelPackage& package);
core::Result<LevelPackage> decode_nml(std::span<const std::uint8_t> bytes);

struct NsmbPatchEntry {
    std::string path;
    std::int32_t expected_file_id{-1};
    std::vector<std::uint8_t> contents;
};

core::Result<std::vector<std::uint8_t>> encode_nsmbpatch(std::span<const NsmbPatchEntry> entries);
core::Result<std::vector<NsmbPatchEntry>> decode_nsmbpatch(std::span<const std::uint8_t> bytes);

struct LevelRecoverySnapshot {
    std::string course_path, bgdat_path;
    LevelPackage package;
};

core::Result<std::vector<std::uint8_t>> encode_level_recovery(const LevelRecoverySnapshot& snapshot);
core::Result<LevelRecoverySnapshot> decode_level_recovery(std::span<const std::uint8_t> bytes);

} // namespace nsmb::project
