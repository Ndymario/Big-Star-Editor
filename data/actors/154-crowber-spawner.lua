-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 154,
    name = "Crowber spawner",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 22, height = 14 },
    renderBounds = { x = 0, y = 0, width = 22, height = 14 },
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Crobers will spawn when Mario is inside this zone.",
        },
    },
}
