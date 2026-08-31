-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 104,
    name = "Event controller - activates Spiked Ball when touched by Snake Block",
    category = "Other",
    notes = "Activates a Spiked Ball when a Snake Block pass over it.",
    fields = {
        {
            key = "spiked_ball_id",
            label = "Spiked Ball ID",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
