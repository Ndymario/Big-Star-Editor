-- Imported from NSMB Editor 5 stageobjsettings.xml.
return {
    schema = 1,
    objectId = 254,
    name = "Spin Block",
    category = "Platforms and ropes",
    color = "#4f86c6",
    shape = "box",
    known = true,
    complete = false,
    notes = "Random notes:\n- When the player is fully submerged in water, they can only use the spin block when ducking, and even then, it just propels the swimming player upward - not putting them into \"spinning mode\". When the player reaches the surface (if there is one), they stop going up.",
    grabBounds = { x = -6, y = 0, width = 26, height = 26 },
    renderBounds = { x = -6, y = 0, width = 26, height = 26 },
    sourceFiles = {
        "/enemy/spin_obj.nsbmd (spin_object)",
    },
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Swiches and controllers",
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
        {
            key = "spin_jump",
            label = "Spin Jump",
            bitOffset = 0,
            bitWidth = 4,
            notes = "does not need to have a value to appear",
        },
    },
    model = readModel("spin_obj.nsbmd"),
}
