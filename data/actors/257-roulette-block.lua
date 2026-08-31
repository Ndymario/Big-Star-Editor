-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 257,
    name = "Roulette Block",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 20, height = 20 },
    renderBounds = { x = 0, y = 0, width = 20, height = 20 },
    sourceFiles = {
        "/BG_ncl/d_2d_A_J_jyotyu_B_ncl.bin",
        "/BG_ncl/d_2d_A_J_jyotyu_ncl.bin",
        "/BG_ncl/d_2d_A_J_jyotyu_R_ncl.bin",
        "/BG_ncl/d_2d_A_J_jyotyu_W_ncl.bin",
        "/obj/A_block_hahen_ncg.bin",
    },
    fields = {
        {
            key = "extra_item",
            label = "Extra item",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Higher than 1 is unused.",
            choices = {
                { value = 0, label = "Blue Shell" },
                { value = 1, label = "Mini Mushroom" },
            },
        },
    },
    graphics = readGraphics("A_block_hahen_ncg.bin"),
    palette = readPalette("d_2d_A_J_jyotyu_B_ncl.bin"),
}
