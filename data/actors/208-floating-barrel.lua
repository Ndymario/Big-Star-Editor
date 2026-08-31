-- Ported from NSMB Editor 5's NSMBStageObj.cs case 208.

local images = {}

local function init(ctx)
    images.Barrel = ctx.loadImage("images/Barrel.png")
end

local function grabBounds(actor)
    -- The upstream selection box is one pixel smaller than the still.
    return { x = actor.pixelX - 16, y = actor.pixelY - 8, width = 48, height = 48 }
end

local function render(actor, renderer)
    return renderer:draw(images.Barrel, { x = actor.pixelX - 16, y = actor.pixelY - 8 })
end

return {
    objectId = 208,
    name = "Floating barrel",
    category = "Platforms and ropes",
    fields = {
        {
            key = "starts_a_bit_submerged",
            label = "Starts a bit submerged",
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
