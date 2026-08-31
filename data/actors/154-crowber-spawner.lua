-- Ported from NSMB Editor 5's NSMBStageObj.cs case 154.

local images = {}

local function init(ctx)
    images.Crow = ctx.loadImage("images/Crow.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 22
    height = 14
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Crow, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 154,
    name = "Crowber spawner",
    category = "Enemies - Ghost House",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Crobers will spawn when Mario is inside this zone.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
