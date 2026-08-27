#include "nsmb/formats/glue.hpp"

#include <algorithm>
#include <limits>
#include <numeric>
#include <unordered_set>

namespace nsmb::formats {
namespace {

std::uint32_t u32(std::span<const std::uint8_t> data, std::size_t offset) {
    return static_cast<std::uint32_t>(data[offset]) |
           (static_cast<std::uint32_t>(data[offset + 1]) << 8U) |
           (static_cast<std::uint32_t>(data[offset + 2]) << 16U) |
           (static_cast<std::uint32_t>(data[offset + 3]) << 24U);
}

void put_u32(std::vector<std::uint8_t>& data, std::size_t offset, std::uint32_t value) {
    data[offset] = static_cast<std::uint8_t>(value);
    data[offset + 1] = static_cast<std::uint8_t>(value >> 8U);
    data[offset + 2] = static_cast<std::uint8_t>(value >> 16U);
    data[offset + 3] = static_cast<std::uint8_t>(value >> 24U);
}

} // namespace

core::Result<GlueBlock> parse_glue_block(std::span<const std::uint8_t> data) {
    if (data.empty()) return GlueBlock{};
    if (data.size() % 4U != 0U)
        return core::make_error(core::ErrorCode::ParseError, "glue block length is not a multiple of four");
    if (data.size() < 12U)
        return core::make_error(core::ErrorCode::ParseError, "glue block is shorter than its flag count and terminator");

    const auto flag_count = static_cast<std::uint64_t>(u32(data, 0));
    const auto entries_begin64 = 4ULL + flag_count * 4ULL;
    if (entries_begin64 > data.size() || entries_begin64 + 8ULL > data.size())
        return core::make_error(core::ErrorCode::ParseError, "glue flag count does not fit the block");
    const auto entries_begin = static_cast<std::size_t>(entries_begin64);

    GlueBlock block;
    block.flags.reserve(static_cast<std::size_t>(flag_count));
    for (std::size_t offset = 4; offset < entries_begin; offset += 4) block.flags.push_back(u32(data, offset));

    struct Entry { std::uint32_t hash{}, offset{}; std::size_t table_index{}; };
    std::vector<Entry> entries;
    std::unordered_set<std::uint32_t> hashes;
    std::size_t cursor = entries_begin;
    while (true) {
        if (cursor > data.size() || 8U > data.size() - cursor)
            return core::make_error(core::ErrorCode::ParseError, "glue value table has no terminator");
        const auto hash = u32(data, cursor);
        const auto offset = u32(data, cursor + 4);
        cursor += 8;
        if (hash == 0) break;
        if (!hashes.insert(hash).second)
            return core::make_error(core::ErrorCode::ParseError, "glue value table contains a duplicate hash");
        entries.push_back({hash, offset, entries.size()});
    }
    const auto payload_begin = cursor;

    auto by_offset = entries;
    std::ranges::sort(by_offset, {}, &Entry::offset);
    for (std::size_t i = 0; i < by_offset.size(); ++i) {
        const auto start = static_cast<std::size_t>(by_offset[i].offset);
        if (start < payload_begin || start >= data.size())
            return core::make_error(core::ErrorCode::ParseError, "glue value offset is outside the payload area");
        if (i != 0 && by_offset[i - 1].offset == by_offset[i].offset)
            return core::make_error(core::ErrorCode::ParseError, "two glue values share one offset");
    }

    block.values.resize(entries.size());
    for (std::size_t order = 0; order < by_offset.size(); ++order) {
        const auto start = static_cast<std::size_t>(by_offset[order].offset);
        const auto end = order + 1U < by_offset.size() ? static_cast<std::size_t>(by_offset[order + 1U].offset) : data.size();
        auto& value = block.values[by_offset[order].table_index];
        value.hash = by_offset[order].hash;
        value.payload.assign(data.begin() + static_cast<std::ptrdiff_t>(start), data.begin() + static_cast<std::ptrdiff_t>(end));
        value.payload_order = order;
    }
    return block;
}

core::Result<std::vector<std::uint8_t>> write_glue_block(const GlueBlock& block) {
    if (block.empty()) return std::vector<std::uint8_t>{};
    if (block.flags.size() > std::numeric_limits<std::uint32_t>::max() ||
        block.values.size() > (std::numeric_limits<std::uint32_t>::max() - 12U) / 8U)
        return core::make_error(core::ErrorCode::InvalidArgument, "glue block has too many entries");
    for (const auto hash : block.flags)
        if (hash == 0) return core::make_error(core::ErrorCode::InvalidArgument, "a glue flag has the reserved zero hash");

    std::unordered_set<std::uint32_t> hashes;
    for (const auto& value : block.values) {
        if (value.hash == 0) return core::make_error(core::ErrorCode::InvalidArgument, "a glue value has the reserved zero hash");
        if (value.payload.empty()) return core::make_error(core::ErrorCode::InvalidArgument, "a glue value has no payload");
        if (!hashes.insert(value.hash).second)
            return core::make_error(core::ErrorCode::InvalidArgument, "glue values contain a duplicate hash");
    }

    const std::size_t payload_begin = 4U + block.flags.size() * 4U + (block.values.size() + 1U) * 8U;
    std::vector<std::size_t> order(block.values.size());
    std::iota(order.begin(), order.end(), 0U);
    std::ranges::stable_sort(order, [&](auto a, auto b) { return block.values[a].payload_order < block.values[b].payload_order; });

    std::vector<std::uint32_t> offsets(block.values.size());
    std::size_t total = payload_begin;
    for (const auto index : order) {
        if (total > std::numeric_limits<std::uint32_t>::max())
            return core::make_error(core::ErrorCode::InvalidArgument, "glue block is too large");
        offsets[index] = static_cast<std::uint32_t>(total);
        const auto padded = (block.values[index].payload.size() + 3U) & ~std::size_t(3U);
        if (padded < block.values[index].payload.size() || total > std::numeric_limits<std::uint32_t>::max() - padded)
            return core::make_error(core::ErrorCode::InvalidArgument, "glue block is too large");
        total += padded;
    }

    std::vector<std::uint8_t> data(total, 0);
    put_u32(data, 0, static_cast<std::uint32_t>(block.flags.size()));
    for (std::size_t i = 0; i < block.flags.size(); ++i) put_u32(data, 4U + i * 4U, block.flags[i]);
    auto cursor = 4U + block.flags.size() * 4U;
    for (std::size_t i = 0; i < block.values.size(); ++i) {
        put_u32(data, cursor, block.values[i].hash);
        put_u32(data, cursor + 4U, offsets[i]);
        cursor += 8U;
    }
    for (const auto index : order)
        std::copy(block.values[index].payload.begin(), block.values[index].payload.end(),
                  data.begin() + static_cast<std::ptrdiff_t>(offsets[index]));
    return data;
}

} // namespace nsmb::formats
