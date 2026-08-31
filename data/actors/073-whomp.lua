-- Ported from NSMB Editor 5's NSMBStageObj.cs case 73.

local images = {}

local function init(ctx)
    images.Whomp = ctx.loadImage("images/Whomp.png")
    images.WhompL = ctx.loadImage("images/WhompL.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.bigger then
        x = x - 38
        y = y - 74
        width = 96
        height = 90
    else
        x = x - 29
        y = y - 57
        width = 78
        height = 73
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if actor.fields.bigger then
        img = images.WhompL
        RenderX = RenderX - 38
        RenderY = RenderY - 74
    else
        img = images.Whomp
        RenderX = RenderX - 29
        RenderY = RenderY - 57
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 73,
    name = "Whomp",
    category = "Enemies - Castle",
    notes = "Seems to have unused ceiling detection.",
    fields = {
        {
            key = "doesn_t_move",
            label = "Doesn't move",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "bigger",
            label = "Bigger",
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
