#include "nsmb/formats/identify.hpp"
namespace nsmb::formats {
static bool magic(std::span<const std::uint8_t> d, char a, char b, char c, char e) { return d.size() >= 4 && d[0] == static_cast<std::uint8_t>(a) && d[1] == static_cast<std::uint8_t>(b) && d[2] == static_cast<std::uint8_t>(c) && d[3] == static_cast<std::uint8_t>(e); }
FormatKind identify_format(std::span<const std::uint8_t> data, std::string_view path_hint) {
    if (magic(data, 'N', 'A', 'R', 'C')) return FormatKind::NarcArchive;
    if (magic(data, 'B', 'M', 'D', '0')) return FormatKind::NsbmdModel;
    if (magic(data, 'B', 'T', 'X', '0')) return FormatKind::NsbtxTexture;
    if (path_hint.find("bgdat") != std::string_view::npos) return FormatKind::BgdatCandidate;
    if (path_hint.find("stage") != std::string_view::npos || path_hint.find("course") != std::string_view::npos) return FormatKind::LevelCandidate;
    return FormatKind::Unknown;
}
std::string_view format_name(FormatKind kind) {
    switch (kind) { case FormatKind::NarcArchive: return "NARC archive"; case FormatKind::NsbmdModel: return "NSBMD model"; case FormatKind::NsbtxTexture: return "NSBTX texture"; case FormatKind::BgdatCandidate: return "NSMB bgdat candidate"; case FormatKind::LevelCandidate: return "NSMB level candidate"; default: return "unknown"; }
}
}
