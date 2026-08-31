-- Ported from NSMB Editor 5's NSMBStageObj.cs case 175.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.Spinning3PointedLargePlatform = ctx.loadImage("images/Spinning3PointedLargePlatform.png")
    images.Spinning3PointedPlatform = ctx.loadImage("images/Spinning3PointedPlatform.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    if d[5] % 0x2 == 1 then
        x = x - 41
        y = y - 41
        width = 121
        height = 99
    else
        x = x - 28
        y = y - 34
        width = 92
        height = 85
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local img
    local drawn = false
    img = images.Spinning3PointedPlatform
    local ThreeLiftX = RenderX - 28
    local ThreeLiftY = RenderY - 34
    if d[5] % 0x2 == 1 then
        img = images.Spinning3PointedLargePlatform
        ThreeLiftX = RenderX - 41
        ThreeLiftY = RenderY - 41
    end
    drawn = gfx.blit(renderer, img, ThreeLiftX, ThreeLiftY) or drawn
    img = images.Clockwise
    if d[3] % 0x2 == 0 then
        img = gfx.transform(img, 0, true, false)
    end
    drawn = gfx.blit(renderer, img, RenderX2 - 7, RenderY2 - 8, 32, 32) or drawn
    return drawn
end

return {
    objectId = 175,
    name = "Three platform rickshaw lift (rotates in place by self)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "longer_steps",
            label = "Longer steps",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
