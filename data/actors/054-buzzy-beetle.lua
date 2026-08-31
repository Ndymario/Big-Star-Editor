-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 54,
    name = "Buzzy Beetle",
    category = "Enemies - Underground",
    color = "#686d76",
    shape = "ellipse",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 0, width = 16, height = 16 },
    renderBounds = { x = 0, y = 0, width = 16, height = 16 },
    sourceFiles = {
        "enemy/met_body.nsbmd",
        "enemy/met_shell.nsbmd",
    },
    fields = {
        {
            key = "spawn_as",
            label = "Spawn as",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Default" },
                { value = 1, label = "Ceiling walker" },
                { value = 2, label = "Shell" },
                { value = 3, label = "Upside-down shell" },
            },
        },
    },
    model = readModel("met_body.nsbmd"),
}
