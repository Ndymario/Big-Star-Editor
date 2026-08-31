-- Ported from NSMB Editor 5's NSMBStageObj.cs case 88.

local images = {}

local function init(ctx)
    images.FireBro = ctx.loadImage("images/FireBro.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 15, width = 18, height = 31 }
end

local function render(actor, renderer)
    return renderer:draw(images.FireBro, grabBounds(actor))
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
