-- Ported from NSMB Editor 5's NSMBStageObj.cs case 47.

local images = {}

local function init(ctx)
    images.FlipGateSmall = ctx.loadImage("images/FlipGateSmall.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.FlipGateSmall, grabBounds(actor))
end

return {
    objectId = 47,
    name = "Flip fence (duplicate sprite)",
    category = "Useless sprites",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
