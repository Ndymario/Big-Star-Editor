-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 226,
    name = "Whirlpool",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -78, y = -164, width = 165, height = 182 },
    renderBounds = { x = -78, y = -164, width = 165, height = 182 },
    sourceFiles = {
        "/enemy/waterland_uzu.nsbmd (Dire Dire Docks_whirlpool) [ported from SM64DS]",
    },
    fields = {
        {
            key = "invisible_when_inactive",
            label = "Invisible when inactive",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Yes" },
            },
        },
    },
    model = readModel("waterland_uzu.nsbmd"),
}
