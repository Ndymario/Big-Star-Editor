#include "nsmb/core/command.hpp"

namespace nsmb::core {

Result<void> CommandHistory::execute(std::unique_ptr<Command> command) {
    if (!command) {
        return make_error(ErrorCode::InvalidArgument, "command is null");
    }

    auto result = command->execute();
    if (!result) {
        return result;
    }

    if (position_ < commands_.size()) {
        if (clean_position_ > position_) {
            clean_position_ = no_clean_position;
        }
        commands_.erase(commands_.begin() + static_cast<std::ptrdiff_t>(position_), commands_.end());
    }
    commands_.push_back(std::move(command));
    ++position_;
    return {};
}

Result<void> CommandHistory::undo() {
    if (!can_undo()) {
        return make_error(ErrorCode::InvalidArgument, "nothing to undo");
    }
    auto result = commands_[position_ - 1U]->undo();
    if (result) {
        --position_;
    }
    return result;
}

Result<void> CommandHistory::redo() {
    if (!can_redo()) {
        return make_error(ErrorCode::InvalidArgument, "nothing to redo");
    }
    auto result = commands_[position_]->execute();
    if (result) {
        ++position_;
    }
    return result;
}

void CommandHistory::clear() {
    commands_.clear();
    position_ = 0;
    clean_position_ = 0;
}

void CommandHistory::mark_clean() noexcept { clean_position_ = position_; }

bool CommandHistory::can_undo() const noexcept { return position_ != 0; }

bool CommandHistory::can_redo() const noexcept { return position_ < commands_.size(); }

bool CommandHistory::dirty() const noexcept { return clean_position_ != position_; }

std::string CommandHistory::undo_name() const {
    return can_undo() ? std::string(commands_[position_ - 1U]->name()) : std::string{};
}

std::string CommandHistory::redo_name() const {
    return can_redo() ? std::string(commands_[position_]->name()) : std::string{};
}

std::vector<std::string> CommandHistory::undo_names() const {
    std::vector<std::string> result; result.reserve(position_);
    for (std::size_t i = position_; i > 0; --i) result.emplace_back(commands_[i - 1]->name());
    return result;
}

std::vector<std::string> CommandHistory::redo_names() const {
    std::vector<std::string> result; result.reserve(commands_.size() - position_);
    for (std::size_t i = position_; i < commands_.size(); ++i) result.emplace_back(commands_[i]->name());
    return result;
}

} // namespace nsmb::core
