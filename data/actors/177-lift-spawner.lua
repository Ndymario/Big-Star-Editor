-- Ported from NSMB Editor 5's NSMBStageObj.cs case 177.

local images = {}

local function init(ctx)
    images.MovingPlatformLeft = ctx.loadImage("images/MovingPlatformLeft.png")
    images.MovingPlatformRight = ctx.loadImage("images/MovingPlatformRight.png")
    images.MovingPlatformSection = ctx.loadImage("images/MovingPlatformSection.png")
    images.arrow_down = ctx.loadImage("images/arrow_down.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width = 16
    if d[3] % 0x4 == 1 then
        y = y - 16
    elseif d[3] % 0x4 == 2 then
        x = x - 64
    elseif d[3] % 0x4 == 3 then
        x = x + 64
    else
        y = y + 16
    end
    width = 64
    return { x = x, y = y, width = width, height = 16 }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local img
    local drawn = false
    img = images.arrow_down
    local arrowLiftX = RenderX2
    if d[3] % 0x4 == 1 then
        RenderY = RenderY - 16
        arrowLiftX = arrowLiftX + 24
    elseif d[3] % 0x4 == 2 then
        RenderX = RenderX - 64
        img = gfx.transform(img, 270, false, false)
    elseif d[3] % 0x4 == 3 then
        RenderX = RenderX + 64
        arrowLiftX = arrowLiftX + 48
        img = gfx.transform(img, 90, false, false)
    else
        RenderY = RenderY + 16
        arrowLiftX = arrowLiftX + 24
        img = gfx.transform(img, 180, false, false)
    end
    drawn = gfx.blit(renderer, images.MovingPlatformLeft, RenderX, RenderY) or drawn
    drawn = gfx.blit(renderer, images.MovingPlatformSection, RenderX + 16, RenderY) or drawn
    drawn = gfx.blit(renderer, images.MovingPlatformSection, RenderX + 32, RenderY) or drawn
    drawn = gfx.blit(renderer, images.MovingPlatformRight, RenderX + 48, RenderY) or drawn
    drawn = gfx.blit(renderer, img, arrowLiftX, RenderY2, 16, 16) or drawn
    return drawn
end

return {
    objectId = 177,
    name = "Lift spawner",
    category = "Platforms and ropes",
    fields = {
        {
            key = "offscreen_pos",
            label = "Offscreen pos.",
            bitOffset = 28,
            bitWidth = 4,
            notes = "Used in the MvsL level. This controls what the lift (spawner, surely?) does while off screen, best I can tell. Position is affected, but speed stays the same.",
            choices = {
                { value = 0, label = "Keep moving" },
                { value = 1, label = "Wait for Mario" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 16,
            bitWidth = 4,
            notes = "2 (Right) and 3 (Left) are unused.",
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Right" },
                { value = 3, label = "Left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
