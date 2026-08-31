-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 33,
    name = "MvsL Battle Star (Big Star)",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 32, height = 32 },
    renderBounds = { x = 0, y = 0, width = 32, height = 32 },
    sourceFiles = {
        "/obj/I_star_red_ncg.bin",
        "/obj/I_enemy256_ncl.bin",
    },
    fields = {
        {
            key = "star_id",
            label = "Star ID",
            bitOffset = 0,
            bitWidth = 8,
            notes = "These must be the same number of stars as in the original level. Each star must have a unique ID, starting from 1. If not, the game will hang. Also, if the ID is set to 0, it doesn't spawn, but the game doesn't crash (as seen in SMB 1-1)",
        },
    },
    graphics = readGraphics("I_star_red_ncg.bin"),
    palette = readPalette("I_enemy256_ncl.bin"),
}
