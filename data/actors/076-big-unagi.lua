-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 76,
    name = "Big Unagi",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "/enemy/morray_big.nsbmd",
    },
    fields = {
        {
            key = "visible_facing_right",
            label = "Visible facing right",
            bitOffset = 16,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Required, or he will not move.",
            valueOffset = 1,
        },
        {
            key = "visible_facing_left",
            label = "Visible facing left",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("morray_big.nsbmd"),
}
