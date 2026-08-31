-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 107,
    name = "Flame Chomp spawner",
    category = "Enemies - Fire",
    color = "#c8563f",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Spawns a Flame Chomp with actor data of: 00 00 00 00 00 00",
    grabBounds = { x = -1, y = -33, width = 53, height = 43 },
    renderBounds = { x = -1, y = -33, width = 53, height = 43 },
    sourceFiles = {
        "/enemy/keronpa.nsbmd",
        "/enemy/keronpa_body.nsbmd",
    },
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
    },
    model = readModel("keronpa.nsbmd"),
}
