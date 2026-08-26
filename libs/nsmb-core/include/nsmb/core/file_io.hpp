#pragma once

#include <cstdint>
#include <filesystem>
#include <span>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::core {

Result<std::vector<std::uint8_t>> read_file(const std::filesystem::path& path);
Result<std::vector<std::uint8_t>> read_file_prefix(const std::filesystem::path& path, std::size_t maximum_size);
Result<void> write_file_atomically(const std::filesystem::path& path, std::span<const std::uint8_t> data);
Result<void> write_text_file_atomically(const std::filesystem::path& path, std::string_view text);

} // namespace nsmb::core
