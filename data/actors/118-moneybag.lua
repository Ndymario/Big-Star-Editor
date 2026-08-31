-- Ported from NSMB Editor 5's NSMBStageObj.cs case 118.

local images = {}

local function init(ctx)
    images.MoneyBag = ctx.loadImage("images/MoneyBag.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 5
    width = 23
    height = 21
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.MoneyBag, RenderX, RenderY - 5) or drawn
    return drawn
end

return {
    objectId = 118,
    name = "Moneybag",
    category = "Enemies - Overground",
    notes = "A greedy bag that hides a lot of coins. Kill it to receive a 1UP. But it takes multiple hits to do so!",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
