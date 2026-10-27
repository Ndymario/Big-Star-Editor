# Building and running from source

Rust 1.90 or newer (the CI gate uses 1.96.1), and for the browser build the `wasm32-unknown-unknown` target and
[trunk](https://trunkrs.dev).

```sh
cargo run --release -p bigstar                      # desktop app
cargo run --release -p bigstar -- path/to/workspace # with that workspace folder filled in
cargo xtask ci                                      # fmt, clippy -D warnings, tests, layering, wasm build
```

Packages of the desktop app (an AppImage, a Windows installer, a macOS disk image and an AUR recipe) are
described in [packaging](../packaging/README.md).

## How it fits together

- **The host** (`bigstar-host`, or built into the desktop app) runs on the machine with the projects. It reads the
  `.nds` in place, keeps the level sessions and their undo history, renders tilesets and Lua actors, runs NCPatcher,
  and writes every edit into a module's NitroFS tree. Nothing outside its workspace folder is reachable.
- **The frontend** (`crates/ui-egui`) is a view of a host. The desktop app (`apps/bigstar`) runs a host in-process
  and can also connect to a remote one; the browser build (`apps/bigstar-web`) always connects to a host, usually
  the one that served the page. Both speak the same MessagePack protocol (`crates/protocol`).

See [Architecture](architecture.md) for the crates and their boundaries.

## Serving to a browser

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

It binds `127.0.0.1` unless told otherwise (`--bind`, `--port`); `--ncpatcher` names the NCPatcher it
runs. A host built without the bundle still serves desktop frontends, and its page says how to add one.

To let people edit ROMs they upload, without a workspace or the token, serve uploads only (or add `--rom-sessions`
to a workspace host, so the token still opens the workspace):

```sh
cargo run --release -p bigstar-host-app -- serve --rom-sessions --data data
```

Connecting without a token offers **Upload ROM…**. The ROM stays in the host's memory for that connection alone, and
**Download ROM** saves it with its edits. A ROM may be up to 64 MiB, and the host holds eight at once.

## Command line

```sh
cargo run -p bigstar-host-app -- inspect-rom path/to/user_provided.nds
cargo run -p bigstar-host-app -- run-patcher my-project --variant en
cargo run -p bigstar-host-app -- patcher-info my-project --rom path/to/clean.nds
```

`run-patcher` and `patcher-info` hand NCPatcher the clean ROM the editor remembers for the project, or the one
`--rom` names. Nothing is unpacked: the ROM is read in place, through its own FAT and name table. Building a ROM is
NCPatcher's job; see [NCPatcher integration](ncpatcher-integration.md) and [Projects](project-format.md).

## Tests

`cargo xtask ci` is the gate. UI behaviour is tested with `egui_kittest`; `cargo run -p bigstar-ui-egui --example
ui_shot -- <dir>` renders the interface headlessly against a built-in demo host, or against a running host given its
address and token. Tests that need a real NSMB ROM run when `BIGSTAR_NSMB_ROM` names one.

## History

Big Star began as a C++20 backend with a Qt 6 editor and an Emscripten level editor. This tree is a port of it
([the port plan](rust-port-plan.md)); the C++ implementation was retired once the port was complete, and lives
on in the git history. The `m*-manual-checklist.md` files and `roadmap.md` date from it.
