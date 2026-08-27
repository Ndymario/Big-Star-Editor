#pragma once

#include <cstdint>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::core {

// A minimal, defensive JSON reader.
//
// The backend deliberately carries no third-party dependencies, and the
// manifest reader already hand-rolls the YAML subset it needs. This is the
// same bargain for the machine-readable documents NCPatcher and nsmbtool
// produce: full JSON, no exceptions, explicit errors, and a recursion limit so
// a hostile or corrupt document cannot exhaust the stack.
//
// Reading only. Nothing in the editor writes JSON.
class JsonValue {
public:
    enum class Kind { Null, Bool, Number, String, Array, Object };

    JsonValue() = default;

    [[nodiscard]] Kind kind() const { return kind_; }
    [[nodiscard]] bool is_null() const { return kind_ == Kind::Null; }
    [[nodiscard]] bool is_bool() const { return kind_ == Kind::Bool; }
    [[nodiscard]] bool is_number() const { return kind_ == Kind::Number; }
    [[nodiscard]] bool is_string() const { return kind_ == Kind::String; }
    [[nodiscard]] bool is_array() const { return kind_ == Kind::Array; }
    [[nodiscard]] bool is_object() const { return kind_ == Kind::Object; }

    // Scalar readers. Each returns the fallback when this value is not of the
    // requested kind, so a caller that has already validated the shape can read
    // without a second check, and one that has not still cannot read garbage.
    [[nodiscard]] bool as_bool(bool fallback = false) const;
    [[nodiscard]] double as_double(double fallback = 0.0) const;
    [[nodiscard]] std::int64_t as_int64(std::int64_t fallback = 0) const;
    [[nodiscard]] const std::string& as_string() const;

    // Container readers. Both return empty views for a value of another kind.
    [[nodiscard]] const std::vector<JsonValue>& elements() const { return elements_; }

    // Object members in the order the document wrote them, which for some
    // documents is meaningful: NCPatcher's `variants` object lists build
    // variants in declaration order, and an editor that offered them sorted
    // would be reordering the project's own answer.
    using Member = std::pair<std::string, JsonValue>;
    [[nodiscard]] const std::vector<Member>& members() const { return members_; }

    // Object lookup. A missing key yields the shared null value rather than an
    // error, so chained access stays readable; `contains` distinguishes an
    // absent key from one explicitly set to null.
    [[nodiscard]] bool contains(std::string_view key) const;
    [[nodiscard]] const JsonValue& operator[](std::string_view key) const;

    // Array indexing. An out-of-range index yields the shared null value.
    [[nodiscard]] const JsonValue& at(std::size_t index) const;
    [[nodiscard]] std::size_t size() const;

    static const JsonValue& null_value();

    static JsonValue make_bool(bool value);
    static JsonValue make_number(double value);
    static JsonValue make_string(std::string value);
    static JsonValue make_array(std::vector<JsonValue> elements);
    static JsonValue make_object(std::vector<Member> members);

private:
    Kind kind_{Kind::Null};
    bool bool_{};
    double number_{};
    std::string string_;
    std::vector<JsonValue> elements_;
    std::vector<Member> members_;
};

// How deep a document may nest before it is refused. NCPatcher's documents are
// three or four levels deep; anything approaching this is not one of them.
inline constexpr int json_max_depth = 64;

Result<JsonValue> parse_json(std::string_view text);

} // namespace nsmb::core
