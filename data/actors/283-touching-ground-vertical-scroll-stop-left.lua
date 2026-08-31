-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 283,
    name = "Touching ground vertical scroll stop left ",
    category = "Camera controllers",
    notes = "This sprite seems to disable all other camera sprites when activated.\n\nIt also changed horizontal scrolling...I think.",
    fields = {
        {
            key = "type",
            label = "Type",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Bottom limit" },
                { value = 1, label = "Top limit" },
            },
        },
        {
            key = "visible_y_pix_past",
            label = "Visible Y pix. past",
            bitOffset = 0,
            bitWidth = 8,
            notes = "Pixels able to be seen past limit.",
        },
        {
            key = "disable_y_pix_past",
            label = "Disable Y pix. past",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Can only be set to a max of 31 pixels. Past that move the sprite.",
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
