-- Ported from NSMB Editor 5's NSMBStageObj.cs case 48.

local images = {}

local function init(ctx)
    images.FlipGateLarge = ctx.loadImage("images/FlipGateLarge.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 192
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FlipGateLarge, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 48,
    name = "Big flip fence",
    category = "Platforms and ropes",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
