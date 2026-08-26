# Architecture

## Decision: strict backend/frontend split

The backend lives under `libs/` and is portable C++20. It must not expose Qt, WinForms, .NET, browser, DOM, or UI-thread concepts. Public backend headers use STL value types such as `std::string`, `std::vector`, `std::span`, and `std::filesystem::path`, plus explicit result/error structures.

The frontend lives under `apps/`. Qt-specific models and widgets are allowed only in `apps/nsmb-qt`; they adapt backend data and do not become the source of truth. The web frontend will use a separate adapter around the same backend.

## Module boundaries

- `nsmb-core`: common infrastructure: `Result<T>`, `Error`, `ByteBuffer`, little-endian reader/writer, atomic file I/O, logging, and undo/redo command interfaces.
- `nsmb-nds`: NDS-level concepts independent from NSMB: GBATEK-backed ROM headers, ARM binary ranges, FNT/FAT structures, overlay tables, extracted ROM roots, file-tree nodes, and extraction/build services.
- `nsmb-formats`: defensive NSMB level and Nitro graphics identification, parsing, and serialization helpers.
- `nsmb-project`: editor project state, manifest serialization, dirty-state tracking, editable level sessions, and backend command history.
- `nsmb-patching`: external process boundary for NCPatcher.

## Error policy

Backend APIs return `nsmb::core::Result<T>` across module boundaries. Implementations may use local RAII and STL facilities internally, but parser/service APIs should report structured errors instead of leaking exceptions to callers.

## Current vertical slice

M0-M4 provides CMake configuration, backend libraries, a CLI, an optional Qt editor, YAML project manifest round-trip, file-tree scanning, NDS header parsing, built-in ROM extraction/build, level and graphics parsing, entity editing, safe persistence, and unit tests. The Qt UI adapts backend-owned editing sessions and command histories; switching views does not discard an edited session.

Course/BGDat saves preserve untouched block bytes, rebuild the structures the editor owns, validate the serialized result by parsing it again, back up both original files, and atomically replace each target. Manifest and ROM writes use the same failure-safe replacement policy. ROM builds consume the manifest's configured extracted tree through `NitroRomService` and refuse output paths inside that input tree.

## Async frontend design

Long-running external tools are modeled as services with structured requests and results. The Qt shell still runs actions synchronously, but the service boundary is suitable for later execution on worker threads with progress reporting and cancellation.

## Project files

Projects are opened by directory, so the project manifest does not serialize an absolute project root. `big-star-project.yaml` stores only project-relative paths such as `extracted-rom/root/...`, which keeps projects portable across machines and avoids leaking user filesystem paths. See `docs/project-format.md`.
