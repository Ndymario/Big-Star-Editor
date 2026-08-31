-- Ported from NSMB Editor 5's NSMBStageObj.cs case 155.

local images = {}

local function init(ctx)
    images.SnowballThrower = ctx.loadImage("images/SnowballThrower.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 32
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SnowballThrower, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 155,
    name = "Snow Spike",
    category = "Enemies - Snow",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
