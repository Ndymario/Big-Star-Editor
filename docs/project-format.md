# Projects

A Big Star project is an NCPatcher project: a directory with an `ncpatcher.yaml` (or `.yml`). The workspace lists every
such directory, named after the directory, and opening one is opening that directory. A legacy `ncpatcher.json` project
is listed too, so the editor can offer to migrate it. Big Star writes no project file of its own; the
`big-star-project.yaml` manifest earlier versions wrote is no longer read, and can be deleted.

## The clean ROM

`ncpatcher.yaml` need name no input ROM: where a clean ROM lies is different on every machine, and the configuration is
the same on all of them. NCPatcher takes it on the command line instead, with `--rom`, and accepts a `rom:` section
without `file` or `dir` when it is given:

```yaml
rom:
  output: build/nds/rom.nds
  backup: backup
```

The editor keeps the path in the project's `.bigstar/workspace.json`, beside the other per-project choices (variant,
module, autosave interval, backups kept), relative to the project when the ROM is inside it and as it is otherwise:

```json
{
  "active_variant": "en",
  "clean_rom": "rom.nds"
}
```

A project whose settings name no clean ROM, but whose `ncpatcher.yaml` does name one with `rom.file` that is there,
takes that one: it is written to `.bigstar/workspace.json` as if it had been chosen.

A project that opens without a usable clean ROM -- none set, the file gone, or a ROM that is not New Super Mario Bros.
-- still opens, says why in the project settings, and the frontend asks for one (`SetCleanRom`). **Build > Clean
ROM…** changes it later. A build is refused until there is one. On a served host it is chosen with a browser of the
workspace and must lie under it; the desktop app's own host takes a ROM anywhere on the machine by its full path, and
asks for it with the system's file dialog.

## The ROM the editor reads

There is no ROM path to configure. A build writes the active variant to the `rom.output` `ncpatcher.yaml` sets, with
`_<variant>` before the extension as NCPatcher names a variant's output (`build/nds/rom_en.nds`), and that is what the
editor reads, as `config dump --json` reports it. Before the variant's first build it reads the clean ROM. A project
with no `rom.output` cannot be built from the editor: NCPatcher would patch the clean ROM in place.

## The ROM is not unpacked

The editor reads the `.nds` where it lies. Nothing walks it into a directory of loose files first: opening a project opens the ROM, and a file's bytes are read from it on demand, through the FAT for an ordinary file and through the container for an archive member. What the editor shows is that read, overridden by a module source whenever the plan says a build would put one there — so an edit is visible before the build that applies it.

File IDs come from the ROM's own table and from NCPatcher's `files-dump` manifest (`RomFileEntry.id`), which is also the only source of provenance, and is what the Filesystem dock shows. None of it is mirrored into a file of the editor's own.

## Module edit targets

The ROM is a build result. Editor writes therefore go to an enabled module's `nitrofs` tree, never into the ROM itself. The active module and project variant are workspace choices; an existing module-supplied file keeps its supplying module while using the active variant layer, and an otherwise vanilla file uses the active module. A member of a Nitro archive is addressed as `a/b.narc!c/d` and written to `<module>/nitrofs/<layer>/b_narc/c/d`, which the next build sweeps back into the container. The Filesystem dock shows a member's file ID beside a loose file's: an archive carries the ROM's name-table structure and numbers its members the same way, so the number is a file ID in the container's table rather than the ROM's, and the footer names which container it belongs to.

Until NCPatcher rebuilds the ROM, these writes appear as **Pending**, with the file ID the next build would assign shown as a prediction. A successful build replaces the variant's output and reloads the generated module and file documents.

The Filesystem dock resolves project-level and selected-variant `files:` entries from NCPatcher's current config before module-tree offers. If NCPatcher is unavailable, it falls back to explicit-file provenance from the last manifest and does not invent prospective file IDs.
