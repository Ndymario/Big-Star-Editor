# Packaging the desktop app

Two packages, each carrying the app and the `data/` catalogs it reads (actors,
backgrounds, tiles). NCPatcher is not in either: it is a tool of its own, and
the editor runs the `ncpatcher` it finds on the `PATH` (or the one set in
Preferences).

| Package | Script | Runs on | Built in CI on |
|---------|--------|---------|----------------|
| `BigStarEditor-<version>-<arch>.AppImage` (x86_64, aarch64) | `appimage/build.sh`, or `appimage/docker-build.sh` | Linux of that architecture, glibc 2.35 or newer | Ubuntu 22.04 |
| `BigStarEditor-<version>-Setup.exe` | `windows/build.sh` and `windows/bigstar.nsi` | 64-bit Windows 10 and 11 | `windows-latest` |

Both take the version from `[workspace.package]` in `Cargo.toml`. The graphics
drivers (Vulkan or OpenGL) and the window system are the machine's own and are
not carried. Each script's header lists what it needs to run by hand, and each
writes only into a work directory (`build-appimage/`, `build-windows/` unless one
is named).

`.github/workflows/package.yml` builds both, from the Actions page or on a tag:

```sh
git tag v1.2.3 && git push --tags   # must match the version in Cargo.toml
```

The workflow then creates the GitHub release with both files attached.

The editor finds `data/` beside the executable (Windows) or at
`../share/bigstar/data` from it (AppImage), see `default_data_dir` in
`crates/host`; `BIGSTAR_DATA` overrides both.

### AppImage in Docker, for any architecture

`appimage/docker-build.sh [x86_64|aarch64 ...]` runs `build.sh` inside an Ubuntu
22.04 container (`appimage/Dockerfile`), so the result does not depend on the
host's distribution or glibc, and a machine of one architecture can build for the
other. With no argument it builds for the host's own.

```sh
packaging/appimage/docker-build.sh            # build-appimage/x86_64/BigStarEditor-<version>-x86_64.AppImage
```

A foreign architecture runs under QEMU (slow); install the emulators once with
`docker run --privileged --rm tonistiigi/binfmt --install all`. Cargo's target
directory lives in a volume per architecture, so repeated builds are incremental.

### The Windows installer

Installs to `Program Files\Big Star Editor` for all users, adds Start menu and
desktop shortcuts and an Add/Remove Programs entry. Uninstalling removes the
program only; settings in `%APPDATA%\bigstar` and your projects stay.
