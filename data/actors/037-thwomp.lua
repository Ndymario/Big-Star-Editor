-- Ported from NSMB Editor 5's NSMBStageObj.cs case 37.

local images = {}

local function init(ctx)
    images.Thwomp = ctx.loadImage("images/Thwomp.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 36 }
end

local function render(actor, renderer)
    return renderer:draw(images.Thwomp, grabBounds(actor))
end

return {
    objectId = 37,
    name = "Thwomp",
    category = "Enemies - Castle",
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Cancelled out by the code, as far as I noticed.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
