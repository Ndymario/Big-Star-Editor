-- Ported from NSMB Editor 5's NSMBStageObj.cs case 253.

local images = {}

local function init(ctx)
    images.FlyingQBlockOverrides = ctx.loadImage("images/FlyingQBlockOverrides.png")
    images.SpikedBlock = ctx.loadImage("images/SpikedBlock.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 2, y = actor.pixelY - 7, width = 19, height = 26 }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.SpikedBlock, grabBounds(actor))
    local frame = actor.fields.contents
    if frame == 2 then
        frame = 3
    end
    if frame <= 3 then
        drawn = renderer:draw(images.FlyingQBlockOverrides, {
            x = actor.pixelX,
            y = actor.pixelY + 1,
            sourceX = frame * 16,
            sourceY = 0,
            sourceWidth = 16,
            sourceHeight = 16,
        }) or drawn
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
