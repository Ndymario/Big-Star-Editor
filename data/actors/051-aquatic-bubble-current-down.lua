-- Ported from NSMB Editor 5's NSMBStageObj.cs case 51.

local images = {}

local function init(ctx)
    images.TubeBubbles = ctx.loadImage("images/TubeBubbles.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY + 32, width = 32, height = 74 }
end

local function render(actor, renderer)
    local bounds = grabBounds(actor)
    bounds.flipY = true
    return renderer:draw(images.TubeBubbles, bounds)
end

return {
    objectId = 51,
    name = "Aquatic bubble current (down)",
    category = "Enemies - Water",
    notes = "Setting both input IDs to zero will make the bubbles always run.",
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
