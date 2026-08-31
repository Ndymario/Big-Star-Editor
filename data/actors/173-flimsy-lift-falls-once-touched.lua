-- Ported from NSMB Editor 5's NSMBStageObj.cs case 173.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    width = d[5] % 0x8
    local s = 2
    if actor.objectId == 174 then
        width = d[5] % 0x10
        width = math.max(0, width - 1)
        s = 1
        x = x - 8
    end
    x = x - ((width * s + 1) * 8)
    width = (width * s + 2) * 16
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    local w = d[5] % 0x8
    local s = 2
    if actor.objectId == 174 then
        w = d[5] % 0x10
        w = math.max(0, w - 1)
        s = 1
        RenderX = RenderX - 8
    end
    RenderX = RenderX - ((w * s + 1) * 8)
    drawn = gfx.blit(renderer, images.MovingPlatformLeft, RenderX, RenderY) or drawn
    for l = 0, (w * s) - 1 do
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, images.MovingPlatformSection, RenderX, RenderY) or drawn
    end
    drawn = gfx.blit(renderer, images.MovingPlatformRight, RenderX + 16, RenderY) or drawn
    return drawn
end

return {
    objectId = 173,
    name = "Flimsy Lift (falls once touched)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Length formula is 2+2×(N11 mod 4). Everything other than 1 is unused. Certain lengths seem to cause it to shift upwards.",
            valueOffset = 1,
        },
        {
            key = "length_2",
            label = "length",
            bitOffset = 40,
            bitWidth = 4,
            notes = "do not exceed 4 or it will not be displayed",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
