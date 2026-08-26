#include "nsmb/core/byte_buffer.hpp"
#include "nsmb/core/command.hpp"
#include "nsmb/core/endian.hpp"
#include "nsmb/core/file_io.hpp"
#include <array>
#include <cassert>
#include <filesystem>
#include <memory>
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
    return 0;
}
