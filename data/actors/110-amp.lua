-- Ported from NSMB Editor 5's NSMBStageObj.cs case 110.

local images = {}

local function init(ctx)
    images.ElectricBall = ctx.loadImage("images/ElectricBall.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 6
    y = y - 6
    local shift = actor.fields.shift_position
    if shift == 1 or shift == 3 then
        x = x + 8
    end
    if shift == 2 or shift == 3 then
        y = y + 8
    end
    height = 28
    width = 27
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local shift = actor.fields.shift_position
    if shift == 1 or shift == 3 then
        RenderX = RenderX + 8
    end
    if shift == 2 or shift == 3 then
        RenderY = RenderY + 8
    end
    drawn = gfx.blit(renderer, images.ElectricBall, RenderX - 6, RenderY - 6) or drawn
    return drawn
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
