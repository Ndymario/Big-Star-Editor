-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 142,
    name = "Event Controller Sequencer",
    category = "Event controllers",
    fields = {
        {
            key = "starter_id",
            label = "Starter ID",
            bitOffset = 40,
            bitWidth = 8,
            notes = "The activator will start running when this event ID is triggered.",
        },
        {
            key = "finisher_id",
            label = "Finisher ID",
            bitOffset = 32,
            bitWidth = 8,
            notes = "The activator will trigger all events from the start event ID to this event ID. If 0, the sprite will trigger every single event from the starting from the starter ID. This may need testing, though.",
        },
        {
            key = "delay",
            label = "Delay",
            bitOffset = 24,
            bitWidth = 8,
            notes = "1 unit = 10 frames",
        },
        {
            key = "mode",
            label = "Mode",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Single" },
                { value = 1, label = "Multi" },
                { value = 2, label = "Deactivate on end" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
