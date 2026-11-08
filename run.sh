#!/bin/sh
# Runs Big Star Editor from the source code in this script's folder, on Linux and macOS, on any architecture Rust
# builds for there (x86_64, aarch64, and the others rustup supports). On Windows, run.bat does the same.
#
# The first run installs what building needs when it is missing:
#   - Linux: a C compiler and pkg-config (the editor builds Lua from source), the Wayland and xkbcommon headers, the
#     X11 and Vulkan libraries the window and the renderer load, and, optionally, xdg-desktop-portal (file and folder
#     dialogs) and fuse3 (the ROM mounted as a folder). They come from the system's package manager (apt, dnf, pacman
#     or zypper), through sudo.
#   - macOS: the Xcode Command Line Tools, which hold the compiler and linker.
#   - Rust, through rustup, for this machine's own architecture. A Rust older than the editor needs is updated.
# Cargo then only recompiles what changed since the last run, so an unchanged tree starts straight away.
# After building, debug symbol files and empty leftover folders are removed; Cargo does not track them, so this never
# causes a recompile.
#
# Everything the installers, the build, the cleanup and the editor print goes to target/run.log, replaced on every run.
# Arguments are passed to the editor, so `./run.sh path/to/workspace` opens that workspace.

set -u

cd "$(dirname "$0")" || exit 1
mkdir -p target
LOG="$(pwd)/target/run.log"
RUST_VERSION=$(sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)
OS=$(uname -s)

stamp() { date '+%Y-%m-%d %H:%M:%S'; }
section() { printf '\n[%s] %s\n' "$(stamp)" "$1" >> "$LOG"; }
say() { printf '%s\n' "$1"; printf '%s\n' "$1" >> "$LOG"; }

failed() {
    echo "The last lines of $LOG:"
    tail -n 30 "$LOG"
    # Started by a double click, the window would close before the message could be read.
    if [ -t 0 ]; then
        printf 'Press Enter to close.'
        read -r _
    fi
    exit 1
}

printf '[%s] Big Star Editor run started in %s (%s %s)\n' "$(stamp)" "$(pwd)" "$OS" "$(uname -m)" > "$LOG"

# ---- Privileges -------------------------------------------------------------------------------------------------------

as_root() {
    if [ "$(id -u)" -eq 0 ]; then
        "$@"
    elif command -v sudo > /dev/null 2>&1; then
        sudo "$@"
    else
        say "Installing needs root, and sudo is not available: run $* as root, then run this script again."
        return 1
    fi
}

# ---- Linux system packages --------------------------------------------------------------------------------------------

# Whether the shared library $1 (such as libX11.so.6) is installed.
has_library() {
    if command -v ldconfig > /dev/null 2>&1; then
        ldconfig -p 2> /dev/null | grep -q "$1"
    else
        /sbin/ldconfig -p 2> /dev/null | grep -q "$1"
    fi
}

# Whether everything building and running needs is installed. The optional packages are installed alongside these,
# but are never a reason to install.
linux_has_everything() {
    command -v cc > /dev/null 2>&1 || return 1
    command -v pkg-config > /dev/null 2>&1 || return 1
    command -v curl > /dev/null 2>&1 || command -v cargo > /dev/null 2>&1 || return 1
    pkg-config --exists wayland-client xkbcommon 2> /dev/null || return 1
    for library in libX11.so.6 libXcursor.so.1 libXi.so.6 libXrandr.so.2 libvulkan.so.1; do
        has_library "$library" || return 1
    done
    return 0
}

ensure_linux_packages() {
    linux_has_everything && return 0
    section "System packages install"
    if command -v apt-get > /dev/null 2>&1; then
        say "Some packages building needs are missing: installing them with apt (sudo asks for your password)..."
        as_root apt-get update >> "$LOG" 2>&1 || return 1
        as_root apt-get install -y build-essential pkg-config curl libwayland-dev libxkbcommon-dev libx11-6 \
            libxcursor1 libxi6 libxrandr2 libvulkan1 mesa-vulkan-drivers xdg-desktop-portal fuse3 >> "$LOG" 2>&1 || return 1
    elif command -v dnf > /dev/null 2>&1; then
        say "Some packages building needs are missing: installing them with dnf (sudo asks for your password)..."
        as_root dnf install -y gcc pkgconf-pkg-config curl wayland-devel libxkbcommon-devel libX11 libXcursor libXi \
            libXrandr vulkan-loader mesa-vulkan-drivers xdg-desktop-portal fuse3 >> "$LOG" 2>&1 || return 1
    elif command -v pacman > /dev/null 2>&1; then
        say "Some packages building needs are missing: installing them with pacman (sudo asks for your password)..."
        as_root pacman -S --needed --noconfirm base-devel pkgconf curl wayland libxkbcommon libx11 libxcursor libxi \
            libxrandr vulkan-icd-loader xdg-desktop-portal fuse3 >> "$LOG" 2>&1 || return 1
    elif command -v zypper > /dev/null 2>&1; then
        say "Some packages building needs are missing: installing them with zypper (sudo asks for your password)..."
        as_root zypper --non-interactive install gcc pkg-config curl wayland-devel libxkbcommon-devel libX11-6 \
            libXcursor1 libXi6 libXrandr2 libvulkan1 xdg-desktop-portal fuse3 >> "$LOG" 2>&1 || return 1
    else
        say "No supported package manager (apt, dnf, pacman or zypper) was found. Install a C compiler, pkg-config, curl,"
        say "the Wayland and xkbcommon development files, and the X11 (libX11, libXcursor, libXi, libXrandr) and Vulkan"
        say "libraries, then run this script again."
        return 1
    fi
    linux_has_everything && { say "System packages installed."; return 0; }
    say "The packages were installed, but something building needs is still missing; target/run.log says what ran."
    return 1
}

# ---- macOS command line tools -----------------------------------------------------------------------------------------

ensure_macos_tools() {
    xcode-select -p > /dev/null 2>&1 && command -v cc > /dev/null 2>&1 && return 0
    section "Xcode Command Line Tools install"
    say "The Xcode Command Line Tools are missing: a window asks to install them. Choose Install, and this script"
    say "goes on once they are in place."
    xcode-select --install >> "$LOG" 2>&1
    # The installer runs in its own window; wait for it, at most an hour.
    waited=0
    until xcode-select -p > /dev/null 2>&1 && command -v cc > /dev/null 2>&1; do
        if [ "$waited" -ge 3600 ]; then
            say "The Xcode Command Line Tools are still not installed. Install them with xcode-select --install, then"
            say "run this script again."
            return 1
        fi
        sleep 5
        waited=$((waited + 5))
    done
    say "Xcode Command Line Tools installed."
    return 0
}

# ---- Rust -------------------------------------------------------------------------------------------------------------

# Whether version $1 (like 1.90.0) is at least $2 (like 1.90).
version_at_least() {
    [ "$(printf '%s\n%s\n' "$2" "$1" | sort -t. -k1,1n -k2,2n -k3,3n | head -n 1)" = "$2" ]
}

# The host rustup should install for. On a Mac with Apple silicon, a terminal running under Rosetta would otherwise
# get the Intel toolchain.
rust_host() {
    if [ "$OS" = Darwin ] && [ "$(sysctl -n hw.optional.arm64 2> /dev/null)" = 1 ]; then
        echo aarch64-apple-darwin
    fi
}

ensure_rust() {
    # Installed in an earlier run, but this shell's PATH predates it.
    if ! command -v cargo > /dev/null 2>&1 && [ -x "$HOME/.cargo/bin/cargo" ]; then
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
    fi
    if ! command -v cargo > /dev/null 2>&1; then
        section "Rust install"
        say "Rust is not installed: installing it with rustup..."
        host=$(rust_host)
        if [ -n "$host" ]; then
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-host "$host" >> "$LOG" 2>&1
        else
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal >> "$LOG" 2>&1
        fi
        PATH="$HOME/.cargo/bin:$PATH"
        export PATH
        if ! command -v cargo > /dev/null 2>&1; then
            say "Rust could not be installed. Install it from https://rustup.rs and run this script again."
            return 1
        fi
        say "Rust installed."
    fi
    current=$(rustc --version 2> /dev/null | sed -n 's/^rustc \([0-9][0-9.]*\).*/\1/p')
    if [ -n "$RUST_VERSION" ] && [ -n "$current" ] && ! version_at_least "$current" "$RUST_VERSION"; then
        section "Rust update"
        if command -v rustup > /dev/null 2>&1; then
            say "Rust $current is older than the $RUST_VERSION the editor needs: updating it with rustup..."
            rustup update stable >> "$LOG" 2>&1 || return 1
        else
            say "Rust $current is older than the $RUST_VERSION the editor needs, and it was not installed with rustup."
            say "Update it, or install Rust from https://rustup.rs, then run this script again."
            return 1
        fi
    fi
    return 0
}

# ---- Run --------------------------------------------------------------------------------------------------------------

case "$OS" in
    Linux) ensure_linux_packages || failed ;;
    Darwin) ensure_macos_tools || failed ;;
    *) say "This script runs on Linux and macOS; on Windows, use run.bat."; failed ;;
esac
ensure_rust || failed

echo "Building..."
section Build
if ! cargo build --release -p bigstar >> "$LOG" 2>&1; then
    printf '[%s] Build failed\n' "$(stamp)" >> "$LOG"
    echo "Build failed."
    failed
fi

echo "Cleaning up..."
section Cleanup
# Debug symbols: split DWARF on Linux, dSYM bundles on macOS.
find target/release -maxdepth 2 \( -name '*.dwp' -o -name '*.dwo' -o -name '*.dSYM' \) -print 2> /dev/null | while IFS= read -r path; do
    echo "Deleted $path" >> "$LOG"
    rm -rf "$path"
done
for folder in target/release/examples target/release/incremental target/doc target/tmp target/package; do
    if [ -d "$folder" ]; then
        echo "Removed $folder" >> "$LOG"
        rm -rf "$folder"
    fi
done

echo "Starting the editor..."
section Editor
cargo run --release -p bigstar -- "$@" >> "$LOG" 2>&1
code=$?
printf '[%s] Editor exited with code %s\n' "$(stamp)" "$code" >> "$LOG"
if [ "$code" -ne 0 ]; then
    echo "The editor exited with code $code."
    failed
fi
exit 0
