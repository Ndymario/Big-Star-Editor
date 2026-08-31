-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 183,
    name = "Four platform rickshaw lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -38, y = -29, width = 91, height = 75 },
    renderBounds = { x = -38, y = -29, width = 91, height = 75 },
    sourceFiles = {
        "/enemy/kuruma4.nsbmd",
        "/enemy/kuruma_lift.nsbtx",
    },
    fields = {
        {
            key = "stick_on_line",
            label = "Stick on line",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "enable_trigger_id",
            label = "Enable Trigger ID",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    model = readModel("kuruma4.nsbmd"),
}
