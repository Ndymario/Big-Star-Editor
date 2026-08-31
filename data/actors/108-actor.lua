-- Ported from NSMB Editor 5's NSMBStageObj.cs case 108.

local images = {}

local function init(ctx)
    images.FlameChomp = ctx.loadImage("images/FlameChomp.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 1
    y = y - 33
    width = 53
    height = 43
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FlameChomp, RenderX - 1, RenderY - 33) or drawn
    return drawn
end

return {
    objectId = 108,
    name = "-",
    category = "Uncategorized",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
