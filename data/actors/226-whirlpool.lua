-- Ported from NSMB Editor 5's NSMBStageObj.cs case 226.

local images = {}

local function init(ctx)
    images.WhirlPool = ctx.loadImage("images/WhirlPool.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 78, y = actor.pixelY - 164, width = 165, height = 182 }
end

local function render(actor, renderer)
    return renderer:draw(images.WhirlPool, grabBounds(actor))
end

return {
    objectId = 226,
    name = "Whirlpool",
    category = "Enemies - Water",
    fields = {
        {
            key = "invisible_when_inactive",
            label = "Invisible when inactive",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "No" },
                { value = 1, label = "Yes" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
