-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 261,
    name = "Map Flying ? Block",
    category = "Other",
    color = "#eeB24b",
    shape = "diamond",
    known = true,
    complete = true,
    notes = "Moves slowly if in water.",
    grabBounds = { x = -12, y = -8, width = 44, height = 23 },
    renderBounds = { x = -12, y = -8, width = 44, height = 23 },
    sourceFiles = {
        "/enemy/pata_block.nsbmd",
        "This NSBMD contains red and gold flying blocks + wings",
        "sprites:",
        "117 and 290",
    },
    fields = {
        {
            key = "loads_from",
            label = "Loads from",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Level start" },
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
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Left" },
                { value = 1, label = "Right" },
            },
        },
        {
            key = "looping_pattern",
            label = "Looping pattern",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Fly off screen" },
                { value = 1, label = "Center of Loop" },
                { value = 2, label = "Edge of Loop" },
            },
        },
        {
            key = "flying_pattern",
            label = "Flying pattern",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Back and forth" },
                { value = 1, label = "Forward 4/back 8" },
                { value = 2, label = "Stays in place" },
            },
        },
    },
    model = readModel("pata_block.nsbmd"),
}
