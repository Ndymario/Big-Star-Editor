#include "nsmb/core/json.hpp"

#include <cmath>
#include <cstdlib>
#include <limits>

namespace nsmb::core {

namespace {

bool is_json_space(char c) {
    return c == ' ' || c == '\t' || c == '\n' || c == '\r';
}

bool is_digit(char c) {
    return c >= '0' && c <= '9';
}

// One pass over the text, tracking an offset so every refusal can say where.
class Parser {
public:
    explicit Parser(std::string_view text) : text_(text) {}

    Result<JsonValue> run() {
        skip_space();
        auto root = parse_value(0);
        if (!root) {
            return root.error();
        }
        skip_space();
        if (pos_ != text_.size()) {
            return fail("trailing content after the top-level value");
        }
        return root;
    }

private:
    Error fail(const std::string& what) const {
        return make_error(ErrorCode::ParseError,
                          "invalid JSON at byte " + std::to_string(pos_) + ": " + what);
    }

    void skip_space() {
        while (pos_ < text_.size() && is_json_space(text_[pos_])) {
            ++pos_;
        }
    }

    bool peek_is(char c) const { return pos_ < text_.size() && text_[pos_] == c; }

    bool consume_literal(std::string_view word) {
        if (text_.compare(pos_, word.size(), word) != 0) {
            return false;
        }
        pos_ += word.size();
        return true;
    }

    Result<JsonValue> parse_value(int depth) {
        if (depth > json_max_depth) {
            return fail("nesting deeper than " + std::to_string(json_max_depth) + " levels");
        }
        if (pos_ >= text_.size()) {
            return fail("expected a value");
        }

        switch (text_[pos_]) {
        case '{': return parse_object(depth);
        case '[': return parse_array(depth);
        case '"': {
            auto text = parse_string();
            if (!text) {
                return text.error();
            }
            return JsonValue::make_string(text.value());
        }
        case 't':
            if (!consume_literal("true")) {
                return fail("expected true");
            }
            return JsonValue::make_bool(true);
        case 'f':
            if (!consume_literal("false")) {
                return fail("expected false");
            }
            return JsonValue::make_bool(false);
        case 'n':
            if (!consume_literal("null")) {
                return fail("expected null");
            }
            return JsonValue{};
        default:
            return parse_number();
        }
    }

    Result<JsonValue> parse_object(int depth) {
        ++pos_; // '{'
        std::vector<JsonValue::Member> members;
        skip_space();
        if (peek_is('}')) {
            ++pos_;
            return JsonValue::make_object(std::move(members));
        }

        for (;;) {
            skip_space();
            if (!peek_is('"')) {
                return fail("expected a member name");
            }
            auto key = parse_string();
            if (!key) {
                return key.error();
            }
            skip_space();
            if (!peek_is(':')) {
                return fail("expected ':' after a member name");
            }
            ++pos_;
            skip_space();
            auto value = parse_value(depth + 1);
            if (!value) {
                return value.error();
            }
            // A duplicate member is ambiguous rather than merely odd: which one
            // did the writer mean? Refuse instead of letting insertion order
            // decide silently.
            for (const auto& member : members) {
                if (member.first == key.value()) {
                    return fail("duplicate member name \"" + key.value() + "\"");
                }
            }
            members.emplace_back(key.value(), std::move(value.value()));
            skip_space();
            if (peek_is(',')) {
                ++pos_;
                continue;
            }
            if (peek_is('}')) {
                ++pos_;
                return JsonValue::make_object(std::move(members));
            }
            return fail("expected ',' or '}' in an object");
        }
    }

    Result<JsonValue> parse_array(int depth) {
        ++pos_; // '['
        std::vector<JsonValue> elements;
        skip_space();
        if (peek_is(']')) {
            ++pos_;
            return JsonValue::make_array(std::move(elements));
        }

        for (;;) {
            skip_space();
            auto value = parse_value(depth + 1);
            if (!value) {
                return value.error();
            }
            elements.push_back(std::move(value.value()));
            skip_space();
            if (peek_is(',')) {
                ++pos_;
                continue;
            }
            if (peek_is(']')) {
                ++pos_;
                return JsonValue::make_array(std::move(elements));
            }
            return fail("expected ',' or ']' in an array");
        }
    }

    Result<std::string> parse_string() {
        ++pos_; // opening quote
        std::string out;
        for (;;) {
            if (pos_ >= text_.size()) {
                return fail("unterminated string");
            }
            const char c = text_[pos_];
            if (c == '"') {
                ++pos_;
                return out;
            }
            if (static_cast<unsigned char>(c) < 0x20) {
                return fail("unescaped control character in a string");
            }
            if (c != '\\') {
                out.push_back(c);
                ++pos_;
                continue;
            }

            ++pos_;
            if (pos_ >= text_.size()) {
                return fail("unterminated escape sequence");
            }
            const char esc = text_[pos_++];
            switch (esc) {
            case '"': out.push_back('"'); break;
            case '\\': out.push_back('\\'); break;
            case '/': out.push_back('/'); break;
            case 'b': out.push_back('\b'); break;
            case 'f': out.push_back('\f'); break;
            case 'n': out.push_back('\n'); break;
            case 'r': out.push_back('\r'); break;
            case 't': out.push_back('\t'); break;
            case 'u': {
                auto escaped = parse_unicode_escape();
                if (!escaped) {
                    return escaped.error();
                }
                append_utf8(out, escaped.value());
                break;
            }
            default:
                return fail("unknown escape sequence");
            }
        }
    }

    // Reads \uXXXX, joining a surrogate pair when one follows. A lone or
    // malformed surrogate becomes U+FFFD rather than an error: the payloads
    // that matter here are file paths and identifiers, and refusing a whole
    // manifest over one bad character helps nobody.
    Result<char32_t> parse_unicode_escape() {
        auto first = parse_hex4();
        if (!first) {
            return first.error();
        }
        const std::uint32_t high = first.value();
        if (high < 0xD800 || high > 0xDBFF) {
            if (high >= 0xDC00 && high <= 0xDFFF) {
                return static_cast<char32_t>(0xFFFD);
            }
            return static_cast<char32_t>(high);
        }

        if (pos_ + 1 >= text_.size() || text_[pos_] != '\\' || text_[pos_ + 1] != 'u') {
            return static_cast<char32_t>(0xFFFD);
        }
        pos_ += 2;
        auto second = parse_hex4();
        if (!second) {
            return second.error();
        }
        const std::uint32_t low = second.value();
        if (low < 0xDC00 || low > 0xDFFF) {
            return static_cast<char32_t>(0xFFFD);
        }
        return static_cast<char32_t>(0x10000u + ((high - 0xD800u) << 10) + (low - 0xDC00u));
    }

    Result<std::uint32_t> parse_hex4() {
        if (pos_ + 4 > text_.size()) {
            return fail("truncated \\u escape");
        }
        std::uint32_t value = 0;
        for (int i = 0; i < 4; ++i) {
            const char c = text_[pos_++];
            value <<= 4;
            if (c >= '0' && c <= '9') {
                value |= static_cast<std::uint32_t>(c - '0');
            } else if (c >= 'a' && c <= 'f') {
                value |= static_cast<std::uint32_t>(c - 'a' + 10);
            } else if (c >= 'A' && c <= 'F') {
                value |= static_cast<std::uint32_t>(c - 'A' + 10);
            } else {
                return fail("non-hexadecimal digit in a \\u escape");
            }
        }
        return value;
    }

    static void append_utf8(std::string& out, char32_t code_point) {
        const auto cp = static_cast<std::uint32_t>(code_point);
        if (cp < 0x80) {
            out.push_back(static_cast<char>(cp));
        } else if (cp < 0x800) {
            out.push_back(static_cast<char>(0xC0 | (cp >> 6)));
            out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
        } else if (cp < 0x10000) {
            out.push_back(static_cast<char>(0xE0 | (cp >> 12)));
            out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
            out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
        } else {
            out.push_back(static_cast<char>(0xF0 | (cp >> 18)));
            out.push_back(static_cast<char>(0x80 | ((cp >> 12) & 0x3F)));
            out.push_back(static_cast<char>(0x80 | ((cp >> 6) & 0x3F)));
            out.push_back(static_cast<char>(0x80 | (cp & 0x3F)));
        }
    }

    Result<JsonValue> parse_number() {
        const std::size_t start = pos_;
        if (peek_is('-')) {
            ++pos_;
        }
        if (pos_ >= text_.size()) {
            return fail("expected a number");
        }
        if (text_[pos_] == '0') {
            ++pos_;
        } else if (is_digit(text_[pos_])) {
            while (pos_ < text_.size() && is_digit(text_[pos_])) {
                ++pos_;
            }
        } else {
            return fail("expected a number");
        }

        if (peek_is('.')) {
            ++pos_;
            if (pos_ >= text_.size() || !is_digit(text_[pos_])) {
                return fail("expected a digit after the decimal point");
            }
            while (pos_ < text_.size() && is_digit(text_[pos_])) {
                ++pos_;
            }
        }

        if (pos_ < text_.size() && (text_[pos_] == 'e' || text_[pos_] == 'E')) {
            ++pos_;
            if (pos_ < text_.size() && (text_[pos_] == '+' || text_[pos_] == '-')) {
                ++pos_;
            }
            if (pos_ >= text_.size() || !is_digit(text_[pos_])) {
                return fail("expected a digit in the exponent");
            }
            while (pos_ < text_.size() && is_digit(text_[pos_])) {
                ++pos_;
            }
        }

        const std::string lexeme(text_.substr(start, pos_ - start));
        const double value = std::strtod(lexeme.c_str(), nullptr);
        if (!std::isfinite(value)) {
            return fail("number out of range");
        }
        return JsonValue::make_number(value);
    }

    std::string_view text_;
    std::size_t pos_{0};
};

} // namespace

bool JsonValue::as_bool(bool fallback) const {
    return kind_ == Kind::Bool ? bool_ : fallback;
}

double JsonValue::as_double(double fallback) const {
    return kind_ == Kind::Number ? number_ : fallback;
}

std::int64_t JsonValue::as_int64(std::int64_t fallback) const {
    if (kind_ != Kind::Number) {
        return fallback;
    }
    // Only an exact integer inside the range doubles represent exactly counts;
    // a fractional or oversized value is not the id or size a caller expects.
    if (number_ != std::floor(number_)) {
        return fallback;
    }
    constexpr double exact_limit = 9007199254740992.0; // 2^53
    if (number_ >= exact_limit || number_ <= -exact_limit) {
        return fallback;
    }
    return static_cast<std::int64_t>(number_);
}

const std::string& JsonValue::as_string() const {
    static const std::string empty;
    return kind_ == Kind::String ? string_ : empty;
}

bool JsonValue::contains(std::string_view key) const {
    if (kind_ != Kind::Object) {
        return false;
    }
    for (const auto& member : members_) {
        if (member.first == key) {
            return true;
        }
    }
    return false;
}

const JsonValue& JsonValue::operator[](std::string_view key) const {
    if (kind_ != Kind::Object) {
        return null_value();
    }
    for (const auto& member : members_) {
        if (member.first == key) {
            return member.second;
        }
    }
    return null_value();
}

const JsonValue& JsonValue::at(std::size_t index) const {
    if (kind_ != Kind::Array || index >= elements_.size()) {
        return null_value();
    }
    return elements_[index];
}

std::size_t JsonValue::size() const {
    if (kind_ == Kind::Array) {
        return elements_.size();
    }
    if (kind_ == Kind::Object) {
        return members_.size();
    }
    return 0;
}

const JsonValue& JsonValue::null_value() {
    static const JsonValue value;
    return value;
}

JsonValue JsonValue::make_bool(bool value) {
    JsonValue out;
    out.kind_ = Kind::Bool;
    out.bool_ = value;
    return out;
}

JsonValue JsonValue::make_number(double value) {
    JsonValue out;
    out.kind_ = Kind::Number;
    out.number_ = value;
    return out;
}

JsonValue JsonValue::make_string(std::string value) {
    JsonValue out;
    out.kind_ = Kind::String;
    out.string_ = std::move(value);
    return out;
}

JsonValue JsonValue::make_array(std::vector<JsonValue> elements) {
    JsonValue out;
    out.kind_ = Kind::Array;
    out.elements_ = std::move(elements);
    return out;
}

JsonValue JsonValue::make_object(std::vector<Member> members) {
    JsonValue out;
    out.kind_ = Kind::Object;
    out.members_ = std::move(members);
    return out;
}

Result<JsonValue> parse_json(std::string_view text) {
    Parser parser(text);
    return parser.run();
}

} // namespace nsmb::core
