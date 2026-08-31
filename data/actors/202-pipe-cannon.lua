-- Ported from NSMB Editor 5's NSMBStageObj.cs case 202.

local images = {}

local geometry = {
    [0] = { image = "PipeRight", x = 1, y = -49, width = 55, height = 65 },
    [1] = { image = "PipeLeft", x = -23, y = -49, width = 55, height = 65 },
    [2] = { image = "PipeShortRight", x = 1, y = -50, width = 40, height = 66 },
    [3] = { image = "PipeShortLeft", x = -8, y = -50, width = 40, height = 66 },
    [5] = { image = "PipeLongRight", x = 1, y = -47, width = 57, height = 63 },
}
local defaultGeometry = { image = "Pipe", x = 0, y = -48, width = 32, height = 64 }

local function init(ctx)
    images.Pipe = ctx.loadImage("images/Pipe.png")
    images.PipeLeft = ctx.loadImage("images/PipeLeft.png")
    images.PipeLongRight = ctx.loadImage("images/PipeLongRight.png")
    images.PipeRight = ctx.loadImage("images/PipeRight.png")
    images.PipeShortLeft = ctx.loadImage("images/PipeShortLeft.png")
    images.PipeShortRight = ctx.loadImage("images/PipeShortRight.png")
end

local function grabBounds(actor)
    local g = geometry[actor.fields.angle_force] or defaultGeometry
    return {
        x = actor.pixelX + g.x,
        y = actor.pixelY + g.y,
        width = g.width,
        height = g.height,
    }
end

local function render(actor, renderer)
    local g = geometry[actor.fields.angle_force] or defaultGeometry
    return renderer:draw(images[g.image], grabBounds(actor))
end

return {
    objectId = 202,
    name = "Pipe Cannon",
    category = "Platforms and ropes",
    fields = {
        {
            key = "angle_force",
            label = "Angle / Force",
            bitOffset = 0,
            bitWidth = 3,
            notes = "1 is unused.",
            choices = {
                { value = 0, label = "Far right" },
                { value = 1, label = "Far left" },
                { value = 2, label = "Medium right" },
                { value = 3, label = "Medium left" },
                { value = 4, label = "Straight up" },
                { value = 5, label = "Very far right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
