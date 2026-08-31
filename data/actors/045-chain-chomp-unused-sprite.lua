-- Ported from NSMB Editor 5's NSMBStageObj.cs case 45.

local images = {}

local function init(ctx)
    images.ChainChomp = ctx.loadImage("images/ChainChomp.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 29, y = actor.pixelY - 28, width = 44, height = 28 }
end

local function render(actor, renderer)
    return renderer:draw(images.ChainChomp, grabBounds(actor))
end

return {
    objectId = 45,
    name = "Chain Chomp (unused sprite)",
    category = "Enemies - Overground",
    notes = "Spawned by Chain Chomp/Log. When spawned via a sprite it can be defeated by ground-pounding its head for some reason.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
