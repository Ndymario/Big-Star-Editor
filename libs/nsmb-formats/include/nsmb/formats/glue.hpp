#pragma once

#include <cstddef>
#include <cstdint>
#include <span>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::formats {

inline constexpr std::size_t nsmb_header_size = 0x20;

struct GlueValue {
    std::uint32_t hash{};
    std::vector<std::uint8_t> payload;
    std::size_t payload_order{};
};

struct GlueBlock {
    std::vector<std::uint32_t> flags;
    std::vector<GlueValue> values; // entry-table order, not payload-offset order

    [[nodiscard]] bool empty() const { return flags.empty() && values.empty(); }
};

[[nodiscard]] constexpr std::uint32_t fnv1a_32(std::string_view text) {
    std::uint32_t hash = 2166136261U;
    for (const unsigned char byte : text) {
        hash ^= byte;
        hash *= 16777619U;
    }
    return hash;
}

core::Result<GlueBlock> parse_glue_block(std::span<const std::uint8_t> data);
core::Result<std::vector<std::uint8_t>> write_glue_block(const GlueBlock& block);

} // namespace nsmb::formats
