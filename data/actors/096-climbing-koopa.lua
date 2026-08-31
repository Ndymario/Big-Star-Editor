-- Ported from NSMB Editor 5's NSMBStageObj.cs case 96.

local images = {}

local function init(ctx)
    images.FenceKoopaGreen = ctx.loadImage("images/FenceKoopaGreen.png")
    images.FenceKoopaRed = ctx.loadImage("images/FenceKoopaRed.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 18
    height = 26
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    if actor.fields.color == 1 then
        img = images.FenceKoopaRed
    else
        img = images.FenceKoopaGreen
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
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
