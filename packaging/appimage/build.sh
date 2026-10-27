#!/bin/sh
# Builds the desktop app and packs it as an AppImage: one file that carries
# the app and the data it reads (the actor, background and tile catalogs).
# NCPatcher is not in it: it is a tool of its own that the app finds on the
# PATH. It runs on any distribution of the architecture it was built on
# (x86_64 or aarch64) whose glibc is at least as new as this machine's,
# which is why it is built on Ubuntu 22.04: in CI (see
# .github/workflows/package.yml) and, for any architecture from any machine,
# in a container (see docker-build.sh).
#
#   packaging/appimage/build.sh [work-dir]
#
# Needs cargo, a C compiler, curl, file, patchelf, pkg-config, and the
# development files of Wayland and xkbcommon. Everything else is written
# under work-dir, by default build-appimage/ in the source tree; the
# AppImage lands there.
set -eu

case $(uname -m) in
    x86_64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) echo "no AppImage can be built on $(uname -m)" >&2; exit 1 ;;
esac

root=$(cd "$(dirname "$0")/../.." && pwd)
work=${1:-$root/build-appimage}
mkdir -p "$work"
work=$(cd "$work" && pwd)
app=$work/AppDir

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")
[ -n "$version" ] || { echo "no version in Cargo.toml" >&2; exit 1; }

(cd "$root" && cargo build --release --locked -p bigstar)

cd "$work"
tools=https://github.com/linuxdeploy
[ -f linuxdeploy ] || curl -fsSL --retry 3 -o linuxdeploy \
    "$tools/linuxdeploy/releases/download/continuous/linuxdeploy-$arch.AppImage"
chmod +x linuxdeploy

rm -rf "$app"
mkdir -p "$app/usr/bin" "$app/usr/share/bigstar" "$app/usr/share/doc/bigstar"
# Where default_data_dir (crates/host) looks for them: ../share/bigstar/data
# from the executable.
cp -r "$root/data" "$app/usr/share/bigstar/data"
cp "$root/LICENSE" "$root/docs/licenses.md" "$app/usr/share/doc/bigstar/"

# The tools are AppImages themselves; unpacking instead of mounting lets them
# run where FUSE is missing, which includes CI and containers.
export APPIMAGE_EXTRACT_AND_RUN=1
# linuxdeploy's own strip is too old for libraries from current distributions
# and fails on them.
export NO_STRIP=1
# The program is most of its size in symbols, so it is stripped here, into a
# copy linuxdeploy is given instead.
mkdir -p "$work/bin"
strip -o "$work/bin/bigstar" "${CARGO_TARGET_DIR:-$root/target}/release/bigstar"

out="BigStarEditor-$version-$arch.AppImage"
rm -f "$out"
export LINUXDEPLOY_OUTPUT_VERSION="$version"
export LDAI_OUTPUT="$out"
./linuxdeploy --appdir "$app" \
    --executable "$work/bin/bigstar" \
    --desktop-file "$root/packaging/bigstar.desktop" \
    --icon-file "$root/packaging/bigstar.png" \
    --output appimage

# For the build log: what the program still takes from the system the
# AppImage runs on. The graphics drivers and the window system are meant to
# be the machine's own; anything else in this list is a library to bring
# along.
echo "Left to the system:"
ldd "$app/usr/bin/bigstar" | sed -n 's/^[[:space:]]*\([^ ]*\) => \(\/[^ ]*\) .*/\1/p' | sort -u | tr '\n' ' '
echo

echo "Created $work/$out"
