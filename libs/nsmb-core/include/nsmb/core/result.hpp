#pragma once

#include <string>
#include <utility>
#include <variant>

namespace nsmb::core {

enum class ErrorCode {
    Ok = 0,
    InvalidArgument,
    OutOfBounds,
    IoError,
    ParseError,
    NotFound,
    ExternalToolFailed,
    Unsupported,
};

struct Error {
    ErrorCode code{ErrorCode::Ok};
    std::string message;
};

template <typename T>
class Result {
public:
    Result(const T& value) : storage_(value) {}
    Result(T&& value) : storage_(std::move(value)) {}
    Result(Error error) : storage_(std::move(error)) {}

    [[nodiscard]] bool has_value() const { return std::holds_alternative<T>(storage_); }
    explicit operator bool() const { return has_value(); }
    [[nodiscard]] const T& value() const { return std::get<T>(storage_); }
    [[nodiscard]] T& value() { return std::get<T>(storage_); }
    [[nodiscard]] const Error& error() const { return std::get<Error>(storage_); }

private:
    std::variant<T, Error> storage_;
};

template <>
class Result<void> {
public:
    Result() = default;
    Result(Error error) : error_(std::move(error)), ok_(false) {}
    [[nodiscard]] bool has_value() const { return ok_; }
    explicit operator bool() const { return ok_; }
    [[nodiscard]] const Error& error() const { return error_; }
private:
    Error error_{};
    bool ok_{true};
};

inline Error make_error(ErrorCode code, std::string message) {
    return Error{code, std::move(message)};
}

} // namespace nsmb::core
