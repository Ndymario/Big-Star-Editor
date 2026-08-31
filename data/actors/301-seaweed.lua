-- Ported from NSMB Editor 5's NSMBStageObj.cs case 301.

local images = {}

local function init(ctx)
    images.SeaWeed = ctx.loadImage("images/SeaWeed.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 32, y = actor.pixelY - 127, width = 90, height = 147 }
end

local function render(actor, renderer)
    return renderer:draw(images.SeaWeed, grabBounds(actor))
end

return {
    objectId = 301,
    name = "Seaweed",
    category = "Effects and decorations",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
