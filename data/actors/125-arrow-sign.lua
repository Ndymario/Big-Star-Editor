-- Ported from NSMB Editor 5's NSMBStageObj.cs case 125.

local images = {}

local function init(ctx)
    images.Arrow = ctx.loadImage("images/Arrow.png")
    images.ArrowRotate45 = ctx.loadImage("images/ArrowRotate45.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 32, height = 32 }
end

local function render(actor, renderer)
    local direction = actor.fields.direction
    local image = direction % 2 == 0 and images.Arrow or images.ArrowRotate45
    local bounds = grabBounds(actor)
    bounds.rotation = (direction // 2) * 90
    return renderer:draw(image, bounds)
end

return {
    objectId = 125,
    name = "Arrow sign",
    category = "Effects and decorations",
    fields = {
        {
            key = "z_order",
            label = "Z order",
            bitOffset = 28,
            bitWidth = 4,
            notes = "0, 2 and 3 are unused.",
            choices = {
                { value = 0, label = "Model" },
                { value = 1, label = "Tileset" },
                { value = 2, label = "Foreground" },
                { value = 3, label = "Background" },
            },
        },
        {
            key = "direction",
            label = "Direction",
            bitOffset = 24,
            bitWidth = 3,
            choices = {
                { value = 0, label = "Up" },
                { value = 1, label = "Up/right" },
                { value = 2, label = "Right" },
                { value = 3, label = "Down/right" },
                { value = 4, label = "Down" },
                { value = 5, label = "Down/left" },
                { value = 6, label = "Left" },
                { value = 7, label = "Up/left" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
