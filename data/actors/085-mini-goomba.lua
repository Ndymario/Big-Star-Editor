-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 85,
    name = "Mini Goomba",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Unused behaviours:\n- Jumps with the music like goombas do [may be goomba leftover]",
    grabBounds = { x = 0, y = 0, width = 8, height = 8 },
    renderBounds = { x = 0, y = 0, width = 8, height = 8 },
    sourceFiles = {
        "/I_kuribo256_ncg.bin",
        "/I_enemy_ncl.bin",
    },
    fields = {
        {
            key = "shift_block_right",
            label = "Shift ½ block  right",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "shift_block_down",
            label = "Shift ½ block down",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    graphics = readGraphics("I_kuribo256_ncg.bin"),
    palette = readPalette("I_enemy_ncl.bin"),
}
