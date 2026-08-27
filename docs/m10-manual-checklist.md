# M10 manual verification

Use a user-provided NSMB project with an editable level and an active module selected in the Filesystem dock.

1. Open **Tools → Tilesets and Backgrounds** and confirm all seven tabs render without changing the level.
2. Change one Map16 quadrant, one object tile, and one behaviour flag. Save, reopen the dialog, and confirm all three values persist.
3. Change the background and foreground screen entries independently and confirm each level layer refreshes from its own screen ID.
4. Export each rendered layer as PNG, import an opaque and a transparent PNG, and confirm palette index 0 remains transparent only for tileset artwork.
5. Export a `.bstileset`, change the selected tileset, import the bundle, and confirm graphics, Map16, objects, and behaviours return together.
6. Confirm only changed files appear as Pending in the Filesystem dock, under the supplying or active module and variant. Run NCPatcher and compare the rebuilt level preview.
7. Cancel the dialog after changes and confirm neither module files nor level slot IDs change.
