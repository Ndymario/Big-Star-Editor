-- Ported from NSMB Editor 5's NSMBStageObj.cs case 130.

local images = {}

local geometry = {
    [0] = { x = 4, y = -11, width = 96, height = 39, drawX = 4, drawY = -11, rotation = 0 },
    [1] = { x = -11, y = -83, width = 39, height = 96, drawX = -39.5, drawY = -54.5, rotation = 270 },
    [2] = { x = -83, y = -11, width = 96, height = 39, drawX = -83, drawY = -11, rotation = 180 },
    [3] = { x = -11, y = 4, width = 39, height = 96, drawX = -39.5, drawY = 32.5, rotation = 90 },
}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.SpinningSpikeBall = ctx.loadImage("images/SpinningSpikeBall.png")
end

local function grabBounds(actor)
    local g = geometry[actor.fields.starting_position]
    return { x = actor.pixelX + g.x, y = actor.pixelY + g.y, width = g.width, height = g.height }
end

local function render(actor, renderer)
    local g = geometry[actor.fields.starting_position]
    local drawn = renderer:draw(images.SpinningSpikeBall, {
        x = actor.pixelX + g.drawX,
        y = actor.pixelY + g.drawY,
        width = 96,
        height = 39,
        rotation = g.rotation,
    })
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 8,
        y = actor.pixelY - 8,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    }) or drawn
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
            bitWidth = 1,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "starting_position",
            label = "Starting position",
            bitOffset = 0,
            bitWidth = 2,
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
