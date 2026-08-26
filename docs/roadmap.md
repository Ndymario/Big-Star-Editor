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
- Added a VS Code-style document workspace with a reusable Preview tab, unique editable level tabs, detachable editor windows, dirty markers, active-document command routing, recoverable docks, and per-project layout restoration.
- Added Select, Pan, placement, path, multi-selection, marquee, snapping, resizing, deletion, and compound undo/redo gestures to the level canvas, plus a searchable Palette dock.
- Added portable Nitro LZ/BLZ, 8bpp tile, palette, screen-map, Map16, and NSMB object-layout decoding with defensive fallbacks.
- Added validated US, European, Japanese, Korean, and Chinese ROM resource profiles and FNT/Overlay 0 resolution for terrain, backgrounds, foregrounds, and the complete actor placement catalog.
- Added asynchronous ROM-backed rendering, stale-result rejection, watched-resource refresh, and last-valid-image preservation during malformed external writes.
- Kept actors as labeled schematic fallbacks until their ROM OAM or model layouts are verified; extracted game artwork remains project-local and is never packaged with the editor.

## M5: NCPatcher 2.x integration

- Replace the legacy `std::system` wrapper with an asynchronous, no-shell NCPatcher 2.x process integration.
- Support `ncpatcher.yaml` and `ncpatcher.yml`; refuse to build directly from legacy `ncpatcher.json`.
- Offer NCPatcher’s verified `migrate --write` workflow for legacy projects, leaving the old JSON untouched until a successful build and optional Move to Trash confirmation.
- Offer NCPatcher project initialization and configuration validation through the v2 CLI.
- Stream structured progress, diagnostics, artifacts, and result summaries into the Qt editor.
- Apply patches transactionally by building a staged ROM, patching a separate output, validating and extracting it, then backing up and replacing the live project tree.
- Support selection of one configured build variant; defer multi-variant application and graphical YAML editing.
- Keep NCPatcher external rather than embedding or vendoring it.
- Defer cancellation until NCPatcher provides a graceful process-tree cancellation contract.

## M6: web frontend

- Compile portable backend to WebAssembly or expose it through another thin adapter.
- Keep browser APIs out of backend modules.
- Build a web UI around an explicit C ABI/binding layer.
