# Reference notes

These notes summarize a limited M0 inspection of the current ecosystem. The old repositories are treated as behavioral and file-format references, not as source to copy.

## NSMB Editor

- The MammaMiaTeam fork is a C# WinForms application licensed under GPL-3.0-or-later notices in source files.
- ROM state is centered around a `ROM` class and Nitro filesystem abstractions, including ARM9/ARM7 binaries, overlay tables, overlays, header data, and file replacement/edit transactions.
- `CodePatcherModern` integrates both Fireflower and NCPatcher by creating a temporary `__tmp` extracted filesystem-like directory containing `arm9.bin`, `arm9ovt.bin`, `arm7.bin`, `arm7ovt.bin`, `header.bin`, a dummy `fnt.bin`, and overlay directories. It runs the patcher from the ROM/project directory, then imports changed binaries and overlays back into the editor's filesystem model.
- Central level-editing references include `NSMBLevel.cs`, `LevelItem.cs`, `ExportedLevel.cs`, editor controls, and level source classes. File formats that appear central include Nitro filesystem/FNT/FAT concepts, overlays, NARC archives, NSMB level/course files, background/tile data, sprites, entrances, paths, zones, tilesets, and graphics/model formats used by DS games.

## Fireflower

- Fireflower provides `nds-extract` and `nds-build` utilities plus patching logic. It expects ROMs to be extracted before patching and rebuilt after patching.
- Example build rules reference `header.bin`, `fnt.bin`, `arm9.bin`, `arm7.bin`, overlay tables, banner/icon data, root data, and overlay directories.
- Fireflower has been superseded by NCPatcher for patching, but its extraction/build tools remain useful service targets.

## NCPatcher

- NCPatcher is a C++ CMake project that reads `ncpatcher.json` from the current working directory.
- The current CLI supports help/version/verbose-style flags, but the main workflow assumes the working directory contains `ncpatcher.json` and target configs.
- The code template's `ncpatcher.json` includes variables/flags, `backup`, `filesystem`, `toolchain`, `arm7`, `arm9`, `pre-build`, `post-build`, and `thread-count` fields. `arm9` commonly points at `arm9.json` and a `build` directory.
- The NSMB code template documents two workflows: standalone extraction/patch/build using Fireflower tools, and NSMBe integration where the editor creates `__tmp`, runs NCPatcher, and imports outputs.

## GBATEK and NSMB code references

- GBATEK is the technical reference for NDS ROM layout, ARM binaries, overlays, filesystem tables, compression, and hardware constraints.
- NSMB Code Reference and NSMB Code Template are useful for symbols, hooks, and patching workflow expectations, but they may involve Nitro SDK expectations. This repository must not include Nintendo SDK material.
