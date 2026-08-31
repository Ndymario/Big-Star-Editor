-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 110,
    name = "Amp",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -6, y = -6, width = 27, height = 28 },
    renderBounds = { x = -6, y = -6, width = 27, height = 28 },
    sourceFiles = {
        "/enemy/bilikyu.nsbmd",
        "/enemy/elec.nsbmd",
    },
    fields = {
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "½ block right" },
                { value = 2, label = "½ block down" },
                { value = 3, label = "½ block right and down" },
            },
        },
        {
            key = "behind_fence",
            label = "Behind fence",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("bilikyu.nsbmd"),
}
