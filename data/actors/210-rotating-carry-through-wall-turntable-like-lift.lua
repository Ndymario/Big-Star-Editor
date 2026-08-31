-- Ported from NSMB Editor 5's NSMBStageObj.cs case 210.

local images = {}

local function init(ctx)
    images.ThruWallPlatform = ctx.loadImage("images/ThruWallPlatform.png")
end

local function grabBounds(actor)
    if actor.fields.direction == 0 then
        return { x = actor.pixelX - 80, y = actor.pixelY, width = 96, height = 16 }
    end
    return { x = actor.pixelX, y = actor.pixelY, width = 96, height = 16 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    box.flipX = actor.fields.direction ~= 0
    return renderer:draw(images.ThruWallPlatform, box)
end

return {
    objectId = 210,
    name = "Rotating carry-through-wall turntable-like lift",
    category = "Platforms and ropes",
    fields = {
        {
            key = "direction",
            label = "Direction",
            bitOffset = 20,
            bitWidth = 4,
            choices = {
                { value = 0, label = "From left to right" },
                { value = 1, label = "From right to left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
