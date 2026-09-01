# NCPatcher 2.x integration

Big Star drives NCPatcher as an external process through its versioned CLI and
JSON documents. It does not embed NCPatcher, parse `ncpatcher.yaml`, invoke a
shell, or change the editor process's working directory.

## Project contract

The Big Star project directory is also the NCPatcher project directory. It
contains `ncpatcher.yaml` (or `.yml`), the pristine input ROM, and `modules/`.
The editor renders `extracted-rom/`, which is a derived view of the most recent
successful build and is never an input to NCPatcher.

Legacy `ncpatcher.json` projects are not built directly. The Qt frontend offers
`ncpatcher migrate --write`; migration is explicit and leaves the old JSON file
in place.

All commands name the project with `-C <project>`. Arguments are passed directly
as an argv vector, so spaces and quotes in paths do not need shell escaping.
Standard output and standard error are captured separately and can be streamed
line by line.

## Commands and documents used

`NCPatcherClient` uses these NCPatcher 2.x commands:

- `version`, `config path`, and `config validate` for preflight checks.
- `config dump --json` for project paths, toolchain, build variants, the
  project and per-variant banner inputs, and each variant's `module-variants`
  layer selection.
- `modules dump` for enabled modules, components, and NitroFS layering.
- `rom files --json` for the file table and build provenance.
- `files plan [--variant NAME] --json` for what a build would place, without
  building.
- `build [--variant NAME] --message-format json --result <path>` for builds.
- `migrate [--write]` for legacy projects.

The client accepts `ncpatcher.config/1`, `ncpatcher.modules/1`,
`ncpatcher.files/1`, and `ncpatcher.result/1`. Every reader refuses an unknown
`schema` value instead of partially interpreting a newer contract. JSON object
members preserve declaration order because variant order is user-visible.

Structured build events expose progress, artifacts, diagnostics (including
source locations), and the final result. Exit codes 0 through 11 are mapped to
named stages so configuration, module resolution, toolchain, compile, link,
patch, ROM I/O, hook failures and cancellation remain distinguishable.

## Transactional apply

The Qt editor applies a successful build as one transaction:

1. Build a ROM into the project's staging area, optionally for one selected
   variant.
2. Validate the staged ROM and extract it into a staged directory.
3. Move the current `extracted-rom/` into the backup area.
4. Move the staged extraction into place.
5. Clear open documents and cached level sessions, then reindex the project.

A failed build or rejected ROM leaves the live extracted tree untouched. The
successful staged ROM is retained for launching in an emulator, and the Build
Output dock logs both it and the backup path.

The variant list is loaded only when its selector is opened. A remembered
variant that is absent from a later query is kept visibly rather than silently
changed. An empty configured list is distinct from a failed query.

## Deliberate limits

- Only one variant can be applied at a time because the editor has one live
  extracted tree.
- Big Star does not provide a graphical YAML editor.

## Cancelling a build

The Build Output dock's **Cancel** sends NCPatcher's documented cancellation
signal -- `SIGINT` on POSIX, `CTRL_BREAK_EVENT` on Windows -- to the build's
process group, so the compilers it spawned stop too rather than being left
running under a stopped parent. The child is started in its own group for
exactly that reason, and the editor's own process is never in the blast radius.

It is a request, not a kill. NCPatcher unwinds, writes its result document, and
exits 11, which the client maps to a `Cancelled` stage and the panel reports as
*Cancelled* rather than as a failure. The apply that would have followed does
not run, so the project is left as it was, and the next build starts from the
same place this one did -- NCPatcher re-patches the pristine binaries it kept,
never its own half-written output.

## Banner

The config reader accepts optional `rom-banner` and per-variant `banner` paths.
NCPatcher 2.0 does not yet include those values in `config dump --json`, so the
editor asks the user to choose the source named by `rom.banner` when they are
absent. Banner data is not a NitroFS file and is therefore never routed into a
module tree or guessed by parsing YAML.

The Fireflower and NSMBe-compatible wrapper build actions were removed in M15.
NCPatcher is the only build path.
