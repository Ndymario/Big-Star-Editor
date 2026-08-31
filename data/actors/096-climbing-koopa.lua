-- Ported from NSMB Editor 5's NSMBStageObj.cs case 96.

local images = {}

local function init(ctx)
    images.FenceKoopaGreen = ctx.loadImage("images/FenceKoopaGreen.png")
    images.FenceKoopaRed = ctx.loadImage("images/FenceKoopaRed.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 18, height = 26 }
end

local function render(actor, renderer)
    local image = actor.fields.color == 1 and images.FenceKoopaRed or images.FenceKoopaGreen
    return renderer:draw(image, { x = actor.pixelX, y = actor.pixelY })
end

return {
    objectId = 96,
    name = "Climbing Koopa",
    category = "Enemies - Castle",
    fields = {
        {
            key = "walking_direction",
            label = "Walking direction",
            bitOffset = 4,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Horizontal" },
                { value = 1, label = "Vertical" },
            },
        },
        {
            key = "fence_position",
            label = "Fence position",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "In front" },
                { value = 1, label = "Behind" },
            },
        },
        {
            key = "faster",
            label = "Faster",
            bitOffset = 12,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Green" },
                { value = 1, label = "Red" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
