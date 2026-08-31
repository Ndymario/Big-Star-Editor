-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 210,
    name = "Rotating carry-through-wall turntable-like lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -80, y = 0, width = 96, height = 16 },
    renderBounds = { x = -80, y = 0, width = 96, height = 16 },
    sourceFiles = {
        "/enemy/W_turntable.nsbmd",
    },
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "From left to right" },
                { value = 1, label = "From right to left" },
            },
        },
    },
    model = readModel("W_turntable.nsbmd"),
}
