-- Ported from NSMB Editor 5's NSMBStageObj.cs case 130.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.SpinningSpikeBall = ctx.loadImage("images/SpinningSpikeBall.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 96
    height = 39
    if (d[5] % 0x4) == 0 then
        x = x + 4
        y = y - 11
    elseif (d[5] % 0x4) == 2 then
        x = x - 83
        y = y - 11
    else
        width = 39
        height = 96
        if (d[5] % 0x4) == 1 then
            x = x - 11
            y = y - 83
        else
            x = x - 11
            y = y + 4
        end
    end
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local img
    local drawn = false
    img = images.SpinningSpikeBall
    local BallChainX = RenderX
    local BallChainY = RenderY
    if (d[5] % 0x4) == 1 then
        img = gfx.transform(img, 270, false, false)
        BallChainX = BallChainX - 11
        BallChainY = BallChainY - 83
    elseif (d[5] % 0x4) == 2 then
        img = gfx.transform(img, 180, false, false)
        BallChainX = BallChainX - 83
        BallChainY = BallChainY - 11
    elseif (d[5] % 0x4) == 3 then
        img = gfx.transform(img, 90, false, false)
        BallChainX = BallChainX - 11
        BallChainY = BallChainY + 4
    else
        BallChainX = BallChainX + 4
        BallChainY = BallChainY - 11
    end
    drawn = gfx.blit(renderer, img, BallChainX, BallChainY) or drawn
    img = images.Clockwise
    if (d[5] & 0x10) // 0x10 == 0 then
        img = gfx.transform(img, 0, true, false)
    end
    drawn = gfx.blit(renderer, img, RenderX2 - 8, RenderY2 - 8, 32, 32) or drawn
    return drawn
end

return {
    objectId = 130,
    name = "Ball 'n' Chain",
    category = "Enemies - Castle",
    notes = "AKA: Flail, pendulum...",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "starting_position",
            label = "Starting position",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Up" },
                { value = 2, label = "Left" },
                { value = 3, label = "Down" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
