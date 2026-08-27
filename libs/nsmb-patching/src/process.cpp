#include "nsmb/patching/process.hpp"

#include <mutex>
#include <sstream>
#include <system_error>
#include <thread>

#ifdef _WIN32
#include <windows.h>
#else
#include <cerrno>
#include <csignal>
#include <cstring>
#include <spawn.h>
#include <sys/wait.h>
#include <unistd.h>
extern char** environ;
#endif

namespace nsmb::patching {

namespace {

// Splits a byte stream into lines and hands each to the callback. Holds the
// tail of a partial line between reads, and flushes it at end of stream so a
// tool whose last line lacks a terminator is not silently dropped.
class LineSplitter {
public:
    LineSplitter(const LineCallback& callback, std::mutex& mutex)
        : callback_(callback), mutex_(mutex) {}

    void feed(const char* data, std::size_t length) {
        for (std::size_t i = 0; i < length; ++i) {
            const char c = data[i];
            if (c == '\n') {
                emit();
            } else if (c != '\r') {
                pending_.push_back(c);
            }
        }
    }

    void finish() {
        if (!pending_.empty()) {
            emit();
        }
    }

private:
    void emit() {
        if (callback_) {
            std::lock_guard<std::mutex> lock(mutex_);
            callback_(pending_);
        }
        pending_.clear();
    }

    const LineCallback& callback_;
    std::mutex& mutex_;
    std::string pending_;
};

std::string shell_display_quote(const std::string& text) {
    bool needs_quotes = text.empty();
    for (const char c : text) {
        if (c == ' ' || c == '\t' || c == '"' || c == '\'' || c == '\\') {
            needs_quotes = true;
            break;
        }
    }
    if (!needs_quotes) {
        return text;
    }
    std::string out = "\"";
    for (const char c : text) {
        if (c == '"' || c == '\\') {
            out.push_back('\\');
        }
        out.push_back(c);
    }
    out.push_back('"');
    return out;
}

} // namespace

std::string describe_command(const ProcessRequest& request) {
    std::ostringstream oss;
    oss << shell_display_quote(request.executable.string());
    for (const auto& argument : request.arguments) {
        oss << ' ' << shell_display_quote(argument);
    }
    return oss.str();
}

std::string windows_command_line(const std::filesystem::path& executable,
                                 const std::vector<std::string>& arguments) {
    // The rule the Windows C runtime parses back: a run of backslashes is
    // literal unless it precedes a quote, in which case each one must be
    // doubled and the quote escaped. Getting this wrong is how a path with a
    // trailing separator swallows the argument after it.
    const auto append = [](std::string& out, const std::string& argument) {
        const bool needs_quotes =
            argument.empty() || argument.find_first_of(" \t\n\v\"") != std::string::npos;
        if (!needs_quotes) {
            out += argument;
            return;
        }
        out.push_back('"');
        for (std::size_t i = 0;; ++i) {
            std::size_t backslashes = 0;
            while (i < argument.size() && argument[i] == '\\') {
                ++i;
                ++backslashes;
            }
            if (i == argument.size()) {
                out.append(backslashes * 2, '\\');
                break;
            }
            if (argument[i] == '"') {
                out.append(backslashes * 2 + 1, '\\');
            } else {
                out.append(backslashes, '\\');
            }
            out.push_back(argument[i]);
        }
        out.push_back('"');
    };

    std::string command_line;
    append(command_line, executable.string());
    for (const auto& argument : arguments) {
        command_line.push_back(' ');
        append(command_line, argument);
    }
    return command_line;
}

#ifdef _WIN32

namespace {

core::Error last_error(const std::string& what) {
    const DWORD code = ::GetLastError();
    return core::make_error(core::ErrorCode::ExternalToolFailed,
                            what + " (Windows error " + std::to_string(code) + ")");
}

struct PipePair {
    HANDLE read{nullptr};
    HANDLE write{nullptr};

    ~PipePair() {
        if (read) {
            ::CloseHandle(read);
        }
        if (write) {
            ::CloseHandle(write);
        }
    }

    bool create() {
        SECURITY_ATTRIBUTES attributes{};
        attributes.nLength = sizeof(attributes);
        attributes.bInheritHandle = TRUE;
        if (!::CreatePipe(&read, &write, &attributes, 0)) {
            return false;
        }
        // Only the write end crosses into the child; an inheritable read end
        // would keep the pipe open after the child exits and hang the reader.
        return ::SetHandleInformation(read, HANDLE_FLAG_INHERIT, 0) != 0;
    }

    void close_write() {
        if (write) {
            ::CloseHandle(write);
            write = nullptr;
        }
    }
};

void pump(HANDLE handle, LineSplitter& splitter) {
    char buffer[4096];
    for (;;) {
        DWORD read_bytes = 0;
        if (!::ReadFile(handle, buffer, sizeof(buffer), &read_bytes, nullptr) || read_bytes == 0) {
            break;
        }
        splitter.feed(buffer, read_bytes);
    }
    splitter.finish();
}

std::wstring widen(const std::string& text) {
    if (text.empty()) {
        return {};
    }
    const int length =
        ::MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), nullptr, 0);
    std::wstring out(static_cast<std::size_t>(length), L'\0');
    ::MultiByteToWideChar(CP_UTF8, 0, text.data(), static_cast<int>(text.size()), out.data(), length);
    return out;
}

} // namespace

core::Result<ProcessResult> run_process(const ProcessRequest& request,
                                        const LineCallback& on_stdout_line,
                                        const LineCallback& on_stderr_line) {
    PipePair out_pipe;
    PipePair err_pipe;
    if (!out_pipe.create() || !err_pipe.create()) {
        return last_error("could not create a pipe for the external tool");
    }

    STARTUPINFOW startup{};
    startup.cb = sizeof(startup);
    startup.dwFlags = STARTF_USESTDHANDLES;
    startup.hStdOutput = out_pipe.write;
    startup.hStdError = err_pipe.write;
    startup.hStdInput = ::GetStdHandle(STD_INPUT_HANDLE);

    std::wstring command_line = widen(windows_command_line(request.executable, request.arguments));
    command_line.push_back(L'\0');

    std::wstring working_directory;
    if (!request.working_directory.empty()) {
        working_directory = request.working_directory.wstring();
    }

    // The child's environment is the parent's plus the request's entries. A
    // NAME=VALUE that already exists is replaced rather than duplicated.
    std::wstring environment_block;
    const bool custom_environment = !request.environment.empty();
    if (custom_environment) {
        std::vector<std::wstring> entries;
        if (LPWCH parent = ::GetEnvironmentStringsW()) {
            for (LPWCH cursor = parent; *cursor;) {
                std::wstring entry(cursor);
                cursor += entry.size() + 1;
                if (!entry.empty() && entry.front() != L'=') {
                    entries.push_back(std::move(entry));
                }
            }
            ::FreeEnvironmentStringsW(parent);
        }
        for (const auto& override_entry : request.environment) {
            const std::wstring wide = widen(override_entry);
            const auto equals = wide.find(L'=');
            if (equals == std::wstring::npos) {
                continue;
            }
            const std::wstring name = wide.substr(0, equals + 1);
            bool replaced = false;
            for (auto& entry : entries) {
                if (entry.size() >= name.size() &&
                    ::CompareStringOrdinal(entry.c_str(), static_cast<int>(name.size()),
                                           name.c_str(), static_cast<int>(name.size()),
                                           TRUE) == CSTR_EQUAL) {
                    entry = wide;
                    replaced = true;
                    break;
                }
            }
            if (!replaced) {
                entries.push_back(wide);
            }
        }
        for (const auto& entry : entries) {
            environment_block += entry;
            environment_block.push_back(L'\0');
        }
        environment_block.push_back(L'\0');
    }

    PROCESS_INFORMATION process{};
    const BOOL started = ::CreateProcessW(
        nullptr, command_line.data(), nullptr, nullptr, TRUE,
        CREATE_UNICODE_ENVIRONMENT,
        custom_environment ? static_cast<LPVOID>(environment_block.data()) : nullptr,
        working_directory.empty() ? nullptr : working_directory.c_str(), &startup, &process);
    if (!started) {
        return last_error("could not start " + request.executable.string());
    }

    // The parent's copies of the write ends must go before reading, or the
    // pipes never report end of stream.
    out_pipe.close_write();
    err_pipe.close_write();

    std::mutex callback_mutex;
    LineSplitter out_splitter(on_stdout_line, callback_mutex);
    LineSplitter err_splitter(on_stderr_line, callback_mutex);
    std::thread out_thread([&] { pump(out_pipe.read, out_splitter); });
    std::thread err_thread([&] { pump(err_pipe.read, err_splitter); });

    ::WaitForSingleObject(process.hProcess, INFINITE);
    out_thread.join();
    err_thread.join();

    DWORD exit_code = 0;
    ::GetExitCodeProcess(process.hProcess, &exit_code);
    ::CloseHandle(process.hThread);
    ::CloseHandle(process.hProcess);

    return ProcessResult{static_cast<int>(exit_code), false};
}

#else

namespace {

core::Error errno_error(const std::string& what, int code) {
    return core::make_error(core::ErrorCode::ExternalToolFailed,
                            what + ": " + std::strerror(code));
}

void pump(int fd, LineSplitter& splitter) {
    char buffer[4096];
    for (;;) {
        const ssize_t read_bytes = ::read(fd, buffer, sizeof(buffer));
        if (read_bytes > 0) {
            splitter.feed(buffer, static_cast<std::size_t>(read_bytes));
            continue;
        }
        if (read_bytes < 0 && errno == EINTR) {
            continue;
        }
        break;
    }
    splitter.finish();
}

} // namespace

core::Result<ProcessResult> run_process(const ProcessRequest& request,
                                        const LineCallback& on_stdout_line,
                                        const LineCallback& on_stderr_line) {
    int out_pipe[2] = {-1, -1};
    int err_pipe[2] = {-1, -1};
    if (::pipe(out_pipe) != 0) {
        return errno_error("could not create a pipe for the external tool", errno);
    }
    if (::pipe(err_pipe) != 0) {
        const int code = errno;
        ::close(out_pipe[0]);
        ::close(out_pipe[1]);
        return errno_error("could not create a pipe for the external tool", code);
    }

    // Built before the fork so the child does nothing that could allocate.
    std::vector<std::string> storage;
    storage.reserve(request.arguments.size() + 1);
    storage.push_back(request.executable.string());
    for (const auto& argument : request.arguments) {
        storage.push_back(argument);
    }
    std::vector<char*> argv;
    argv.reserve(storage.size() + 1);
    for (auto& entry : storage) {
        argv.push_back(entry.data());
    }
    argv.push_back(nullptr);

    std::vector<std::string> environment_storage;
    std::vector<char*> envp;
    if (!request.environment.empty()) {
        for (char** entry = environ; entry && *entry; ++entry) {
            environment_storage.emplace_back(*entry);
        }
        for (const auto& override_entry : request.environment) {
            const auto equals = override_entry.find('=');
            if (equals == std::string::npos) {
                continue;
            }
            const std::string name = override_entry.substr(0, equals + 1);
            bool replaced = false;
            for (auto& entry : environment_storage) {
                if (entry.compare(0, name.size(), name) == 0) {
                    entry = override_entry;
                    replaced = true;
                    break;
                }
            }
            if (!replaced) {
                environment_storage.push_back(override_entry);
            }
        }
        envp.reserve(environment_storage.size() + 1);
        for (auto& entry : environment_storage) {
            envp.push_back(entry.data());
        }
        envp.push_back(nullptr);
    }

    const std::string working_directory = request.working_directory.string();

    const pid_t pid = ::fork();
    if (pid < 0) {
        const int code = errno;
        ::close(out_pipe[0]);
        ::close(out_pipe[1]);
        ::close(err_pipe[0]);
        ::close(err_pipe[1]);
        return errno_error("could not fork to run " + request.executable.string(), code);
    }

    if (pid == 0) {
        // Child. Only async-signal-safe calls from here to the exec.
        ::close(out_pipe[0]);
        ::close(err_pipe[0]);
        if (::dup2(out_pipe[1], STDOUT_FILENO) < 0 || ::dup2(err_pipe[1], STDERR_FILENO) < 0) {
            ::_exit(127);
        }
        ::close(out_pipe[1]);
        ::close(err_pipe[1]);
        // The directory change happens here, in the child, so the editor's own
        // working directory is untouched and safe to use from other threads.
        if (!working_directory.empty() && ::chdir(working_directory.c_str()) != 0) {
            ::_exit(127);
        }
        if (envp.empty()) {
            ::execvp(argv[0], argv.data());
        } else {
            ::execve(argv[0], argv.data(), envp.data());
            if (errno == ENOENT) {
                environ = envp.data();
                ::execvp(argv[0], argv.data());
            }
        }
        ::_exit(127);
    }

    ::close(out_pipe[1]);
    ::close(err_pipe[1]);

    std::mutex callback_mutex;
    LineSplitter out_splitter(on_stdout_line, callback_mutex);
    LineSplitter err_splitter(on_stderr_line, callback_mutex);
    std::thread out_thread([&] { pump(out_pipe[0], out_splitter); });
    std::thread err_thread([&] { pump(err_pipe[0], err_splitter); });

    int status = 0;
    while (::waitpid(pid, &status, 0) < 0) {
        if (errno != EINTR) {
            status = 0;
            break;
        }
    }

    out_thread.join();
    err_thread.join();
    ::close(out_pipe[0]);
    ::close(err_pipe[0]);

    if (WIFSIGNALED(status)) {
        return ProcessResult{128 + WTERMSIG(status), true};
    }
    const int exit_code = WIFEXITED(status) ? WEXITSTATUS(status) : 1;
    if (exit_code == 127) {
        return core::make_error(core::ErrorCode::NotFound,
                                "could not run " + request.executable.string() +
                                    " (not found, not executable, or its working directory is missing)");
    }
    return ProcessResult{exit_code, false};
}

#endif

} // namespace nsmb::patching
