-- Ported from NSMB Editor 5's NSMBStageObj.cs case 227.

local images = {}

local function init(ctx)
    images.Log = ctx.loadImage("images/Log.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 128, y = actor.pixelY - 32, width = 256, height = 64 }
end

local function render(actor, renderer)
    return renderer:draw(images.Log, grabBounds(actor))
end

return {
    objectId = 227,
    name = "Floating log",
    category = "Platforms and ropes",
    fields = {
        {
            key = "unknown_4",
            label = "Unknown 4",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used as 5 in-game.",
        },
        {
            key = "spin",
            label = "Spin",
            bitOffset = 40,
            bitWidth = 4,
            notes = "it don't need to get a value",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
