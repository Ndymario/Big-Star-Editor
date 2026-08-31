-- Ported from NSMB Editor 5's NSMBStageObj.cs case 88.

local images = {}

local function init(ctx)
    images.FireBro = ctx.loadImage("images/FireBro.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 15
    width = 18
    height = 31
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.FireBro, RenderX, RenderY - 15) or drawn
    return drawn
end

return {
    objectId = 88,
    name = "Fire Bro.",
    category = "Enemies - Overground",
    notes = "This enemy is similar to the Hammer Bro., but throws fire balls",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
