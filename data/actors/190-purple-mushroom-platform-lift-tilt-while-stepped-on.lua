-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 190,
    name = "Purple Mushroom Platform lift (tilt while stepped on)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = false,
    grabBounds = { x = -16, y = 0, width = 64, height = 112 },
    renderBounds = { x = -16, y = 0, width = 64, height = 112 },
    sourceFiles = {
        "/enemy/I_kinoko_ashib_s.nsbtx",
    },
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "unknown_value_11",
            label = "Unknown value 11",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    model = readModel("I_kinoko_ashib_s.nsbtx"),
}
