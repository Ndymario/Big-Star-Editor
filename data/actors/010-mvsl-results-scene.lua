-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 10,
    name = "MvsL Results Scene",
    category = "Uncategorized",
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
