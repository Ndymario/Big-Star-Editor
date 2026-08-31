-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 208,
    name = "Floating barrel",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -16, y = -8, width = 48, height = 48 },
    renderBounds = { x = -16, y = -8, width = 48, height = 48 },
    sourceFiles = {
        "/enemy/barrel.nsbmd",
    },
    fields = {
        {
            key = "starts_a_bit_submerged",
            label = "Starts a bit submerged",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("barrel.nsbmd"),
}
