-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 279,
    name = "Lava",
    category = "Effects and decorations",
    notes = "See Water\n\nWithout either input ID, the lava will rise automatically. If you set a value for either one of the IDs by itself, the lava will rise to the height you select when that ID is activated. ID 2 (the Target event row) will only make the lava rise, and will wait for an off command from the ID 2 activator again. If you set values for both IDs, ID 1 (the Trigger event row) will make the lava go up then down.",
    fields = {
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
            label = "Tiles to rise",
            bitOffset = 8,
            bitWidth = 8,
        },
        {
            key = "speed_to_rise",
            label = "Speed to rise",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Modulo 128. If lower than 128 then the lava will rise, if greater or equal than 128 the lava will lower.",
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
