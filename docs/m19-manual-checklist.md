# M19 graphics editors manual checklist

Use a real NSMB ROM or NCPatcher project. No ROM or game asset is packaged with Big Star. The aim is that someone used
to NSMB Editor finds every step where they expect it.

1. Open a level, then **Tools > Tilesets and Backgrounds**. On **Graphics**, paint with the left button and the right
   button and confirm each paints its own colour from the palette on the right. Ctrl+click a pixel and confirm the
   colour under it becomes the left colour.
2. Use fill, line, rectangle (with and without Shift) and the selection: copy, paste under the pointer, flip and
   delete. Confirm Ctrl+Z undoes each as one step, including a long brush stroke.
3. Change a palette colour with the sliders and with the hex field, then undo it. Choose **Palette 1** in the
   Palettes list and confirm the sheet redraws in it and the swatches on the right are that palette's. Choose the
   Background in the Bitmaps list and confirm its graphics show.
4. Click **Export this bitmap with all palettes…**, edit the PNG in another program (keep its size), and bring it back
   with **Import this bitmap with all palettes…**. Confirm the sheet and both palettes look as edited in every palette,
   and that one Ctrl+Z undoes the import. Try **Import this bitmap with existing palette…** and **…and recreate
   palette…** too.
5. On **Map16**, drag a 2x2 rectangle in the tile picker and paint a block with it. Pick a tile from slot 0 in the
   picker, paint a quadrant of the level tileset with it, and confirm it draws as the level does. Right-click a block
   on the sheet and confirm its tiles become the brush. Paint a single quadrant in the block inspector.
6. Shift-drag over several blocks and confirm the pattern repeats over the rectangle. Use Flip X/Y and the palette tool
   over a region.
7. On **Objects**, choose a cell and click blocks in the picker: each replaces a cell and moves on. Shift+click inserts
   instead. Alt+click and Shift+click a cell to toggle the repeat outlines. Drag on the preview to size it and confirm
   the object repeats as it would in a level. Add, duplicate, copy, paste, move and delete objects.
8. On **Behaviors**, drag over several blocks, tick a flag and confirm every chosen block has it.
9. On **Background**, pick tiles and paint the layer; zoom with Ctrl+wheel and pan with the middle button. Save, reopen
   the level and confirm the background renders as painted.
10. From the Filesystem dock, open an NCGR that has an NCLR of the same name beside it and confirm it opens drawn in
    that palette. Detach it, close and reopen the NCGR, and confirm it stays detached. Then open an NCLR from another folder and choose **Attach to** the NCGR
    in the question that comes up; confirm the sheet is drawn in it. Drag a second palette file from the tree onto
    the Palettes list and confirm both files are listed with their rows, the new one shown; choose a row of the first
    and confirm the sheet redraws in it. Export with all palettes and confirm the PNG holds every row of both. Detach
    one with the button beside its name and confirm the other stays. Change a colour and confirm Save writes that
    NCLR as well. Close and reopen the NCGR and confirm the same palettes come back, the same one shown. Open an NSCR
    that has an NCGR and NCLR of its name beside it and confirm it opens with both attached. Open one that has none,
    drag an NCGR onto its **Graphics file** row and two NCLRs onto its palette list, switch between them, and paint
    it from the tile picker. Save both and confirm the module holds the edits. Open an NCLR once more and choose
    **Open on its own**.
11. Open the ROM banner and paint the icon with the same tools; confirm undo works there too.
