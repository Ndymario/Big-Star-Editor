#pragma once
#include <string_view>
namespace nsmb::core {
class Logger {
public:
    virtual ~Logger() = default;
    virtual void info(std::string_view message) = 0;
    virtual void warning(std::string_view message) = 0;
    virtual void error(std::string_view message) = 0;
};
}
