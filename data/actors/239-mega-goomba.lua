-- Ported from NSMB Editor 5's NSMBStageObj.cs case 239.

local images = {}

local function init(ctx)
    images.MegaGoomba = ctx.loadImage("images/MegaGoomba.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 73, y = actor.pixelY - 8, width = 16, height = 19 }
end

local function render(actor, renderer)
    return renderer:draw(images.MegaGoomba, grabBounds(actor))
end

return {
    objectId = 239,
    name = "Mega Goomba",
    category = "Enemies - Overground",
    notes = "This is a boss used in World 4.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
