#!/bin/sh
# Builds the desktop app for Windows and its installer: a setup exe carrying
# the app and the data it reads (the actor, background and tile catalogs).
# NCPatcher is not in it: it is a tool of its own that the app finds on the
# PATH. CI runs it on every packaging run (see .github/workflows/package.yml).
#
#   packaging/windows/build.sh [work-dir]
#
# Run it in Git Bash (or any shell with sed, cp and cygpath) on a Windows
# machine with Rust's MSVC toolchain and the Visual C++ build tools, and NSIS
# installed from https://nsis.sourceforge.io/ (or `choco install nsis`).
# Everything is written under work-dir, by default build-windows/ in the
# source tree: dist/ is the folder the installer installs, and the Setup exe
# lands beside it.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)
work=${1:-$root/build-windows}
mkdir -p "$work"
work=$(cd "$work" && pwd)
dist=$work/dist

version=$(sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\(.*\)"/\1/p' "$root/Cargo.toml")
[ -n "$version" ] || { echo "no version in Cargo.toml" >&2; exit 1; }

(cd "$root" && cargo build --release --locked -p bigstar)

rm -rf "$dist"
mkdir -p "$dist/docs"
cp "${CARGO_TARGET_DIR:-$root/target}/release/bigstar.exe" "$dist/"
# Where default_data_dir (crates/host) looks for them: beside the executable.
cp -r "$root/data" "$dist/data"
cp "$root/LICENSE" "$dist/"
cp "$root/docs/licenses.md" "$dist/docs/"

# A Unicode NSIS installer reads the license page's text file in the ANSI code
# page unless it starts with a UTF-8 byte-order mark, which would turn a
# (c) sign or curly quotes into "Â©" and the like. The installer gets a copy
# with one, beside dist/ and not in it, so LICENSE itself stays as it is.
license=$work/LICENSE.installer.txt
printf '\357\273\277' > "$license"
cat "$root/LICENSE" >> "$license"

makensis=$(command -v makensis || true)
for candidate in "/c/Program Files (x86)/NSIS/makensis.exe" "/c/Program Files/NSIS/makensis.exe"; do
    [ ! -x "$candidate" ] || makensis=$candidate
done
[ -n "$makensis" ] || {
    echo "makensis not found: install NSIS from https://nsis.sourceforge.io/" >&2
    exit 1
}
out=$work/BigStarEditor-$version-Setup.exe
rm -f "$out"
# makensis wants Windows paths, and MSYS must not rewrite the defines into
# paths of its own.
MSYS2_ARG_CONV_EXCL='*' "$makensis" \
    "-DVERSION=$version" \
    "-DVIVERSION=$version.0" \
    "-DDIST_DIR=$(cygpath -w "$dist")" \
    "-DLICENSE_FILE=$(cygpath -w "$license")" \
    "-DICON=$(cygpath -w "$root/packaging/bigstar.ico")" \
    "-DOUTFILE=$(cygpath -w "$out")" \
    "$(cygpath -w "$root/packaging/windows/bigstar.nsi")"

echo "Created $out"
