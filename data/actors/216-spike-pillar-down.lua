-- Ported from NSMB Editor 5's NSMBStageObj.cs case 216.

local images = {}

local function init(ctx)
    images.GiantSpike = ctx.loadImage("images/GiantSpike.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 128
    width = 64
    height = 144
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GiantSpike, RenderX, RenderY - 128) or drawn
    return drawn
end

return {
    objectId = 216,
    name = "Spike Pillar - down",
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
                { value = 8, label = "Forward with delay" },
            },
        },
        {
            key = "forward_delay",
            label = "Forward delay",
            bitOffset = 12,
            bitWidth = 12,
            notes = "60 frames = 1 second.",
        },
        {
            key = "backward_delay",
            label = "Backward delay",
            bitOffset = 0,
            bitWidth = 12,
            notes = "60 frames = 1 second.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
