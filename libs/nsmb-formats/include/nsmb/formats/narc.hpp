#pragma once

#include <cstddef>
#include <cstdint>
#include <filesystem>
#include <span>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::formats {

struct NarcArchive {
    std::vector<std::uint8_t> name_table;
    std::vector<std::vector<std::uint8_t>> files;

    [[nodiscard]] core::Result<std::size_t> find(const std::filesystem::path& path) const;
};

[[nodiscard]] bool is_narc(std::span<const std::uint8_t> data);
core::Result<NarcArchive> parse_narc(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_narc(const NarcArchive& archive);

} // namespace nsmb::formats
