-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 281,
    name = "Poisoned Water",
    category = "Effects and decorations",
    notes = "See Water",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "unknown_6",
            label = "Unknown 6",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Nybbles 6 and 7 don't seem to have an effect. But they are used ingame. Nybbles 4 and 5 could still do something.",
        },
        {
            key = "unknown_7",
            label = "Unknown 7",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "tiles_to_rise",
            label = "Tiles to Rise",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Modulo 128. If lower than 128 then the poisoned water will rise, if greater or equal than 128 the poisoned water will lower.",
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
