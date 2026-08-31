-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 197,
    name = "Swinging/able vine",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "Requires BOTH sprite sets \"Vine\" and \"Rope\".\nIf not, it crashes.",
    grabBounds = { x = -16, y = 0, width = 32, height = 64 },
    renderBounds = { x = -16, y = 0, width = 32, height = 64 },
    sourceFiles = {
        "/enemy/W_tuta.nsbmd ([SOMETHING]_[ivy/vine])",
    },
    fields = {
        {
            key = "swing_angle",
            label = "Swing angle",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "dont_restrict_angle",
            label = "Dont restrict angle",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("W_tuta.nsbmd"),
}
