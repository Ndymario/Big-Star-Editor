-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 234,
    name = "Burner (Small)",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 64, height = 16 },
    renderBounds = { x = 0, y = 0, width = 64, height = 16 },
    sourceFiles = {
        "/enemy/burner1x3.nsbmd",
        "/enemy/fire_cannon.nsbmd",
    },
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
                { value = 2, label = "Up" },
                { value = 3, label = "Down" },
            },
        },
    },
    model = readModel("burner1x3.nsbmd"),
}
