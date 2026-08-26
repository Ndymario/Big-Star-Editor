#include "nsmb/core/file_io.hpp"

#include <fstream>
#include <iterator>
#include <limits>
#include <system_error>

#ifdef _WIN32
#define NOMINMAX
#include <windows.h>
#endif

namespace nsmb::core {

namespace {

Result<void> replace_file(const std::filesystem::path& temporary, const std::filesystem::path& target) {
#ifdef _WIN32
    if (!MoveFileExW(temporary.c_str(), target.c_str(), MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH)) {
        std::error_code ignored;
        std::filesystem::remove(temporary, ignored);
        return make_error(ErrorCode::IoError, "could not replace " + target.string());
    }
#else
    std::error_code ec;
    std::filesystem::rename(temporary, target, ec);
    if (ec) {
        std::error_code ignored;
        std::filesystem::remove(temporary, ignored);
        return make_error(ErrorCode::IoError, "could not replace " + target.string());
    }
#endif
    return {};
}

} // namespace

Result<std::vector<std::uint8_t>> read_file(const std::filesystem::path& path) {
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return make_error(ErrorCode::IoError, "could not open " + path.string());
    }
    std::vector<std::uint8_t> data(std::istreambuf_iterator<char>(in), {});
    if (in.bad()) {
        return make_error(ErrorCode::IoError, "could not finish reading " + path.string());
    }
    return data;
}

Result<std::vector<std::uint8_t>> read_file_prefix(const std::filesystem::path& path, std::size_t maximum_size) {
    if (maximum_size > static_cast<std::size_t>(std::numeric_limits<std::streamsize>::max())) {
        return make_error(ErrorCode::InvalidArgument, "requested file prefix is too large");
    }
    std::ifstream in(path, std::ios::binary);
    if (!in) {
        return make_error(ErrorCode::IoError, "could not open " + path.string());
    }
    std::vector<std::uint8_t> data(maximum_size);
    if (!data.empty()) {
        in.read(reinterpret_cast<char*>(data.data()), static_cast<std::streamsize>(data.size()));
        data.resize(static_cast<std::size_t>(in.gcount()));
    }
    if (in.bad()) {
        return make_error(ErrorCode::IoError, "could not finish reading " + path.string());
    }
    return data;
}

Result<void> write_file_atomically(const std::filesystem::path& path, std::span<const std::uint8_t> data) {
    if (path.empty() || data.size() > static_cast<std::size_t>(std::numeric_limits<std::streamsize>::max())) {
        return make_error(ErrorCode::InvalidArgument, "output path is empty or file is too large");
    }
    std::error_code ec;
    const auto parent = path.parent_path();
    if (!parent.empty()) {
        std::filesystem::create_directories(parent, ec);
        if (ec) {
            return make_error(ErrorCode::IoError, "could not create directory for " + path.string());
        }
    }

    // ponytail: the desktop editor is the sole writer; use unique temp names if multi-process saves are added.
    auto temporary = path;
    temporary += ".bse-tmp";
    {
        std::ofstream out(temporary, std::ios::binary | std::ios::trunc);
        if (!out) {
            return make_error(ErrorCode::IoError, "could not write " + temporary.string());
        }
        if (!data.empty()) {
            out.write(reinterpret_cast<const char*>(data.data()), static_cast<std::streamsize>(data.size()));
        }
        out.flush();
        if (!out) {
            out.close();
            std::error_code ignored;
            std::filesystem::remove(temporary, ignored);
            return make_error(ErrorCode::IoError, "could not finish writing " + temporary.string());
        }
    }
    return replace_file(temporary, path);
}

Result<void> write_text_file_atomically(const std::filesystem::path& path, std::string_view text) {
    return write_file_atomically(path, {reinterpret_cast<const std::uint8_t*>(text.data()), text.size()});
}

} // namespace nsmb::core
