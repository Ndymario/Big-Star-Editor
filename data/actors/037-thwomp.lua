-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 37,
    name = "Thwomp",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = false,
    grabBounds = { x = 0, y = 0, width = 32, height = 36 },
    renderBounds = { x = 0, y = 0, width = 32, height = 36 },
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
            notes = "Cancelled out by the code, as far as I noticed.",
        },
    },
    model = readModel("dossun.nsbmd"),
}
