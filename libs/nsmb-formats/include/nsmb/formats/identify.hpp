#pragma once

#include <cstdint>
#include <span>
#include <string_view>

#include "nsmb/formats/graphics.hpp"

namespace nsmb::formats {

enum class FormatKind {
    Unknown,
    NarcArchive,
    Graphics,
    MessageBundle,
    BgdatCandidate,
    LevelCandidate,
};

FormatKind identify_format(std::span<const std::uint8_t> data, std::string_view path_hint = {});
std::string_view format_name(FormatKind kind);

} // namespace nsmb::formats
