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

- Implemented editable backend sessions for objects, sprites, entrances, paths, progress paths, zones, and tileset references.
- Implemented branching command history and wired it to Qt Undo/Redo actions and shortcuts.
- Added a Qt property editor, add/delete controls, path-node controls, selection highlighting, dirty-state warnings, and per-level session caching.

## M4: save/rebuild pipeline

- Implemented loss-preserving course/BGDat serialization for every currently editable entity and reference type.
- Added round-trip validation, atomic file replacement, original-file backups, and manifest size updates.
- Wired the Qt File menu to create/open projects, import ROMs, save edits, invoke NCPatcher, and build ROMs.
- Hardened external-tool argument quoting and exercised the built-in ROM service against synthetic ROMs.
- Validated portable manifest paths and file indexes, made manifest/ROM output atomic, and prevented accidental project or extraction overwrites.
- Removed the misleading arbitrary raw-graphics sample; the M4 canvas honestly remains a schematic entity preview.

## M5: patching integration

- Harden NCPatcher config management.
- Add async execution, log streaming, progress, cancellation, and diagnostics.
- Evaluate whether tighter integration or upstream NCPatcher CLI changes are needed.

## M6: web frontend

- Compile portable backend to WebAssembly or expose it through another thin adapter.
- Keep browser APIs out of backend modules.
- Build a web UI around an explicit C ABI/binding layer.
