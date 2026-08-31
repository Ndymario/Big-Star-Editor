-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 270,
    name = "Camera vertical scrolling (unused actor)",
    category = "Camera controllers",
    notes = "controlls if a view can scroll up or down, similar to the option within a view's menu?",
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
