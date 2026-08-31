-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 238,
    name = "Banzai Bill",
    category = "Enemies - Overground",
    fields = {
        {
            key = "follow_camera",
            label = "Follow camera",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "fixed_direction",
            label = "Fixed direction",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Requires Direction field set to 1 (Fixed)",
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Auto=Towards the player,Fixed=Specified in fixed direction field",
            choices = {
                { value = 0, label = "Auto" },
                { value = 1, label = "Fixed" },
                { value = 2, label = "Right" },
                { value = 3, label = "Left" },
                { value = 4, label = "Down" },
                { value = 5, label = "Up" },
                { value = 6, label = "Down-left" },
                { value = 7, label = "Down-right" },
                { value = 8, label = "Up-left" },
                { value = 9, label = "Up-right" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
