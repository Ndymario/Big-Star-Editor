-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 64,
    name = "Enemy-in-Pipe Generator",
    category = "Uncategorized",
    color = "#8b8f96",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "Uses sound set 8 for Bob-ombs",
    grabBounds = { x = 0, y = 0, width = 32, height = 32 },
    renderBounds = { x = 0, y = 0, width = 32, height = 32 },
    fields = {
        {
            key = "enemy",
            label = "Enemy",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Goomba" },
                { value = 1, label = "Bob-omb" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Left" },
                { value = 3, label = "Right" },
            },
        },
        {
            key = "slower_spawn_rate",
            label = "Slower Spawn Rate",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Unused (From NSMBW)",
            checkboxValue = 1,
        },
    },
}
