# Big Star Editor

Big Star Editor is a New Super Mario Bros. DS editor, written in Rust: a host that holds your projects and their ROMs,
and one egui frontend that runs as a desktop app or in a browser. It does **not** include ROM data, Nintendo SDK
files, Nitro SDK contents or game assets.

## How it fits together

- **The host** (`bigstar-host`, or built into the desktop app) runs on the machine with the projects. It reads the
  `.nds` in place, keeps the level sessions and their undo history, renders tilesets and Lua actors, runs NCPatcher,
  and writes every edit into a module's NitroFS tree. Nothing outside its workspace folder is reachable.
- **The frontend** (`crates/ui-egui`) is a view of a host. The desktop app (`apps/bigstar`) runs a host in-process
  and can also connect to a remote one; the browser build (`apps/bigstar-web`) always connects to a host, usually
  the one that served the page. Both speak the same MessagePack protocol (`crates/protocol`).

See [Architecture](docs/architecture.md) for the crates and their boundaries.

## What it edits

- **Levels**: objects, sprites, entrances, zones, views, paths and progress paths on a canvas with real tilesets and
  actor graphics, with selection, marquee, resize, Ctrl-drag clone, snapping, copy/paste compatible with the
  earlier Qt editor, named undo history, the property panels, the actor picker and the level-data tables. Question
  blocks and bricks show what they hold with NSMB Editor's icons, on the canvas and in the palette, and a sprite whose object bank the level does not load is tinted red, with
  a button in its properties that loads the bank. Several levels open
  as documents, in tabs or windows of their own; a workspace is remembered per host and project.
- **Files**: the Filesystem dock lists the ROM as the active variant's modules change it, opens NARCs like folders,
  and adds, replaces, extracts and removes module files. Files open in the editor that understands them, through
  any LZ10/LZ11 wrapper: the graphics editor (NCLR/NCGR/NSCR/NCER/NANR/TEX0 and headerless sheets, paired with
  their palettes), the message editor (BMG text, escapes, attributes, IDs, text export/import), the uiStudio layout
  pages (BNBL/BNCL/BNCD/ENPG), the ROM banner and the ROM user metadata (NSMBeUD), or hex.
- **Tilesets and backgrounds**: graphics, Map16, objects, behaviours, background and foreground maps and the level's
  slot IDs, with quantised PNG and `.bstileset` import/export. Map16 entries that name another tileset's tiles are
  drawn the way the game composes them ([Tileset memory](docs/tileset-memory.md)), and blocks the game hands to its
  3D engine are drawn from the ROM's own textures by Lua scripts ([Lua tiles](docs/lua-tiles.md)).
- **Backgrounds** are placed per view as the game places them, named and animated -- drifting clouds, lava, colour
  cycles -- by one Lua script each ([Lua backgrounds](docs/lua-backgrounds.md)).
- **Actors** are one `.lua` file each, declaring their fields and drawing themselves ([Lua actors](docs/lua-actors.md)).
- **ROMs directly**: **Open ROM…** on the desktop edits a `.nds` picked with the system's file dialog, as NSMB Editor
  does: no workspace, no project, and every save writes into the ROM, laid out the way NCPatcher lays out a replaced
  file. In the browser, a host started with `serve --rom-sessions` takes an uploaded ROM, keeps it in memory for that
  connection alone, and gives it back as a download. Builds, variants and patches need a module project.
- **Builds** run NCPatcher on the host for the chosen variant, stream progress and diagnostics into the Output panel,
  and read the ROM a successful build produces. `.nsmbpatch` export/import and xdelta create/apply are in the
  Distribution menu.
- Recovery snapshots of unsaved levels, backups of what a save overwrites, and five interface languages (English,
  Dutch, Portuguese, Spanish, French), in Preferences.

## Building

Rust 1.90 or newer (the CI gate uses 1.96.1), and for the browser build the `wasm32-unknown-unknown` target and
[trunk](https://trunkrs.dev).

```sh
cargo run --release -p bigstar                      # desktop app
cargo run --release -p bigstar -- path/to/workspace # with that workspace folder filled in
cargo xtask ci                                      # fmt, clippy -D warnings, tests, layering, wasm build
```

### Serving to a browser

```sh
cd apps/bigstar-web && trunk build --release && cd ../..
cargo run --release -p bigstar-host-app -- serve path/to/workspace --data data
```

The host embeds the bundle `trunk` wrote to `dist/web` when it is built, and prints a link with its token:

```
Serving "workspace" on ws://127.0.0.1:7878/ws
Token: …
Open http://127.0.0.1:7878/?token=… in a browser
```

It binds `127.0.0.1` unless told otherwise (`--bind`, `--port`); `--ncpatcher` and `--xdelta` name the tools it
runs. A host built without the bundle still serves desktop frontends, and its page says how to add one.

To let people edit ROMs they upload, without a workspace or the token, serve uploads only (or add `--rom-sessions`
to a workspace host, so the token still opens the workspace):

```sh
cargo run --release -p bigstar-host-app -- serve --rom-sessions --data data
```

Connecting without a token offers **Upload ROM…**. The ROM stays in the host's memory for that connection alone, and
**Download ROM** saves it with its edits. A ROM may be up to 64 MiB, and the host holds eight at once.

### Command line

```sh
cargo run -p bigstar-host-app -- inspect-rom path/to/user_provided.nds
cargo run -p bigstar-host-app -- import-rom path/to/user_provided.nds my-project
cargo run -p bigstar-host-app -- run-patcher my-project --variant en
```

`import-rom` copies the ROM into the project and writes `big-star-project.yaml` with project-relative paths. Nothing
is unpacked: the ROM is read in place, through its own FAT and name table. Building a ROM is NCPatcher's job; see
[NCPatcher integration](docs/ncpatcher-integration.md).

## Tests

`cargo xtask ci` is the gate. UI behaviour is tested with `egui_kittest`; `cargo run -p bigstar-ui-egui --example
ui_shot -- <dir>` renders the interface headlessly against a built-in demo host, or against a running host given its
address and token. Tests that need a real NSMB ROM run when `BIGSTAR_NSMB_ROM` names one.

## History

Big Star began as a C++20 backend with a Qt 6 editor and an Emscripten level editor. This tree is a port of it
([the port plan](docs/rust-port-plan.md)); the C++ implementation was retired once the port was complete, and lives
on in the git history. The `docs/m*-manual-checklist.md` files and `docs/roadmap.md` date from it.
