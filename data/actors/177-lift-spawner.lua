-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 177,
    name = "Lift spawner",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = true,
    grabBounds = { x = 0, y = 16, width = 64, height = 16 },
    renderBounds = { x = 0, y = 16, width = 64, height = 16 },
    sourceFiles = {
        "/enemy/n_lift.nsbtx (normal_lift)",
    },
    fields = {
        {
            key = "offscreen_pos",
            label = "Offscreen pos.",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used in the MvsL level. This controls what the lift (spawner, surely?) does while off screen, best I can tell. Position is affected, but speed stays the same.",
            choices = {
                { value = 0, label = "Keep moving" },
                { value = 1, label = "Wait for Mario" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 16,
            bitWidth = 4,
            notes = "2 (Right) and 3 (Left) are unused.",
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Right" },
                { value = 3, label = "Left" },
            },
        },
    },
    model = readModel("n_lift.nsbtx"),
}
