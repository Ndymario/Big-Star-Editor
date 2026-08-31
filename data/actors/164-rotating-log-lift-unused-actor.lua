-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 164,
    name = "Rotating log lift (unused actor)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -24, y = 0, width = 64, height = 16 },
    renderBounds = { x = -24, y = 0, width = 64, height = 16 },
    sourceFiles = {
        "/enemy/r_lift_L.nsbmd (rotate_lift_Large)",
        "/enemy/r_lift_S.nsbmd (rotate_lift_Small)",
    },
    fields = {
        {
            key = "longer",
            label = "Longer",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("r_lift_L.nsbmd"),
}
