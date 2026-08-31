-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 81,
    name = "Skeeter (Water Bug)",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -16, y = -5, width = 41, height = 21 },
    renderBounds = { x = -16, y = -5, width = 41, height = 21 },
    sourceFiles = {
        "/enemy/bakudan.nsbmd",
        "/enemy/bomb_menbo.nsbmd",
    },
    fields = {
        {
            key = "mobility",
            label = "Mobility",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Moves around" },
                { value = 1, label = "Stays in place" },
            },
        },
    },
    model = readModel("bomb_menbo.nsbmd"),
}
