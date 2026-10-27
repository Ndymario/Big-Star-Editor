#!/bin/sh
# Builds the AppImage in a container, so that it can be made for any
# architecture from any machine that has Docker, and so that its glibc is
# always Ubuntu 22.04's whatever the host runs.
#
#   packaging/appimage/docker-build.sh [x86_64|aarch64 ...]
#
# With no argument, the host's own architecture. A foreign one runs under
# QEMU and is slow; set it up once with
#
#   docker run --privileged --rm tonistiigi/binfmt --install all
#
# The AppImages land in build-appimage/<arch>/. Each architecture keeps its
# own cargo target directory in a Docker volume (bigstar-appimage-<arch>), so
# repeated builds are incremental and never mix with the host's target/.
set -eu

root=$(cd "$(dirname "$0")/../.." && pwd)

if [ $# -eq 0 ]; then
    case $(uname -m) in
        aarch64 | arm64) set -- aarch64 ;;
        *) set -- x86_64 ;;
    esac
fi

for arch in "$@"; do
    case $arch in
        x86_64) platform=linux/amd64 ;;
        aarch64) platform=linux/arm64 ;;
        *) echo "unknown architecture $arch (x86_64 or aarch64)" >&2; exit 1 ;;
    esac
    image=bigstar-appimage-builder:$arch
    out=$root/build-appimage/$arch
    mkdir -p "$out"

    docker build --platform "$platform" -t "$image" -f "$root/packaging/appimage/Dockerfile" \
        "$root/packaging/appimage"

    # The container's user is the caller's, so nothing in the tree ends up
    # owned by root. HOME and the cargo registry go to the volume.
    docker run --rm --platform "$platform" \
        --user "$(id -u):$(id -g)" \
        -e HOME=/work/home \
        -e CARGO_HOME=/work/cargo \
        -e CARGO_TARGET_DIR=/work/target \
        -v "$root:/src" \
        -v "bigstar-appimage-$arch:/work" \
        -v "$out:/out" \
        "$image" sh -c '
            mkdir -p /work/home /work/target /work/cargo
            cd /src
            packaging/appimage/build.sh /work/build
            # Copied beside it, then renamed over it: an AppImage still
            # running from the old file keeps reading that one, where
            # writing into it would crash it.
            for file in /work/build/BigStarEditor-*.AppImage; do
                name=$(basename "$file")
                cp "$file" "/out/.$name.new" && mv -f "/out/.$name.new" "/out/$name"
            done
        '
    echo "Built $(ls "$out"/BigStarEditor-*-"$arch".AppImage)"
done
