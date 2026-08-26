#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::core {

class LittleEndianReader {
public:
    explicit LittleEndianReader(std::span<const std::uint8_t> data);
    [[nodiscard]] Result<std::uint16_t> u16(std::size_t offset) const;
    [[nodiscard]] Result<std::uint32_t> u32(std::size_t offset) const;
private:
    std::span<const std::uint8_t> data_;
};

class LittleEndianWriter {
public:
    explicit LittleEndianWriter(std::vector<std::uint8_t>& data);
    Result<void> u16(std::size_t offset, std::uint16_t value);
    Result<void> u32(std::size_t offset, std::uint32_t value);
private:
    std::vector<std::uint8_t>& data_;
};

} // namespace nsmb::core
