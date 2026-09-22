-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 142,
    name = "Event Controller Sequencer",
    category = "Event controllers",
    notes = "Trigger event: the sequencer starts running when this event ID is triggered.\nTarget event: the sequencer triggers every event from the starter ID up to this one. If 0, it triggers every single event from the starter ID onward. This may need testing, though.",
    fields = {
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
