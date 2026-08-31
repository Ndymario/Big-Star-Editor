-- Ported from NSMB Editor 5's NSMBStageObj.cs case 150.

local images = {}

local function init(ctx)
    images.SpikeBass = ctx.loadImage("images/SpikeBass.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 3, y = actor.pixelY - 10, width = 45, height = 53 }
end

local function render(actor, renderer)
    return renderer:draw(images.SpikeBass, grabBounds(actor))
end

return {
    objectId = 150,
    name = "Spike Bass (unused sprite)",
    category = "Enemies - Water",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
