-- Ported from NSMB Editor 5's NSMBStageObj.cs case 63.

local images = {}

local function init(ctx)
    images.ScuttleBug = ctx.loadImage("images/ScuttleBug.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 8
    width = 46
    height = 24
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.ScuttleBug, RenderX, RenderY - 8) or drawn
    return drawn
end

return {
    objectId = 63,
    name = "Scuttlebug spawner",
    category = "Other",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
