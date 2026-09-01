#pragma once

#include <cstdint>
#include <filesystem>
#include <functional>
#include <mutex>
#include <string>
#include <string_view>
#include <vector>

#include "nsmb/core/result.hpp"

namespace nsmb::patching {

// Launching an external tool without a shell.
//
// The previous implementation built one command string, quoted it by hand, and
// handed it to std::system() after changing the process-wide working
// directory. That is three problems in one call: the quoting is a shell
// injection surface, the chdir is global state that no threaded build can
// share, and std::system() captures no output at all -- so a failed patcher run
// could report nothing but a number.
//
// Here the argument vector is passed through to the operating system intact.
// On POSIX that is execvp, which never sees a shell. On Windows the arguments
// are joined using the exact quoting rule the C runtime parses back, so the
// child reconstructs the vector that was sent.
// A handle on a running child, so a long build can be stopped.
//
// NCPatcher documents SIGINT (POSIX) or CTRL_BREAK_EVENT (Windows) to the
// process group as its cancellation signal, and answers with exit code 11 after
// writing its result document. That is a request, not a kill: the tool unwinds
// on its own terms, and an interrupted build is safe to run again because
// NCPatcher re-patches the pristine binaries it kept rather than its own
// half-written output.
//
// Because a child is put in its own process group, the signal reaches the
// compiler processes it spawned as well -- otherwise those keep running and the
// build only appears to have stopped.
//
// One token drives one run. `request()` is safe to call from any thread, and
// safe to call before the child exists (it fires as soon as it does), after it
// has exited (it does nothing), and more than once.
class ProcessCancel {
public:
    ProcessCancel() = default;
    ProcessCancel(const ProcessCancel&) = delete;
    ProcessCancel& operator=(const ProcessCancel&) = delete;

    void request();
    [[nodiscard]] bool requested() const;

    // Forgets any earlier request, so one token can drive the next run.
    void reset();

    // Used by run_process. `id` is the child's pid on POSIX and its process id
    // on Windows.
    void attach(std::intptr_t id);
    void detach();

private:
    mutable std::mutex mutex_;
    bool requested_{false};
    std::intptr_t id_{0};
};

struct ProcessRequest {
    std::filesystem::path executable;
    std::vector<std::string> arguments;

    // The child's working directory. Empty means inherit. Nothing here ever
    // changes the calling process's own directory.
    std::filesystem::path working_directory;

    // Extra environment entries for the child, as NAME=VALUE. The parent's
    // environment is inherited and these are layered on top.
    std::vector<std::string> environment;

    // Optional. Outlives the call when set.
    ProcessCancel* cancel{};
};

struct ProcessResult {
    int exit_code{};

    // True when the child was killed by a signal rather than exiting normally.
    // A caller that only checks exit_code would otherwise read a termination as
    // some ordinary failure code.
    bool terminated_by_signal{};
};

// Receives one line at a time, without its terminator.
//
// Called from a reader thread rather than the caller's, but serialised: two
// callbacks never run at once, and both have returned before run_process does.
using LineCallback = std::function<void(std::string_view line)>;

// Renders a command the way a shell would, purely so it can be logged or shown
// in a diagnostic. Never use the result to actually run anything -- that is the
// mistake this module exists to remove.
std::string describe_command(const ProcessRequest& request);

// Joins arguments using the quoting rule the Windows C runtime parses back.
// Exposed so it can be tested directly on every platform, since the round trip
// it has to satisfy is not observable from a POSIX run.
std::string windows_command_line(const std::filesystem::path& executable,
                                 const std::vector<std::string>& arguments);

// Runs to completion. Either callback may be empty, in which case that stream
// is drained and discarded rather than left to fill and block the child.
core::Result<ProcessResult> run_process(const ProcessRequest& request,
                                        const LineCallback& on_stdout_line,
                                        const LineCallback& on_stderr_line);

} // namespace nsmb::patching
