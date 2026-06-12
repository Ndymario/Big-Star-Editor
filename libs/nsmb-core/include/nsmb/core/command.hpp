#pragma once
#include <string_view>
#include "nsmb/core/result.hpp"
namespace nsmb::core {
class Command {
public:
    virtual ~Command() = default;
    [[nodiscard]] virtual std::string_view name() const = 0;
    virtual Result<void> execute() = 0;
    virtual Result<void> undo() = 0;
};
}
