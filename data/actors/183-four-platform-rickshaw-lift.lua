-- Ported from NSMB Editor 5's NSMBStageObj.cs case 183.

local images = {}

local function init(ctx)
    images.Spinning4PointedPlatform = ctx.loadImage("images/Spinning4PointedPlatform.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 38, y = actor.pixelY - 29, width = 91, height = 75 }
end

local function render(actor, renderer)
    return renderer:draw(images.Spinning4PointedPlatform, grabBounds(actor))
end

return {
    objectId = 183,
    name = "Four platform rickshaw lift",
    category = "Platforms and ropes",
    fields = {
        {
            key = "stick_on_line",
            label = "Stick on line",
            bitOffset = 20,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "enable_trigger_id",
            label = "Enable Trigger ID",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
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
