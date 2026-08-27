# M12 manual checklist

Automated tests cover the exchange codecs, unsafe-path and duplicate refusal, compound history, clipboard round trips, visual palette structure, Project Hub invariants, and recovery snapshot lifecycle. Use a user-provided NSMB DS project for the visual and interoperability checks below.

## Hub and editor parity

- Exercise all five Project Hub tabs and restart with the Hub active.
- Verify scrollable thumbnail actor categories and live name/category filtering.
- Exercise the minimap, grid, 8-pixel snap, DS screens, zoom presets, fullscreen, and a Shift+Alt-draggable reference image.
- Exercise `C O S E V Z P G`, arrow nudging, WASD scrolling, middle/Alt panning, Ctrl-drag/Ctrl+D cloning, and PgUp/PgDn ordering.
- Paste a mixed selection with a custom actor and confirm paste, import, and hex replacement are each single named history steps.

## Exchange and tools

- Round-trip a level through `.nml`; when NSMB Editor is available, verify cross-editor import in both directions.
- Refuse malformed hex and malformed `.nml` without changing the open level.
- Find known objects and sprites across multiple levels, then follow Sprite Event Viewer rows back to their placements.
- Export changed module files as `.nsmbpatch`; import into another project and verify file-ID warnings, backups, module routing, and rollback.
- Create and apply an xdelta3 patch using file paths containing spaces, then boot the output ROM.

## Recovery and language

- After the one-minute autosave interval, terminate with a dirty level and verify startup recovery. Save once and repeat with Discard/No to verify both cleanup paths.
- Restart in English, Dutch, Portuguese, Spanish, and French and verify the translated project shell and Hub vocabulary.

The same checks are appended as items 27–38 in the living `docs/m4-manual-checklist.md`.
