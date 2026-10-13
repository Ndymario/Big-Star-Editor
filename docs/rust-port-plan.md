# Big Star Editor: Rust rewrite with a remote-capable egui frontend

Phased plan, written to be executed by Sonnet one phase per session segment.

## Context

Big Star Editor (`/d/Projects/Desktop/Big-Star-Editor`) is a C++20 backend (`libs/`, ~9k lines +
~8k lines of tests) with a Qt 6 Widgets frontend (`apps/nsmb-qt`, ~12.5k lines + 2.4k of tests)
and a narrow JS/Emscripten level editor (`apps/nsmb-web`). The goal is one frontend for desktop
and browser, modelled on `/d/Projects/Web/wordcraft` (Cargo workspace, swappable `ui-egui` crate,
`trunk` web app, `xtask` gates).

Decisions already made by the user. Do not reopen them:

- **Full Rust rewrite** of backend and frontend.
- **The browser never opens ROMs.** A *host* process runs on the machine holding the projects; the
  frontend is a remote view of it. The backend never compiles to wasm, so Lua, NCPatcher, the
  filesystem and file watching are host-side only.
- **Desktop = embedded + remote.** The desktop app runs the host in-process and can also connect
  to a remote host. The browser build is always remote. One protocol for both.
- **Host scope = a workspace root.** List/open/create projects under it; import a `.nds` already
  under it through a host-side file browser. Nothing outside the root is reachable.
- **Access = one user, token.** Bind `127.0.0.1` by default. Several tabs of one user may connect.
- **Migration = same repo, staged.** Cargo workspace beside the CMake tree. C++ stays buildable
  and is the reference until the final phase deletes it.

## Rules for every phase

1. **Start:** read `docs/rust-port-plan.md` (this plan, copied into the repo in Phase 0) and its
   Status table. Do the first phase not marked done. Read the C++ files the phase names in full
   before writing Rust; they are the specification, together with the docs the phase names.
2. **Port, don't redesign.** Same behaviour, same error cases, same bytes out. Every C++ test in
   the named test files becomes a Rust `#[test]` with the same inputs and assertions.
3. **Code rules.** Edition 2024; `unsafe_code = "forbid"`; non-test code has no `unwrap`,
   `expect`, `panic!`, `todo!`, `unimplemented!`, indexing with unchecked input-derived numbers
   (use `get`, checked arithmetic). Errors go through `bigstar_core::Error`/`Result`.
4. **Layering.** `core`, `nds`, `formats`, `level`, `protocol`, `client`, `ui-egui`,
   `apps/bigstar-web` must build for `wasm32-unknown-unknown`. `ui-egui` must not depend on
   `project`, `patching` or `host`. Nothing below `ui-egui` depends on egui/eframe/rfd.
5. **Do not touch** `libs/`, `apps/nsmb-*`, `CMakeLists.txt` or `data/` unless the phase says so.
6. **Gate:** `cargo xtask ci` passes (fmt check, clippy `-D warnings`, tests, layers, wasm).
7. **Finish:** tick the phase in the Status table of `docs/rust-port-plan.md` with a one-line
   note of anything deferred, make **one commit** with the message the phase gives (end it with
   the Co-Authored-By line; no Claude-Session trailer), then **stop**. Report what was done and
   what was deferred in a few lines and tell the user to run `/compact` before the next phase.
   Do not begin the next phase in the same turn.
8. If a phase cannot be finished (missing ROM, missing tool, an ambiguity the C++ does not
   settle), commit what passes the gate, record the blocker in the Status table, stop and say so.
9. UI phases: after building, take a headless screenshot (the `ui_shot` example from Phase 6)
   and look at it before committing.

## Target layout

```
Cargo.toml                 workspace
crates/core                Error/Result, byte reader/writer, endian, atomic file I/O, command history
crates/nds                 ROM header, FNT/FAT, overlays, banner
crates/formats             level, glue, G2D, TEX0, BMG, NARC, tileset, UI resources, compression
crates/level               EditableLevel, tilemap composition, .nml exchange (pure, no filesystem)
crates/patching            no-shell process runner, NCPatcher 2.x client
crates/project             manifest, RomReader, EffectiveFilesystem, filesystem view, graphics
                           resources, module data, patch apply, Lua actors/tiles
crates/protocol            serde request / response / event types, protocol version
crates/host                the service: workspace root, projects, sessions, watchers, patcher runs
crates/client              Connection trait; InProcess (native) and WebSocket (native + wasm)
crates/ui-egui             the frontend
apps/bigstar               desktop app: embedded host + "Connect to host..."
apps/bigstar-web           trunk + eframe WebRunner
apps/bigstar-host          headless `serve <root>` plus the CLI verbs of apps/nsmb-cli
xtask                      ci, layers, wasm, oracle
```

Crate package names are `bigstar-<dir>`. Dependencies are declared once in
`[workspace.dependencies]`. Use: `serde`, `serde_json` (feature `preserve_order`), `thiserror`,
`log`, `rmp-serde`, `mlua` (features `lua54`, `vendored`), `notify`, `axum` + `tokio` (host app
only), `ewebsock`, `egui`/`eframe`/`egui_extras`/`egui_dock`/`egui_kittest`, `rfd`, `image`
(png only), `sha2`. Take egui-family versions from `/d/Projects/Web/wordcraft/Cargo.toml`.

## Host / frontend contract

- **Transport.** Requests with ids, matching responses, and unsolicited events; MessagePack with
  named fields in binary frames. `InProcess` passes the same enums over channels to a host on a
  worker thread. `WebSocket` uses `ewebsock` for native and wasm alike. The first message is a
  handshake with protocol version and token.
- **Host is authoritative.** It owns `EditableLevel` sessions and undo history, keyed by
  project + variant + level. The frontend sends typed mutations (the compound
  `update_entities` / `add_entities` / `remove_entities` shape of
  `libs/nsmb-project/include/nsmb/project/editable_level.hpp`) and gets back a snapshot with a
  revision, undo/redo names and dirty flag. A drag is previewed locally and submitted once on
  release.
- **Graphics.** Host decodes, frontend paints. Map16 block atlases per tileset slot and
  background/foreground images travel as RGBA textures keyed by content hash and are cached by
  the frontend. The tilemap is composed client-side with the pure code in `crates/level`. Lua
  stays on the host and returns each sprite's draw list (`LuaActorDrawCommand`, images replaced
  by texture ids) and grab/render bounds.
- **Events.** Log lines, NCPatcher progress and diagnostics, build state, debounced file-change
  notices, session changes from another tab, recovery offers.
- **Files across the wire.** Interchange files (PNG, `.nml`, `.bstileset`, `.nsmbpatch`, BMG
  text) may be uploaded/downloaded through the frontend's file picker or read/written at a path
  under the workspace root. ROMs only come from under the root.
- **Server.** `GET /` serves the web bundle; `GET /ws` upgrades after a constant-time token
  check. Every client path is canonicalised and refused if it leaves the root, symlinks included.
- **State split.** Active variant/module, graphics pairing and generated path persist on the host
  in the project directory. Layout, zoom, pan, language and recent hosts persist on the frontend
  through eframe storage.

## Phases

Paths under "Port" are relative to the repo root. Each phase lists its commit message.

### Phase 0: skeleton
- Create branch `rust-port` from `beta`.
- Copy this plan to `docs/rust-port-plan.md` and append a `## Status` table with one row per
  phase (number, title, done, notes).
- Root `Cargo.toml` (resolver 3, edition 2024, workspace lints as in rule 3), `rustfmt.toml`,
  `clippy.toml` allowing unwrap in tests, `.gitignore` entries for `target/` and `dist/`.
- All crates and apps from the layout as compiling stubs; `apps/bigstar-web` builds a stub for
  wasm and prints a message natively, as `apps/wordcraft-web/src/main.rs` does.
- `xtask` with `ci`, `layers`, `wasm`, adapted from `/d/Projects/Web/wordcraft/xtask/src/main.rs`.
- Commit: `Start the Rust workspace beside the C++ tree`

### Phase 1: core and nds
- Port `libs/nsmb-core/src/*.cpp` and headers to `crates/core`, except `json.cpp` (replaced by
  `serde_json`) and logging (use `log`). Port `libs/nsmb-core/tests/core_tests.cpp`.
- Port `libs/nsmb-nds/src/rom.cpp` to `crates/nds` and `libs/nsmb-nds/tests/rom_tests.cpp`.
- File I/O in `core` sits behind `#[cfg(not(target_arch = "wasm32"))]`.
- Commit: `Port the core and NDS layers`

### Phase 2: formats I (levels, archives, compression)
- Port `libs/nsmb-formats/src/{level,glue,narc,identify}.cpp` and the LZ10/LZ11/BLZ/Yaz0 codecs
  (find them in `graphics.cpp`) to `crates/formats`. Port `glue_tests.cpp` and the compression
  cases of `graphics_tests.cpp`.
- Commit: `Port level, glue, NARC and compression codecs`

### Phase 3: the level model
- Port `libs/nsmb-project/src/editable_level.cpp` (and `module_data.cpp` if it needs it) to
  `crates/level`, with `CommandHistory` from `core`. Port the level cases of
  `libs/nsmb-project/tests/project_tests.cpp` and `module_data_tests.cpp`.
- Derive `serde` on every type that will cross the protocol.
- Commit: `Port the editable level and its history`

### Phase 4: project I and the CLI
- Port `libs/nsmb-project/src/{manifest,project,rom_reader,level_catalog}.cpp` to
  `crates/project`. Keep the hand-written YAML subset so manifests stay byte-compatible
  (`docs/project-format.md`). Port `rom_reader_tests.cpp`, the remaining `project_tests.cpp`
  cases, and `tests/synthetic_rom.hpp` as a `test-support` module other crates can use.
- `apps/bigstar-host`: `--version`, `inspect-rom`, `import-rom`, matching
  `apps/nsmb-cli/main.cpp` output.
- Commit: `Port manifests, in-place ROM reading and the first CLI verbs`

### Phase 5: protocol, host, client
- `crates/protocol`: handshake, workspace listing, host file browser (under root), project
  create/open/import, level list, level open/snapshot/mutate/undo/redo/save/close, log events.
  Start the level messages from the tables in `apps/nsmb-web/README.md`.
- `crates/host`: the service over those messages, with root confinement.
- `crates/client`: `Connection` trait, `InProcess`, `WebSocket`.
- `apps/bigstar-host serve <root> [--bind] [--port]`: axum, token printed at start, `/ws`.
- Tests: protocol round trips through `InProcess` on the synthetic ROM; path-escape refusals
  (`..`, absolute, symlink out of root); wrong-token refusal; one WebSocket smoke test.
- Commit: `Add the host service, its protocol and both transports`

### Phase 6: UI vertical slice
- `crates/ui-egui`: connect screen (embedded on native / host URL + token), workspace project
  list, level list, schematic level canvas (rectangles and markers), select, move, undo, redo,
  save. `Services` trait for file dialogs and storage, after wordcraft's.
- `apps/bigstar` (eframe native, embedded host), `apps/bigstar-web` (copy the shape of
  `/d/Projects/Web/wordcraft/apps/wordcraft-web/{Trunk.toml,index.html,src/web.rs}`).
- `crates/ui-egui/examples/ui_shot.rs` for headless screenshots; first `egui_kittest` tests.
- Verify all three: desktop embedded, desktop connected to `serve`, browser via `trunk serve`.
- **Checkpoint:** in the stop message, ask the user to try it before Phase 7.
- Commit: `First end-to-end slice: desktop and browser against one host`

### Phase 7: formats II (graphics)
- Port the rest of `libs/nsmb-formats/src/graphics.cpp`, plus `g2d.cpp` and `tex0.cpp`, with
  `graphics_tests.cpp`, `g2d_tests.cpp`, `tex0_tests.cpp`.
- Commit: `Port Nitro 2D, TEX0 and tile graphics codecs`

### Phase 8: formats III (messages, tilesets, UI resources)
- Port `message.cpp`, `tileset.cpp`, `ui_resources.cpp` with their three test files.
- Commit: `Port BMG, tileset and UI resource codecs`

### Phase 9: patching
- Port `libs/nsmb-patching/src/{process,ncpatcher}.cpp` to `crates/patching` on
  `std::process::Command` (argv only, both streams captured, cancellation). Port
  `patching_tests.cpp`; port `apps/nsmb-qt/tests/fake_ncpatcher.cpp` as a test binary.
  Spec: `docs/ncpatcher-integration.md`.
- CLI: `run-patcher [--variant]`, `patcher-info`.
- Commit: `Port the process runner and the NCPatcher client`

### Phase 10: project II (the effective filesystem)
- Port `effective_filesystem.cpp`, `filesystem_view.cpp`, `sprite_definitions.cpp` and whatever
  of `module_data.cpp` remains, with their test files.
- Commit: `Port the effective filesystem and its view`

### Phase 11: project III (graphics resources, tilemap, exchange)
- Port `graphics_resources.cpp`, `level_tilemap.cpp`, `level_exchange.cpp`, `patch_apply.cpp`
  with `level_tilemap_tests.cpp` and `level_exchange_tests.cpp`. `level_tilemap` and
  `level_exchange` go in `crates/level` if they need no filesystem; otherwise split the pure part
  out so the frontend can compose tilemaps. Spec: `docs/tileset-memory.md`.
- Commit: `Port graphics resolution, tilemap composition and exchange formats`

### Phase 12: Lua
- Port `lua_sandbox.cpp`, `lua_actors.cpp`, `lua_tiles.cpp` on `mlua`. The script API must not
  change: every file in `data/actors` and `data/tiles` loads unmodified. Port
  `lua_actors_tests.cpp` (2.1k lines) and `lua_tiles_tests.cpp`, including the
  `tests/fixtures/signboard-module` fixture. Specs: `docs/lua-actors.md`, `docs/lua-tiles.md`.
- Add a test that loads all shipped scripts and fails on any load error.
- Commit: `Port the Lua sandbox, actors and tile overlays`

### Phase 13: oracle
- This phase may edit `apps/nsmb-cli/main.cpp`: add an `oracle <project>` verb printing, per ROM
  file, the detected format and SHA-256 of parse-then-reserialize output; per level, entity JSON
  and tilemap hash; per actor, the draw list at default settings. Add the same verb to
  `bigstar-host` and `cargo xtask oracle <project>` to build both and diff.
- Run it on the coop NCPatcher test project (location is in the user's memory note on
  NCPatcher). Fix every Rust mismatch, adding a regression test per fix. If no project with a
  ROM is available, stop and ask.
- Commit: `Add the C++/Rust oracle and close the differences it found`

### Phase 14: host graphics and events
- Protocol + host: texture delivery by content hash, tileset atlases, background/foreground
  images, per-sprite draw lists and bounds, actor and object catalogs for the picker, file
  watching with debounce and stale-generation rejection (see "Async frontend design" in
  `docs/architecture.md`), variant/module selection.
- Tests through `InProcess`.
- Commit: `Serve decoded graphics, actor draw lists and change events`

### Phase 15: level canvas
- Port the drawing and interaction of `apps/nsmb-qt/src/LevelPreviewWidget.{hpp,cpp}`: real
  tiles and actors, unified click selection across layers, right-click create, eight resize
  knobs with cursors, marquee, 8-pixel snapping, Ctrl-drag clone, arrow nudging, WASD scroll,
  middle/Space/Alt pan, z-order, paths and nodes, zoom presets, grid, DS-screen overlay, minimap.
- Port the matching cases of `apps/nsmb-qt/tests/qt_tests.cpp` to `egui_kittest`.
- Commit: `Level canvas with real graphics and the full gesture set`

### Phase 16: properties, picker, level data
- Port `LevelPropertiesWidget` (side tabs per entity kind, generated sprite-setting fields from
  actor definitions, raw settings fallback), the permanent picker (object sub-tabs per tileset
  slot, searchable sprite list, custom actor tree) and `LevelDataWidget`.
- Compressed Base64 clipboard for every entity kind; named multi-step undo/redo menus.
- Commit: `Properties, picker and level data panels`

### Phase 17: workspace and level tools
- `egui_dock` tab well with a reusable Preview tab and one document per level; detached windows
  through egui viewports on desktop only; layout, zoom and pan restored per host + project.
- Level hex editor, `.nml` import/export, object/sprite finders, sprite-event viewer, dirty-close
  prompts, minute-interval recovery snapshots (host-side) with the startup offer.
- Commit: `Document workspace, recovery and the level tools`

### Phase 18: filesystem dock
- Port `FilesystemDock` and `ProjectTreeModel`: module-aware rows with status, provenance,
  variant, id and size columns; archives expanding like folders (`a/b.narc!c/d`); footer
  details; add, replace, extract, remove; pending-build banner; the generic hex document from
  `FileEditors`. Opening a file picks the editor from `identify`.
- Commit: `Filesystem dock and file documents`

### Phase 19: graphics editor
- Port `GraphicsEditorDialog` and `GraphicsCanvas` as a document tab: palette/graphics/tilemap
  pairing remembered per file, NCGR/NCBR and headerless sheets with explicit width, NSCR
  composition, NCER cells, NANR and TEX0 viewing, pixel painting, BGR555 palette editing with
  egui's colour picker, save to the module target keeping any LZ wrapper.
- Commit: `Graphics editor`

### Phase 20: message and resource editors
- Port `MessageEditorDialog` (BMG text, escapes, attributes, IDs, filter, text export/import),
  `UiResourceEditorDialog` (BNBL/BNCL/BNCD/ENPG), the banner editor and the NSMBeUD editor.
- Commit: `Message, UI resource, banner and metadata editors`

### Phase 21: tileset workspace
- Port `TilesetEditorDialog`: slot IDs, graphics, Map16, objects, behaviours, backgrounds,
  foregrounds; quantised PNG and `.bstileset` import/export; only changed resources written.
- Commit: `Tileset and background workspace`

### Phase 22: builds
- Port `PatcherPanel` and the build paths of `MainWindow.cpp`: Output dock with streamed
  progress and clickable diagnostics, Cancel, project-wide variant selector on the toolbar,
  Extract ROM, `.nsmbpatch` export/import, xdelta create/apply, legacy-config migration offer,
  ROM reload after a build.
- Commit: `Builds, variants and patch exchange`

### Phase 23: finish
- Five-language string table ported from `MainWindow.cpp`, structured after
  `/d/Projects/Web/wordcraft/crates/ui-egui/src/i18n`; preferences (language, autosave minutes,
  backups kept); edits from one tab appearing in another; `bigstar-host` embedding the trunk
  bundle; README and `docs/architecture.md` rewritten for the Rust tree.
- Walk the `docs/m*-manual-checklist.md` files against the Rust editor and list every gap in the
  Status table.
- Commit: `Localization, preferences, embedded web bundle and docs`

### Phase 24: remove the C++ tree
- **Only after the user says so explicitly.** Delete `libs/`, `apps/nsmb-qt`, `apps/nsmb-web`,
  `apps/nsmb-cli`, `CMakeLists.txt` and the `oracle` xtask; fix docs that mention them.
- Commit: `Retire the C++ and Qt implementation`

## Verification

- Every phase: `cargo xtask ci`; the CMake build is untouched and must still configure and pass
  `ctest` (check it in Phases 0 and 13, where it could be affected).
- Backend correctness: ported unit tests per phase, then the Phase 13 oracle on a real project;
  rerun the oracle after any later backend change.
- Host: protocol tests through `InProcess` with the synthetic ROM and fake NCPatcher.
- UI: `egui_kittest` tests and `ui_shot` screenshots each UI phase, compared by eye with the Qt
  editor on the same level.
- End to end from Phase 6 on: `cargo run -p bigstar` on a local project; `bigstar-host serve
  <root>` with the desktop app connected; `trunk serve` in `apps/bigstar-web` against the same
  host; edit in one, observe in the other, save, and from Phase 22 run a build.

## Risks

- **Byte fidelity.** Codecs preserve unknown bytes and untouched LZ streams. A codec is not done
  until the oracle matches on a whole ROM.
- **Lua version.** The C++ build links the system Lua (5.5 on this machine); the Rust host
  vendors 5.4. Phase 12's load-everything test is the guard.
- **Latency.** Remote editing relies on local drag preview and texture caching; check Phase 6
  and Phase 15 over a throttled connection.
- **Exposure.** The host writes files and launches NCPatcher; root confinement and the token
  check are tested in Phase 5 and must stay green.

## Status

| Phase | Title | Done | Notes |
|---|---|---|---|
| 0 | Skeleton | yes | |
| 1 | Core and nds | yes | `Command` and `CommandHistory` are generic over the edited target (commands are lent it per call). FNT paths are `/`-separated strings. JSON and logging are `serde_json` and `log`. |
| 2 | Formats I (levels, archives, compression) | yes | `identify_graphics_format` and `is_bmg` are ported here as small stand-ins so `identify_format` works; phases 7 and 8 add the codecs. `NsmbLevelEntrance.type` is `kind`. `NarcArchive::find` takes a `/`-separated `&str`. Compressor byte-equality with C++ is only asserted on the small reference cases; the phase 13 oracle covers whole ROMs. |
| 3 | The level model | yes | The module data catalog types and JSON parsers live in `crates/level`; `load_module_data_catalog` (filesystem, incl. the missing-directory case of `module_data_tests.cpp`) moves to `crates/project` in phase 4. `EditableLevel` keeps its state in a private `LevelState` that the generic `CommandHistory` lends to commands. `reorder_entity` takes a `ReorderKind` enum instead of an int; `resolve_sprite_object_id` returns `Option<u32>` instead of -1. |
| 4 | Project I and the CLI | yes | Deferred: `nsmb_graphics_profile` and the patch-apply cases of `project_tests.cpp` (phases 11 and 9/11). `load_module_data_catalog` and its missing-directory test landed here. The `bigstar-host` binary is named `bigstar-host` (package `bigstar-host-app`); `run-patcher` and `patcher-info` wait for phase 9. `RomReader::read`/`read_member` take a path, `read_id`/`read_member_id` the numeric id. The synthetic ROM is `bigstar_project::test_support` behind the `test-support` feature. |
| 5 | Protocol, host, client | yes | Saves go to `<project>/edits/<rom path>` (archive members to `<archive>_<ext>/<member>`), and a level reopens from there before the ROM; phase 10 replaces it with the module target. No save backups yet (phase 17). `Command` is now `Send`, so level sessions can live on the host's worker thread. `client::InProcess` takes any `protocol::Service`, which keeps `client` wasm-safe; the server reuses it per socket. `GET /ws` checks the token from the query before upgrading and again in the Hello; `GET /` is a placeholder until phase 23. Edits carry `base_revision` and a stale one is refused. Project/close is refused while a level is dirty. The WebSocket smoke test and the wrong-token tests live in `apps/bigstar-host`. |
| 6 | UI vertical slice | yes | Checked: the embedded host through `apps/bigstar/tests/embedded.rs` (import ROM through the file browser, open, drag, save, reload from disk); the WebSocket transport with `ui_shot <dir> <host> <token>` against `bigstar-host serve` on the coop ROM (level 1-1 with 197 objects drew correctly); `trunk build` for `bigstar-web`. Not checked: the web bundle in a real browser, and the desktop window on a display. `Services` is a trait (storage, folder picker, embedded host, default host address); settings live in `$XDG_CONFIG_HOME/bigstar/settings.json`, in `localStorage` on the web. Sprites sit on tiles like objects (x16 pixels), everything else on pixels; objects and sprites snap to tiles, the rest to 8 px. A drag is previewed locally and sent once on release as one `Update`. `ui-egui` has a `demo` host (one made-up level) for screenshots and tests, since it may not depend on `host`. Deferred to phase 15: marquee, resize knobs, right-click create, arrow nudging, real tiles. The host holds one open project for all connections, so a second tab opening a project replaces the first tab's (refused while a level is dirty); a per-connection project is a protocol change for a later phase. |
| 7 | Formats II (graphics) | yes | Checked beyond the unit tests: `write_tex0(parse_tex0(x)) == x` for every `.nsbmd` in the coop module (the C++ fixture test; set `BIGSTAR_COOP_MODULE` to point at a checkout outside `deps/`) and for 372 of 373 models in a clean NSMB (U) ROM (the other has no TEX0 block, `NotFound` as in C++). The ROM holds no RGCN/RLCN/RCSN/RECN/RNAN files, so the Nitro 2D codecs are only covered by the synthetic round trips until the phase 13 oracle. `quantize_bgr555` sorts boxes with a stable sort where the C++ uses `std::sort`, so ties between equal channel values can split differently. `NitroHeader.magic` and `Nitro2dBlock.magic` are `[u8; 4]` (`FourCc`). TEX0 names are read lossily as UTF-8. Defaulted C++ arguments are explicit (`decode_bgr555_palette(data, true)`, `mark_palette_blocks_transparent(p, 256)`, `quantize_bgr555(img, 256, 128)`, `compress_tex4x4(img, 8192)`). |
| 8 | Formats III (messages, tilesets, UI resources) | yes | Checked beyond the unit tests on a clean NSMB (U) ROM: all 3 BMG files, the jyotyu and nohara tilesets (untouched write is byte-identical; objects re-encoded from scratch decode to the same layouts) and the banner round trip. BMG text is a `String`, so invalid UTF-8 in a UTF-8 file and unpaired surrogates in a UTF-16 one become U+FFFD (C++ carried the bytes through). `banner_title`/`set_banner_title` use `Vec<u16>`/`&[u16]` for the C++ `u16string`. `BmgPart::text`/`BmgPart::escape` constructors added for tests and callers. |
| 9 | Patching | yes | `run_process` is `std::process::Command` with the child in its own process group (`process_group(0)` / `CREATE_NEW_PROCESS_GROUP`); cancellation on Unix is `killpg(SIGINT)` through `nix`. Windows has no safe way to raise CTRL_BREAK_EVENT (it needs FFI and the workspace forbids `unsafe`), so there `ProcessCancel` runs `taskkill /T /F` and a cancelled build reports a failure, not `Cancelled` -- not verified on Windows. The process tests re-run their own binary as the fixture (`tests/process.rs`, `harness = false`); the fake NCPatcher is the `bigstar-fake-ncpatcher` bin of `bigstar-patching`. `BuildObserver` callbacks are boxed `FnMut + Send`; the client logs through a `LogSink` (default: the `log` crate). Checked against a real NCPatcher (the `rewrite` branch of the local NCPatcher checkout, built on macOS, reports 1.0.4): `patcher-info` on an `init` project, and `run-patcher` streaming diagnostics and mapping exit 6 to `compilation failed`. That NCPatcher is older than the coop module's config (`region-catalog`), so the coop project was not built. |
| 10 | Project II (effective filesystem) | no | |
| 11 | Project III (graphics resources, tilemap, exchange) | no | |
| 12 | Lua | no | |
| 13 | Oracle | no | |
| 14 | Host graphics and events | no | |
| 15 | Level canvas | no | |
| 16 | Properties, picker, level data | no | |
| 17 | Workspace and level tools | no | |
| 18 | Filesystem dock | no | |
| 19 | Graphics editor | no | |
| 20 | Message and resource editors | no | |
| 21 | Tileset workspace | no | |
| 22 | Builds | no | |
| 23 | Finish | no | |
| 24 | Remove the C++ tree | no | |
