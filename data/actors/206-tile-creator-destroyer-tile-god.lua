-- Imported from NSMB Editor 5 stageobjsettings.xml.
local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 16, height = 16 }
end


return {
    objectId = 206,
    name = "Tile Creator/Destroyer (Tile God)",
    category = "Event controllers",
    notes = "To change what this can spawn, swap tiles around in map16. Can be used to make very intresting things, like a Star Coin select screen.",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 4,
            bitWidth = 4,
            notes = "0 and 1 are the same: 1 block.",
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Ditto.",
        },
        {
            key = "block_type",
            label = "Block type",
            bitOffset = 12,
            bitWidth = 4,
            notes = "If you destroy a tile with 3 (blue coin), it will destroy the tile with no shards. (8 through 14 are duplicates of previous options.))",
            choices = {
                { value = 0, label = "Red block" },
                { value = 1, label = "Brick block" },
                { value = 2, label = "Blue coin" },
                { value = 3, label = "Stone block" },
                { value = 4, label = "Wood block" },
                { value = 5, label = "Nothing" },
                { value = 6, label = "Right facing red pipe top edge" },
                { value = 7, label = "Broken red pipe top right edge" },
                { value = 15, label = "Used block" },
            },
        },
        {
            key = "pattern",
            label = "Pattern",
            bitOffset = 16,
            bitWidth = 4,
            notes = "All other than 0 (solid) are unused.",
            choices = {
                { value = 0, label = "Solid" },
                { value = 1, label = "Checkerboard" },
                { value = 2, label = "Reverse checkerboard" },
            },
        },
        {
            key = "effect_action",
            label = "Effect/action",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Off/destroy first" },
                { value = 1, label = "On/create first" },
            },
        },
        {
            key = "permanent",
            label = "Permanent",
            bitOffset = 28,
            bitWidth = 4,
            notes = "If this is checked it will not react to the event turning off.",
            checkboxValue = 1,
        },
    },
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
