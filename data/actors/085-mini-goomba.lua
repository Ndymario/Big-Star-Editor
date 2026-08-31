-- Ported from NSMB Editor 5's NSMBStageObj.cs case 85.

local images = {}

local function init(ctx)
    images.MiniGoomba = ctx.loadImage("images/MiniGoomba.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    if actor.fields.shift_block_right then
        x = x + 8
    end
    if actor.fields.shift_block_down then
        y = y + 8
    end
    return { x = x, y = y, width = 8, height = 8 }
end

local function render(actor, renderer)
    local bounds = grabBounds(actor)
    return renderer:draw(images.MiniGoomba, { x = bounds.x, y = bounds.y })
end

return {
    objectId = 85,
    name = "Mini Goomba",
    category = "Enemies - Overground",
    notes = "Unused behaviours:\n- Jumps with the music like goombas do [may be goomba leftover]",
    fields = {
        {
            key = "shift_block_right",
            label = "Shift ½ block  right",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "shift_block_down",
            label = "Shift ½ block down",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
