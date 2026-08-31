-- Ported from NSMB Editor 5's NSMBStageObj.cs case 263.

local images = {}

local function init(ctx)
    images.Door = ctx.loadImage("images/Door.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 32, width = 32, height = 48 }
end

local function render(actor, renderer)
    return renderer:draw(images.Door, grabBounds(actor))
end

return {
    objectId = 263,
    name = "Event Activated Door",
    category = "Event controllers",
    fields = {
        {
            key = "input_id",
            label = "Input ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "dest_area",
            label = "Dest. area",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "dest_entrance",
            label = "Dest. entrance",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
