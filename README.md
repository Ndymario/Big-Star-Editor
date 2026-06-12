# Big Star Editor

Big Star Editor is the start of a modern, portable C++ rewrite/rearchitecture of the New Super Mario Bros. DS editor. The first milestone is intentionally a foundation: backend libraries, service boundaries, a CLI, a Qt 6 desktop shell, tests, and documentation. It does **not** include ROM data, Nintendo SDK files, Nitro SDK contents, game assets, or full NSMB level editing.

## Architecture at a glance

- `libs/nsmb-core`: result/error types, byte buffers, endian utilities, logging and command interfaces.
- `libs/nsmb-nds`: GBATEK-backed Nintendo DS ROM/header/FNT/FAT/overlay structures plus built-in extraction/build services.
- `libs/nsmb-formats`: defensive format-identification and initial Nitro graphics helpers for future NSMB/NDS parsers.
- `libs/nsmb-project`: frontend-agnostic editor project model and manifest load/save.
- `libs/nsmb-patching`: narrow service wrapper for NCPatcher.
- `apps/nsmb-cli`: minimal CLI for inspection and service command construction.
- `apps/nsmb-qt`: optional Qt 6 Widgets desktop shell with project opening and level-candidate listing.
- `apps/nsmb-web`: documented placeholder for a future WebAssembly/web adapter.

Project manifests are YAML files named `big-star-project.yaml`. They store project-relative paths only; the project directory is implied by where the manifest is opened.

## Build requirements

- CMake 3.20 or newer.
- A C++20 compiler:
  - Windows: Visual Studio 2022 or recent LLVM/MinGW.
  - Linux: GCC 11+ or Clang 14+.
  - macOS: AppleClang from a recent Xcode.
- Optional: Qt 6 Widgets development package for `apps/nsmb-qt`.

## Configure, build, and test

```sh
cmake -S . -B build -DBSE_BUILD_TESTS=ON
cmake --build build
ctest --test-dir build --output-on-failure
```

If Qt 6 is unavailable, CMake skips `nsmb-qt` automatically. To disable Qt probing explicitly:

```sh
cmake -S . -B build -DBSE_BUILD_QT=OFF
```

## Platform notes

### Windows

```powershell
cmake -S . -B build -G "Visual Studio 17 2022" -DBSE_BUILD_TESTS=ON
cmake --build build --config Release
ctest --test-dir build -C Release --output-on-failure
```

Install Qt 6 with the Qt Online Installer or a package manager and ensure `CMAKE_PREFIX_PATH` points at the Qt installation if CMake cannot find it.

### Linux

```sh
sudo apt install cmake g++ qt6-base-dev # package names vary by distro
cmake -S . -B build -DBSE_BUILD_TESTS=ON
cmake --build build -j
ctest --test-dir build --output-on-failure
```

### macOS

```sh
brew install cmake qt
cmake -S . -B build -DCMAKE_PREFIX_PATH="$(brew --prefix qt)" -DBSE_BUILD_TESTS=ON
cmake --build build
ctest --test-dir build --output-on-failure
```

## CLI examples

```sh
./build/apps/nsmb-cli/nsmb-cli --version
./build/apps/nsmb-cli/nsmb-cli inspect-rom path/to/homebrew_or_user_provided.nds
./build/apps/nsmb-cli/nsmb-cli import-rom path/to/user_provided.nds my-project
./build/apps/nsmb-cli/nsmb-cli run-patcher my-project
./build/apps/nsmb-cli/nsmb-cli build-rom my-project output.nds
```

`import-rom` uses the built-in DS ROM service to extract ARM binaries, overlay tables, FNT/FAT data, banner data, and the NitroFS tree, then writes `big-star-project.yaml` with project-relative paths. `build-rom` rebuilds a ROM from the extracted project tree through the same backend service. Full level editing remains roadmap work.
