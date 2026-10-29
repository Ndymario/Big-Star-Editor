# Architecture

## Decision: the host holds the projects, the frontend is a view

Big Star is split where the ROM is. A **host** runs on the machine with the projects: it reads ROMs, runs Lua and
NCPatcher, watches files and writes edits. A **frontend** shows what a host holds and sends it requests. The browser
never opens a ROM; the desktop app runs a host in-process and can also connect to a remote one. Both frontends are
the same `ui-egui` crate, and both speak one protocol.

A host is scoped to a **workspace root**: it lists, creates, imports and opens projects under it, and refuses any
path that leaves it, symlinks included. Access is one user with a token; the host binds `127.0.0.1` by default.
Several frontends may connect at once and see each other's edits.

## Crates

Wasm-safe (they build for `wasm32-unknown-unknown`, and `cargo xtask ci` checks it):

- `core`: `Error`/`Result`, little-endian reading and writing, atomic file writes, command history.
- `nds`: the DS ROM header, FNT/FAT, overlay tables, banner sizing.
- `formats`: NSMB levels and glue, Nitro 2D (NCLR/NCGR/NSCR/NCER/NANR), TEX0, BMG, NARC, tilesets and tilemaps,
  uiStudio resources, banner, ROM user metadata, LZ10/LZ11/BLZ, quantisation, the 3D tile substitution.
- `level`: `EditableLevel` and its commands, tileset references, module data catalogs, `.nml` and recovery
  snapshots. Pure: no filesystem.
- `protocol`: the requests, responses and events, MessagePack on the wire.
- `client`: a connection to a host: `InProcess` (a host session on a worker thread) or `WebSocket`.
- `ui-egui`: the frontend. It must not depend on `project`, `patching` or `host`, and nothing below it may depend on
  egui, eframe or rfd.

Host-only:

- `patching`: the no-shell process runner and the NCPatcher 2 client (config, modules, file plans, builds, migration,
  extraction).
- `project`: the project manifest, `RomReader` (a `.nds` read in place), `EffectiveFilesystem` (a read answered by
  the module a build would use, else the ROM), the filesystem view, graphics resolution, Lua actors and tiles, the
  transactional patch apply.
- `host`: the `Service` behind every connection: level sessions keyed by id (opening a level or a project that is
  already open joins it), graphics as content-hashed textures, the filesystem dock's rows, file writes into module
  targets, builds, recovery and backups.

Apps: `bigstar` (desktop: eframe, rfd, an embedded host), `bigstar-web` (the trunk bundle), `bigstar-host` (the
command line and `serve`, which hands out the bundle and `/ws`), and `xtask` (the gates).

## The protocol

A frontend sends `Request`s with ids and gets `Response`s; the host also pushes `Event`s to every connection: level
changes, files changed on disk, graphics generations, settings, log lines and build progress. `PROTOCOL_VERSION` is
checked in the `Hello`, and a frontend that disagrees is refused.

Edits carry the revision they were made against, and a stale one is refused, so two frontends cannot overwrite each
other's work; the loser refetches. `LevelChanged` tells the other frontends to refetch the level. Graphics carry a
generation: when a watched file or the variant changes, the generation grows, everything served before is stale,
and frontends ask again while keeping what is on screen until the replacement arrives.

## Saving

Level saves go to the project's edit targets, are parsed again before they are written, back up what they overwrite
into `.bigstar/backups/levels-…`, and write both files or neither. File and tileset edits go into the module that
supplies the file, or the active one; new NitroFS files must be under `z_new/`; a member of an archive is written
beside it for the next build to sweep back in. An NCPatcher project never writes into the ROM. Backups are kept per kind,
as many as the project's preference says, and swept when a project opens and whenever one is added.

Unsaved levels are written to `.bigstar/autosave` as often as the project's preference says (every minute by
default). The next session offers them back.

## Editing a ROM directly

A `.nds` can also be edited without a project, and without a workspace: a workspace is where NCPatcher projects live, and
a ROM edited directly is not one. It comes from one of two places.

- **On the desktop**, **Open ROM…** asks the system's file dialog for a `.nds` and starts `Host::for_local_roms`, a
  host in the app's process with no workspace that opens and saves ROMs by their full path (`OpenRom`, `SaveRomAs`).
  Only that host takes a path for a ROM; a workspace host refuses one, inside its root or out. The ROM is the
  whole connection, so closing it closes the connection, as **Close Workspace** does for a workspace.
- **In the browser**, a connection uploads the ROM in pieces of at most `ROM_CHUNK_BYTES` (`UploadRom`) and fetches it
  back with its edits the same way (`DownloadRom`). Each uploaded ROM gets a host of its own -- its own open ROM,
  levels, graphics and events -- that belongs to that connection, has no workspace, and writes nothing to disk; it
  ends with the connection. `HostOptions` bounds the size of a ROM and how many the host holds at once.
  `serve --rom-sessions` lets a frontend without the token in as a guest, which can upload a ROM and reach nothing of
  the workspace, its events included; `serve --rom-sessions` without a root serves uploads and nothing else. `Welcome`
  tells the frontend whether its connection has a workspace.

Either way the host holds the ROM in memory, and every save -- a level, a file, a tileset, the banner -- is kept as an
edit by address over it, then the whole ROM is written: `write_edited_rom`
packs member edits back into their archive (in the LZ wrapper it came in) and `NdsRomWriter` lays the files into the
original the way NCPatcher's in-place commit does -- back into their own extent when they fit, appended after the used
ROM when they do not. Writing from the original every time keeps a ROM saved a hundred times the size of one saved
once. A ROM on this machine is written into its file on every save; the first write of a session backs up the file it
overwrites into `.bigstar/backups/rom-…` beside it, and autosaves go to `.bigstar/autosave/<rom name>/`. An uploaded
ROM is written only for a download, and has no backups or autosaves.

This is not a return of the ROM builder M16 removed. An NCPatcher project still never repacks anything: NCPatcher builds
its ROM from the modules. A ROM edited directly has no modules to build from, so writing into it is the only way an
edit can land. Only existing files can be replaced; adding or removing one would renumber file IDs. Variants, modules,
builds and glue editing need an NCPatcher project, and the host refuses them by name.

## Builds

Building is NCPatcher's job alone. A build runs on its own host thread: it saves the edited levels, builds the active
variant to a staging ROM, validates what came out, and only then makes it the project's ROM. Progress, diagnostics
and log lines stream to every frontend as `Event::Build`. When a build applies, the host closes its level sessions,
reads the new ROM and announces it, and frontends reopen their levels as they do after a variant switch.

## Frontend

`BigStarApp` owns a `Session` (the connection, its pending requests, and everything the host has said) and the
widgets: the level canvas, the property and level-data panels, the picker, the Filesystem dock, the document
workspace (egui_dock), the file editors, the tileset workspace, the Output panel. Platform services (stored values,
file dialogs, the embedded host) come in through the `Services` trait, so the same crate runs on the desktop, in a
browser and under test. Everything the person does is either a local view change or a request; nothing is edited in
the frontend that the host does not confirm.

How it looks is decided in one place. `theme.rs` holds the palette as tokens, light and dark, and turns them into the
egui style; `controls.rs` and `dialogs.rs` build every button, field, drop-down, tab row, list row, status line and
dialog from those tokens. Screens ask for a control there instead of making egui's own, and a test in `controls.rs`
fails when one does not. The theme (one of six families, its dark or
light mode, and whether the system's accent colour replaces the family's) is chosen in Preferences and kept under the
`theme` key. The shell supplies the system's accent through `Services::accent_color`: the desktop portal on Linux,
the registry on Windows, `defaults` on macOS, and CSS `AccentColor` in the browser.

## Testing

Ported C++ cases are Rust tests beside the code they test. UI behaviour is tested with `egui_kittest` against
`DemoConnection`, an in-memory host with a level, art and files of its own; `examples/ui_shot.rs` renders the
interface headlessly, against the demo or a running host. Tests that need a real NSMB ROM run when
`BIGSTAR_NSMB_ROM` names one. `cargo xtask ci` runs fmt, clippy with warnings denied, the tests, the layering check
and the wasm build.

## Network filesystem decision

Big Star does not define a separate remote-filesystem protocol. An active module's NitroFS tree is an ordinary
project directory and can live on an OS-mounted network volume or in a synchronised folder; the host's watcher,
atomic writes, provenance and pending-build behaviour stay the same. The host protocol is for editing, not for
moving files around.

## Project files

A project is an NCPatcher project directory, found by its `ncpatcher.yaml`; Big Star writes no manifest of its own.
The host keeps its per-project choices (variant, module, the clean ROM, autosave interval, backups kept) in
`.bigstar/workspace.json` in the project directory. The clean ROM is the one path that differs between machines, so
it lives there rather than in `ncpatcher.yaml`, and reaches NCPatcher as `--rom`. See `docs/project-format.md`.

Everything the editor writes for itself -- settings, backups, autosaves, build staging, trash, the virtual ROM's record
and what it lost -- lives under the one `.bigstar/` folder, in the project or beside a ROM edited directly. The loose
`.big-star-*` files and folders earlier versions wrote are moved into it when the project or ROM opens.
