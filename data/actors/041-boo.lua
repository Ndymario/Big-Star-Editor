-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 41,
    name = "Boo",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "This sprite will freeze and remain frozen indefinitely if placed in a view with music that includes a \"bahp\" note, such as music IDs 1A and 63.",
    grabBounds = { x = 0, y = 0, width = 24, height = 20 },
    renderBounds = { x = 0, y = 0, width = 24, height = 20 },
    sourceFiles = {
        "/obj/I_teresa256_ncg.bin",
        "/obj/I_enemy_B_ncl.bin",
        "/obj/I_enemy_W_ncl.bin",
        "/obj/I_enemy_R_ncl.bin",
        "/obj/I_enemy_ncl.bin",
    },
    fields = {
        {
            key = "faster",
            label = "Faster",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    graphics = readGraphics("I_teresa256_ncg.bin"),
    palette = readPalette("I_enemy_B_ncl.bin"),
}
