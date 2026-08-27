#include "nsmb/formats/message.hpp"

#include "nsmb/core/endian.hpp"

#include <algorithm>
#include <cctype>
#include <charconv>
#include <filesystem>
#include <map>

namespace nsmb::formats {
namespace {

constexpr std::size_t header_size = 0x20;
constexpr std::size_t section_header_size = 8;
// Nintendo's own tooling rounds every section up to 32 bytes, and readers take the size from
// the section header, so the padding is invisible to the game either way.
constexpr std::size_t section_alignment = 32;
// The escape marker is 0x1A in every encoding; only the unit width around it changes.
constexpr std::uint8_t escape_marker = 0x1a;

bool has_magic(std::span<const std::uint8_t> bytes, std::string_view value, std::size_t offset = 0) {
    if (bytes.size() < offset + value.size()) return false;
    return std::equal(value.begin(), value.end(), bytes.begin() + static_cast<std::ptrdiff_t>(offset));
}

std::uint16_t read16(std::span<const std::uint8_t> bytes, std::size_t offset) {
    return static_cast<std::uint16_t>(bytes[offset] | bytes[offset + 1] << 8U);
}

std::uint32_t read32(std::span<const std::uint8_t> bytes, std::size_t offset) {
    return static_cast<std::uint32_t>(bytes[offset]) | static_cast<std::uint32_t>(bytes[offset + 1]) << 8U |
           static_cast<std::uint32_t>(bytes[offset + 2]) << 16U | static_cast<std::uint32_t>(bytes[offset + 3]) << 24U;
}

void append16(std::vector<std::uint8_t>& out, std::uint16_t value) {
    out.push_back(static_cast<std::uint8_t>(value));
    out.push_back(static_cast<std::uint8_t>(value >> 8U));
}

void append32(std::vector<std::uint8_t>& out, std::uint32_t value) {
    for (int shift = 0; shift < 32; shift += 8) out.push_back(static_cast<std::uint8_t>(value >> shift));
}

// CP1252 differs from Latin-1 only across 0x80..0x9F. Undefined slots keep their byte value so a
// file that happens to contain one still survives a load/save cycle unchanged.
constexpr std::array<char32_t, 32> cp1252_high{
    0x20ac, 0x0081, 0x201a, 0x0192, 0x201e, 0x2026, 0x2020, 0x2021, 0x02c6, 0x2030, 0x0160,
    0x2039, 0x0152, 0x008d, 0x017d, 0x008f, 0x0090, 0x2018, 0x2019, 0x201c, 0x201d, 0x2022,
    0x2013, 0x2014, 0x02dc, 0x2122, 0x0161, 0x203a, 0x0153, 0x009d, 0x017e, 0x0178};

void append_utf8(std::string& out, char32_t code) {
    if (code < 0x80) {
        out.push_back(static_cast<char>(code));
    } else if (code < 0x800) {
        out.push_back(static_cast<char>(0xc0 | (code >> 6U)));
        out.push_back(static_cast<char>(0x80 | (code & 0x3f)));
    } else if (code < 0x10000) {
        out.push_back(static_cast<char>(0xe0 | (code >> 12U)));
        out.push_back(static_cast<char>(0x80 | ((code >> 6U) & 0x3f)));
        out.push_back(static_cast<char>(0x80 | (code & 0x3f)));
    } else {
        out.push_back(static_cast<char>(0xf0 | (code >> 18U)));
        out.push_back(static_cast<char>(0x80 | ((code >> 12U) & 0x3f)));
        out.push_back(static_cast<char>(0x80 | ((code >> 6U) & 0x3f)));
        out.push_back(static_cast<char>(0x80 | (code & 0x3f)));
    }
}

core::Result<std::u32string> utf8_codepoints(std::string_view text) {
    std::u32string out;
    for (std::size_t i = 0; i < text.size();) {
        const auto lead = static_cast<std::uint8_t>(text[i]);
        std::size_t extra{};
        char32_t code{};
        if (lead < 0x80) { code = lead; extra = 0; }
        else if ((lead & 0xe0) == 0xc0) { code = lead & 0x1fU; extra = 1; }
        else if ((lead & 0xf0) == 0xe0) { code = lead & 0x0fU; extra = 2; }
        else if ((lead & 0xf8) == 0xf0) { code = lead & 0x07U; extra = 3; }
        else return core::make_error(core::ErrorCode::InvalidArgument, "message text is not valid UTF-8");
        if (i + extra >= text.size())
            return core::make_error(core::ErrorCode::InvalidArgument, "message text ends inside a UTF-8 sequence");
        for (std::size_t n = 1; n <= extra; ++n) {
            const auto trail = static_cast<std::uint8_t>(text[i + n]);
            if ((trail & 0xc0) != 0x80)
                return core::make_error(core::ErrorCode::InvalidArgument, "message text is not valid UTF-8");
            code = (code << 6U) | (trail & 0x3fU);
        }
        out.push_back(code);
        i += extra + 1;
    }
    return out;
}

std::string utf16_to_utf8(const std::u16string& text) {
    std::string out;
    for (std::size_t i = 0; i < text.size(); ++i) {
        char32_t code = text[i];
        if (code >= 0xd800 && code < 0xdc00 && i + 1 < text.size() && text[i + 1] >= 0xdc00 && text[i + 1] < 0xe000) {
            code = 0x10000 + ((code - 0xd800) << 10U) + (text[i + 1] - 0xdc00);
            ++i;
        }
        append_utf8(out, code);
    }
    return out;
}

core::Result<std::vector<std::uint8_t>> encode_text(std::string_view text, BmgEncoding encoding) {
    std::vector<std::uint8_t> out;
    if (encoding == BmgEncoding::Utf8) {
        out.assign(text.begin(), text.end());
        return out;
    }
    auto codes = utf8_codepoints(text);
    if (!codes) return codes.error();
    for (const auto code : codes.value()) {
        switch (encoding) {
        case BmgEncoding::Utf16:
            if (code >= 0x10000) {
                const auto value = code - 0x10000;
                append16(out, static_cast<std::uint16_t>(0xd800 + (value >> 10U)));
                append16(out, static_cast<std::uint16_t>(0xdc00 + (value & 0x3ff)));
            } else {
                append16(out, static_cast<std::uint16_t>(code));
            }
            break;
        case BmgEncoding::Cp1252: {
            if (code < 0x80 || (code >= 0xa0 && code < 0x100)) { out.push_back(static_cast<std::uint8_t>(code)); break; }
            const auto found = std::find(cp1252_high.begin(), cp1252_high.end(), code);
            if (found == cp1252_high.end())
                return core::make_error(core::ErrorCode::InvalidArgument,
                                        "character cannot be written in this file's CP1252 encoding");
            out.push_back(static_cast<std::uint8_t>(0x80 + (found - cp1252_high.begin())));
            break;
        }
        default:
            return core::make_error(core::ErrorCode::Unsupported, "unsupported BMG text encoding");
        }
    }
    return out;
}

std::size_t unit_width(BmgEncoding encoding) { return encoding == BmgEncoding::Utf16 ? 2 : 1; }

// One DAT1 string, split into literal runs and escape sequences. Escapes carry their own total
// byte length, so an unrecognised escape type still passes through a load/save cycle intact.
core::Result<std::vector<BmgPart>> decode_message(std::span<const std::uint8_t> pool,
                                                  std::size_t offset,
                                                  BmgEncoding encoding) {
    if (offset > pool.size())
        return core::make_error(core::ErrorCode::OutOfBounds, "message offset points outside the DAT1 pool");

    const auto width = unit_width(encoding);
    // The smallest escape is marker + length + type, plus the marker's second unit under UTF-16.
    const std::size_t minimum_escape = width + 2;
    std::vector<BmgPart> parts;
    std::u16string wide;
    std::string narrow;

    const auto flush = [&]() -> core::Result<void> {
        if (encoding == BmgEncoding::Utf16) {
            if (wide.empty()) return {};
            parts.push_back(BmgPart{false, utf16_to_utf8(wide), 0, {}});
            wide.clear();
            return {};
        }
        if (narrow.empty()) return {};
        std::string text;
        if (encoding == BmgEncoding::Utf8) {
            text = narrow;
        } else {
            for (const auto c : narrow) {
                const auto byte = static_cast<std::uint8_t>(c);
                append_utf8(text, byte >= 0x80 && byte < 0xa0 ? cp1252_high[byte - 0x80] : static_cast<char32_t>(byte));
            }
        }
        parts.push_back(BmgPart{false, std::move(text), 0, {}});
        narrow.clear();
        return {};
    };

    for (std::size_t p = offset; p + width <= pool.size();) {
        const auto unit = encoding == BmgEncoding::Utf16 ? read16(pool, p) : pool[p];
        if (unit == 0) {
            if (const auto flushed = flush(); !flushed) return flushed.error();
            return parts;
        }
        if (unit == escape_marker) {
            if (p + minimum_escape > pool.size())
                return core::make_error(core::ErrorCode::ParseError, "escape sequence runs past the DAT1 pool");
            const std::size_t length = pool[p + width];
            if (length < minimum_escape || p + length > pool.size())
                return core::make_error(core::ErrorCode::ParseError, "escape sequence declares an impossible length");
            if (const auto flushed = flush(); !flushed) return flushed.error();
            BmgPart part;
            part.escape = true;
            part.escape_type = pool[p + width + 1];
            const auto payload = pool.subspan(p + width + 2, length - width - 2);
            part.escape_data.assign(payload.begin(), payload.end());
            parts.push_back(std::move(part));
            p += length;
            continue;
        }
        if (encoding == BmgEncoding::Utf16) wide.push_back(static_cast<char16_t>(unit));
        else narrow.push_back(static_cast<char>(unit));
        p += width;
    }
    return core::make_error(core::ErrorCode::ParseError, "message is not terminated inside the DAT1 pool");
}

core::Result<std::vector<std::uint8_t>> encode_message(const BmgMessage& message, BmgEncoding encoding) {
    const auto width = unit_width(encoding);
    std::vector<std::uint8_t> out;
    for (const auto& part : message.parts) {
        if (!part.escape) {
            auto encoded = encode_text(part.text, encoding);
            if (!encoded) return encoded.error();
            out.insert(out.end(), encoded.value().begin(), encoded.value().end());
            continue;
        }
        const auto length = width + 2 + part.escape_data.size();
        if (length > 0xff)
            return core::make_error(core::ErrorCode::InvalidArgument, "escape sequence payload is too long to store");
        out.push_back(escape_marker);
        if (width == 2) out.push_back(0);
        out.push_back(static_cast<std::uint8_t>(length));
        out.push_back(part.escape_type);
        out.insert(out.end(), part.escape_data.begin(), part.escape_data.end());
    }
    out.insert(out.end(), width, 0);
    return out;
}

void pad_section(std::vector<std::uint8_t>& body) {
    while ((body.size() + section_header_size) % section_alignment != 0) body.push_back(0);
}

void append_section(std::vector<std::uint8_t>& out, std::string_view magic, const std::vector<std::uint8_t>& body) {
    out.insert(out.end(), magic.begin(), magic.end());
    append32(out, static_cast<std::uint32_t>(section_header_size + body.size()));
    out.insert(out.end(), body.begin(), body.end());
}

std::string hex_byte(std::uint8_t value) {
    static constexpr char digits[] = "0123456789ABCDEF";
    return {digits[value >> 4U], digits[value & 0x0f]};
}

} // namespace

bool is_bmg(std::span<const std::uint8_t> bytes, std::string_view path) {
    if (has_magic(bytes, "MESGbmg1")) return true;
    if (bytes.empty()) {
        auto extension = std::filesystem::path(path).extension().string();
        std::ranges::transform(extension, extension.begin(), [](unsigned char c) { return static_cast<char>(std::tolower(c)); });
        return extension == ".bmg";
    }
    return false;
}

std::string_view bmg_encoding_name(BmgEncoding encoding) {
    switch (encoding) {
    case BmgEncoding::Cp1252: return "CP1252";
    case BmgEncoding::Utf16: return "UTF-16";
    case BmgEncoding::ShiftJis: return "Shift-JIS";
    case BmgEncoding::Utf8: return "UTF-8";
    case BmgEncoding::Unknown: break;
    }
    return "unspecified";
}

core::Result<Bmg> parse_bmg(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < header_size || !has_magic(bytes, "MESGbmg1"))
        return core::make_error(core::ErrorCode::ParseError, "file does not start with the MESGbmg1 header");

    Bmg value;
    value.encoding = static_cast<BmgEncoding>(bytes[0x10]);
    if (value.encoding == BmgEncoding::Unknown) value.encoding = BmgEncoding::Utf16;  // what the DS games assume
    if (value.encoding == BmgEncoding::ShiftJis)
        return core::make_error(core::ErrorCode::Unsupported, "Shift-JIS BMG files are not supported yet");
    if (value.encoding != BmgEncoding::Cp1252 && value.encoding != BmgEncoding::Utf16 && value.encoding != BmgEncoding::Utf8)
        return core::make_error(core::ErrorCode::Unsupported, "unrecognised BMG text encoding");
    std::copy_n(bytes.begin() + 0x11, value.header_reserved.size(), value.header_reserved.begin());

    std::vector<std::uint32_t> offsets;
    std::vector<std::vector<std::uint8_t>> attributes;
    std::vector<std::uint32_t> ids;
    std::span<const std::uint8_t> pool;
    bool seen_inf1{}, seen_dat1{};

    for (std::size_t p = header_size; p + section_header_size <= bytes.size();) {
        const std::string magic(reinterpret_cast<const char*>(bytes.data() + p), 4);
        if (!std::ranges::all_of(magic, [](char c) { return std::isalnum(static_cast<unsigned char>(c)); })) break;
        auto size = static_cast<std::size_t>(read32(bytes, p + 4));
        // A truncated final section is common in ripped files; take what is actually there.
        if (size < section_header_size || p + size > bytes.size()) size = bytes.size() - p;
        const auto body = bytes.subspan(p + section_header_size, size - section_header_size);
        value.section_order.push_back(magic);

        if (magic == "INF1" && !seen_inf1) {
            seen_inf1 = true;
            if (body.size() < 8) return core::make_error(core::ErrorCode::ParseError, "INF1 section is too small");
            const auto count = read16(body, 0);
            const auto entry_size = read16(body, 2);
            value.file_id = read16(body, 4);
            value.default_color = body[6];
            value.inf1_reserved = body[7];
            if (entry_size < 4)
                return core::make_error(core::ErrorCode::ParseError, "INF1 entries are too small to hold a DAT1 offset");
            value.attribute_size = static_cast<std::uint16_t>(entry_size - 4);
            if (8 + static_cast<std::size_t>(count) * entry_size > body.size())
                return core::make_error(core::ErrorCode::ParseError, "INF1 declares more entries than the section holds");
            for (std::size_t i = 0; i < count; ++i) {
                const auto entry = 8 + i * entry_size;
                offsets.push_back(read32(body, entry));
                attributes.emplace_back(body.begin() + static_cast<std::ptrdiff_t>(entry + 4),
                                        body.begin() + static_cast<std::ptrdiff_t>(entry + entry_size));
            }
        } else if (magic == "DAT1" && !seen_dat1) {
            seen_dat1 = true;
            pool = body;
        } else if (magic == "MID1" && !value.has_mid1) {
            value.has_mid1 = true;
            if (body.size() < 8) return core::make_error(core::ErrorCode::ParseError, "MID1 section is too small");
            const auto count = read16(body, 0);
            std::copy_n(body.begin() + 2, value.mid1_reserved.size(), value.mid1_reserved.begin());
            if (8 + static_cast<std::size_t>(count) * 4 > body.size())
                return core::make_error(core::ErrorCode::ParseError, "MID1 declares more IDs than the section holds");
            for (std::size_t i = 0; i < count; ++i) ids.push_back(read32(body, 8 + i * 4));
        } else {
            value.extra_sections.push_back(BmgSection{magic, std::vector<std::uint8_t>(body.begin(), body.end())});
        }
        p += size;
    }

    if (!seen_inf1 || !seen_dat1)
        return core::make_error(core::ErrorCode::ParseError, "BMG file is missing its INF1 or DAT1 section");

    value.messages.reserve(offsets.size());
    for (std::size_t i = 0; i < offsets.size(); ++i) {
        auto parts = decode_message(pool, offsets[i], value.encoding);
        if (!parts) return parts.error();
        BmgMessage message;
        message.parts = std::move(parts.value());
        message.attributes = std::move(attributes[i]);
        if (i < ids.size()) message.id = ids[i];
        value.messages.push_back(std::move(message));
    }
    return value;
}

core::Result<std::vector<std::uint8_t>> write_bmg(const Bmg& value) {
    if (value.messages.size() > 0xffff)
        return core::make_error(core::ErrorCode::InvalidArgument, "a BMG file cannot hold more than 65535 messages");

    const auto width = unit_width(value.encoding);
    const auto entry_size = static_cast<std::size_t>(4) + value.attribute_size;

    std::vector<std::uint8_t> pool(width, 0);  // offset 0 is the shared empty string
    std::map<std::vector<std::uint8_t>, std::uint32_t> seen{{std::vector<std::uint8_t>(width, 0), 0U}};
    std::vector<std::uint8_t> inf1;
    append16(inf1, static_cast<std::uint16_t>(value.messages.size()));
    append16(inf1, static_cast<std::uint16_t>(entry_size));
    append16(inf1, value.file_id);
    inf1.push_back(value.default_color);
    inf1.push_back(value.inf1_reserved);

    for (const auto& message : value.messages) {
        auto encoded = encode_message(message, value.encoding);
        if (!encoded) return encoded.error();
        auto slot = seen.find(encoded.value());
        if (slot == seen.end()) {
            slot = seen.emplace(encoded.value(), static_cast<std::uint32_t>(pool.size())).first;
            pool.insert(pool.end(), encoded.value().begin(), encoded.value().end());
        }
        append32(inf1, slot->second);
        auto attributes = message.attributes;
        attributes.resize(value.attribute_size, 0);
        inf1.insert(inf1.end(), attributes.begin(), attributes.end());
    }

    std::vector<std::uint8_t> mid1;
    if (value.has_mid1) {
        append16(mid1, static_cast<std::uint16_t>(value.messages.size()));
        mid1.insert(mid1.end(), value.mid1_reserved.begin(), value.mid1_reserved.end());
        for (std::size_t i = 0; i < value.messages.size(); ++i)
            append32(mid1, value.messages[i].id.value_or(static_cast<std::uint32_t>(i)));
    }

    pad_section(inf1);
    pad_section(pool);
    pad_section(mid1);

    std::vector<std::uint8_t> out(header_size, 0);
    const std::string_view magic = "MESGbmg1";
    std::copy(magic.begin(), magic.end(), out.begin());
    out[0x10] = static_cast<std::uint8_t>(value.encoding);
    std::copy(value.header_reserved.begin(), value.header_reserved.end(), out.begin() + 0x11);

    std::size_t sections{};
    std::vector<std::string> emitted;
    const auto emit = [&](const std::string& name) {
        if (std::ranges::find(emitted, name) != emitted.end()) return;
        if (name == "INF1") append_section(out, name, inf1);
        else if (name == "DAT1") append_section(out, name, pool);
        else if (name == "MID1") { if (!value.has_mid1) return; append_section(out, name, mid1); }
        else {
            const auto extra = std::ranges::find_if(value.extra_sections, [&](const auto& s) { return s.magic == name; });
            if (extra == value.extra_sections.end()) return;
            append_section(out, name, extra->body);
        }
        emitted.push_back(name);
        ++sections;
    };

    for (const auto& name : value.section_order) emit(name);
    for (const auto& name : {std::string("INF1"), std::string("DAT1"), std::string("MID1")}) emit(name);
    for (const auto& extra : value.extra_sections) emit(extra.magic);

    core::LittleEndianWriter writer(out);
    if (const auto written = writer.u32(0x08, static_cast<std::uint32_t>(out.size())); !written) return written.error();
    if (const auto written = writer.u32(0x0c, static_cast<std::uint32_t>(sections)); !written) return written.error();
    return out;
}

std::string bmg_message_text(const BmgMessage& message) {
    std::string out;
    for (const auto& part : message.parts) {
        if (!part.escape) { out += part.text; continue; }
        out.push_back('[');
        out += std::to_string(static_cast<unsigned>(part.escape_type));
        out.push_back(':');
        for (const auto byte : part.escape_data) out += hex_byte(byte);
        out.push_back(']');
    }
    return out;
}

std::string bmg_message_plain_text(const BmgMessage& message) {
    std::string out;
    for (const auto& part : message.parts) if (!part.escape) out += part.text;
    std::ranges::replace(out, '\n', ' ');
    std::ranges::replace(out, '\r', ' ');
    return out;
}

std::vector<BmgPart> parse_bmg_message_text(std::string_view text) {
    std::vector<BmgPart> parts;
    std::string literal;
    const auto flush = [&] {
        if (literal.empty()) return;
        parts.push_back(BmgPart{false, std::move(literal), 0, {}});
        literal.clear();
    };

    for (std::size_t i = 0; i < text.size();) {
        if (text[i] != '[') { literal.push_back(text[i++]); continue; }
        const auto close = text.find(']', i);
        const auto colon = text.find(':', i);
        if (close == std::string_view::npos || colon == std::string_view::npos || colon > close) {
            literal.push_back(text[i++]);
            continue;
        }
        const auto type_text = text.substr(i + 1, colon - i - 1);
        auto hex_text = text.substr(colon + 1, close - colon - 1);
        unsigned type{};
        const auto parsed = std::from_chars(type_text.data(), type_text.data() + type_text.size(), type);
        const auto valid_hex = std::ranges::all_of(hex_text, [](char c) { return std::isxdigit(static_cast<unsigned char>(c)); });
        if (type_text.empty() || parsed.ec != std::errc{} || parsed.ptr != type_text.data() + type_text.size() ||
            type > 0xff || !valid_hex) {
            literal.push_back(text[i++]);
            continue;
        }
        flush();
        BmgPart part;
        part.escape = true;
        part.escape_type = static_cast<std::uint8_t>(type);
        std::string padded(hex_text);
        if (padded.size() % 2 != 0) padded.insert(padded.begin(), '0');
        for (std::size_t n = 0; n < padded.size(); n += 2) {
            unsigned byte{};
            std::from_chars(padded.data() + n, padded.data() + n + 2, byte, 16);
            part.escape_data.push_back(static_cast<std::uint8_t>(byte));
        }
        parts.push_back(std::move(part));
        i = close + 1;
    }
    flush();
    return parts;
}

} // namespace nsmb::formats
