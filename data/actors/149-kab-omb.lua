-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 149,
    name = "Kab-omb",
    category = "Enemies - Fire",
    color = "#c8563f",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = -3, width = 16, height = 19 },
    renderBounds = { x = 0, y = -3, width = 16, height = 19 },
    sourceFiles = {
        "/enemy/hanabi.nsbmd",
    },
    fields = {
        {
            key = "chases_mario",
            label = "Chases Mario",
            bitOffset = 0,
            bitWidth = 4,
            notes = "This makes him chase Mario if hit by a fireball/Raining Debris. If not set, he will run left and right. 0 (unchecked) is unused.",
            checkboxValue = 1,
        },
    },
    model = readModel("hanabi.nsbmd"),
}
