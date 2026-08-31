-- Ported from NSMB Editor 5's NSMBStageObj.cs case 108.

local images = {}

local function init(ctx)
    images.FlameChomp = ctx.loadImage("images/FlameChomp.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 1, y = actor.pixelY - 33, width = 53, height = 43 }
end

local function render(actor, renderer)
    return renderer:draw(images.FlameChomp, grabBounds(actor))
end

return {
    objectId = 108,
    name = "-",
    category = "Uncategorized",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
