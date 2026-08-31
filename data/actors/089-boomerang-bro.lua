-- Ported from NSMB Editor 5's NSMBStageObj.cs case 89.

local images = {}

local function init(ctx)
    images.BoomerangBro = ctx.loadImage("images/BoomerangBro.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 18
    width = 18
    height = 34
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BoomerangBro, RenderX, RenderY - 18) or drawn
    return drawn
end

return {
    objectId = 89,
    name = "Boomerang Bro.",
    category = "Enemies - Overground",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
