-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 63,
    name = "Scuttlebug spawner",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -8, width = 46, height = 24 },
    renderBounds = { x = 0, y = -8, width = 46, height = 24 },
    sourceFiles = {
        "/enemy/gasagoso.nsbmd",
    },
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
    },
    model = readModel("gasagoso.nsbmd"),
}
