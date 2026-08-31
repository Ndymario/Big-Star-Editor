-- Ported from NSMB Editor 5's NSMBStageObj.cs case 147.

local images = {}

local geometry = {
    [0] = { image = "SpinyBeetleUp", x = 0, y = -4, width = 16, height = 20 },
    [1] = { image = "SpinyBeetleLeft", x = -4, y = 0, width = 20, height = 16 },
    [2] = { image = "SpinyBeetleDown", x = 0, y = 0, width = 16, height = 20 },
    [3] = { image = "SpinyBeetleRight", x = 0, y = 0, width = 20, height = 16 },
}

local function init(ctx)
    images.SpinyBeetleDown = ctx.loadImage("images/SpinyBeetleDown.png")
    images.SpinyBeetleLeft = ctx.loadImage("images/SpinyBeetleLeft.png")
    images.SpinyBeetleRight = ctx.loadImage("images/SpinyBeetleRight.png")
    images.SpinyBeetleUp = ctx.loadImage("images/SpinyBeetleUp.png")
end

local function grabBounds(actor)
    local g = geometry[actor.fields.spike_direction] or geometry[0]
    return { x = actor.pixelX + g.x, y = actor.pixelY + g.y, width = g.width, height = g.height }
end

local function render(actor, renderer)
    local g = geometry[actor.fields.spike_direction] or geometry[0]
    return renderer:draw(images[g.image], grabBounds(actor))
end

return {
    objectId = 147,
    name = "Spike Top",
    category = "Enemies - Underground",
    notes = "A beetle with a spike on its head, that crawls slowly on all sides of a surface.\n\nUnused behaviours:\n- Can be defeated by Drill Stomp [may be spiny leftover?]",
    fields = {
        {
            key = "spike_direction",
            label = "Spike direction",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Left" },
                { value = 2, label = "Down" },
                { value = 3, label = "Right" },
            },
        },
        {
            key = "travel_direction",
            label = "Travel direction",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Clockwise" },
                { value = 1, label = "Counter-clockwise" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
