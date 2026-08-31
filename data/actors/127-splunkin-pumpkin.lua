-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 127,
    name = "Splunkin (pumpkin)",
    category = "Enemies - Ghost House",
    color = "#7b5aa6",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "A walking pumpkin.",
    grabBounds = { x = 0, y = -2, width = 18, height = 18 },
    renderBounds = { x = 0, y = -2, width = 18, height = 18 },
    sourceFiles = {
        "/enemy/kabochan.nsbmd",
    },
    fields = {
        {
            key = "explodes_at_start",
            label = "Explodes at start",
            bitOffset = 0,
            bitWidth = 4,
            notes = "At spawn. Unused.",
            checkboxValue = 1,
        },
    },
    model = readModel("kabochan.nsbmd"),
}
