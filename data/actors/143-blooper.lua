-- Ported from NSMB Editor 5's NSMBStageObj.cs case 143.

local images = {}

local function init(ctx)
    images.Blooper = ctx.loadImage("images/Blooper.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 16, width = 26, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.Blooper, grabBounds(actor))
end

return {
    objectId = 143,
    name = "Blooper",
    category = "Enemies - Water",
    fields = {
        {
            key = "avoids_mario",
            label = "Avoids Mario",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
