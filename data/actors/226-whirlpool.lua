-- Ported from NSMB Editor 5's NSMBStageObj.cs case 226.

local images = {}

local function init(ctx)
    images.WhirlPool = ctx.loadImage("images/WhirlPool.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 78
    y = y - 164
    width = 165
    height = 182
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.WhirlPool, RenderX - 78, RenderY - 164) or drawn
    return drawn
end

return {
    objectId = 226,
    name = "Whirlpool",
    category = "Enemies - Water",
    fields = {
        {
            key = "invisible_when_inactive",
            label = "Invisible when inactive",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Yes" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
