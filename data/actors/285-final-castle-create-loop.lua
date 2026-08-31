-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 285,
    name = "Final Castle Create Loop",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    fields = {
        {
            key = "id",
            label = "ID",
            bitOffset = 0,
            bitWidth = 8,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Actual width is found by reversing the nybbles and subtracting 15",
        },
    },
}
