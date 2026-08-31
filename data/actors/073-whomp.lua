-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 73,
    name = "Whomp",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Seems to have unused ceiling detection.",
    grabBounds = { x = -29, y = -57, width = 78, height = 73 },
    renderBounds = { x = -29, y = -57, width = 78, height = 73 },
    sourceFiles = {
        "/enemy/battan.nsbmd",
    },
    fields = {
        {
            key = "doesn_t_move",
            label = "Doesn't move",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "bigger",
            label = "Bigger",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("battan.nsbmd"),
}
