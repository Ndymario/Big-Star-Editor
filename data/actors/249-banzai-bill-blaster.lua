-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 249,
    name = "Banzai Bill Blaster",
    category = "Enemies - Castle",
    color = "#535967",
    shape = "ellipse",
    known = true,
    complete = true,
    notes = "Sprite set: 5-1",
    grabBounds = { x = -32, y = -64, width = 64, height = 80 },
    renderBounds = { x = -32, y = -64, width = 64, height = 80 },
    sourceFiles = {
        "/enemy/mag_killer.nsbmd",
        "/enemy/magnum_battery.nsbmd",
        "/enemy/magnum_battery2.nsbmd",
    },
    fields = {
        {
            key = "shooting_direction",
            label = "Shooting direction",
            bitOffset = 8,
            bitWidth = 4,
            notes = "1 and 2 are unused.",
            choices = {
                { value = 1, label = "Left" },
                { value = 2, label = "Right" },
                { value = 0, label = "Both" },
            },
        },
        {
            key = "unknown_11",
            label = "Unknown (11)",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Stored by the game for ulterior use but seems to never be called, still needs testing.",
        },
    },
    model = readModel("mag_killer.nsbmd"),
}
