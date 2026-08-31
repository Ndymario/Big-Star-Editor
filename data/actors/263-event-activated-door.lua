-- Ported from NSMB Editor 5's NSMBStageObj.cs case 263.

local images = {}

local function init(ctx)
    images.Door2 = ctx.loadImage("images/Door.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 32
    width = 32
    height = 48
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Door2, RenderX, RenderY - 32) or drawn
    return drawn
end

return {
    objectId = 263,
    name = "Event Activated Door",
    category = "Event controllers",
    fields = {
        {
            key = "input_id",
            label = "Input ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "dest_area",
            label = "Dest. area",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "dest_entrance",
            label = "Dest. entrance",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
