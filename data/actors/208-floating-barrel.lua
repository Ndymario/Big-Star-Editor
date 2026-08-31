-- Ported from NSMB Editor 5's NSMBStageObj.cs case 208.

local images = {}

local function init(ctx)
    images.Barrel = ctx.loadImage("images/Barrel.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 16
    y = y - 8
    width = 48
    height = 48
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Barrel, RenderX - 16, RenderY - 8) or drawn
    return drawn
end

return {
    objectId = 208,
    name = "Floating barrel",
    category = "Platforms and ropes",
    fields = {
        {
            key = "starts_a_bit_submerged",
            label = "Starts a bit submerged",
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
