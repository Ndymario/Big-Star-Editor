# Roadmap

The milestones of the C++ implementation, kept as history. The Rust tree that replaced it was built phase by phase
in [the port plan](rust-port-plan.md), whose Status table records what each phase did and left.

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

- Replaced the legacy `std::system` and process-wide `chdir` path with a no-shell process runner that passes argv intact and captures both streams.
- Added an external NCPatcher 2.x client for configuration, module graph, file manifest, build, migration, and version commands.
- Added strict readers for the versioned config, module, file, and result documents while preserving declaration order for variants.
- Added structured progress, diagnostics, artifacts, and result summaries to an asynchronous Build Output dock with clickable source locations.
- Added lazy, per-project single-variant selection and CLI `run-patcher --variant` and `patcher-info` commands.
- Refused direct legacy `ncpatcher.json` builds and offered the verified `migrate --write` workflow instead.
- Applied builds transactionally: build and extract in staging, validate, back up the previous extracted tree, swap, clear stale sessions, and reindex.
- Kept NCPatcher external; multi-variant apply, graphical YAML editing, and cancellation remain deferred.

## M6: accurate level model and NARC archives

- Replaced guessed level fields with the full 14-block record model: header, camera, screen, entrance, stage object, view, zone, path, and object-bank data.
- Added signed 1/4096 `fx32` values for path speed and acceleration and compile-time record-size checks.
- Made views and all 16 object-bank slots editable; bank choices show the overlay found by signature-scanning the ROM's ARM9 table instead of relying on a region-specific offset.
- Preserved unedited and unknown bytes, including block 0 data beyond its 32-byte header and block 13 records after the object-bank slots.
- Added strict NARC read/write support with preserved FNT bytes, indexed member replacement, bounds checks, and byte-identical untouched round trips.
- Removed the unused M0 `Level`/`Area`/entity model in favor of `EditableLevel` as the single level representation.

## M7: glue and custom stage objects

- Added strict readers for the generated `level_data.json` and `stageobjects.json` catalogs, with an empty-catalog fallback when either optional file is absent.
- Added the loss-preserving glue block codec, including the complete malformed-input refusal set, table/payload order preservation, dry-run writes, and byte-identical preservation of unreadable data.
- Custom placements now carry hashes in memory. Registration is derived in stable loaded order plus first-placement order, drops unused entries, and assigns numeric stage IDs only while serializing.
- Added a grouped Custom actors palette, hash-aware canvas rendering, editable declared/undeclared level-data rows, and a read-only view of the registration that will be written.
- Routed vanilla and custom stage-object lookup through `EditableLevel`, returning `-1` for foreign or unresolved hashes instead of indexing a vanilla table.

## M8: module-aware filesystem explorer

- Added a filesystem view that joins the extracted NitroFS tree, the last `ncpatcher.files/1` manifest, and the currently enabled module trees.
- Reproduced NCPatcher's layer-before-module precedence, base-variant fallback, variant-to-module layer mappings, `into:` prefixes, component glob subtraction, current base/variant `files:` precedence from the resolved config, and outermost `_narc` convention.
- Added Vanilla, Replaced, Added, Pending, and Orphaned states with byte-level stale detection, including NARC member comparison.
- Added a Filesystem dock with active module/variant selection, a path/status list whose columns are resizable and whose provenance/variant/id/size columns are opt-in from the header menu, a footer reporting those details for the selected file, recoverable removal, and add/replace/extract/hex-edit actions.
- Routed level saves into the supplying module or active module target. The extracted ROM remains a derived view and pending writes receive no fabricated file ID.
- Added a pending-build banner and debounced watchers for external changes throughout enabled module NitroFS trees.

## M9: graphics subsystem

- Added strict, loss-preserving NCLR, NCGR, NSCR, NCER, and NANR readers and writers with malformed-offset and round-trip coverage.
- Added NSBTX/NSBMD TEX0 parsing and same-layout writing for every DS texture format, including deterministic format-5 4×4 compression and rendering.
- Added Nitro LZ10/LZ11 compressors, Yaz0 compression/decompression, and backwards-BLZ compression alongside the existing decoders.
- Added a filesystem graphics dialog for NCGR/raw indexed-pixel painting, NCLR/raw/TEX0 palette editing with the native colour picker, and NSCR/NCER/NANR/TEX0 viewing.
- Routed graphics saves into the supplying or active module target and retained an input file's LZ10/LZ11 wrapper.
- Replaced labelled actor diamonds with decoded 2D sprite sheets or model textures for the verified actor resource catalog; unknown actors retain the safe fallback.

## M10: tilesets, Map16, behaviours, and backgrounds

- Added inverse codecs for BGR555 palettes, 4bpp/8bpp tiles, screen maps, Map16, and NSMB object layouts, preserving untouched LZ streams and object-index metadata.
- Added an editable multi-file tileset model for graphics, palettes, Map16 quadrants, object layouts, and four-byte tile behaviours, including the complete flag/subtype vocabulary and named parameter meanings.
- Added an editable NSC/tilemap model for background and foreground graphics, palettes, tile indices, palette banks, and flips.
- Added deterministic median-cut BGR555 quantisation, PNG import/export, exact tile deduplication for imported tilemaps, and a validated versioned `.bstileset` import/export container.
- Added a seven-tab Qt editor for level slot IDs, tileset graphics, Map16, objects, behaviours, backgrounds, and foregrounds under **Tools → Tilesets and Backgrounds**.
- Split the foreground graphics and screen IDs in the level model, fixed foreground resource resolution, and preserved both through undo/redo and course serialization.
- Routed only changed graphics resources to their supplying or active module target; unchanged resources remain byte-identical and do not become pending replacements.

## M11: sprite definitions and rendering

- Added strict actor metadata with 0-based bit offsets, intentional overlap support, choices, categories, colours, shapes, render bounds, and grab bounds; this metadata now lives in the per-actor Lua files.
- Added ordered plugin → project → local → shipped loading with first-definition-wins resolution by ROM class/object ID and a plugin-directory seam that does not require a plugin host.
- Added a reproducible GPL-compatible import of all 305 NSMB Editor class definitions, including its metadata, 48-bit field layout, zero-settings bounds, and documented ROM filenames; no bundled sprite PNG or ROM data is included.
- Generated named sprite-setting rows in the property dock while retaining raw settings for undefined actors and deliberately giving module objects no generated fields.
- Applied definition render bounds and shapes to the canvas and definition grab bounds to scene sizing, hit testing, marquee selection, and selection outlines.
- Categorized the searchable visual palette while preserving decoded actor thumbnails and the module/component custom-actor tree.
- Replaced the split shipped JSON/C++ actor catalog with sandboxed, per-actor Lua files. Project and enabled-module actors use the same metadata, named-field, bounds, and filename-resolved graphics API.
- Kept actor Lua states alive as render programs with cached raw/NCGR/NCLR and NSBMD/NSBTX TEX0 resources, multiple transformed draw calls, source tile/frame selection, and settings-dependent grab/render bounds.
- Ported Pokey's dynamic bounds and body/head composition from `NSMBStageObj.cs` as the first ROM-backed behavioral actor; NSMBe's PNG previews remain intentionally excluded.
- Added synthetic backend and Qt coverage for schema refusal, layered precedence, bit reads/writes, generated fields, and definition-driven hit bounds.

## M12: hub window and UX parity

- Added a permanent five-tab Project Hub for level listing, tilesets, backgrounds, the effective filesystem, and common tools; its active state now participates in workspace restoration.
- Replaced the entity-kind combo with side tabs and the vanilla actor list with categorized, searchable, scrollable thumbnail grids while retaining the module/component custom-actor tree.
- Added a minimap, fine grid, DS-screen overlay, 50/100/200% zoom presets, fullscreen, draggable reference backgrounds, 8-pixel snapping, Ctrl-drag cloning, keyboard entity-tab switching, arrow nudging, WASD scrolling, Alt/middle-button panning, and PageUp/PageDown z-order.
- Added compressed Base64 cut/copy/paste for every canvas entity kind, compound paste/import/hex commands, and named multi-step undo/redo split menus.
- Added strict NSMBe-compatible `.nml` level exchange and a validated level hex editor, plus project-wide object/sprite finders and a sprite-event viewer.
- Added strict compressed `.nsmbpatch` bundles with safe paths, duplicate refusal, file-ID warnings, transactional module writes, backups, and rollback; xdelta3 create/apply uses the no-shell process service.
- Added minute-interval dirty-level recovery snapshots with atomic writes, startup recovery, stale-snapshot cleanup, and explicit save/discard semantics.
- Added a five-language startup locale selector and clean-room translations for the project shell and M12 hub vocabulary.
- Added synthetic exchange-format, history, clipboard, palette, hub, and recovery coverage; see `docs/m12-manual-checklist.md` for ROM-backed validation.

## M13: remaining tools

- Added strict, loss-preserving BNBL/BNCL/BNCD and ENPG codecs with editable layout, cell/OAM, indexed-image, and palette surfaces; LZ10/LZ11 wrappers remain intact.
- Added DS icon/title banner editing with all version-available languages, BGR555 palette editing, preserved extension bytes, and regenerated banner CRCs. The typed config reader accepts resolved project/per-variant banner targets when NCPatcher exposes them and otherwise prompts for the configured source.
- Added the ASCII section/key/value `NSMBeUD` codec and a module-aware editor for NitroFS file ID 131, available both from Tools and the Filesystem dock.
- Decided against an editor-specific network filesystem. Mounted/network-synchronised module directories already work through ordinary filesystem watching; a new remote protocol has no defined authentication, conflict, offline, or recovery contract and would duplicate the active-module workflow.
- Added synthetic round-trip, malformed-input, Qt-surface, resolved-banner, and isolated legacy-workspace coverage; see `docs/m13-manual-checklist.md` for ROM-backed validation.

## M14: web frontend

- Compiled the portable level backend to WebAssembly behind a versioned,
  browser-neutral C ABI with opaque sessions, named JSON snapshots, defensive
  mutations, undo/redo, structured errors, and owned serialization buffers.
- Kept browser APIs out of `libs/`; Emscripten builds only portable core,
  formats, and editable-level sources while excluding project-directory,
  patching, CLI, Qt, and process adapters.
- Added a dependency-free responsive web editor that opens local course/BGDat
  pairs, renders a schematic canvas and entity tables, edits the main record
  types, adds/deletes rows, runs undo/redo, and downloads both serialized files.
- Added native ABI tests and exercised the compiled WebAssembly bundle in the
  in-app browser against a real `J01_1` level; see `docs/m14-manual-checklist.md`.

## M15: UI/UX overhaul

- Removed the canvas tool modes. Left-click selects whatever is under it across
  every layer, right-click creates the kind the Properties panel is showing, and
  panning stays on middle-drag, Space and Alt-drag.
- Gave objects, zones and views eight resize knobs with per-edge cursors; views
  could not be resized at all before, and top/left knobs now move the origin
  rather than only the extent.
- Replaced the alphabetised palette toolbox with a permanent picker: one Objects
  sub-tab per loaded tileset slot, a searchable sprite list, and the custom
  actor tree. Object previews render at their natural size with id and slot
  tooltips.
- Made the build variant real project-wide state: one selector on the main
  toolbar, level sessions and file-editor documents keyed by variant, open
  documents closed and reopened on a switch, and graphics resolution redirected
  through the active variant's module layers.
- Collapsed the two graphics editors into one that pairs files the way NSMB
  Editor does: a palette, graphics and tilemap chosen together, remembered per
  file. NCGR renders through the chosen NCLR, NSCR composes a real tilemap, and
  NCER draws its cells. `character_format` (NCBR bitmap sheets) and the `CPOS`
  source size are honoured, and headerless `_ncg.bin` sheets take an explicit
  tile width instead of a square-root guess.
- Moved the graphics, message and hex editors out of modal dialogs into document
  tabs, so the shell stays usable while a file is open. The tileset editor keeps
  its dialog: its save spans many resources and does not fit the one-file
  document model.
- Merged the duplicated log and build-output docks into one Output dock,
  removed the Project Hub and the Fireflower and NSMBe-wrapper build actions
  along with the now-unused legacy patch adapter in `nsmb-patching`,
  added a main toolbar, and folded the Language menu into Edit ▸ Preferences.
- Added synthetic coverage for unified selection, the eight knobs, the single
  variant selector, palette pairing, NCBR layout, explicit raw widths, the
  merged editor and file document tabs; see `docs/m15-manual-checklist.md` for
  ROM-backed validation.

## M16: one resolver, one ROM, archives you can open

- Stopped re-deriving NCPatcher's module resolution. `files plan` answers what a
  build would place -- ids, sizes, actions and provenance, with `planned: true`
  and prospective ids for `z_new/` additions -- so roughly 120 lines of
  layering, module order, `into:`, the archive-folder convention, component glob
  subtraction and brace expansion are gone. Two implementations of precedence
  rules cannot be kept in step by testing either one alone.
- Read variant layers from `config dump --json` instead of hand-parsing
  `ncpatcher.yaml`, which the integration document had always claimed the editor
  never did.
- Retired the extracted tree. `RomReader` opens the built `.nds` in place and
  `EffectiveFilesystem` answers a read as the module source when a build would
  use one and the ROM otherwise -- so a pending edit is visible before the build
  that applies it, without a derived tree that existed only to be corrected
  afterwards. Manifest version 3 replaces `rom.extracted_path` with
  `rom.rom_path`; a version 2 manifest still opens.
- Applied a build by moving one file. No extraction, no backup, no swap: a
  project built a hundred times is the size of one built once, and two
  consecutive builds of the coop project produce byte-identical ROMs.
- Deleted the editor's own ROM builder and the CLI's `build-rom`. Repacking a
  derived tree was never a legitimate way to produce a ROM. **File > Extract
  ROM...** delegates to `ncpatcher rom extract`, so there is one extractor and
  one layout.
- Made archives expand like folders. The view emits a row per member with its
  own status and provenance and compares each against the container's copy at
  that id, so an archive with one stale member now says which member. The dock
  splits an address on `!` before `/` and lists the container's remaining
  members when somebody opens it, not for every archive in the game on project
  open. Compressed containers (`.carc`) are read through their wrapper.
- Exposed the file IDs of NARC/CARC members. A `.narc` carries the ROM's File
  Name Table structure and numbers its members the same way, so a member has a
  file ID for the same reason a loose file does -- it is game code's address for
  it, and both ndspy and NSMB Editor read a NARC exactly this way. The dock had
  been printing a dash. Now the ID column and the footer show it, the footer and
  a tooltip name the container whose table it belongs to, and NCPatcher's
  manifest spells the field `id` rather than denying a member has one.
- Offered Cancel on a running build, mapping NCPatcher's exit code 11 to a
  `Cancelled` stage rather than a failure.
- Bounded `.big-star-backups/`: the ten most recent of each kind, swept on
  project open and whenever a write adds one.

## M17: editing a ROM directly

- Added ROM mode next to module projects. On the desktop, **Open ROM...** on the connect screen picks a `.nds` with
  the system's file dialog and edits it with a host that has no workspace, as NSMB Editor does; every save writes
  into it, **Save ROM As...** moves later saves to another file, and **Close ROM** ends that connection. Reads go through the same `EffectiveFilesystem`,
  with the session's edits answering first, so the level canvas, file editors, tileset editor and Lua actors work
  unchanged.
- Uploaded ROMs for the browser: `UploadRom`/`DownloadRom` in pieces, one private host per uploaded ROM that dies with
  its connection and never touches disk, limits on size and count, and `serve --rom-sessions` for guests without the
  token (or, without a root, a host that serves only uploads). The browser saves downloads through a Blob and reads
  uploads through the page's file picker.
- Ported the in-place half of NCPatcher's `NdsRom::commit` as `NdsRomWriter` (`crates/nds`): replaced files go back
  into their extent when they fit and are appended otherwise; the FAT, used size, device capacity and header CRC are
  updated, and a commit with nothing staged returns the input unchanged. Archives go back in the wrapper they came in.
- `RomReader::from_bytes` reads a ROM held in memory, so neither kind of ROM has to be read from disk again.
- Hid what needs a module project in ROM mode -- Build, Run Patcher, the variant selector, the Distribution menu, the
  module picker and the Level Data (glue) tab -- and made the host refuse it.


## M18: NCPatcher projects only

- Retired `big-star-project.yaml`. A project is an NCPatcher project -- a directory with an `ncpatcher.yaml`, or a
  legacy `ncpatcher.json` offered for migration -- named after its directory. Big Star edits those, or a ROM directly,
  and nothing else; "module projects" are now called NCPatcher projects throughout.
- Dropped `CreateProject`, `ImportRom`, the workspace's **Import ROM...** and the CLI's `import-rom`: setting up a
  project is `ncpatcher init`'s job. The `oracle` verb takes a ROM instead of a project. Protocol 17.
- Moved the clean ROM out of `ncpatcher.yaml` into the project's `.big-star-workspace.json` (`clean_rom`), passed to
  every NCPatcher command as `--rom`. NCPatcher now accepts a `rom:` section without `file` or `dir` when `--rom` is
  given. A project without a usable clean ROM opens and asks for one (`SetCleanRom`, **Build > Clean ROM...**).
- The desktop app's own host takes a clean ROM outside the workspace by its full path, chosen with the system's file
  dialog; a served host keeps to the workspace browser. A project with no clean ROM in its settings takes the
  `rom.file` its `ncpatcher.yaml` names, when that file is there.
- Dropped `big-star-build.nds`. A build lands on the variant's output from `config dump` -- `rom.output` with
  `_<variant>` before the extension, which NCPatcher now derives for a single `--variant` build as it did for
  `--all-variants` -- and that is the ROM the editor reads, or the clean ROM before the variant's first build.
