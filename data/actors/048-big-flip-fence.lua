-- Ported from NSMB Editor 5's NSMBStageObj.cs case 48.

local images = {}

local function init(ctx)
    images.FlipGateLarge = ctx.loadImage("images/FlipGateLarge.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 192, height = 64 }
end

local function render(actor, renderer)
    return renderer:draw(images.FlipGateLarge, grabBounds(actor))
end

return {
    objectId = 48,
    name = "Big flip fence",
    category = "Platforms and ropes",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
