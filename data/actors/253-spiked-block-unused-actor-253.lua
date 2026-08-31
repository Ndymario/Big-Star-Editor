-- Ported from NSMB Editor 5's NSMBStageObj.cs case 253.

local images = {}

local function init(ctx)
    images.FlyingQBlockOverrides = ctx.loadImage("images/FlyingQBlockOverrides.png")
    images.SpikedBlock = ctx.loadImage("images/SpikedBlock.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 2
    y = y - 7
    width = 19
    height = 26
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderY2 = RenderY
    local width, height = 0, 0
    local drawn = false
    drawn = gfx.blit(renderer, images.SpikedBlock, RenderX - 2, RenderY - 7) or drawn
    RenderY2 = actor.fields.contents
    if RenderY2 == 2 then
        RenderY2 = 3
    end
    if RenderY2 <= 3 then
        drawn = gfx.blit(renderer, images.FlyingQBlockOverrides, RenderX, RenderY + 1, nil, nil, { x = RenderY2 * 16, y = 0, width = 16, height = 16 }) or drawn
    end
    return drawn
end

return {
    objectId = 253,
    name = "Spiked ? Block (unused actor 253)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Clockwise" },
                { value = 1, label = "Counterclockwise" },
            },
        },
        {
            key = "contents",
            label = "Contents",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Coin" },
                { value = 1, label = "Powerup" },
                { value = 2, label = "1 up" },
                { value = 3, label = "1-up" },
                { value = 4, label = "Powerup (throw down)" },
                { value = 5, label = "Crash" },
                { value = 6, label = "Mushroom (throws down)" },
                { value = 7, label = "Coin" },
                { value = 8, label = "Coin" },
                { value = 9, label = "Crash" },
                { value = 10, label = "Coin" },
                { value = 11, label = "Mini Mushroom" },
                { value = 12, label = "Crash" },
                { value = 13, label = "Coin" },
                { value = 14, label = "Mushroom/Flower" },
                { value = 15, label = "Mushroom/Flower spawning like touchscreen" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
