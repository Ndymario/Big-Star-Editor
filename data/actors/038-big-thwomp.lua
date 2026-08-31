-- Ported from NSMB Editor 5's NSMBStageObj.cs case 38.

local images = {}

local function init(ctx)
    images.GiantThwomp = ctx.loadImage("images/GiantThwomp.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 64, height = 74 }
end

local function render(actor, renderer)
    return renderer:draw(images.GiantThwomp, grabBounds(actor))
end

return {
    objectId = 38,
    name = "Big Thwomp",
    category = "Enemies - Castle",
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused. Cancelled out by the code, as far as I noticed.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
