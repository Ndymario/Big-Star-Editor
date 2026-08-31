-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 227,
    name = "Floating log",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = false,
    grabBounds = { x = -128, y = -32, width = 256, height = 64 },
    renderBounds = { x = -128, y = -32, width = 256, height = 64 },
    sourceFiles = {
        "/enemy/W_maruta.nsbmd",
    },
    fields = {
        {
            key = "unknown_4",
            label = "Unknown 4",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used as 5 in-game.",
        },
        {
            key = "spin",
            label = "Spin",
            bitOffset = 40,
            bitWidth = 4,
            notes = "it don't need to get a value",
        },
    },
    model = readModel("W_maruta.nsbmd"),
}
