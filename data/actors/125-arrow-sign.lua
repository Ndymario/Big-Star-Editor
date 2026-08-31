-- Ported from NSMB Editor 5's NSMBStageObj.cs case 125.

local images = {}

local function init(ctx)
    images.Arrow = ctx.loadImage("images/Arrow.png")
    images.ArrowRotate45 = ctx.loadImage("images/ArrowRotate45.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = 32
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    local direction = d[2] % 8
    if direction == 0 or direction == 2 or direction == 4 or direction == 6 then
        img = images.Arrow
    else
        img = images.ArrowRotate45
    end
    if direction == 2 or direction == 3 then
        img = gfx.transform(img, 90, false, false)
    end
    if direction == 4 or direction == 5 then
        img = gfx.transform(img, 180, false, false)
    end
    if direction == 6 or direction == 7 then
        img = gfx.transform(img, 270, false, false)
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY, 32, 32) or drawn
    return drawn
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
            bitWidth = 4,
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
