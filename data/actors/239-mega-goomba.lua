-- Ported from NSMB Editor 5's NSMBStageObj.cs case 239.

local images = {}

local function init(ctx)
    images.MegaGoomba = ctx.loadImage("images/MegaGoomba.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    x = x + 73
    y = y - 8
    height = 19
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.MegaGoomba, RenderX + 73, RenderY - 8) or drawn
    return drawn
end

return {
    objectId = 239,
    name = "Mega Goomba",
    category = "Enemies - Overground",
    notes = "This is a boss used in World 4.",
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
