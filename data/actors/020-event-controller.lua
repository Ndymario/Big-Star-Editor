-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 20,
    name = "Event Controller",
    category = "Event controllers",
    notes = "Activates events upon loading",
    fields = {
        {
            key = "duration",
            label = "Duration",
            bitOffset = 24,
            bitWidth = 8,
            notes = "The number of ticks the event will stay active",
        },
        {
            key = "mode",
            label = "Mode",
            bitOffset = 4,
            bitWidth = 1,
            choices = {
                { value = 1, label = "Activate" },
                { value = 0, label = "Deactivate" },
            },
        },
        {
            key = "disable_actor",
            label = "Disable actor",
            bitOffset = 0,
            bitWidth = 1,
            notes = "Destroys the actor immediately upon creation",
            checkboxValue = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
