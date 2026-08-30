# M15 UI/UX manual checklist

Use a real, extracted NSMB project with at least two configured NCPatcher
variants. No ROM or game asset is packaged with Big Star.

1. Open a project and a level. Click a sprite, an object, a view and a zone in
   turn without touching any toolbar. Confirm each click selects the thing under
   the cursor and that the Properties panel switches to its tab by itself.
2. Select a view, then a zone, then a multi-tile object. Confirm eight resize
   knobs appear on each, that the cursor changes per edge, and that dragging the
   top or left knobs moves that edge rather than the opposite one.
3. Open the Palette dock. Confirm one Objects sub-tab per loaded tileset slot,
   that a pipe or other multi-tile object previews whole rather than as one
   corner, and that hovering names the object and its slot. Pick it, right-click
   the canvas, and confirm it is placed.
4. Switch the Properties panel to Entrances, then right-click the canvas.
   Confirm an entrance is created — the panel's tab is what a right-click makes.
5. Switch variant on the toolbar. Confirm open levels close, the level reopens
   from the new layer, and its tileset graphics come from that layer. Confirm
   the Filesystem dock's list and its "Save to module" target follow.
6. Open an NCGR from the Filesystem dock and choose its NCLR in **Palette file**.
   Confirm the real artwork replaces the grey ramp. Repeat for a headerless
   `_ncg.bin` + `_ncl.bin` pair, setting **Width** by hand, and confirm the
   choice is remembered when the file is reopened.
7. Open an NSCR and an NCER, pair each with that graphics and palette, and
   confirm a composed tilemap and a drawn cell rather than coloured identifier
   blocks and outlines.
8. With an editor tab open, confirm the rest of the shell still works: switch
   tabs, edit the level, run the patcher. Nothing should be blocked behind it.
9. Open a BNBL and the ROM user metadata. Confirm both appear in the same
   graphics editor rather than a second one, and that Hex edit also opens a tab.
10. Run a build. Confirm one Output dock carries progress, diagnostics and log
    with no duplicated lines.
