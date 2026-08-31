-- Ported from NSMB Editor 5's NSMBStageObj.cs case 222.

local images = {}

local function init(ctx)
    images.GiantSpike = ctx.loadImage("images/GiantSpike.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 144, height = 64 }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    return renderer:draw(images.GiantSpike, {
        x = box.x + 40,
        y = box.y - 40,
        width = 64,
        height = 144,
        rotation = 90,
    })
end

return {
    objectId = 222,
    name = "Fast Spike Pillar - Left",
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
