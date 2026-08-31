-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 224,
    name = "Giant Wiggler",
    category = "Enemies - Overground",
    notes = "Todo: test path values and other nybbles",
    fields = {
        {
            key = "path_id",
            label = "Path ID",
            bitOffset = 0,
            bitWidth = 4,
            notes = "make paths straight and to a view edge block, or the sprite will not appear",
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
