#include "nsmb/core/logging.hpp"

namespace nsmb::core {

std::string_view log_level_name(LogLevel level) {
    switch (level) {
    case LogLevel::Info: return "info";
    case LogLevel::Warning: return "warning";
    case LogLevel::Error: return "error";
    }
    return "info";
}

NullLogger& NullLogger::instance() {
    static NullLogger logger;
    return logger;
}

} // namespace nsmb::core
