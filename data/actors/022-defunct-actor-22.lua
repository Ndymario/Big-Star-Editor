-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 22,
    name = "Defunct Actor 22",
    category = "Useless sprites",
    notes = "All it has are the destructor and constructor. So completely useless.",
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
