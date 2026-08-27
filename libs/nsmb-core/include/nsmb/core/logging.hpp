#pragma once

#include <functional>
#include <string>
#include <string_view>

namespace nsmb::core {

enum class LogLevel { Info, Warning, Error };

std::string_view log_level_name(LogLevel level);

class Logger {
public:
    virtual ~Logger() = default;
    virtual void log(LogLevel level, std::string_view message) = 0;

    void info(std::string_view message) { log(LogLevel::Info, message); }
    void warning(std::string_view message) { log(LogLevel::Warning, message); }
    void error(std::string_view message) { log(LogLevel::Error, message); }
};

// Discards everything. The default wherever a Logger is optional, so callers
// never have to branch on a null pointer.
class NullLogger final : public Logger {
public:
    void log(LogLevel, std::string_view) override {}

    static NullLogger& instance();
};

// Forwards to a callable. This is how a frontend receives backend diagnostics
// without the backend knowing anything about it: the Qt shell hands over a
// lambda that appends to its Log dock, the CLI one that writes to stderr.
class CallbackLogger final : public Logger {
public:
    using Sink = std::function<void(LogLevel, std::string_view)>;

    explicit CallbackLogger(Sink sink) : sink_(std::move(sink)) {}

    void log(LogLevel level, std::string_view message) override {
        if (sink_) {
            sink_(level, message);
        }
    }

private:
    Sink sink_;
};

} // namespace nsmb::core
