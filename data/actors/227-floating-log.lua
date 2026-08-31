-- Ported from NSMB Editor 5's NSMBStageObj.cs case 227.

local images = {}

local function init(ctx)
    images.Log = ctx.loadImage("images/Log.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 128
    y = y - 32
    width = 256
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Log, RenderX - 128, RenderY - 32) or drawn
    return drawn
end

return {
    objectId = 227,
    name = "Floating log",
    category = "Platforms and ropes",
    fields = {
        {
            key = "unknown_4",
            label = "Unknown 4",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used as 5 in-game.",
        },
        {
            key = "spin",
            label = "Spin",
            bitOffset = 40,
            bitWidth = 4,
            notes = "it don't need to get a value",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
