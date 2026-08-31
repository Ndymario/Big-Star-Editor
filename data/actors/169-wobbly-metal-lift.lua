-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 169,
    name = "Wobbly metal lift",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    notes = "Wobbles when the player steps on it. Duh!",
    grabBounds = { x = 0, y = 0, width = 192, height = 32 },
    renderBounds = { x = 0, y = 0, width = 192, height = 32 },
    sourceFiles = {
        "/enemy/W_rift.nsbmd (SOMETHING_LIFT)",
        "possibly: W_kusari.nsbmd ([SOMETHING]_[SOMETHING]) (might be for something else)",
    },
    fields = {
        {
            key = "maximum_tilting",
            label = "Maximum tilting",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 3, label = "7.5°" },
                { value = 2, label = "15°" },
                { value = 1, label = "30°" },
                { value = 0, label = "60°" },
            },
        },
    },
    model = readModel("W_rift.nsbmd"),
}
