-- Ported from the NSMBe Signboard plugin (Objects/Signboard.cs), as a check
-- that a module actor can express what a compiled NSMBe plugin could.

local images = {}

local function init(ctx)
    for i = 1, 10 do images["Arrow" .. i] = ctx.loadImage("images/Arrow" .. i .. ".png") end
    for i = 1, 32 do images["Pipe" .. i] = ctx.loadImage("images/Pipe" .. i .. ".png") end
    images.PipeJoint = ctx.loadImage("images/PipeJoint.png")
end

-- The plugin shifts by half a block before growing the box upward, so the
-- shifts apply at the smaller scale too.
local function grabBounds(actor)
    local smaller = actor.fields.smaller
    local size = smaller and 16 or 32
    local x = actor.pixelX - (actor.fields.shift_left and (smaller and 4 or 8) or 0) - (size - 16) // 2
    local y = actor.pixelY + (actor.fields.shift_down and 8 or 0) - (size - 16)
    return { x = x, y = y, width = size, height = size }
end

local function render(actor, renderer)
    local model = actor.fields.model_type
    local name
    if actor.fields.pipe_mode then
        name = "Pipe" .. model + 1
    elseif model <= 9 then
        name = "Arrow" .. model + 1
    else
        name = "PipeJoint"
    end
    return renderer:draw(images[name], grabBounds(actor))
end

return {
    objectId = 77,
    name = "Arrow signboard",
    category = "Decoration",
    fields = {
        { key = "pipe_mode", label = "Pipe mode", bitOffset = 28, bitWidth = 4, checkboxValue = 1 },
        { key = "model_type", label = "Model type", bitOffset = 24, bitWidth = 4 },
        { key = "smaller", label = "Smaller", bitOffset = 16, bitWidth = 4, checkboxValue = 1 },
        { key = "shift_left", label = "Shift ½ block left", bitOffset = 12, bitWidth = 4, checkboxValue = 13 },
        { key = "shift_down", label = "Shift ½ block down", bitOffset = 8, bitWidth = 4, checkboxValue = 14 },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
