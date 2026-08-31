-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 215,
    name = "Haunted lift",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = -64, y = -8, width = 128, height = 16 },
    renderBounds = { x = -64, y = -8, width = 128, height = 16 },
    sourceFiles = {
        "/enemy/savage_lift.nsbmd",
    },
    fields = {
        {
            key = "y_shift_pixels",
            label = "Y-Shift (pixels)",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    model = readModel("savage_lift.nsbmd"),
}
