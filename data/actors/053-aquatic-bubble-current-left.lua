-- Ported from NSMB Editor 5's NSMBStageObj.cs case 53.

local images = {}

local function init(ctx)
    images.TubeBubbles = ctx.loadImage("images/TubeBubbles.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 90, y = actor.pixelY, width = 74, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.TubeBubbles, {
        x = actor.pixelX - 69,
        y = actor.pixelY - 21,
        width = 32,
        height = 74,
        rotation = 270,
    })
end

return {
    objectId = 53,
    name = "Aquatic bubble current (left)",
    category = "Enemies - Water",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
