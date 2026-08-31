-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 277,
    name = "Scroll and Mario stop sideways",
    category = "Camera controllers",
    notes = "Remember, the DS screen is 16 tiles wide. Make sure you allow for the full width of the screen or you camera will scroll shaky. Using X coordinates, numerically these should be 15 tiles apart.\n\nThis sprite, along with 198/199, doesn't always work. Try moving 276s up or down if they don't work.\n\nSprite must be placed at the bottom of the limit. Height is counted from the sprite position up.\n\nWhen using retro scroll horizontal, you have to select \"stop from scrolling left\".\n\nThese can be passed through using a connected pipe.\n",
    fields = {
        {
            key = "retro_scroll",
            label = "Retro scroll",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "Horizontal " },
                { value = 2, label = "Vertical" },
            },
        },
        {
            key = "tile_height_up",
            label = "Tile height up",
            bitOffset = 4,
            bitWidth = 8,
            notes = "You can calculate by map Y coordinates.",
            valueOffset = 1,
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Stop from scrolling right" },
                { value = 1, label = "Stop from scrolling left" },
            },
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
