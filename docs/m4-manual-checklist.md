# M4 manual ROM checklist

Use an imported ROM project with at least one available level. These checks intentionally require human visual confirmation; no UI automation is assumed.

1. Single-click two different levels. Confirm the same read-only **Preview** tab updates instead of opening more tabs.
2. Double-click a level twice. Confirm one editable tab opens and the second double-click focuses it. Double-click another level and confirm it gets a separate tab.
3. Right-click an editable tab, choose **Move into New Window**, edit there, then use **Document > Move Back to Main Window**. Confirm Properties and Undo/Redo follow the focused editor.
4. Close Preview and each dock. Restore them from **View**, then choose **View > Reset Layout**.
5. Set different zoom and pan positions, detach one editor, close the application, reopen the same project, and confirm the workspace is restored.
6. Use Select, Space-drag or Pan, the mouse wheel, Shift-click, and a marquee. Move a mixed selection, resize an object or zone, then Undo and Redo. Confirm each gesture is one history step; press Escape during a gesture and confirm it is cancelled.
7. Search the Palette and select a terrain object, actor, entrance, zone, path, or progress path. Confirm left-click only selects the toolbar's active category and right-click creates the selected Palette entry. Toggle Snap and hold Alt for a temporary snap bypass. Delete a multi-selection and undo it once.
8. Select several objects or sprites of the same kind. Confirm common Properties values are shown, differing values display an em dash, and changing one common field updates the whole selection in one undo step.
9. Compare terrain, background, and foreground graphics with the game. Change the level's tileset/background references and confirm the canvas and Palette repaint without reopening the level.
10. Search for actors not already used by the level, including Goomba and Koopa. Confirm all actor types are available and appear as clearly labeled fallbacks rather than incorrectly assembled ROM tiles.
11. Replace a watched graphics file directly, then by an atomic save from another tool. Confirm affected documents refresh after a short delay. During a malformed write, confirm the previous valid render remains and a diagnostic appears in Log.
12. Use **View > Refresh Graphics** and confirm graphics reload on demand. Save the project, reopen it, and confirm both the edits and workspace state persist.
13. Open **Views** in Properties, edit every named field, save and reopen the level, and confirm the view rectangle and values persist.
14. Open **Object-bank slots** in Properties. Confirm banks 1–10 show the corresponding overlay number, edit a used slot, save and reopen, and confirm records after the first 16 bytes of block 13 remain unchanged.
15. Open a level with signed path coordinates and nonzero speed, acceleration, delay, and flags. Edit and round-trip it without changing the fixed-point raw values unintentionally.
16. Open the real module project after generating its catalogs. Confirm **Custom actors** groups entries by module and component and lists both `coop.CoopFlagActor.Default` and `.Big`, even though they share object ID 387.
17. Place `.Default`, save, and inspect Level data. Confirm registration assigns it stage ID 326. Add `.Big`, delete every `.Default`, save again, and confirm `.Big` is renumbered to 326.
18. Open a glue-free level and save without adding level data. Confirm block 0 remains exactly 32 bytes. Corrupt a glue tail, reopen the level, and confirm the error is shown read-only and saving preserves block 0 byte-for-byte.
19. Place or load a custom actor hash absent from the current catalog. Confirm it remains registered across a save and renders as a clearly labeled foreign actor rather than a random vanilla actor.
20. Build the ROM and boot a level containing a custom actor. Confirm the actor spawns with the selected variant's placement geometry.
21. Open the **Filesystem** dock on a module project. Confirm its tree shows path and status only, that provenance, variant, file ID, and size for the selected file appear in the footer above the buttons, that right-clicking the header toggles those as extra columns, that column widths can be dragged, and that the column choices and the active module/variant choices survive reopening the project.
22. Edit and save a vanilla level. Confirm the extracted copy is unchanged, a copy appears under the active module's `nitrofs/<variant>/course/`, and both files show as Pending until a build.
23. Add a new `z_new/` file, replace an existing file, and remove a module-supplied file. Confirm the new file has no ID, the replacement is Pending, and removal is recoverable from Trash and appears as reverting/orphaned.
24. Drop, replace, and remove a file using another application. Confirm the Filesystem dock refreshes after the debounce without using a manual refresh command.
25. Run NCPatcher from the pending banner. Confirm the extracted tree is transactionally refreshed, Pending rows settle to Added or Replaced with their manifest IDs, and a removed replacement returns to Vanilla.
26. With a project that uses `base-variant`, `module-variants`, `into:`, disabled component file globs, and `_narc` member folders, compare the displayed winners and provenance with NCPatcher's generated `files.json`.
27. Open each Project Hub tab and confirm Level Listing, Tilesets, Backgrounds, Filesystem, and Tools target the same active project as their dock/menu counterparts. Restart with the Hub active and confirm it is restored.
28. Confirm each vanilla palette category is a scrollable thumbnail grid and search matches both actor names and category names. Confirm custom actors remain grouped by module/component.
29. Toggle Minimap, Grid, 8 px, DS Screen, each zoom preset, and Fullscreen. Load a reference image and Shift+Alt-drag it to a useful alignment.
30. Use `C O S E V Z P G` to switch entity tabs, arrows to nudge, WASD to scroll, middle- and Alt-drag to pan, Ctrl-drag/Ctrl+D to clone, and PgUp/PgDn to change z-order.
31. Copy a mixed selection containing a custom sprite, paste it, and undo once. Confirm all pasted entities and the custom hash appear/disappear together. Inspect the Undo/Redo split menus for meaningful newest-first names.
32. Export a level as `.nml`, modify it, import the export, and undo once. Open the same export in NSMB Editor if available and confirm both course and BGDat data are accepted.
33. Hex-edit both course and BGDat. Confirm malformed hex or malformed level bytes are refused and a valid edit is one undo step.
34. Find a known object and sprite across the project, open a result, and confirm the correct entity is selected. Open Sprite Event Viewer and confirm both event bytes are represented.
35. Export changed module files as `.nsmbpatch`, import into another compatible project, and confirm writes target the supplier/active module. Exercise a file-ID mismatch, existing-file backup, and failed-write rollback.
36. Create and apply an xdelta3 patch using paths containing spaces. Confirm the output boots and no shell interpretation occurs.
37. Leave a dirty level open for over one minute, terminate the process without closing normally, reopen the project, and recover it. Confirm Save clears the snapshot; repeat and choose Discard/No to confirm it is removed.
38. Restart once in each of English, Dutch, Portuguese, Spanish, and French. Confirm the Project Hub and core menu vocabulary change without altering project data.
39. Open representative BNBL, BNCL, BNCD, and ENPG resources from the Filesystem dock. Edit coordinates, a cell/OAM field, pixels, and palette colours; build and boot the affected UI while confirming untouched padding and compressed wrappers survive.
40. Configure `rom.banner`, edit the icon, palette, and every available language title, then build each variant with a banner override. Confirm the output ROM shows the expected icon/title and the pristine input ROM is unchanged.
41. Open **ROM User Metadata** on a ROM whose file ID 131 is `NSMBeUD`, change names/notes, and confirm the save lands in the supplier or active module layer and remains readable after NCPatcher rebuilds.
42. Build one historical project with Fireflower and one with an external NSMBe-compatible wrapper. Confirm a pre-existing `__tmp` is refused, successful temporary workspaces are removed, the live extraction is untouched, and each chosen output ROM boots.
43. Build and serve `nsmb-web`, open a real course/BGDat pair, and confirm its entity counts and schematic canvas match the desktop editor without uploading either file.
44. In the web editor, change a value, add/delete a row, use Undo/Redo, download both serialized files, and reopen them. Confirm edits survive and unsupported/out-of-range values are refused cleanly.
