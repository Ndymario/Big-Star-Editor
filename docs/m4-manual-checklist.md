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
