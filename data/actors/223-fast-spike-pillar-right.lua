-- Ported from NSMB Editor 5's NSMBStageObj.cs case 223.

local images = {}

local function init(ctx)
    images.GiantSpike = ctx.loadImage("images/GiantSpike.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 128
    width = 144
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.GiantSpike
    img = gfx.transform(img, 270, false, false)
    drawn = gfx.blit(renderer, img, RenderX - 128, RenderY) or drawn
    return drawn
end

return {
    objectId = 223,
    name = "Fast Spike Pillar - Right",
    category = "Enemies - Castle",
    fields = {
        {
            key = "speed",
            label = "Speed",
            bitOffset = 28,
            bitWidth = 4,
            choices = {
                { value = 3, label = "Doesn't move" },
                { value = 2, label = "Slowest" },
                { value = 0, label = "Medium" },
                { value = 1, label = "Fastest" },
            },
        },
        {
            key = "start",
            label = "Start",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Forward" },
                { value = 2, label = "Backward" },
                { value = 8, label = "With delay" },
            },
        },
        {
            key = "distance",
            label = "Distance",
            bitOffset = 20,
            bitWidth = 4,
            notes = "Time travel is the same no matter the distance.",
            choices = {
                { value = 1, label = "7 tiles" },
                { value = 3, label = "10 tiles" },
                { value = 2, label = "14 tiles" },
                { value = 0, label = "16 tiles" },
            },
        },
        {
            key = "forward_delay",
            label = "Forward delay",
            bitOffset = 12,
            bitWidth = 8,
            notes = "60 frames = 1 second.",
        },
        {
            key = "backward_delay",
            label = "Backward delay",
            bitOffset = 0,
            bitWidth = 8,
            notes = "60 frames = 1 second.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
