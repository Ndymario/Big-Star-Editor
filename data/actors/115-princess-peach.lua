-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 115,
    name = "Princess Peach",
    category = "Bosses and cutscenes",
    color = "#b84b4b",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Seems to only say \"Help!\" when paired with the Final Boss Controller.",
    grabBounds = { x = 0, y = -22, width = 29, height = 38 },
    renderBounds = { x = 0, y = -22, width = 29, height = 38 },
    sourceFiles = {
        "/enemy/peach.nsbmd",
    },
    fields = {
        {
            key = "ignore_cinematics",
            label = "Ignore cinematics",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Used in 1-Castle battle, at least",
            checkboxValue = 1,
        },
    },
    model = readModel("peach.nsbmd"),
}
