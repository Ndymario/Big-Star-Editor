//! Launching an external tool without a shell. Port of `libs/nsmb-patching/src/process.cpp`.
//!
//! The argument vector reaches the operating system intact through `std::process::Command`, which never involves a
//! shell. Both output streams are captured separately and handed over a line at a time, the working directory is set
//! for the child only, and a child is started in its own process group so a cancellation reaches the compilers it
//! spawned as well.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use bigstar_core::{Error, ErrorCode, Result};

/// A handle on a running child, so a long build can be stopped.
///
/// NCPatcher documents SIGINT (POSIX) or CTRL_BREAK_EVENT (Windows) to the process group as its cancellation
/// signal, and answers with exit code 11 after writing its result document. That is a request, not a kill: the tool
/// unwinds on its own terms, and an interrupted build is safe to run again because NCPatcher re-patches the pristine
/// binaries it kept rather than its own half-written output.
///
/// One token drives one run. [`ProcessCancel::request`] is safe to call from any thread, and safe to call before the
/// child exists (it fires as soon as it does), after it has exited (it does nothing), and more than once.
#[derive(Debug, Default)]
pub struct ProcessCancel {
    state: Mutex<CancelState>,
}

#[derive(Debug, Default)]
struct CancelState {
    requested: bool,
    id: u32,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Asks one running child, and everything it spawned, to stop.
#[cfg(unix)]
fn signal_group(id: u32) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;
    // The child leads its own group, so its pid names the group, and the compiler processes the build spawned are
    // interrupted too rather than left running under a stopped parent.
    if let Ok(pid) = i32::try_from(id) {
        let _ = killpg(Pid::from_raw(pid), Signal::SIGINT);
    }
}

/// Windows has no safe way to raise CTRL_BREAK_EVENT for another process group (it needs FFI, and this workspace
/// forbids `unsafe`), so the tree is ended with `taskkill` instead. NCPatcher then cannot write its result document
/// and the run reports a failure rather than `Cancelled`.
#[cfg(windows)]
fn signal_group(id: u32) {
    let _ = Command::new("taskkill").args(["/T", "/F", "/PID", &id.to_string()]).stdout(Stdio::null()).stderr(Stdio::null()).status();
}

#[cfg(not(any(unix, windows)))]
fn signal_group(_id: u32) {}

impl ProcessCancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn request(&self) {
        let mut state = lock(&self.state);
        state.requested = true;
        if state.id != 0 {
            signal_group(state.id);
        }
    }

    pub fn requested(&self) -> bool {
        lock(&self.state).requested
    }

    /// Forgets any earlier request, so one token can drive the next run.
    pub fn reset(&self) {
        lock(&self.state).requested = false;
    }

    /// Used by [`run_process`]. `id` is the child's process id.
    pub fn attach(&self, id: u32) {
        let mut state = lock(&self.state);
        state.id = id;
        // A cancel that arrived while the child was still being started is not lost: it fires the moment there is
        // something to send it to.
        if state.requested && state.id != 0 {
            signal_group(state.id);
        }
    }

    pub fn detach(&self) {
        lock(&self.state).id = 0;
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProcessRequest {
    pub executable: PathBuf,
    pub arguments: Vec<String>,
    /// The child's working directory. Empty means inherit. Nothing here ever changes the calling process's own
    /// directory.
    pub working_directory: PathBuf,
    /// Extra environment entries for the child, as `NAME=VALUE`. The parent's environment is inherited and these are
    /// layered on top.
    pub environment: Vec<String>,
    pub cancel: Option<Arc<ProcessCancel>>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcessResult {
    pub exit_code: i32,
    /// True when the child was killed by a signal rather than exiting normally. A caller that only checks
    /// `exit_code` would otherwise read a termination as some ordinary failure code.
    pub terminated_by_signal: bool,
}

/// Receives one line at a time, without its terminator. Called from a reader thread, but serialised: two callbacks
/// never run at once, and both have returned before [`run_process`] does.
pub type LineCallback<'a> = &'a mut (dyn FnMut(&str) + Send);

fn shell_display_quote(text: &str) -> String {
    let needs_quotes = text.is_empty() || text.contains([' ', '\t', '"', '\'', '\\']);
    if !needs_quotes {
        return text.to_owned();
    }
    let mut out = String::from("\"");
    for c in text.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('"');
    out
}

/// Renders a command the way a shell would, purely so it can be logged or shown in a diagnostic. Never use the
/// result to actually run anything -- that is the mistake this module exists to remove.
pub fn describe_command(request: &ProcessRequest) -> String {
    let mut out = shell_display_quote(&request.executable.to_string_lossy());
    for argument in &request.arguments {
        out.push(' ');
        out.push_str(&shell_display_quote(argument));
    }
    out
}

/// Joins arguments using the quoting rule the Windows C runtime parses back: a run of backslashes is literal unless
/// it precedes a quote, in which case each one must be doubled and the quote escaped. Exposed so it can be tested on
/// every platform.
pub fn windows_command_line(executable: &Path, arguments: &[String]) -> String {
    fn append(out: &mut String, argument: &str) {
        let needs_quotes = argument.is_empty() || argument.contains([' ', '\t', '\n', '\x0b', '"']);
        if !needs_quotes {
            out.push_str(argument);
            return;
        }
        out.push('"');
        let mut backslashes = 0usize;
        for c in argument.chars() {
            match c {
                '\\' => backslashes += 1,
                '"' => {
                    out.extend(std::iter::repeat_n('\\', backslashes * 2 + 1));
                    out.push('"');
                    backslashes = 0;
                }
                _ => {
                    out.extend(std::iter::repeat_n('\\', backslashes));
                    out.push(c);
                    backslashes = 0;
                }
            }
        }
        out.extend(std::iter::repeat_n('\\', backslashes * 2));
        out.push('"');
    }

    let mut command_line = String::new();
    append(&mut command_line, &executable.to_string_lossy());
    for argument in arguments {
        command_line.push(' ');
        append(&mut command_line, argument);
    }
    command_line
}

/// Splits a byte stream into lines. Holds the tail of a partial line between reads, and flushes it at end of stream
/// so a tool whose last line lacks a terminator is not silently dropped.
fn pump(mut stream: impl Read, mut emit: impl FnMut(&str)) {
    let mut pending: Vec<u8> = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let read = match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(_) => break,
        };
        for &byte in buffer.get(..read).unwrap_or_default() {
            match byte {
                b'\n' => {
                    emit(&String::from_utf8_lossy(&pending));
                    pending.clear();
                }
                b'\r' => {}
                _ => pending.push(byte),
            }
        }
    }
    if !pending.is_empty() {
        emit(&String::from_utf8_lossy(&pending));
    }
}

#[cfg(unix)]
fn own_process_group(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(windows)]
fn own_process_group(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
fn own_process_group(_command: &mut Command) {}

fn not_runnable(request: &ProcessRequest) -> Error {
    Error::new(
        ErrorCode::NotFound,
        format!("could not run {} (not found, not executable, or its working directory is missing)", request.executable.display()),
    )
}

struct Sinks<'a, 'b> {
    out: Option<LineCallback<'a>>,
    err: Option<LineCallback<'b>>,
}

/// Runs to completion. Either callback may be `None`, in which case that stream is drained and discarded rather
/// than left to fill and block the child.
pub fn run_process(
    request: &ProcessRequest,
    on_stdout_line: Option<LineCallback<'_>>,
    on_stderr_line: Option<LineCallback<'_>>,
) -> Result<ProcessResult> {
    let mut command = Command::new(&request.executable);
    command.args(&request.arguments).stdin(Stdio::inherit()).stdout(Stdio::piped()).stderr(Stdio::piped());
    // The directory change happens in the child, so the editor's own working directory is untouched and safe to use
    // from other threads.
    if !request.working_directory.as_os_str().is_empty() {
        command.current_dir(&request.working_directory);
    }
    for entry in &request.environment {
        if let Some((name, value)) = entry.split_once('=') {
            command.env(name, value);
        }
    }
    own_process_group(&mut command);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) if matches!(error.kind(), std::io::ErrorKind::NotFound | std::io::ErrorKind::PermissionDenied) => {
            return Err(not_runnable(request));
        }
        Err(error) => {
            return Err(Error::new(ErrorCode::ExternalToolFailed, format!("could not start {}: {error}", request.executable.display())));
        }
    };
    if let Some(cancel) = &request.cancel {
        cancel.attach(child.id());
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let sinks = Mutex::new(Sinks { out: on_stdout_line, err: on_stderr_line });
    let status = std::thread::scope(|scope| {
        if let Some(stdout) = stdout {
            let sinks = &sinks;
            scope.spawn(move || {
                pump(stdout, |line| {
                    if let Some(callback) = lock(sinks).out.as_mut() {
                        callback(line);
                    }
                })
            });
        }
        if let Some(stderr) = stderr {
            let sinks = &sinks;
            scope.spawn(move || {
                pump(stderr, |line| {
                    if let Some(callback) = lock(sinks).err.as_mut() {
                        callback(line);
                    }
                })
            });
        }
        child.wait()
    });
    if let Some(cancel) = &request.cancel {
        cancel.detach();
    }
    let status =
        status.map_err(|error| Error::new(ErrorCode::ExternalToolFailed, format!("could not wait for {}: {error}", request.executable.display())))?;

    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return Ok(ProcessResult { exit_code: 128 + signal, terminated_by_signal: true });
        }
    }
    let exit_code = status.code().unwrap_or(1);
    // What a shell or exec wrapper reports for a program it could not start.
    if cfg!(unix) && exit_code == 127 {
        return Err(not_runnable(request));
    }
    Ok(ProcessResult { exit_code, terminated_by_signal: false })
}
