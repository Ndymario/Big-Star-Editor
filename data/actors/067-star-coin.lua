-- Ported from NSMB Editor 5's NSMBStageObj.cs case 67.

local images = {}

local function init(ctx)
    images.StarCoin = ctx.loadImage("images/StarCoin.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    if actor.fields.shift_block_right then
        x = x + 8
    end
    if actor.fields.shift_block_down then
        y = y + 8
    end
    return { x = x, y = y, width = 32, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.StarCoin, grabBounds(actor))
end

return {
    objectId = 67,
    name = "Star Coin",
    category = "Event controllers",
    fields = {
        {
            key = "target_id",
            label = "Target ID",
            bitOffset = 40,
            bitWidth = 8,
        },
        {
            key = "unknown_2_3",
            label = "Unknown 2-3",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Defunct Target ID? Used in some levels uselessly.",
        },
        {
            key = "non_timed_event",
            label = "Non-timed event",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "unknown_7",
            label = "Unknown 7",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "behind_fence",
            label = "Behind fence",
            bitOffset = 12,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "number",
            label = "Number",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Star Coin-1" },
                { value = 1, label = "Star Coin-2" },
                { value = 2, label = "Star Coin-3" },
            },
        },
        {
            key = "shift_block_right",
            label = "Shift ½ block right",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "shift_block_down",
            label = "Shift ½ block down",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
