-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 278,
    name = "Water",
    category = "Effects and decorations",
    notes = "There is no way to make water only appear in a zone; like in NSMBW, but you can emulate this with the unused water tiles (which has its limitations) or by quickly rising the water when you enter an area and lowering it when you exit it (with the event controllers).",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
            notes = "On ID 2 the water will rise, but have to be activated again with a stop command to lower.",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
            notes = "On ID 1 the water will rise then lower. (This is wrong, Hiccup thinks[E version])",
        },
        {
            key = "water_lowering_s_delay",
            label = "Water lowering's delay",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Any value other than 0 will override both Input IDs.",
        },
        {
            key = "water_rising_s_delay",
            label = "Water rising's delay",
            bitOffset = 24,
            bitWidth = 4,
            notes = "Any value other than 0 will override both Input IDs.",
        },
        {
            key = "opacity",
            label = "Opacity",
            bitOffset = 16,
            bitWidth = 5,
        },
        {
            key = "tiles_to_move",
            label = "Tiles to move",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Modulo 128. If lower than 128 then the water will rise, if greater or equal than 128 the water will lower. Lowering water does not work properly. If you want lowering water, rise it with a enter-view event, then \"unrise\" it or get the code hack that fixes the problem.",
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
