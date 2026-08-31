-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 69,
    name = "Hammer Bro. from map",
    category = "Enemies - Overground",
    color = "#b5651d",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "If a Hammer Bro is on the level this actor is placed in, upon starting the level a Hammer Bro. will spawn there.",
    grabBounds = { x = 0, y = -18, width = 18, height = 34 },
    renderBounds = { x = 0, y = -18, width = 18, height = 34 },
    sourceFiles = {
        "/enemy/hammer_bros.nsbmd",
        "/enemy/I_hanmmer.nsbmd",
    },
    fields = {
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level Start" },
                { value = 1, label = "Midway point" },
            },
        },
        {
            key = "can_spawn_mega_mushroom",
            label = "Can spawn Mega Mushroom",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "disable_downward_jump",
            label = "Disable downward jump",
            bitOffset = 8,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    model = readModel("hammer_bros.nsbmd"),
}
