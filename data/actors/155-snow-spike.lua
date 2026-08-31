-- Ported from NSMB Editor 5's NSMBStageObj.cs case 155.

local images = {}

local function init(ctx)
    images.SnowballThrower = ctx.loadImage("images/SnowballThrower.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.SnowballThrower, grabBounds(actor))
end

return {
    objectId = 155,
    name = "Snow Spike",
    category = "Enemies - Snow",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
