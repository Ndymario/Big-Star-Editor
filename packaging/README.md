# Packaging the desktop app

Three packages, each carrying the app and the `data/` catalogs it reads (actors,
backgrounds, tiles). NCPatcher is not in either: it is a tool of its own, and
the editor runs the `ncpatcher` it finds on the `PATH` (or the one set in
Preferences).

| Package | Script | Runs on | Built in CI on |
|---------|--------|---------|----------------|
| `BigStarEditor-<version>-<arch>.AppImage` (x86_64, aarch64) | `appimage/build.sh`, or `appimage/docker-build.sh` | Linux of that architecture, glibc 2.35 or newer | Ubuntu 22.04 |
| `BigStarEditor-<version>-Setup.exe` | `windows/build.sh` and `windows/bigstar.nsi` | 64-bit Windows 10 and 11 | `windows-latest` |
| `BigStarEditor-<version>-macOS.dmg` (universal: Apple silicon and Intel) | `macos/build.sh` | macOS 11 or newer | `macos-latest` |

All take the version from `[workspace.package]` in `Cargo.toml`. The graphics
drivers (Vulkan or OpenGL) and the window system are the machine's own and are
not carried. Each script's header lists what it needs to run by hand, and each
writes only into a work directory (`build-appimage/`, `build-windows/`,
`build-macos/` unless one is named).

The AUR package ([`aur/`](aur/README.md)) builds from source instead of carrying a binary.

`.github/workflows/package.yml` builds all three, from the Actions page or on a tag:

```sh
git tag v1.2.3 && git push --tags   # must match the version in Cargo.toml
```

The workflow then creates the GitHub release with the three files attached.

The editor finds `data/` beside the executable (Windows), at
`../share/bigstar/data` from it (AppImage) or at `../Resources/data` from it
(macOS), see `default_data_dir` in `crates/host`; `BIGSTAR_DATA` overrides them.

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

### The macOS app

`macos/build.sh` builds the app for both Apple silicon and Intel and joins
them into one universal `Big Star Editor.app`, inside a disk image that has a
link to Applications to drag it onto. It needs the Xcode command line tools and
`rustup target add aarch64-apple-darwin x86_64-apple-darwin`.

An app started from the Finder does not see the `PATH` a shell sets, so
NCPatcher installed with Homebrew or in a home directory is best set in
Preferences.

#### Signing and notarizing

A downloaded app opens without a warning only when it is signed with a
**Developer ID Application** certificate and notarized by Apple. An *Apple
Development* certificate does not do: it is for running on your own devices.
Without one the app is signed ad hoc, and the first time it is opened macOS
refuses it until it is allowed under **System Settings > Privacy & Security >
Open Anyway**.

Once, to set it up:

1. Create the certificate: in Xcode, **Settings > Accounts >** your team **>
   Manage Certificates… > + > Developer ID Application** (only the team's
   Account Holder can). It lands in your login keychain.
2. For notarizing, create an API key on App Store Connect, under **Users and
   Access > Integrations > App Store Connect API**, with the *Developer* role.
   Download the `.p8` file (it can be downloaded only once) and note its Key ID
   and the Issuer ID above the list.

To sign and notarize a build on your Mac, store the key under a profile name
and give the script both:

```sh
xcrun notarytool store-credentials bigstar --key AuthKey_XXXXXXXXXX.p8 --key-id XXXXXXXXXX --issuer <issuer-id>
MACOS_SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)" \
    MACOS_NOTARY_PROFILE=bigstar packaging/macos/build.sh
```

For CI, export the certificate with its private key from Keychain Access
(**My Certificates**, right-click it, **Export…**, as a `.p12` with a password)
and add these repository secrets under **Settings > Secrets and variables >
Actions**:

| Secret | What it holds |
|--------|---------------|
| `MACOS_CERTIFICATE` | The `.p12` in base64: `base64 -i certificate.p12 \| pbcopy` |
| `MACOS_CERTIFICATE_PASSWORD` | The password it was exported with |
| `MACOS_NOTARY_KEY` | The text of the `.p8` file |
| `MACOS_NOTARY_KEY_ID` | The key's Key ID |
| `MACOS_NOTARY_ISSUER` | The Issuer ID (left empty for an individual key) |

With only the first two the app is signed but not notarized; with none it is
signed ad hoc. The workflow warns in either case.
