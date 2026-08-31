-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 46,
    name = "Fire Snake (unused Actor 46)",
    category = "Enemies - Fire",
    notes = "Unknown difference to used Fire Snake.",
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
