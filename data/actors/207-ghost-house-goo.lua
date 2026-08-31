-- Ported from NSMB Editor 5's NSMBStageObj.cs case 207.

local images = {}

local function init(ctx)
    images.GhostGoo = ctx.loadImage("images/GhostGoo.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 32, width = 256, height = 48 }
end

local function render(actor, renderer)
    return renderer:draw(images.GhostGoo, grabBounds(actor))
end

return {
    objectId = 207,
    name = "Ghost house goo",
    category = "Enemies - Ghost House",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
