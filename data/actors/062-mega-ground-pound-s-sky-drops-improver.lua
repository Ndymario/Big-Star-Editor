-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 62,
    name = "Mega Ground Pound's sky drops improver",
    category = "Uncategorized",
    notes = "Only increase the CHANCE to drop Goombas when Groundpounding.",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
