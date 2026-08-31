-- Ported from NSMB Editor 5's NSMBStageObj.cs case 111.

local images = {}

local function init(ctx)
    images.Wiggler = ctx.loadImage("images/Wiggler.png")
    images.WigglerL = ctx.loadImage("images/WigglerL.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if actor.fields.bigger then
        y = y - 44
        width = 92
        height = 60
    else
        y = y - 14
        width = 52
        height = 30
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    if d[5] == 1 then
        drawn = gfx.blit(renderer, images.WigglerL, RenderX, RenderY - 44) or drawn
    else
        drawn = gfx.blit(renderer, images.Wiggler, RenderX, RenderY - 14) or drawn
    end
    return drawn
end

return {
    objectId = 111,
    name = "Wiggler",
    category = "Enemies - Overground",
    fields = {
        {
            key = "bigger",
            label = "Bigger",
            bitOffset = 0,
            bitWidth = 4,
            notes = "1 is unused and gives it glitchy animations and hitbox.",
            checkboxValue = 1,
        },
        {
            key = "unknown_4",
            label = "Unknown 4",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used in W4-4",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
