-- Ported from NSMB Editor 5's NSMBStageObj.cs case 81.

local images = {}

local function init(ctx)
    images.WaterBug = ctx.loadImage("images/WaterBug.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 16
    y = y - 5
    width = 41
    height = 21
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.WaterBug, RenderX - 16, RenderY - 5) or drawn
    return drawn
end

return {
    objectId = 81,
    name = "Skeeter (Water Bug)",
    category = "Enemies - Water",
    fields = {
        {
            key = "mobility",
            label = "Mobility",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Moves around" },
                { value = 1, label = "Stays in place" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
