-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 125,
    name = "Arrow sign",
    category = "Effects and decorations",
    color = "#5b9b86",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 32, height = 32 },
    renderBounds = { x = 0, y = 0, width = 32, height = 32 },
    sourceFiles = {
        "/obj/I_dokan_yajirushi_ncg.bin",
    },
    fields = {
        {
            key = "z_order",
            label = "Z order",
            bitOffset = 28,
            bitWidth = 4,
            notes = "0, 2 and 3 are unused.",
            choices = {
                { value = 0, label = "Model" },
                { value = 1, label = "Tileset" },
                { value = 2, label = "Foreground" },
                { value = 3, label = "Background" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Up/right" },
                { value = 2, label = "Right" },
                { value = 3, label = "Down/right" },
                { value = 4, label = "Down" },
                { value = 5, label = "Down/left" },
                { value = 6, label = "Left" },
                { value = 7, label = "Up/left" },
            },
        },
    },
}
