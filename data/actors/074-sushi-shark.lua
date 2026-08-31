-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 74,
    name = "Sushi (shark)",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = false,
    notes = "Spawned by the Sushi spawner.\n\nThis sprite's settings seem to be an absolute mess of contradicting behaviours. ",
    grabBounds = { x = -27, y = -8, width = 71, height = 26 },
    renderBounds = { x = -27, y = -8, width = 71, height = 26 },
    sourceFiles = {
        "/enemy/hojiro.nsbmd",
    },
    fields = {
        {
            key = "unknown_5",
            label = "Unknown 5",
            bitOffset = 24,
            bitWidth = 4,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 12,
            bitWidth = 4,
            notes = "3 is unused. Requires 11 to be set to 1, 2 or 3.",
        },
        {
            key = "small",
            label = "Small",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Used as 1 by Sushi spawner.",
        },
        {
            key = "movement",
            label = "Movement",
            bitOffset = 0,
            bitWidth = 4,
            notes = "0 is unused. 2 used by Sushi spawner. Used as 3 by the sprite in 1-A.",
            choices = {
                { value = 0, label = "Back & forth" },
            },
        },
    },
    model = readModel("hojiro.nsbmd"),
}
