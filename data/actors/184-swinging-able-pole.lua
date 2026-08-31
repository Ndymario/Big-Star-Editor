-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 184,
    name = "Swinging/able pole",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = -11, y = -11, width = 22, height = 75 },
    renderBounds = { x = -11, y = -11, width = 22, height = 75 },
    sourceFiles = {
        "/enemy/W_negi.nsbmd",
        "/enemy/W_bou.nsbtx",
    },
    fields = {
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Height is 4+(N11 mod 8).",
            valueOffset = 4,
        },
        {
            key = "sticks_on_line",
            label = "Sticks on line",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "starting_direction",
            label = "Starting direction",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
    },
    model = readModel("W_negi.nsbmd"),
}
