-- Ported from NSMB Editor 5's NSMBStageObj.cs case 110.

local images = {}

local function init(ctx)
    images.ElectricBall = ctx.loadImage("images/ElectricBall.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX - 6, actor.pixelY - 6
    local shift = actor.fields.shift_position
    if shift == 1 or shift == 3 then
        x = x + 8
    end
    if shift == 2 or shift == 3 then
        y = y + 8
    end
    return { x = x, y = y, width = 27, height = 28 }
end

local function render(actor, renderer)
    local bounds = grabBounds(actor)
    return renderer:draw(images.ElectricBall, { x = bounds.x, y = bounds.y })
end

return {
    objectId = 110,
    name = "Amp",
    category = "Enemies - Castle",
    fields = {
        {
            key = "shift_position",
            label = "Shift position",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "½ block right" },
                { value = 2, label = "½ block down" },
                { value = 3, label = "½ block right and down" },
            },
        },
        {
            key = "behind_fence",
            label = "Behind fence",
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
