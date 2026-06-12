# NCPatcher integration

## Current M0 approach

`nsmb-patching` exposes a `PatcherService` interface and an `NCPatcherExternalTool` implementation. The implementation validates a project directory, creates a minimal `ncpatcher.json` when missing, constructs a command, and can run the external executable in a controlled working directory.

ROM extraction and building are represented by `RomExtractionService`, `RomBuildService`, and `ExternalToolRomBuildService` wrappers for `nds-extract` and `nds-build`. `import-rom` now invokes `nds-extract`, writes project-relative YAML metadata, and generates Fireflower-style `buildrules.txt` for later rebuilds.

## Current constraints observed in references

- NCPatcher expects `ncpatcher.json` in the current working directory.
- The NSMB code template asks users to avoid whitespace in paths and to use forward slashes in JSON paths.
- The existing editor integration creates a `__tmp` filesystem directory, runs the patcher from the ROM/project directory, and imports changed binaries/overlays back into the editor.
- Standalone workflows require extract -> patch -> build sequencing.

## Recommended NCPatcher improvements

- Stable CLI flags for config, filesystem input, output/build directory, backup directory, and target files.
- Explicit input/output paths instead of current-working-directory assumptions.
- Machine-readable JSON diagnostics.
- Progress reporting for editor UI integration.
- Dry-run/validate mode for checking `ncpatcher.json` and toolchain availability.
- Stable error codes.
- Schema/version reporting for `ncpatcher.json`.
- Library/API mode or embeddable core for future tighter integration.
- Path handling that supports whitespace and native platform separators.
