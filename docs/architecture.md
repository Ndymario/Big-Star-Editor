# Architecture

## Decision: strict backend/frontend split

The backend lives under `libs/` and is portable C++20. It must not expose Qt, WinForms, .NET, browser, DOM, or UI-thread concepts. Public backend headers use STL value types such as `std::string`, `std::vector`, `std::span`, and `std::filesystem::path`, plus explicit result/error structures.

The frontend lives under `apps/`. Qt-specific models and widgets are allowed only in `apps/nsmb-qt`; they adapt backend data and do not become the source of truth. The web frontend uses a separate C ABI adapter around the same backend.

## Module boundaries

- `nsmb-core`: common infrastructure: `Result<T>`, `Error`, `ByteBuffer`, little-endian reader/writer, atomic file I/O, logging, and undo/redo command interfaces.
- `nsmb-nds`: NDS-level concepts independent from NSMB: GBATEK-backed ROM headers, ARM binary ranges, FNT/FAT structures, overlay tables, extracted ROM roots, file-tree nodes, and extraction/build services.
- `nsmb-formats`: defensive NSMB level parsing/serialization plus portable Nitro compression, editable tile/palette/screen-map/Map16/object-layout codecs, TEX0, quantisation, and tileset bundles.
- `nsmb-project`: editor project state, manifest serialization, dirty-state tracking, editable level sessions, compound command history, regional ROM resource resolution, and decoded graphics assembly.
- `nsmb-patching`: no-shell external process boundary for NCPatcher.

## Error policy

Backend APIs return `nsmb::core::Result<T>` across module boundaries. Implementations may use local RAII and STL facilities internally, but parser/service APIs should report structured errors instead of leaking exceptions to callers.

## Current vertical slice

M0-M4 provides CMake configuration, backend libraries, a CLI, an optional Qt editor, YAML project manifest round-trip, file-tree scanning, NDS header parsing, built-in ROM extraction/build, level and graphics parsing, entity editing, safe persistence, and unit tests. The Qt UI adapts backend-owned editing sessions and command histories; closing a view does not discard its edited session.

The Qt workspace uses a reusable read-only Preview tab and one unique editable document per open level. Documents can move between the tab well and separate windows. Docks, open documents, detached-window geometry, zoom, and pan are restored per project with `QSettings`; the backend project and level sessions remain the source of truth.

Editable canvases provide native Qt tools for selection, panning, placement, paths, marquee selection, snapping, movement, supported resizing, and deletion. A gesture or same-kind multi-edit is submitted as one compound backend command so Undo/Redo follows user intent. Parsed level views remain visible but read-only until their serializer is implemented.

ROM graphics are resolved from the active extracted tree through `fnt.bin` and validated region-specific Overlay 0 tables. Portable backend codecs assemble and edit terrain, tileset objects, backgrounds, and foregrounds; Qt supplies only PNG I/O and widget presentation. Sandboxed per-actor Lua files own actor metadata and select verified sprite sheets, palettes, or model textures by filename through the active module-aware filesystem view; unknown layouts retain labeled fallbacks. Extracted artwork is never embedded in the application.

Course/BGDat saves preserve untouched block bytes, rebuild the structures the editor owns, validate the serialized result by parsing it again, back up both original files, and atomically replace each target. Manifest and ROM writes use the same failure-safe replacement policy. ROM builds consume the manifest's configured extracted tree through `NitroRomService` and refuse output paths inside that input tree.

## Async frontend design

Long-running external tools are modeled as services with structured requests and results. The Qt shell still runs external-tool actions synchronously, but the service boundary is suitable for later execution on worker threads with progress reporting and cancellation.

Large graphics resources decode on the global Qt thread pool. Each request carries a generation token, so a result is applied only when it still belongs to the current level state. `QFileSystemWatcher` events are debounced and refresh only documents that use the changed file. Failed refreshes leave the last valid image in place and emit diagnostics instead of blanking the canvas.

ROM-backed visual behavior is covered by the focused human checks in `docs/m4-manual-checklist.md`.

## WebAssembly boundary

`apps/nsmb-web/include/big_star_web.h` is the explicit versioned boundary between
portable C++ and the browser. Emscripten compiles the formats/core code and the
portable `EditableLevel` implementation, but not project-directory, patching,
CLI, Qt, or process adapters. Opaque handles own sessions; named JSON snapshots
carry presentation data; typed mutation calls feed the same command history as
the desktop editor; serialization exposes owned course/BGDat byte spans.

The JavaScript shell owns browser file inputs, DOM rendering, Blob downloads,
and WebAssembly memory copies. No browser type or lifecycle leaks into `libs/`.
The initial web scope is intentionally a local schematic level editor rather
than a second ROM/module manager.

## Network filesystem decision

Big Star does not define a separate remote-filesystem protocol. An active
module's NitroFS tree is an ordinary project directory and can already live on
an OS-mounted network volume or in a synchronised folder; the existing watcher,
atomic-write, provenance, and pending-build behavior remains the same. Building
another transport without an authentication, conflict-resolution, offline, and
recovery contract would duplicate that workflow while weakening its safety.

## Project files

Projects are opened by directory, so the project manifest does not serialize an absolute project root. `big-star-project.yaml` stores only project-relative paths such as `extracted-rom/root/...`, which keeps projects portable across machines and avoids leaking user filesystem paths. See `docs/project-format.md`.
