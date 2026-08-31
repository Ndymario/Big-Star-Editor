-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 28,
    name = "Cheep Cheep",
    category = "Enemies - Water",
    color = "#3f7fc4",
    shape = "ellipse",
    known = true,
    complete = false,
    notes = "There are more values that are used by the Cheep Cheep spawner.",
    grabBounds = { x = -1, y = 4, width = 19, height = 18 },
    renderBounds = { x = -1, y = 4, width = 19, height = 18 },
    sourceFiles = {
        "/enemy/pukupuku.nsbmd",
    },
    fields = {
        {
            key = "slower",
            label = "Slower",
            bitOffset = 12,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "type",
            label = "Type",
            bitOffset = 0,
            bitWidth = 4,
            notes = "'Swims' on walls: makes fish follow the ground and swim up walls and on ceilings.",
            choices = {
                { value = 0, label = "Swim" },
                { value = 1, label = "Green (chases Mario)" },
                { value = 2, label = "Jumping (unused)" },
                { value = 3, label = "'Swims' on walls (unused)" },
            },
        },
        {
            key = "wall_placement",
            label = "Wall placement",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Only works with 'swims on walls' type.",
            choices = {
                { value = 0, label = "Top" },
                { value = 1, label = "Left" },
                { value = 2, label = "Below" },
                { value = 3, label = "Right" },
            },
        },
    },
    model = readModel("pukupuku.nsbmd"),
}
