-- Ported from NSMB Editor 5's NSMBStageObj.cs case 159.

local images = {}

local function init(ctx)
    images.MidpointFlag = ctx.loadImage("images/MidpointFlag.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 22, height = 33 }
end

local function render(actor, renderer)
    return renderer:draw(images.MidpointFlag, grabBounds(actor))
end

return {
    objectId = 159,
    name = "Checkpoint",
    category = "Other",
    notes = "The midway point. The ID of the Start position needs to be configured in the View's settings.",
    fields = {
        {
            key = "unknown_11",
            label = "Unknown 11",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
