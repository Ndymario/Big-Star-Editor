#include "nsmb/core/endian.hpp"

namespace nsmb::core {

LittleEndianReader::LittleEndianReader(std::span<const std::uint8_t> data) : data_(data) {}
Result<std::uint16_t> LittleEndianReader::u16(std::size_t offset) const {
    if (offset > data_.size() || 2 > data_.size() - offset) return make_error(ErrorCode::OutOfBounds, "u16 read is out of bounds");
    return static_cast<std::uint16_t>(data_[offset] | (data_[offset + 1] << 8));
}
Result<std::uint32_t> LittleEndianReader::u32(std::size_t offset) const {
    if (offset > data_.size() || 4 > data_.size() - offset) return make_error(ErrorCode::OutOfBounds, "u32 read is out of bounds");
    return static_cast<std::uint32_t>(data_[offset] | (data_[offset + 1] << 8) | (data_[offset + 2] << 16) | (data_[offset + 3] << 24));
}
LittleEndianWriter::LittleEndianWriter(std::vector<std::uint8_t>& data) : data_(data) {}
Result<void> LittleEndianWriter::u16(std::size_t offset, std::uint16_t value) {
    if (offset > data_.size() || 2 > data_.size() - offset) return make_error(ErrorCode::OutOfBounds, "u16 write is out of bounds");
    data_[offset] = static_cast<std::uint8_t>(value & 0xff);
    data_[offset + 1] = static_cast<std::uint8_t>((value >> 8) & 0xff);
    return {};
}
Result<void> LittleEndianWriter::u32(std::size_t offset, std::uint32_t value) {
    if (offset > data_.size() || 4 > data_.size() - offset) return make_error(ErrorCode::OutOfBounds, "u32 write is out of bounds");
    data_[offset] = static_cast<std::uint8_t>(value & 0xff);
    data_[offset + 1] = static_cast<std::uint8_t>((value >> 8) & 0xff);
    data_[offset + 2] = static_cast<std::uint8_t>((value >> 16) & 0xff);
    data_[offset + 3] = static_cast<std::uint8_t>((value >> 24) & 0xff);
    return {};
}

} // namespace nsmb::core
