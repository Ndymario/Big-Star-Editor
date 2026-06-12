#include "nsmb/core/byte_buffer.hpp"

namespace nsmb::core {

ByteBuffer::ByteBuffer(std::vector<std::uint8_t> bytes) : bytes_(std::move(bytes)) {}
std::size_t ByteBuffer::size() const noexcept { return bytes_.size(); }
bool ByteBuffer::empty() const noexcept { return bytes_.empty(); }
std::span<const std::uint8_t> ByteBuffer::bytes() const noexcept { return bytes_; }
std::span<std::uint8_t> ByteBuffer::bytes() noexcept { return bytes_; }

Result<std::uint8_t> ByteBuffer::at(std::size_t offset) const {
    if (offset >= bytes_.size()) return make_error(ErrorCode::OutOfBounds, "byte offset is outside the buffer");
    return bytes_[offset];
}

Result<std::span<const std::uint8_t>> ByteBuffer::slice(std::size_t offset, std::size_t length) const {
    if (offset > bytes_.size() || length > bytes_.size() - offset) {
        return make_error(ErrorCode::OutOfBounds, "slice extends past the end of the buffer");
    }
    return std::span<const std::uint8_t>(bytes_.data() + offset, length);
}

Result<void> ByteBuffer::write(std::size_t offset, std::span<const std::uint8_t> data) {
    if (offset > bytes_.size() || data.size() > bytes_.size() - offset) {
        return make_error(ErrorCode::OutOfBounds, "write extends past the end of the buffer");
    }
    std::copy(data.begin(), data.end(), bytes_.begin() + static_cast<std::ptrdiff_t>(offset));
    return {};
}

void ByteBuffer::append(std::span<const std::uint8_t> data) { bytes_.insert(bytes_.end(), data.begin(), data.end()); }

} // namespace nsmb::core
