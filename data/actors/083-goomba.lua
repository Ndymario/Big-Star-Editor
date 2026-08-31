-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 83,
    name = "Goomba",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "this is the classic enemy that we can find it in all Super Mario games.",
    grabBounds = { x = 0, y = -2, width = 16, height = 18 },
    renderBounds = { x = 0, y = -2, width = 16, height = 18 },
    sourceFiles = {
        "/obj/I_kuribo256_ncg.bin",
    },
    fields = {
        {
            key = "spawn_type",
            label = "Spawn Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Makes the Goomba spawn differently. 1 used for Mega Mario ground pound. 2 used for Enemy Spawner.",
            choices = {
                { value = 0, label = "Normal" },
                { value = 1, label = "Spinning" },
                { value = 2, label = "Rising" },
            },
        },
    },
    graphics = readGraphics("I_kuribo256_ncg.bin"),
    palette = readPalette("I_enemy_ncl.bin"),
}
