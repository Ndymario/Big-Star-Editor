-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 175,
    name = "Three platform rickshaw lift (rotates in place by self)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -28, y = -34, width = 92, height = 85 },
    renderBounds = { x = -28, y = -34, width = 92, height = 85 },
    sourceFiles = {
        "/enemy/kuruma3_L.nsbmd [wheel/rickshaw_3_Large)",
        "/enemy/kuruma3_S.nsbmd (wheel/rickshaw_3_Small)",
        "/enemy/kuruma_lift.nsbtx (wheel/rickshaw_lift)",
    },
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "longer_steps",
            label = "Longer steps",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    model = readModel("kuruma3_L.nsbmd"),
}
