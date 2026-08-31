-- Ported from NSMB Editor 5's NSMBStageObj.cs case 35.

local images = {}

local function init(ctx)
    images.BomOmb = ctx.loadImage("images/BomOmb.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 2, width = 16, height = 18 }
end

local function render(actor, renderer)
    return renderer:draw(images.BomOmb, grabBounds(actor))
end

return {
    objectId = 35,
    name = "Bob-omb",
    category = "Enemies - Castle",
    notes = "A walking bomb.",
    fields = {
        {
            key = "behaviour",
            label = "Behaviour",
            bitOffset = 28,
            bitWidth = 4,
            notes = "8 is used by ones thrown out of the Monty Tank.",
            choices = {
                { value = 0, label = "Default" },
                { value = 4, label = "Pipe generator" },
                { value = 2, label = "Self-destructs (240 frames)" },
                { value = 8, label = "Self-destructs (241 frames)" },
            },
        },
        {
            key = "pipe_generator_direction",
            label = "Pipe generator direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Down" },
                { value = 2, label = "Left" },
                { value = 3, label = "Right" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
