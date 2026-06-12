#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::core {

class ByteBuffer {
public:
    ByteBuffer() = default;
    explicit ByteBuffer(std::vector<std::uint8_t> bytes);

    [[nodiscard]] std::size_t size() const noexcept;
    [[nodiscard]] bool empty() const noexcept;
    [[nodiscard]] std::span<const std::uint8_t> bytes() const noexcept;
    [[nodiscard]] std::span<std::uint8_t> bytes() noexcept;

    [[nodiscard]] Result<std::uint8_t> at(std::size_t offset) const;
    [[nodiscard]] Result<std::span<const std::uint8_t>> slice(std::size_t offset, std::size_t length) const;
    Result<void> write(std::size_t offset, std::span<const std::uint8_t> data);
    void append(std::span<const std::uint8_t> data);

private:
    std::vector<std::uint8_t> bytes_;
};

} // namespace nsmb::core
