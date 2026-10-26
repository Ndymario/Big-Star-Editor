# The environment the AppImage is built in: Ubuntu 22.04, the oldest release
# whose glibc (2.35) still covers current distributions. The image holds the
# tools only; the source is mounted when it runs (docker-build.sh).
#
# Built once per architecture with --platform, which docker runs under QEMU
# when it is not the machine's own.
FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive \
    RUSTUP_HOME=/opt/rustup \
    CARGO_HOME=/opt/cargo \
    PATH=/opt/cargo/bin:$PATH \
    APPIMAGE_EXTRACT_AND_RUN=1

RUN apt-get update && apt-get install -y --no-install-recommends \
        build-essential ca-certificates curl file git patchelf pkg-config xz-utils \
        libwayland-dev libxkbcommon-dev \
        desktop-file-utils squashfs-tools zsync \
    && rm -rf /var/lib/apt/lists/*

RUN curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable

# Writable by whoever the container runs as: the volume mounted here takes
# its permissions from this directory.
RUN mkdir -m 777 /work

WORKDIR /src
