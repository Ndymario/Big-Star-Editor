-- Ported from NSMB Editor 5's NSMBStageObj.cs case 89.

local images = {}

local function init(ctx)
    images.BoomerangBro = ctx.loadImage("images/BoomerangBro.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 18, width = 18, height = 34 }
end

local function render(actor, renderer)
    return renderer:draw(images.BoomerangBro, grabBounds(actor))
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
