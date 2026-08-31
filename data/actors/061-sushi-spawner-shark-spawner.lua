-- Ported from NSMB Editor 5's NSMBStageObj.cs case 61.

local images = {}

local function init(ctx)
    images.Infinity = ctx.loadImage("images/Infinity.png")
    images.Shark = ctx.loadImage("images/Shark.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 27
    y = y - 8
    width = 71
    height = 26
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Shark, RenderX - 27, RenderY - 8) or drawn
    drawn = gfx.blit(renderer, images.Infinity, RenderX - 4, RenderY + 1) or drawn
    return drawn
end

return {
    objectId = 61,
    name = "Sushi spawner (shark spawner)",
    category = "Enemies - Water",
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
