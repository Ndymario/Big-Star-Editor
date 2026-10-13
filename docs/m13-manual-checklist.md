# M13 manual checklist

Automated tests cover loss-preserving round trips, malformed-input refusal,
editable surfaces and resolved banner configuration. Use user-provided ROMs and historical code projects for the checks
that require visual or game-runtime confirmation.

## UI resources

- Open representative BNBL and BNCL files, change placements and alignments,
  add/delete rows, then boot the affected UI and compare its touch/graphic
  layout. Confirm a no-change save is byte-identical.
- Open BNCD, edit a cell and OAM field, then paint its 4bpp and 8bpp texture
  views. Confirm invalid subentry references are refused.
- Open an ENPG loading image, paint indexed pixels, edit several BGR555 palette
  colours, and boot it. Repeat with an LZ10- or LZ11-wrapped resource and confirm
  the original compression kind remains.

## ROM-owned data

- Configure `rom.banner`, edit the icon, palette, and all language titles, then
  build and inspect the console menu entry. Repeat with a variant-level banner
  override and confirm the editor targets that resolved source.
- On a ROM with `NSMBeUD` at NitroFS file ID 131, edit names and notes from both
  Tools and the Filesystem dock. Confirm the save lands in the supplying or
  active module layer and survives an NCPatcher rebuild.

## Network and mounted trees

The Fireflower and NSMBe-wrapper build actions were removed in M15; the checks
that covered them no longer apply.

- Place a project or active module tree on an OS-mounted network volume and
  confirm ordinary watcher and atomic-save behavior. No editor-specific remote
  transport is expected or supported.
