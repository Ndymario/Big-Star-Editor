-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 239,
    name = "Mega Goomba",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "This is a boss used in World 4.",
    grabBounds = { x = 73, y = -8, width = 16, height = 19 },
    renderBounds = { x = 73, y = -8, width = 16, height = 19 },
    sourceFiles = {
        "/enemy/kuribo.nsbmd",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    model = readModel("kuribo.nsbmd"),
}
