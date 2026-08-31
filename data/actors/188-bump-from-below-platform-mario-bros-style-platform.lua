-- Ported from NSMB Editor 5's NSMBStageObj.cs case 188.

local images = {}

local function init(ctx)
    images.BumpPlatformLeft = ctx.loadImage("images/BumpPlatformLeft.png")
    images.BumpPlatformRight = ctx.loadImage("images/BumpPlatformRight.png")
    images.BumpPlatformSection = ctx.loadImage("images/BumpPlatformSection.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - (d[5] * 8 + 6)
    width = 16 * (d[5] - 1) + 28
    height = 10
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    RenderX = RenderX - (d[5] * 8 + 6)
    drawn = gfx.blit(renderer, images.BumpPlatformLeft, RenderX, RenderY) or drawn
    RenderX = RenderX + 14
    for l = 0, (d[5] - 1) - 1 do
        drawn = gfx.blit(renderer, images.BumpPlatformSection, RenderX, RenderY) or drawn
        RenderX = RenderX + 16
    end
    if d[5] > 0 then
        drawn = gfx.blit(renderer, images.BumpPlatformRight, RenderX, RenderY) or drawn
    end
    return drawn
end

return {
    objectId = 188,
    name = "Bump from below platform (Mario Bros. style platform)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
