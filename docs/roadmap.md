# Roadmap

## M0: project skeleton

- CMake project, backend libraries, CLI, optional Qt 6 Widgets shell, tests, docs, and external-tool service boundaries.
- Manifest save/load and synthetic unit tests.
- No full ROM import, level parser, or editor feature parity.

## M1: ROM import/list filesystem

- Harden the built-in `NitroRomService` extraction path and broaden synthetic ROM coverage.
- Populate `Project` with an extracted file tree and derived format metadata.
- Add safe NDS header and filesystem metadata inspection.

## M2: level format read-only viewer

- Identify NSMB level/course file pairs and related NARC/tile/graphics resources.
- Implement defensive read-only parsers with synthetic tests.
- Show a read-only level outline in Qt.

## M3: level editing and undo/redo

- Add domain commands for sprites, entrances, paths, zones, and tileset references.
- Wire backend command history to Qt actions.
- Keep UI models as adapters over backend state.

## M4: save/rebuild pipeline

- Serialize modified project files.
- Harden the built-in ROM build service for edited NitroFS trees and overlays.
- Validate output paths and preserve user backups.

## M5: patching integration

- Harden NCPatcher config management.
- Add async execution, log streaming, progress, cancellation, and diagnostics.
- Evaluate whether tighter integration or upstream NCPatcher CLI changes are needed.

## M6: web frontend

- Compile portable backend to WebAssembly or expose it through another thin adapter.
- Keep browser APIs out of backend modules.
- Build a web UI around an explicit C ABI/binding layer.
