-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 38,
    name = "Big Thwomp",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = false,
    grabBounds = { x = 0, y = 0, width = 64, height = 74 },
    renderBounds = { x = 0, y = 0, width = 64, height = 74 },
    sourceFiles = {
        "/enemy/dossun.nsbmd",
        "/enemy/dossun.nsbtp",
    },
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused. Cancelled out by the code, as far as I noticed.",
        },
    },
    model = readModel("dossun.nsbmd"),
}
