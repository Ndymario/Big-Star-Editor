-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 193,
    name = "Blue Mushroom Platform (Expanding/contracting)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -8, y = 0, width = 32, height = 32 },
    renderBounds = { x = -8, y = 0, width = 32, height = 32 },
    sourceFiles = {
        "/enemy/I_kinoko_ashib_y.nsbtx",
        "/enemy/I_kinoko_kuki_y.nsbtx",
        "/enemy/I_kinoko_kuki_yl.nsbtx",
    },
    fields = {
        {
            key = "short_when_expanded",
            label = "Short when Expanded",
            bitOffset = 8,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "start_expanded",
            label = "Start Expanded",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "stalk_height",
            label = "Stalk Height",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    model = readModel("I_kinoko_ashib_y.nsbtx"),
}
