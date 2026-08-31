-- Ported from NSMB Editor 5's NSMBStageObj.cs case 170.

local images = {}

local function init(ctx)
    images.TiltingRock = ctx.loadImage("images/TiltingRock.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 8
    width = 64
    height = 40
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.TiltingRock, RenderX, RenderY - 8) or drawn
    return drawn
end

return {
    objectId = 170,
    name = "Wobble Rock",
    category = "Platforms and ropes",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
