# Project manifest format

Big Star Editor projects are opened by selecting the project directory. The manifest is `big-star-project.yaml` inside that directory.

The manifest intentionally avoids absolute paths. Paths are stored relative to the project directory so projects can move between machines and operating systems. The project directory itself is not serialized because it is implied by the manifest location.

```yaml
format: big-star-editor-project
version: 3
name: "Example Project"
rom:
  rom_path: "big-star-build.nds"
  game_title: "NEW MARIO"
  game_code: "A2DE"
```

The backend owns a deliberately small YAML subset without a third-party YAML dependency. `format`, a supported `version`, and a nonempty `name` are required. Quoted strings support `\\`, `\"`, `\n`, `\r`, and `\t` escapes.

`rom.rom_path` must remain inside the project directory. Absolute paths, parent traversal, Windows drive prefixes, and backslashes are rejected. Manifest writes use an adjacent temporary file followed by atomic replacement, so a failed write does not leave a partially written manifest.

It names the ROM the editor reads: `big-star-build.nds`, which is what a successful build writes, falling back to the imported `rom.nds` before a project has ever been built. Both are files in the project directory, not directories.

## The ROM is not unpacked

The editor reads the `.nds` where it lies. Nothing walks it into a directory of loose files first: opening a project opens the ROM, and a file's bytes are read from it on demand, through the FAT for an ordinary file and through the container for an archive member. What the editor shows is that read, overridden by a module source whenever the plan says a build would put one there — so an edit is visible before the build that applies it.

Version 1 stored a walk of an unpacked tree as a `files:` list of `path`/`size` pairs — for a full NSMB tree, roughly 4,000 lines that had to be rewritten on every build and carried no file IDs. File IDs come from NCPatcher's `files-dump` manifest (`RomFileEntry.id`), which is also the only source of provenance, and is what the Filesystem dock shows. Version 2 dropped the list. Version 3 drops the unpacked tree itself: `rom.extracted_path` becomes `rom.rom_path`. A version 1 or 2 manifest still opens — its `files:` block and its `extracted_path` are validated and then ignored — and is rewritten as version 3 on the next save.

## Module edit targets

The ROM is a build result. Editor writes therefore go to an enabled module's `nitrofs` tree, never into the ROM itself. The active module and project variant are workspace choices; an existing module-supplied file keeps its supplying module while using the active variant layer, and an otherwise vanilla file uses the active module. A member of a Nitro archive is addressed as `a/b.narc!c/d` and written to `<module>/nitrofs/<layer>/b_narc/c/d`, which the next build sweeps back into the container. The Filesystem dock shows a member's file ID beside a loose file's: an archive carries the ROM's name-table structure and numbers its members the same way, so the number is a file ID in the container's table rather than the ROM's, and the footer names which container it belongs to.

Until NCPatcher rebuilds the ROM, these writes appear as **Pending**, with the file ID the next build would assign shown as a prediction. A successful build replaces `big-star-build.nds` and reloads the generated module and file documents.

The Filesystem dock resolves project-level and selected-variant `files:` entries from NCPatcher's current config before module-tree offers. If NCPatcher is unavailable, it falls back to explicit-file provenance from the last manifest and does not invent prospective file IDs.
