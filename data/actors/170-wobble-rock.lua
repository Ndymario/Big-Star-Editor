-- Ported from NSMB Editor 5's NSMBStageObj.cs case 170.

local images = {}

local function init(ctx)
    images.TiltingRock = ctx.loadImage("images/TiltingRock.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 8, width = 64, height = 40 }
end

local function render(actor, renderer)
    return renderer:draw(images.TiltingRock, grabBounds(actor))
end

return {
    objectId = 170,
    name = "Wobble Rock",
    category = "Platforms and ropes",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
