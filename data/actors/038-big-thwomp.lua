-- Ported from NSMB Editor 5's NSMBStageObj.cs case 38.

local images = {}

local function init(ctx)
    images.GiantThwomp = ctx.loadImage("images/GiantThwomp.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 64
    height = 74
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GiantThwomp, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 38,
    name = "Big Thwomp",
    category = "Enemies - Castle",
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused. Cancelled out by the code, as far as I noticed.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
