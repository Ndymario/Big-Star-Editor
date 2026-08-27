#include "nsmb/core/byte_buffer.hpp"
#include "nsmb/core/command.hpp"
#include "nsmb/core/endian.hpp"
#include "nsmb/core/file_io.hpp"
#include "nsmb/core/json.hpp"
#include "nsmb/core/logging.hpp"
#include <array>
#include <cassert>
#include <filesystem>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

namespace {
class SetValueCommand final : public nsmb::core::Command {
public:
    SetValueCommand(int& target, int value) : target_(target), before_(target), after_(value) {}
    std::string_view name() const override { return "Set value"; }
    nsmb::core::Result<void> execute() override {
        target_ = after_;
        return {};
    }
    nsmb::core::Result<void> undo() override {
        target_ = before_;
        return {};
    }

private:
    int& target_;
    int before_{};
    int after_{};
};
} // namespace

namespace {

void test_json() {
    using nsmb::core::JsonValue;
    using nsmb::core::parse_json;

    const auto doc = parse_json(R"({
        "schema": "ncpatcher.files/1",
        "count": 3,
        "ratio": -1.5e2,
        "ok": true,
        "missing": null,
        "files": [ { "id": 1957, "path": "z_new/reserved" } ],
        "escapes": "a\"b\\c\/d\né😀"
    })");
    assert(doc);
    const JsonValue& root = doc.value();
    assert(root.is_object());
    assert(root["schema"].as_string() == "ncpatcher.files/1");
    assert(root["count"].as_int64() == 3);
    assert(root["ratio"].as_double() == -150.0);
    assert(root["ok"].as_bool());
    assert(root.contains("missing") && root["missing"].is_null());
    assert(root["files"].size() == 1);
    assert(root["files"].at(0)["id"].as_int64() == 1957);
    assert(root["files"].at(0)["path"].as_string() == "z_new/reserved");
    assert(root["escapes"].as_string() == "a\"b\\c/d\n\xc3\xa9\xf0\x9f\x98\x80");

    // Absent keys and wrong kinds read as the fallback rather than throwing,
    // so a consumer that has validated the shape can read without re-checking.
    assert(root["nope"].is_null());
    assert(root["nope"]["deeper"].is_null());
    assert(root["schema"].as_int64(-1) == -1);
    assert(root["count"].as_string().empty());
    assert(root["files"].at(9).is_null());

    // A fractional or oversized number is not the id a caller asked for.
    const auto fractional = parse_json("{\"a\": 1.5, \"b\": 1e300}");
    assert(fractional);
    assert(fractional.value()["a"].as_int64(-1) == -1);
    assert(fractional.value()["b"].as_int64(-1) == -1);

    // Every refusal below is a document that would otherwise be read as far as
    // it goes and then acted on.
    assert(!parse_json(""));
    assert(!parse_json("{"));
    assert(!parse_json("{\"a\":1,}"));
    assert(!parse_json("[1,]"));
    assert(!parse_json("{\"a\":1} trailing"));
    assert(!parse_json("{\"a\":1,\"a\":2}"));   // duplicate member
    assert(!parse_json("{'a':1}"));             // single quotes are not JSON
    assert(!parse_json("\"unterminated"));
    assert(!parse_json("\"bad \\q escape\""));
    assert(!parse_json("01"));
    assert(!parse_json("1."));
    assert(!parse_json("1e"));
    assert(!parse_json("tru"));
    assert(!parse_json("nan"));

    // Object members keep the order the document wrote them. NCPatcher lists
    // build variants in declaration order inside a JSON object, so a parser
    // that sorted members would silently reorder the project's own answer.
    const auto ordered = parse_json(
        R"({"variants":{"en":1,"fr":2,"de":3,"it":4,"sp":5,"ja":6,"ko":7,"zh":8,"pt":9}})");
    assert(ordered);
    const auto& variants = ordered.value()["variants"].members();
    const std::vector<std::string> expected = {"en", "fr", "de", "it", "sp",
                                               "ja", "ko", "zh", "pt"};
    assert(variants.size() == expected.size());
    for (std::size_t i = 0; i < expected.size(); ++i) {
        assert(variants[i].first == expected[i]);
        assert(variants[i].second.as_int64() == static_cast<std::int64_t>(i) + 1);
    }

    // Depth is bounded so a corrupt document cannot exhaust the stack.
    std::string deep(nsmb::core::json_max_depth + 8, '[');
    assert(!parse_json(deep));
}

void test_logging() {
    using nsmb::core::CallbackLogger;
    using nsmb::core::LogLevel;

    std::vector<std::string> lines;
    CallbackLogger logger([&lines](LogLevel level, std::string_view message) {
        lines.emplace_back(std::string(nsmb::core::log_level_name(level)) + ": " + std::string(message));
    });
    logger.info("built");
    logger.warning("stale");
    logger.error("failed");
    assert(lines.size() == 3);
    assert(lines[0] == "info: built");
    assert(lines[1] == "warning: stale");
    assert(lines[2] == "error: failed");

    // The null logger is the default wherever logging is optional; it must be
    // safe to call and must not require a branch at the call site.
    nsmb::core::Logger& sink = nsmb::core::NullLogger::instance();
    sink.error("discarded");
}

} // namespace

int main() {
    using namespace nsmb::core;
    std::vector<std::uint8_t> data(4);
    LittleEndianWriter writer(data);
    assert(writer.u16(0, 0x1234));
    assert(writer.u32(0, 0x89abcdef));
    LittleEndianReader reader(data);
    assert(reader.u16(0).value() == 0xcdef);
    assert(reader.u32(0).value() == 0x89abcdef);
    assert(!reader.u32(1));
    ByteBuffer buffer({1, 2, 3});
    assert(buffer.at(1).value() == 2);
    assert(!buffer.at(3));
    assert(buffer.slice(1, 2).value().size() == 2);
    std::uint8_t patch[] = {9, 8};
    assert(buffer.write(1, patch));
    assert(buffer.at(1).value() == 9);
    assert(!buffer.write(2, patch));

    int value = 1;
    CommandHistory history;
    assert(history.execute(std::make_unique<SetValueCommand>(value, 2)));
    assert(value == 2 && history.can_undo() && history.dirty());
    history.mark_clean();
    assert(!history.dirty());
    assert(history.undo());
    assert(value == 1 && history.can_redo() && history.dirty());
    assert(history.redo());
    assert(value == 2 && !history.dirty());
    assert(history.undo());
    assert(history.execute(std::make_unique<SetValueCommand>(value, 3)));
    assert(value == 3 && !history.can_redo() && history.dirty());

    const auto output = std::filesystem::temp_directory_path() / "big-star-core-atomic-test.bin";
    assert(write_file_atomically(output, std::array<std::uint8_t, 3>{1, 2, 3}));
    assert(write_file_atomically(output, std::array<std::uint8_t, 2>{4, 5}));
    const auto saved = read_file(output);
    assert(saved && (saved.value() == std::vector<std::uint8_t>{4, 5}));
    const auto prefix = read_file_prefix(output, 1);
    assert(prefix && (prefix.value() == std::vector<std::uint8_t>{4}));
    assert(!std::filesystem::exists(output.string() + ".bse-tmp"));
    std::filesystem::remove(output);

    test_json();
    test_logging();
    return 0;
}
