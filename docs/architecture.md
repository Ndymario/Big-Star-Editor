# Architecture

## Decision: strict backend/frontend split

The backend lives under `libs/` and is portable C++20. It must not expose Qt, WinForms, .NET, browser, DOM, or UI-thread concepts. Public backend headers use STL value types such as `std::string`, `std::vector`, `std::span`, and `std::filesystem::path`, plus explicit result/error structures.

The frontend lives under `apps/`. Qt-specific models and widgets are allowed only in `apps/nsmb-qt`; they adapt backend data and do not become the source of truth. The web frontend will use a separate adapter around the same backend.

## Module boundaries

- `nsmb-core`: common infrastructure: `Result<T>`, `Error`, `ByteBuffer`, little-endian reader/writer, logging, and undo/redo command interfaces.
- `nsmb-nds`: NDS-level concepts independent from NSMB: ROM headers, extracted ROM roots, file-tree nodes, ARM/overlay placeholders, and extraction/build service interfaces.
- `nsmb-formats`: NSMB/NDS format detection and future parsers. M0 deliberately includes only conservative identification stubs.
- `nsmb-project`: editor project state, manifest serialization, dirty-state tracking, and future domain objects (`Level`, `Area`, `Zone`, `Sprite`, `Entrance`, `Path`, `TilesetRef`).
- `nsmb-patching`: external process boundaries for NCPatcher, `nds-extract`, and `nds-build`.

## Error policy

Backend APIs return `nsmb::core::Result<T>` across module boundaries. Implementations may use local RAII and STL facilities internally, but parser/service APIs should report structured errors instead of leaking exceptions to callers.

## Initial vertical slice

M0 provides CMake configuration, backend libraries, a CLI, optional Qt shell, project manifest round-trip, file-tree scanning, NDS header parsing, command construction for external tools, and unit tests. Level editing is intentionally stubbed.

## Async frontend design

Long-running external tools are modeled as services with structured requests and results. The Qt M0 shell logs placeholders synchronously, but the service boundary is suitable for later execution on worker threads with progress reporting and cancellation.
