-- Ported from NSMB Editor 5's NSMBStageObj.cs case 43.

local images = {}

local function init(ctx)
    images.MontyTank = ctx.loadImage("images/MontyTank.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 374
    y = y - 37
    width = 72
    height = 52
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.MontyTank, RenderX - 374, RenderY - 37) or drawn
    return drawn
end

return {
    objectId = 43,
    name = "Monty Tank",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
