#pragma once

#include <compare>
#include <cstdint>

namespace nsmb::core {

class Fx32 {
public:
    static constexpr std::int32_t scale = 4096;
    constexpr Fx32() = default;
    static constexpr Fx32 from_raw(std::int32_t raw) { return Fx32(raw); }
    static constexpr Fx32 from_integer(std::int32_t value) { return Fx32(value * scale); }
    [[nodiscard]] constexpr std::int32_t raw() const { return raw_; }
    [[nodiscard]] constexpr double value() const { return static_cast<double>(raw_) / scale; }
    auto operator<=>(const Fx32&) const = default;
private:
    explicit constexpr Fx32(std::int32_t raw) : raw_(raw) {}
    std::int32_t raw_{};
};

} // namespace nsmb::core
