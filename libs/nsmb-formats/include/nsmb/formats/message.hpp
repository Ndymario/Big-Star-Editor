#pragma once

#include <array>
#include <cstdint>
#include <optional>
#include <span>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::formats {

// The encoding byte NDS BMG files carry at header offset 0x10.
enum class BmgEncoding : std::uint8_t { Unknown = 0, Cp1252 = 1, Utf16 = 2, ShiftJis = 3, Utf8 = 4 };

// One run of a message: either literal text or one of the game's inline escape sequences.
struct BmgPart {
    bool escape{};
    std::string text;                     // UTF-8, meaningful only when escape is false
    std::uint8_t escape_type{};
    std::vector<std::uint8_t> escape_data;
    bool operator==(const BmgPart&) const = default;
};

struct BmgMessage {
    std::vector<BmgPart> parts;
    std::vector<std::uint8_t> attributes;  // INF1 entry bytes after the 4-byte DAT1 offset
    std::optional<std::uint32_t> id;       // MID1 message ID, when the file carries that section
    bool operator==(const BmgMessage&) const = default;
};

// A section this model does not interpret (STR1, FLW1, FLI1, ...), kept so saves stay lossless.
struct BmgSection {
    std::string magic;
    std::vector<std::uint8_t> body;        // bytes after the 8-byte section header, padding included
    bool operator==(const BmgSection&) const = default;
};

struct Bmg {
    BmgEncoding encoding{BmgEncoding::Utf16};
    std::uint16_t file_id{};
    std::uint8_t default_color{};
    std::uint8_t inf1_reserved{};                    // the INF1 byte after the default colour
    std::uint16_t attribute_size{};                  // INF1 entry size minus the 4-byte offset
    bool has_mid1{};
    std::array<std::uint8_t, 6> mid1_reserved{};     // MID1 bytes after its entry count
    std::array<std::uint8_t, 15> header_reserved{};  // header bytes 0x11..0x1F
    std::vector<BmgMessage> messages;
    std::vector<BmgSection> extra_sections;
    std::vector<std::string> section_order;          // section magics in the order they were read
};

[[nodiscard]] bool is_bmg(std::span<const std::uint8_t> bytes, std::string_view path = {});
[[nodiscard]] std::string_view bmg_encoding_name(BmgEncoding encoding);
core::Result<Bmg> parse_bmg(std::span<const std::uint8_t> bytes);
core::Result<std::vector<std::uint8_t>> write_bmg(const Bmg& value);

// Messages round-trip through the "[type:HEXDATA]" convention the NDS BMG Editor uses, so
// text exported from one tool can be pasted into the other.
[[nodiscard]] std::string bmg_message_text(const BmgMessage& message);
[[nodiscard]] std::vector<BmgPart> parse_bmg_message_text(std::string_view text);
[[nodiscard]] std::string bmg_message_plain_text(const BmgMessage& message);

} // namespace nsmb::formats
