#include "nsmb/formats/message.hpp"

#include "nsmb/formats/identify.hpp"

#include <iostream>

namespace {

int failures{};
void check(bool condition, const char* message) { if (!condition) { std::cerr << "FAIL: " << message << '\n'; ++failures; } }

using namespace nsmb::formats;

BmgPart text(std::string value) { return BmgPart{false, std::move(value), 0, {}}; }
BmgPart escape(std::uint8_t type, std::vector<std::uint8_t> data) { return BmgPart{true, {}, type, std::move(data)}; }

Bmg sample() {
    Bmg value;
    value.encoding = BmgEncoding::Utf16;
    value.file_id = 7;
    value.attribute_size = 4;
    value.has_mid1 = true;
    value.messages.push_back(BmgMessage{{escape(255, {0, 0, 1, 0}), text(" Luigi has appeared!")}, {1, 2, 3, 4}, 100});
    value.messages.push_back(BmgMessage{{text("Café ★")}, {5, 6, 7, 8}, 101});
    value.messages.push_back(BmgMessage{{}, {0, 0, 0, 0}, 102});
    value.messages.push_back(BmgMessage{{text("Café ★")}, {9, 0, 0, 0}, 103});
    return value;
}

} // namespace

int main() {
    const auto original = sample();
    auto encoded = write_bmg(original);
    check(static_cast<bool>(encoded), "a UTF-16 BMG file writes");
    if (!encoded) return 1;

    const auto& bytes = encoded.value();
    check(bytes.size() % 32 == 0, "every section is padded to the 32-byte alignment");
    check(is_bmg(bytes), "the written file identifies as BMG by magic");
    check(is_bmg({}, "script/course.bmg"), "a .bmg name identifies as BMG without bytes");
    check(nsmb::formats::identify_format(bytes) == FormatKind::MessageBundle, "the shared identifier reports a message bundle");
    check(nsmb::formats::identify_format({}, "script/course.bmg") == FormatKind::MessageBundle,
          "course.bmg is a message bundle rather than a level candidate");

    auto parsed = parse_bmg(bytes);
    check(static_cast<bool>(parsed), "the written file parses back");
    if (!parsed) { std::cerr << parsed.error().message << '\n'; return 1; }
    check(parsed.value().messages == original.messages, "messages, attributes and IDs survive a round trip");
    check(parsed.value().file_id == 7 && parsed.value().attribute_size == 4, "INF1 header fields survive a round trip");
    auto rewritten = write_bmg(parsed.value());
    check(rewritten && rewritten.value() == bytes, "an untouched load/save cycle is byte-identical");

    // Identical strings must share one DAT1 offset the way Nintendo's own files do, and the
    // empty message must fall on the terminator the pool opens with.
    const auto entry_offset = [&bytes](std::size_t index) {
        const auto at = 0x30 + index * 8;
        return static_cast<std::uint32_t>(bytes[at] | bytes[at + 1] << 8 | bytes[at + 2] << 16 | bytes[at + 3] << 24);
    };
    check(entry_offset(1) == entry_offset(3), "identical messages share a single DAT1 string");
    check(entry_offset(2) == 0, "an empty message points at the pool's leading terminator");

    const auto& first = parsed.value().messages[0];
    check(bmg_message_text(first) == "[255:00000100] Luigi has appeared!", "escapes render in the [type:HEXDATA] form");
    check(bmg_message_plain_text(first) == " Luigi has appeared!", "the plain preview drops escapes");
    check(parse_bmg_message_text(bmg_message_text(first)) == first.parts, "the [type:HEXDATA] form parses back");
    check(parse_bmg_message_text("100% [not:anescape] [1:0z] done")[0].text == "100% [not:anescape] [1:0z] done",
          "text that only looks like an escape stays literal");
    check(parse_bmg_message_text("[1:F]")[0].escape_data == std::vector<std::uint8_t>{0x0f},
          "an odd-length escape payload is left-padded");

    // Escape payloads the model does not understand still have to survive untouched.
    Bmg opaque = original;
    opaque.messages[0].parts = {escape(200, {0xde, 0xad, 0xbe, 0xef, 0x01})};
    auto opaque_bytes = write_bmg(opaque);
    auto opaque_parsed = opaque_bytes ? parse_bmg(opaque_bytes.value()) : nsmb::core::Result<Bmg>(opaque_bytes.error());
    check(opaque_parsed && opaque_parsed.value().messages[0].parts == opaque.messages[0].parts,
          "an unrecognised escape type round trips by length");

    // The 8-bit encodings use a narrower escape header, and CP1252 has its own high range.
    for (const auto encoding : {BmgEncoding::Cp1252, BmgEncoding::Utf8}) {
        Bmg narrow;
        narrow.encoding = encoding;
        narrow.attribute_size = 0;
        narrow.messages.push_back(BmgMessage{{text("Café"), escape(3, {0x42})}, {}, std::nullopt});
        auto narrow_bytes = write_bmg(narrow);
        auto narrow_parsed = narrow_bytes ? parse_bmg(narrow_bytes.value()) : nsmb::core::Result<Bmg>(narrow_bytes.error());
        check(narrow_parsed && narrow_parsed.value().messages == narrow.messages, "an 8-bit encoded file round trips");
    }

    Bmg unmappable;
    unmappable.encoding = BmgEncoding::Cp1252;
    unmappable.messages.push_back(BmgMessage{{text("★")}, {}, std::nullopt});
    check(!write_bmg(unmappable), "a character CP1252 cannot hold is refused rather than mangled");

    check(!parse_bmg(std::vector<std::uint8_t>{'N', 'A', 'R', 'C'}), "a non-BMG file is refused");
    std::vector<std::uint8_t> truncated(bytes.begin(), bytes.begin() + 0x30);
    check(!parse_bmg(truncated), "a file cut off inside its sections is refused");
    std::vector<std::uint8_t> shift_jis(bytes.begin(), bytes.end());
    shift_jis[0x10] = 3;
    check(!parse_bmg(shift_jis), "Shift-JIS is refused rather than silently mangled");

    if (failures == 0) std::cout << "message tests passed\n";
    return failures == 0 ? 0 : 1;
}
