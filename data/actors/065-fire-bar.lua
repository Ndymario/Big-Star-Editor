-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 65,
    name = "Fire Bar",
    category = "Enemies - Fire",
    color = "#c8563f",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 12, height = 12 },
    renderBounds = { x = 0, y = 0, width = 12, height = 12 },
    sourceFiles = {
        "/enemy/firebar1.nsbmd",
    },
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "enable_second_bar",
            label = "Enable second bar",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "radius",
            label = "Radius",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "faster",
            label = "Faster",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Completes a turn exactly 4/3 times as fast if checked.",
            checkboxValue = 1,
        },
    },
    model = readModel("firebar1.nsbmd"),
}
