#include "nsmb/formats/identify.hpp"

#include "nsmb/formats/message.hpp"

namespace nsmb::formats {

namespace {

bool magic(std::span<const std::uint8_t> data, char a, char b, char c, char d) {
    return data.size() >= 4 && data[0] == static_cast<std::uint8_t>(a) && data[1] == static_cast<std::uint8_t>(b) &&
           data[2] == static_cast<std::uint8_t>(c) && data[3] == static_cast<std::uint8_t>(d);
}

} // namespace

FormatKind identify_format(std::span<const std::uint8_t> data, std::string_view path_hint) {
    if (magic(data, 'N', 'A', 'R', 'C')) {
        return FormatKind::NarcArchive;
    }
    if (identify_graphics_format(data, path_hint) != GraphicsFormatKind::Unknown) {
        return FormatKind::Graphics;
    }
    if (is_bmg(data, path_hint)) {
        return FormatKind::MessageBundle;
    }
    if (path_hint.find("bgdat") != std::string_view::npos) {
        return FormatKind::BgdatCandidate;
    }
    if (path_hint.find("stage") != std::string_view::npos || path_hint.find("course") != std::string_view::npos) {
        return FormatKind::LevelCandidate;
    }
    return FormatKind::Unknown;
}

std::string_view format_name(FormatKind kind) {
    switch (kind) {
    case FormatKind::NarcArchive:
        return "NARC archive";
    case FormatKind::Graphics:
        return "graphics";
    case FormatKind::MessageBundle:
        return "BMG message bundle";
    case FormatKind::BgdatCandidate:
        return "NSMB bgdat candidate";
    case FormatKind::LevelCandidate:
        return "NSMB level candidate";
    case FormatKind::Unknown:
        return "unknown";
    }
    return "unknown";
}

} // namespace nsmb::formats
