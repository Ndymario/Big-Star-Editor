#pragma once
#include <cstddef>
#include <memory>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::core {
class Command {
public:
    virtual ~Command() = default;
    [[nodiscard]] virtual std::string_view name() const = 0;
    virtual Result<void> execute() = 0;
    virtual Result<void> undo() = 0;
};

class CommandHistory {
public:
    Result<void> execute(std::unique_ptr<Command> command);
    Result<void> undo();
    Result<void> redo();

    void clear();
    void mark_clean() noexcept;

    [[nodiscard]] bool can_undo() const noexcept;
    [[nodiscard]] bool can_redo() const noexcept;
    [[nodiscard]] bool dirty() const noexcept;
    [[nodiscard]] std::string undo_name() const;
    [[nodiscard]] std::string redo_name() const;

private:
    static constexpr std::size_t no_clean_position = static_cast<std::size_t>(-1);

    std::vector<std::unique_ptr<Command>> commands_;
    std::size_t position_{};
    std::size_t clean_position_{};
};
} // namespace nsmb::core
