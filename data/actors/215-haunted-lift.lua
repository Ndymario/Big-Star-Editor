-- Ported from NSMB Editor 5's NSMBStageObj.cs case 215.

local images = {}

local function init(ctx)
    images.GhostPlatform = ctx.loadImage("images/GhostPlatform.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 64,
        y = actor.pixelY - 8 + actor.fields.y_shift_pixels,
        width = 128,
        height = 16,
    }
end

local function render(actor, renderer)
    return renderer:draw(images.GhostPlatform, grabBounds(actor))
end

return {
    objectId = 215,
    name = "Haunted lift",
    category = "Enemies - Ghost House",
    fields = {
        {
            key = "y_shift_pixels",
            label = "Y-Shift (pixels)",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
