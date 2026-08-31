-- Ported from NSMB Editor 5's NSMBStageObj.cs case 150.

local images = {}

local function init(ctx)
    images.SpikeBass = ctx.loadImage("images/SpikeBass.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 3
    y = y - 10
    width = 45
    height = 53
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SpikeBass, RenderX - 3, RenderY - 10) or drawn
    return drawn
end

return {
    objectId = 150,
    name = "Spike Bass (unused sprite)",
    category = "Enemies - Water",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
