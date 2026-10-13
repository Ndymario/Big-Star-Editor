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
| 10 | Project II (effective filesystem) | yes | `module_data.cpp` was already fully ported (phases 3-4). `sprite_definitions` lives in `crates/level` (pure, so the frontend can build setting fields); field shifts that are undefined in C++ (width 0 or 64) are defined here. `project` now depends on `patching` for the NCPatcher documents. `EffectiveFilesystem::source` returns `Option<PathBuf>` for the C++ empty path; `FilesystemViewRequest` borrows the ROM as `Option<&mut RomReader>`. Only the synthetic tests were run: the local NCPatcher cannot plan the coop module (see phase 9), so the view has not been checked against a real plan. |
| 11 | Project III (graphics resources, tilemap, exchange) | yes | Split: `level_tilemap` (with `DecodedTileset` and `render_tileset_object`) and `level_exchange` are in `crates/level` so the frontend can compose tilemaps; `graphics_resources`, `patch_apply` and the tile-overlay types plus `overlay_map16_images`/`overlay_map16_sheet` (the non-Lua half of `lua_tiles.cpp`) are in `crates/project`; phase 12 adds the Lua loader. The `nsmb_graphics_profile`, object-bank and patch-apply cases of `project_tests.cpp` landed here; patch apply's test re-runs its own binary as the fake NCPatcher (`tests/patch_apply.rs`). Checked on a clean NSMB (U) ROM: `resolve_level_graphics` + `load_level_tilesets` + `rasterize_level_objects` for `course/A01_1` (394 objects, three tilesets composed) rendered correctly by eye. `.nsmbpatch` path safety splits on both `/` and `\` on every platform (the C++ splits on `/` only on POSIX). C++ narrows the 16-bit tileset ids to `uint8_t` table indices; the Rust does the same. |
| 12 | Lua | yes | `mlua` 0.11 with vendored Lua 5.4 and the `send` feature; the C++ links the system Lua 5.5. All 305 shipped actors and 8 tile scripts load, and the whole of `lua_actors_tests.cpp` (910 assertions over the shipped actors) passes unchanged on 5.4; the shipped-actor section was machine-translated and spot-checked by mutation. Checked on a clean NSMB (U) ROM: `data/tiles` resolves 163 textured blocks with no errors, and jyotyu's Map16 sheet renders the ROM's pipe textures by eye. `ctx` loaders and `renderer` methods are `mlua` scoped functions, so a kept handle errors after `init`/`render` returns (as the C++ does). Deviations: `actor:field(key)` works as `docs/lua-actors.md` documents (the C++ reads the wrong stack slot and always errors); error messages are Lua's own text without mlua wrapping, but are not byte-identical to the C++ (no test asserts them); `render_lua_actor` returns `LuaActorRender { commands, custom_rendered }` instead of an out-parameter; the resolver, image decoder and stage-object resolver are `&dyn Fn` (decoder and stage resolver optional). The image decoder is still injected; phase 14 supplies one. |
| 13 | Oracle | yes | `nsmb-cli oracle <project> <data-dir>` and `bigstar-host oracle` print, per ROM file (and per NARC member, one level deep), the identified format and the SHA-256 of parse-then-write for NARC, LZ10/LZ11 (decompress, recompress), G2D, TEX0 (plus every texture rendered), BMG and UI resources; per arm9 overlay the BLZ decompress/recompress; the banner round trip; the shipped tile scripts against the ROM; per `course/*.bin` with a `_bgdat.bin`, every entity, the serialized course and bgdat, each tileset's decoded Map16 pixels (with tile overlays), the rasterised tilemap and the background/foreground images; and per shipped actor its bounds and draw list at default settings (stills sized from their PNG header so neither side needs a PNG codec). `cargo xtask oracle <project>` builds both (C++ in `build/`) and diffs line by line. The coop project's built ROM was not available on this machine (and the local NCPatcher cannot build it), so it ran instead on a clean NSMB (U) ROM and five real hacks under `~/NSMB-DS-Modding` and `~/Nextcloud` (Ordinary Ztar Mod, Ordinary NSMB Mod, Working King Mario, NSMB Decomp, an MvsL template hack): all six agree line for line (42-50k lines each), so no Rust fixes were needed. C++ `ctest`: 1 failure, `nsmb-lua-actors-tests` line 263, pre-existing and not caused by this phase -- line 260 keeps a pointer into the temporary `sprite_definitions()` catalog (use after free); the Rust port keeps the catalog alive. Rerun `cargo xtask oracle` on the coop project's built ROM when one exists. |
| 14 | Host graphics and events | yes | Protocol 2. `GetTextures` delivers RGBA by content hash (first 16 bytes of SHA-256 over size and pixels, hex; pixels as MessagePack binary). `LevelGraphics` gives per slot a Map16 atlas (`ATLAS_COLUMNS` = 32 blocks a row), the object layouts (the frontend rasterises with `bigstar_level`; previews and erase/inert effects are computed from the same layouts, so there is no separate object-catalog request), and the background/foreground textures, each with its own error. `ActorCatalog` lists the actor definitions and the sprite-type to class-id table; `RenderActors` returns draw lists with texture ids plus grab/render bounds per `ActorInput`. Actor layers are project `bigstar/actors`, `actors`, then enabled modules, then the shipped `data/actors`; tile layers likewise for `tiles`. Stills decode with `image` on the host. `LevelSnapshot.sprite_hashes` carries module sprite hashes. A `notify` watcher on the open project debounces 250 ms and emits `FilesChanged`; changes outside `edits/` and `.big-star*` drop the cached graphics and emit `GraphicsChanged { generation }` -- answers carry their generation so a frontend drops stale ones. Active variant/module and the generated path persist in `<project>/.big-star-workspace.json`; `SetActiveVariant` is refused while a level is dirty, closes the level sessions and emits `SettingsChanged` + `GraphicsChanged`. NCPatcher (`HostOptions.ncpatcher`, `serve --ncpatcher`) is asked once per open/variant for variants, modules and the plan behind the effective filesystem; without it the ROM is read alone and `patcher_error` says why. `serve --data` sets the scripts directory (default `BIGSTAR_DATA`, then beside the executable, then the source tree). Checked on a clean NSMB (U) project through the service: level A01_1's graphics (19 ms) and all 39 sprites' draw lists (46 ms) composed into the correct level image. Not done: level saves still go to `edits/` rather than the module target (needs the module graph in the save path; phase 22 brings builds); the UI only records generation/settings (phase 15 draws). |
| 15 | Level canvas | yes | `canvas.rs` paints and `level_view.rs` decides, so every gesture rule is unit-testable. Real tiles: the frontend rasterises the objects with `rasterize_level_objects` (cached, keyed by the objects with the gesture preview applied and the tileset serial) and draws each written cell as a quad from its slot's atlas, one mesh per slot, nearest sampling. Actors draw from `RenderActors` lists (rotation, flips, opacity, all six kinds); grab/render bounds come from the draw list, else the definition; an actor whose list is still on its way is drawn from its previous list moved, or not at all, so nothing flickers to a marker; failed lists fall back to the numbered marker. The session requests `LevelGraphics`, `ActorCatalog`, `RenderActors` (only sprites whose key is new) and `GetTextures` (once per id, kept across levels), drops answers older than the newest `GraphicsChanged` generation and keeps stale art on screen until the new arrives; graphics failures go to the log, not the notice bar. Gestures as `LevelPreviewWidget`: unified front-to-back hit test (views, zones, objects, sprites, entrances, path nodes; ±3 px), click/Shift toggle, marquee, right-click create of the chosen kind (snapped; unsigned kinds refused past the origin, objects clamped), eight knobs with resize cursors on a single object/zone/view, one snap for the whole drag (16 for all-tile selections, else 16/8 from "8 px"; Alt frees it), moves past the origin dropped whole, Ctrl-drag clone (the move waits for the copies, even if released first), Ctrl+D, PageUp/PageDown reorder, arrow nudge, WASD (S is the sprite kind, as in Qt), O/S/E/Z/P/G/V kinds, middle/Space/Alt pan, wheel/pinch zoom about the pointer (0.02-16, pins), Fit/50/100/200%, grid, DS-screen overlay, minimap, Fullscreen, Escape cancels. Undo names match `EditableLevel`'s ("Move selection", "Resize selection", "Paste selection", "Delete selection", "Add object", ...). The demo host now wraps a real `EditableLevel` plus drawn-in-code tileset art and three actors. Checked by eye on the clean ROM's A01_1 through `bigstar-host serve` over WebSocket. Ported Qt cases: rightClickCreates, customPalettePlacementCarriesHash, objectResizeHandleCommitsAndEscapeCancels, everyResizableKindHasEightKnobs, topLeftKnobMovesTheViewOrigin, selectionIgnoresTheCreateKind, containersYieldTheClick, the canvas half of spriteDefinitionsDriveFieldsAndGrabBounds, canvasChromeFollowsThePalette (map surface and minimap; the harness paints no page behind panels), the clone/z-order/signed-path parts of clipboardCloneAndZOrder, unsignedEntitiesSnapBack, entrancesZonesAndViewsHoldTheSameBound, outOfBoundsQuadrantIsTinted, anObjectInFrontCullsTheOneBehind (the last four render through wgpu). Deferred or changed: copy/cut/paste and the `C` shortcut, the properties preview of a resize (`propertiesPreviewAResize`) and the picker cases go with Phase 16; the reference image (needs a file from `Services`) is not ported yet; `nodelessKindsStayResizable` has nothing to test, since only paths carry a node. Deliberate differences: several selected nodes of one path all move (Qt kept only the last), a whole-path selection moves every node (Qt moved none); a dragged actor's draw list is translated rather than re-run each frame; an actor that returns false gets the marker over what it drew, as `docs/lua-actors.md` says, not Qt's cached zero-settings thumbnail; the camera keeps the level point at the canvas corner rather than re-centring on the scene bounds every paint. |
| 16 | Properties, picker, level data | yes | Protocol 3: `LevelSnapshot` carries the glue block, `stage_object_id_base` and `registration_hashes`; `LevelEdit::SetGlue`; `Request::ProjectData` answers the module catalog and the object-bank overlay table. `properties.rs` is `LevelPropertiesWidget` as data (rows, editors, notes, representability, the gesture preview, and the edit one turned row makes, with the Qt undo names: "Edit object"/"Edit sprite"/... single, "Edit selection" for several); `properties_ui.rs` draws it: side tabs Course/Banks/Objects/Sprites/Entrances/Views/Zones/Paths/Progress, item and node pickers, label-left rows with peers paired, number/choice/tri-state checkbox editors, hex display, notes on an ℹ button, the actor header with a filterable type picker and the actor-info window, the raw settings box, Add/Delete and Add/Delete Node. A drag on a number writes once, when it ends. Panel and canvas follow each other: a one-kind canvas selection shows that kind (several items edit together), none shows Course, mixed leaves it; picking a tab, item or node selects it on the canvas; the panel's kind is what a right-click creates, and the kind shortcuts (now with C for Course) jump to the kind's first item as in Qt. The canvas toolbar is one non-wrapping line so a kind change cannot resize and re-fit the canvas. Picker: objects per decoded slot, previewed at their own size (capped 8x8) from the atlas, with the eraser and draws-nothing glyphs; actors by category with stills drawn by their own programs (asked for only when a category is open); module actors by module and component; one filter over all. Level Data tab: flags as checkboxes, values as hex, undeclared keys offered, read-only registration list, read-only on a glue error. Clipboard: `BigStar-Entities/1` + Base64 of LZ10 JSON with sorted keys, the same bytes as Qt's, through egui's Copy/Cut/Paste events; every field range-checked as Qt's reader. Undo and Redo have ⏷ menus of named steps that take several at once. Ported Qt cases: multiPropertiesEditIsOneUndoStep, spriteDefinitionsDriveFields, spriteSettingsSurviveTheFieldsThatReadThem, actorNotesStayOnTheFieldTheyBelongTo, aChoiceFieldOffersItsLabels, aCheckboxFieldTogglesOnlyTheBitsItOwns, aValueOutsideTheChoiceListIsShown, anUnrepresentableCheckboxReadsAsIndeterminate, aMixedSelectionLeavesEveryEditorUndecided (the numeric case; the panel shows no declared fields for several sprites), theSpritePickerRetypesTheSpriteByName, spriteFieldsStayAtTheirIndex, propertiesPreviewAResizeBeforeItIsCommitted, objectPreviewsShowTheWholeObjectPerTilesetSlot, objectsThatDrawNothingSayWhichNothingTheyAre, thePickerNamesTheObjectsThatDrawNothing, and the clipboard half of clipboardCloneAndZOrder; plus kittest runs of each panel. Fixed while checking on the clean ROM: `find_object_bank_overlays` (C++ and Rust) looked for zeros where a retail ARM9 holds 0xFFFFFFFF for slots without banks, so it never found the table; it now accepts either and returns the table as found (regression tests in `project` and an opt-in `BIGSTAR_NSMB_ROM` host test). Differences: copying several nodes of one path copies it once (Qt once per node); entity path node rows take the i16 range (Qt's 0-65535 showed negative nodes as a dash); a sprite's properties tab follows a picker choice. |
| 17 | Workspace and level tools | yes | Protocol 4: `LevelBytes`, `FindEntities`, `Recovery`, `Recover` requests and `LevelEdit::ReplaceSerialized`. The session holds every open level by host id with an active one; opens are tagged Document, Preview, Refresh or Reopen (a variant switch re-keys each view to its new host id, keeping camera and selection), and edits, graphics and draw lists route by level id. `workspace.rs`: an `egui_dock` tab well with the reusable Preview tab (the level picked in the list, read-only, fitted, with its counts and ids) and one tab per level; a tab context menu moves a document to a window of its own through `show_viewport_immediate` (native on the desktop, embedded in the page on the web) with "Move Back to Main Window"; the active document is the focused tab or detached window, and the panels, menus and shortcuts act on it. Open documents (and which were detached), the one in front, whether Preview is shown, and each document's zoom and pan are remembered per host and project through `Services` and restored after the level list arrives. The level list moved to a left panel beside the Palette (click previews, Edit or double-click opens). Level menu (`tools.rs`): hex editor (both files as hex, one "Hex edit level" step, half bytes refused), `.nml` export (with the ROM file ids) and import ("Import level") through new `Services::save_file`/`open_file` (rfd on the desktop; `MemoryServices` keeps a test shelf), object and sprite finders over every level the ROM holds including unsaved sessions (a pick opens the level and selects the entity), the sprite-event viewer. Closing a document, the project, the connection or the window with unsaved edits asks first. Recovery is host-side: a thread writes `.big-star-autosave/<hex of area>.bsrecover` (`encode_level_recovery`) for every dirty session each minute and removes the rest; on project open the frontend asks `Recovery` and offers what an earlier session left ("Recover autosave" steps, dirty documents); declining deletes them. Host tests cover bytes/replace, finders and the autosave cycle; kittest covers documents and focus, the close prompt, memory across runs, hex, nml round trip, finder, event viewer and the recovery offer; the demo host now has two levels. Not done: the Qt editor keeps level sessions after their tab closes; here closing a tab closes the host session (after the prompt). egui_dock's own floating windows are off; detaching is the viewport path only. |
| 18 | Filesystem dock | yes | Protocol 5: `Filesystem` (the view as rows: status, module/component, variant, id or planned id, member id, size, whether a module supplies it, an icon kind read from the head of the file), `ListArchive`, `ReadFile`, `WriteFile`, `FileTargets`, `RemoveFile`; `AreaSummary` names the course and BGDat files as the dock does (`a.narc!b` inside an archive). Host: the view is `build_filesystem_view` over the ROM, NCPatcher's plan and the last build's `files.json` (under `generated_path`), cached until files change or the variant switches; without NCPatcher the module graph falls back to `generated/modules.json`, as the Qt window does, and `modules/<key>` in the project wins over what was reported; writes go through `module_file_target` into the supplying module or the active one (a path the ROM lacks must be under `z_new/`); the icon classifier (`filesystem.rs`) ports `classify_entry` including the LZ-offset magics. Frontend (`files.rs`): a Filesystem tab beside Levels and Palette with the tree in NitroFS order (a folder by the lowest id it holds, members by their archive's ids, natural names otherwise), archives expanding like folders and listing their other members on first open, a Columns menu for Status/Provenance/Variant/File ID/Size, status colours, the footer line, Save-to-module choice, the pending banner, Add (destination prompt, `z_new/` rule, a warning naming the variants that already hold the path), Replace, Remove, Extract, Open, Hex edit. `editors.rs` ports `FileEditors::classify` (BMG by magic and parse; graphics by uiStudio/metadata/graphics identification, the full probes arrive with the editors) and opens files as hex documents in the tab well, saved back to the module. The level list now nests world paths ("Mario vs. Luigi > Multicard") and shows one-area levels as one row, with course/BGDat paths on hover and "Show … in Filesystem". Checked on the clean ROM through a real host. Differences: Remove moves the module file into the project's `.big-star-trash/<time>/` rather than the OS trash (no trash crate); the Run NCPatcher button belongs to the build dock (Phase 21); column widths and order are not persisted; until Phases 19-20 a graphics or message file opens as hex. Without an NCPatcher plan, reads come from the ROM even after a write, as in the Qt editor. |
| 19 | Graphics editor | yes | `graphics_editor.rs`: a pure `GraphicsDoc` (open by identification and parse, `render` to an image plus info line and swatches, `paint`, `set_color`, companion palette/graphics, `output` re-wrapped in the file's own LZ10/LZ11) and its egui view as a file document in the tab well. NCLR and raw palettes (editable swatches with egui's colour picker, rounded to BGR555), NCGR (CPOS width, NCBR bitmap arrangement not de-tiled) and headerless sheets (4bpp/8bpp choice, explicit width, square-root guess otherwise) with click-and-drag painting of the chosen index, NSCR and raw screen maps composed with paired graphics and palette (flips, palette rows, index 0 see-through; tile-id colours until paired), NCER cells through `render_ncer_cell` or OAM outlines, NANR sequence summaries, TEX0 textures and palettes from NSBTX/NSBMD; grey ramp until a palette is paired; zoom to fit 512 in whole steps up to 16, nearest sampling, pixel grid when cells are 8 points or more. The palette and graphics companions are offered from the file's folder (inside an archive, its folder there), read through `ReadFile`, and the pairing and width are remembered per file through `Services`. `editors::classify` now uses the editor's own probe, as `GraphicsEditorPanel::supports` does. Ported Qt cases: graphicsEditorPaintsRawCharacterPixels, aPairedPaletteColoursTheGraphicsInsteadOfTheGreyRamp, aBitmapFormattedNcgrIsNotDeTiled, aHeaderlessSheetHonoursTheWidthItIsGiven, plus screen composition, BGR555 palette editing and LZ round trip, and a kittest run from the dock through pairing, painting and saving. Checked on the clean ROM: `BG_ncg/d_2d_A_J_jyotyu_ncg.bin` with its palette decodes to the World 1 blocks at 8bpp. Not here: the uiStudio pages and ROM metadata of `oneGraphicsEditorOpensBothFormatFamilies` are Phase 20 (until then they open as hex); colour edits use egui's picker button on the selected swatch rather than a double-click dialog. |
| 20 | Message and resource editors | yes | Protocol 6 (`Banner`, `WriteBanner`). `message_editor.rs`: a pure `MessageDoc` (list rows with plain-text previews, case-insensitive filter by text, ID or index, the selected message's `[type:HEXDATA]` text, MID1 ID and INF1 attribute hex committed back on change, stats, add/duplicate with the next free ID, remove after a confirmation, escape insertion at the cursor after validation, the NDS BMG Editor text export and an import that updates in place and appends past the end) and its view as a file document; ID and attribute fields appear only when the file carries them. `resource_editor.rs`: the `UiResourcePages` as pages of the graphics editor: JNBL/JNCL tables kept as typed text (Qt's base-0 numbers, the packed-field limits, reserved bits kept from the old row) beside a 256×192 preview, add and delete rows; JNCD cells, OAM and its texture at 4bpp or 8bpp on a grey ramp; ENPG 256×256 with its own palette; the banner's icon, palette and per-language titles (UTF-16, 128 units); NSMBeUD sections and entries with add section/entry and delete. Painting and swatches share `graphics_editor::{picture, swatches}`. LZ wrappers come off in the workspace and go back on save (`editors::wrap`). Tools menu (renamed from Level, as in Qt) gains ROM Banner…, which the host serves from the variant's or project's configured banner once it exists and the ROM's otherwise, and writes to that file (refused with Qt's messages when NCPatcher reports no config or no banner), and ROM User Metadata…, which opens NitroFS file 131 from the listing and saves into its module. Ported Qt cases: uiResourceEditorsPreserveUntouchedFiles, messageEditorRoundTripsAndEditsBmgMessages, messageEditorHidesFieldsTheFileDoesNotCarry, openingAFileChoosesOneEditorThroughAnyLzWrapper, the uiStudio/metadata half of oneGraphicsEditorOpensBothFormatFamilies; plus host banner tests (a shell-script NCPatcher) and kittest runs for messages, a touch layout, file 131 and the banner. Checked on the clean ROM: `script/course.bmg`, `uiStudio/UI_O_menu_select_d.bnbl`/`.bncl`, the JNCD texture of `UI_O_1P_course_select_d.nce.bncd` and the LZ-wrapped `demo/staffroll_cover.enpg`. Not here: the modal dialog frames (everything is a tab document); colour edits use the swatch picker button rather than a double-click dialog. |
| 21 | Tileset workspace | yes | Protocol 7 (`TilesetResources`, `WriteFiles`). The host serves a level's three tilesets and both backgrounds as the files `resolve_level_graphics` finds (ROM path and bytes each) with its references and the blocks its tileset's tile scripts claim; `WriteFiles` writes several files into their modules with one invalidation. The overlay substitution moved to `bigstar_formats::{TileOverlay, overlay_map16_sheet, overlay_map16_images}` (taking a lookup), so the frontend draws 3D blocks the same way; `bigstar_project` keeps the catalog wrappers. `tileset_editor.rs`: a pure `TilesetDoc` and its tab (Tools → Tilesets and Backgrounds…, one per level, following re-keyed levels): slot choice, Slots (the five references, saved as a `SetTilesetReferences` edit), Graphics (32 tiles wide), Map16 (16 wide, composed against all three tilesets and with 3D tiles by default, click to pick, quadrant tile/palette/flips), Objects (cells as typed, add/delete object keeping one, add row/tile, remove tile, control), Behaviours (flags, subtype, parameter and its meaning, raw hex), Background and Foreground (click to pick, entry fields), quantised PNG import/export (the `image` crate, now a normal dependency of ui-egui) and `.bstileset` import/export; Save writes only resources whose bytes changed (refusing one with no ROM path) and logs the count, as `openTilesetEditor` did. Ported Qt case: tilesetEditorExposesAllM10SurfacesAndPreservesResources; plus object, behaviour-table, background, bundle, no-target and PNG tests, a host `WriteFiles` test, an opt-in real-ROM test (A01_1 resolves jyotyu, yakata, nohara), and a kittest from the Tools menu through Save. Checked on the clean ROM: A01_1's tilesets, Map16, objects and behaviours. Not here: real backgrounds draw clear, as in Qt: their screens name tiles from 576 and palette rows such as 11, and both `parse_tilemap` and `load_background_graphics` read screens with no offsets; that belongs with the level canvas's backgrounds, not this port. The workspace is a tab with Save/Cancel rather than a modal dialog. |
| 22 | Builds | yes | Protocol 8 (`BuildConfig`, `MigrateConfig`, `Build`, `CancelBuild`, `ExtractRom`, `ExportPatch`, `ImportPatch`, `Xdelta`; `Event::Build` with `Started`/`Progress`/`Diagnostic`/`Log`/`Artifact`/`Finished`). Host (`host/build.rs`): a build saves the edited levels, refuses a project without `ncpatcher.yaml` or still on `ncpatcher.json`, and runs `apply_patch_transactionally` on its own thread with the observer streaming events and a `ProcessCancel` for Cancel; an applied build is read in place (the manifest re-indexed against `big-star-build.nds`, the index and ROM swapped, level sessions closed) and announced with `GraphicsChanged` and `SettingsChanged`, so frontends reopen their levels as after a variant switch. Extract ROM runs NCPatcher's extractor into a workspace folder; `.nsmbpatch` export takes every non-vanilla module file with its file ID, import writes all or none and answers `PatchIdsDiffer` until accepted; xdelta runs `xdelta3 -e/-d -f -s` (`--xdelta`, like `--ncpatcher`) on workspace paths. Frontend (`builds.rs`): the Output panel (status, progress, Cancel, the diagnostics with their headline and a location that opens a message, and the log the build lines join), Build and Distribution menus, a ▶ Run Patcher button and the window-wide variant selector on the top bar, the migration offer, the ID-mismatch question, and Extract ROM/xdelta windows asking for workspace paths. The menu Qt called Tools now is (Phase 20). The session now asks for the project settings on open. Tests: host config detection, a full build through a shell-script NCPatcher (progress, diagnostic location, log, applied ROM, saved edit, levels reopened), patch export/import with the ID check (a scripted plan), xdelta arguments and failure; kittests for the Output panel, the variant selector and patch exchange. Checked on the clean ROM project: Run Patcher says it has no `ncpatcher.yaml`. Not here: choosing the NCPatcher or xdelta3 executable with a file dialog (the host takes them as options), since the tools run where the host does; Qt's modal "complete" box (a notice says it). |
| 23 | Finish | yes | Protocol 9 (`SetPreferences`; `ProjectSettings.autosave_minutes`/`backups_kept`). `ui-egui/src/i18n`: `Language` (en, nl, pt, es, fr) and one file per language holding `uiText`'s seventeen strings (the wordcraft layout named in the plan is not on this machine, so it follows its shape from the description: a module per language, English fallback); applied to the menus, the Filesystem tab, the hex editor and the tileset tab, and live rather than at the next start. Preferences (View → Preferences…): the language is kept in `Services`, the snapshot interval and backups kept are the project's, kept by the host in `.big-star-workspace.json`; the autosave thread now ticks and honours the interval, level saves and patch imports back up what they overwrite into `.big-star-backups/<kind>-<UTC stamp>-<pid>` and sweep to the preference, and opening a project sweeps too. Opening the project that is already open now joins it, as `OpenLevel` does, instead of reloading and announcing `ProjectClosed` to everyone; with that an edit, undo or save in one frontend appears in another (desktop test with two `Session`s on one host). `bigstar-host` embeds `dist/web` at build time (`build.rs`, or `BIGSTAR_WEB_DIST`), serves it at `/` with its files, prints `Open http://…/?token=…`, and the web app takes `?token=` and the page's host; checked in a browser against the clean-ROM workspace: connect, open the project and 1-1. README and `docs/architecture.md` rewritten for the Rust tree. Phase 10's deferred item, which phase 22 did not pick up: level saves now go to the module target (the supplier, else the active module; refused without one, with Qt's message), archive members beside their archive, and levels and the finder read through the effective filesystem, so a build includes them and a saved level reads back through NCPatcher's plan; the `edits/` directory is gone. Projects without modules (the clean-ROM workspace) cannot save levels, as in Qt. Checklist walk (`docs/m*-manual-checklist.md`), gaps: the side panels are fixed (no closing docks, no View → Reset Layout; M4 4); no View → Refresh Graphics (M4 12); Filesystem columns come from a Columns button, not a header right-click, have no draggable widths and are not remembered across runs (M4 21); the pending-build note has no Run NCPatcher button of its own (the toolbar has one; M4 25); no Project Hub, so its vocabulary (Project Hub, Level Listing, Open Editor, File, Save Project, Cut/Copy/Paste) has nothing to translate (M4 27, 38; M12); no reference-image overlay (M4 29); Alt pans rather than bypassing snap (M4 7, as M4 30 asks); no app-configuration `actors` layer between the modules and the shipped data (M11); uiStudio and metadata pages are titled "UI Resource" tabs (M15 9); real backgrounds draw clear, as in Qt (phase 21); the C++ browser editor of a local course/BGDat pair has no counterpart, by the decision that the browser is a view of a host (M4 43–44, M14). Items that need the game running (booting a build, banner, UI resources, xdelta output) and NSMB Editor interoperability remain for a person with the ROM. |
| 24 | Remove the C++ tree | yes | Deleted `libs/`, `apps/nsmb-qt`, `apps/nsmb-web`, `apps/nsmb-cli` and `CMakeLists.txt`, and the `oracle` xtask; the signboard-module fixture the Lua actor tests load moved to `crates/project/tests/fixtures`. `bigstar-host oracle` stays, as a report two builds can be diffed with. `.gitignore` keeps only what the Rust tree and leftover local `build/` directories need. README gained a History section; `docs/actor-cleanup-brief.md` and `docs/ncpatcher-integration.md` name the Rust tests and the host; the M14 checklist and items 43–44 of M4 are marked retired; `docs/roadmap.md` says it is the C++ history. Source comments that name the C++ file a module was ported from are kept as provenance; the files are in the git history. |
