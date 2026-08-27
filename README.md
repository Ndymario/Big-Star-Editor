# Big Star Editor

Big Star Editor is a modern, portable C++ rewrite/rearchitecture of the New Super Mario Bros. DS editor. It now has backend libraries, service boundaries, a CLI, a Qt 6 desktop editor, tests, and documentation. It does **not** include ROM data, Nintendo SDK files, Nitro SDK contents, or game assets.

## Architecture at a glance

- `libs/nsmb-core`: result/error types, byte buffers, endian utilities, atomic file I/O, logging, and command interfaces.
- `libs/nsmb-nds`: GBATEK-backed Nintendo DS ROM/header/FNT/FAT/overlay structures plus built-in extraction/build services.
- `libs/nsmb-formats`: defensive NSMB level, Nitro 2D/TEX0, and DS compression codecs.
- `libs/nsmb-project`: frontend-agnostic project model, manifest load/save, editable level sessions, and undo/redo commands.
- `libs/nsmb-patching`: narrow service wrapper for NCPatcher.
- `apps/nsmb-cli`: minimal CLI for inspection and service command construction.
- `apps/nsmb-qt`: optional Qt 6 Widgets editor with project/ROM file workflows, level listing, property editing, selection highlighting, undo/redo, safe saves, patching, and ROM builds.
- `apps/nsmb-web`: dependency-free browser level editor over an explicit C ABI and WebAssembly adapter.

Project manifests are YAML files named `big-star-project.yaml`. They store project-relative paths only; the project directory is implied by where the manifest is opened.

## Build requirements

- CMake 3.20 or newer.
- A C++20 compiler:
  - Windows: Visual Studio 2022 or recent LLVM/MinGW.
  - Linux: GCC 11+ or Clang 14+.
  - macOS: AppleClang from a recent Xcode.
- Optional: Qt 6 Widgets development package for `apps/nsmb-qt`.
- Optional: Emscripten for the browser frontend.

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

## WebAssembly frontend

```sh
emcmake cmake -S . -B build-wasm \
  -DBSE_BUILD_TESTS=OFF -DBSE_BUILD_QT=OFF -DCMAKE_BUILD_TYPE=Release
cmake --build build-wasm --target nsmb-web
python3 -m http.server --directory build-wasm/apps/nsmb-web
```

Open `http://127.0.0.1:8000/`. The browser editor keeps the selected course and
BGDat files local, supports the main level entity tables plus undo/redo, and
downloads the serialized pair. See `apps/nsmb-web/README.md` for its ABI and
scope.

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

`import-rom` uses the built-in DS ROM service to extract ARM binaries, overlay tables, FNT/FAT data, banner data, and the NitroFS tree, then atomically writes `big-star-project.yaml` with validated project-relative paths. It refuses to overwrite an existing project or a nonempty extraction directory. `build-rom` rebuilds a ROM from the extracted path recorded in that manifest and atomically replaces the chosen output.

The Qt editor can create and open projects, import `.nds` files, edit levels and Nitro graphics, inspect the module-aware filesystem, run NCPatcher, and build a new ROM. Its graphics dialog views NCLR/NCGR/NSCR/NCER/NANR and TEX0 resources, paints indexed character pixels, and edits BGR555 palettes with the native colour picker. The tileset workspace edits graphics, Map16, objects, behaviours, background/foreground NSC maps and slot IDs, with quantized PNG and `.bstileset` import/export. Sprite definitions use Big Star's strict, zero-based JSON schema to supply names, categories, property fields, render shapes, and grab bounds; layers load in plugin, project, local, then shipped order with the first object ID winning. The M12 desktop workspace adds a five-tab Project Hub, visual actor grids, minimap and canvas overlays, compressed entity clipboard, named history, `.nml`/`.nsmbpatch`/xdelta exchange, level search and event tools, crash recovery, and five-language shell localization. Level and graphics saves target module NitroFS files so the derived extracted tree is never mistaken for authoritative data.
