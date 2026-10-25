# NCPatcher 2.x integration

Big Star drives NCPatcher as an external process through its versioned CLI and
JSON documents. It does not embed NCPatcher, parse `ncpatcher.yaml`, resolve module trees
itself, invoke a shell, or change the editor process's working directory.

## Project contract

A Big Star project is an NCPatcher project directory: one with an
`ncpatcher.yaml` (or `.yml`), and `modules/`. Big Star writes no project file of
its own. `ncpatcher.yaml` need name no input ROM -- where the clean ROM lies
differs from machine to machine -- so the editor keeps it in the project's
`.big-star-workspace.json` and passes it as `--rom` on every command that reads
the configuration. NCPatcher accepts a `rom:` section without `file` or `dir`
when `--rom` is given. A project with no clean ROM in its settings whose
configuration names one (`rom.file`, as `config dump` reports it) takes that.

The editor reads the active variant's build where it lies: the `rom.output`
that `config dump --json` reports, with `_<variant>` before the extension as
NCPatcher names a variant's output, and before a first build the clean ROM
itself. Nothing is unpacked. An extracted
directory is an explicit export produced by **Build > Extract ROM…**, which
delegates to `ncpatcher rom extract` so there is one extractor and one layout,
and it is never an input the editor reads back.

Legacy `ncpatcher.json` projects are not built directly. The editor offers
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
- `rom extract` for **File > Extract ROM...**, so the tree Big Star exports is
  one NCPatcher reads.
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

## Resolution happens once, in NCPatcher

The editor does not work out which module supplies which ROM destination. It
asks. `files plan` answers with the same `ncpatcher.files/1` document a build
writes -- every id, path, size, action and provenance -- with `planned: true`
and prospective ids for the files a build would append. The Filesystem dock is
then a join of three things it does not derive: that plan, the last build's
`files-dump` manifest, and the bytes each source currently holds. `Pending`
means those disagree.

This replaces roughly 120 lines that re-implemented layering, module order,
`into:` prefixes, the archive-folder convention, component glob subtraction and
brace expansion. Two implementations of precedence rules cannot be kept in step
by testing either one alone, and the editor's copy was the one that could
silently disagree with the ROM the user shipped.

Three consequences worth stating:

- A plan reads the same sources a build does, so a destination whose source a
  `pre-build` hook has not generated yet is still planned -- id, path and
  provenance -- and its entry carries `source-missing`. Its size is the one
  thing genuinely unknown, so the dock reports the last build for it rather
  than a Pending mark nothing can clear.
- An archive the build edits carries a `members` array: id, path, and the
  module, component and variant behind each member. A member's `id` is a file id
  in the container's own name table -- a `.narc` carries the ROM's File Name
  Table structure and numbers its members the same way, which is how game code
  loads one, and how ndspy and NSMB Editor both read a NARC. It is read together
  with the container's path and means nothing without it, exactly as the
  enclosing entry's `id` means nothing inside the archive. A container entry can
  carry no more provenance than its members agree on, which for members from
  three modules is none -- so the members are where the answer is. The
  Filesystem dock emits a row per member and judges each against the container's
  copy at that id, which is how an archive with one stale member says *which*.
  Members a build would not write are not in the document at all; the dock lists
  those, with the ids the container gives them, when somebody opens it.
- Without NCPatcher there is no plan, and the dock reports the last build
  rather than guessing past it.

`module_file_target` stays in the editor because it answers the write-side
question no build document does: where in a module's tree a new edit goes. It
reads `into:` from the module dump, and spells an archive address `a/b.narc!c/d`
as the folder `a/b_narc/c/d` -- for whatever extension a game gives its
archives, `.carc` included.

## Applying a build

The host applies a successful build as one transaction:

1. Build a ROM from the clean one into the project's staging area (`--out`),
   optionally for one selected variant.
2. Validate the staged ROM's header.
3. Move it to the variant's output, where a build without `--out` would have
   written it, and re-point the reader at it. A project with no `rom.output`
   is refused: its build would land on the clean ROM.
4. Clear open documents and cached level sessions, then reindex the project.

A failed build or a rejected ROM leaves the project untouched, and the staging
area is removed either way. Losing the previous build is not a loss: NCPatcher
rebuilds it from the pristine binaries it keeps, which is what makes rebuilding
idempotent, so nothing is moved aside first. One file arrives; a project built a
hundred times is the size of one built once.

The variant list is loaded only when its selector is opened. A remembered
variant that is absent from a later query is kept visibly rather than silently
changed. An empty configured list is distinct from a failed query.

## Deliberate limits

- Only one variant can be applied at a time, because a project holds one built
  ROM.
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

`config dump --json` reports `rom-banner` and each variant's `banner`, so the
editor writes an edited banner to the file the project configured and never asks
the user which one that is. A project that configures no banner is told so
rather than offered a file picker: a banner written somewhere no build reads is
worse than one not written at all. Banner data is not a NitroFS file and is
therefore never routed into a module tree.

The Fireflower and NSMBe-compatible wrapper build actions were removed in M15.
NCPatcher is the only build path.
