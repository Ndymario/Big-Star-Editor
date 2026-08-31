-- Ported from NSMB Editor 5's NSMBStageObj.cs case 128.

local images = {}

local function init(ctx)
    images.MummiPokey = ctx.loadImage("images/MummiPokey.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 256, y = actor.pixelY + 16, width = 512, height = 95 }
end

local function render(actor, renderer)
    return renderer:draw(images.MummiPokey, grabBounds(actor))
end

return {
    objectId = 128,
    name = "Mummipokey",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
