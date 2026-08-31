-- Ported from NSMB Editor 5's NSMBStageObj.cs case 118.

local images = {}

local function init(ctx)
    images.MoneyBag = ctx.loadImage("images/MoneyBag.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 5, width = 23, height = 21 }
end

local function render(actor, renderer)
    return renderer:draw(images.MoneyBag, grabBounds(actor))
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
