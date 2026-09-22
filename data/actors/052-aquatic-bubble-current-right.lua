-- Ported from NSMB Editor 5's NSMBStageObj.cs case 52.

local images = {}

local function init(ctx)
    images.TubeBubbles = ctx.loadImage("images/TubeBubbles.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 32, y = actor.pixelY, width = 74, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.TubeBubbles, {
        x = actor.pixelX + 53,
        y = actor.pixelY - 21,
        width = 32,
        height = 74,
        rotation = 90,
    })
end

return {
    objectId = 52,
    name = "Aquatic bubble current (right)",
    category = "Enemies - Water",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
