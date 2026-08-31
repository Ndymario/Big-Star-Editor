-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 255,
    name = "Spin Block (unused Actor 255)",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = false,
    notes = "Not complete because: Unknown values, unknown use\nWon't appear if Sprite 95 is not in immediate vicinity.",
    grabBounds = { x = -6, y = 0, width = 26, height = 26 },
    renderBounds = { x = -6, y = 0, width = 26, height = 26 },
    sourceFiles = {
        "/enemy/spin_obj.nsbmd",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "unknown_8",
            label = "Unknown 8",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Used in-game. Doesn't do anything notable.",
        },
        {
            key = "unknown_9",
            label = "Unknown 9",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Used in-game. Doesn't do anything notable.",
        },
    },
    model = readModel("spin_obj.nsbmd"),
}
