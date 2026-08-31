-- Ported from NSMB Editor 5's NSMBStageObj.cs case 65.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.FireBarBall = ctx.loadImage("images/FireBarBall.png")
    images.FireBarMiddle = ctx.loadImage("images/FireBarMiddle.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = (actor.fields.radius) * 10
    y = y - width
    if actor.fields.enable_second_bar then
        x = x - width
        width = width * 2
    end
    width = width + 12
    height = width
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2, RenderY2 = RenderX, RenderY
    local img
    local drawn = false
    local clockFireBarX = RenderX2 - 8
    local clockFireBarY = RenderY2 - 8
    RenderY2 = (actor.fields.radius) * 10
    if actor.fields.enable_second_bar then
        RenderX = RenderX - RenderY2
        RenderY = RenderY + RenderY2
    end
    while not (RenderX == RenderX2 + RenderY2 + 10) do
        if RenderX == RenderX2 then
            drawn = gfx.blit(renderer, images.FireBarMiddle, RenderX + 4, RenderY + 4) or drawn
        else
            drawn = gfx.blit(renderer, images.FireBarBall, RenderX, RenderY) or drawn
        end
        RenderX = RenderX + 10
        RenderY = RenderY - 10
    end
    img = images.Clockwise
    if (d[4] & 0x10) // 0x10 == 0 then
        img = gfx.transform(img, 0, true, false)
    end
    drawn = gfx.blit(renderer, img, clockFireBarX, clockFireBarY, 32, 32) or drawn
    return drawn
end

return {
    objectId = 65,
    name = "Fire Bar",
    category = "Enemies - Fire",
    fields = {
        {
            key = "rotation",
            label = "Rotation",
            bitOffset = 12,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Counterclockwise" },
                { value = 1, label = "Clockwise" },
            },
        },
        {
            key = "enable_second_bar",
            label = "Enable second bar",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "radius",
            label = "Radius",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "faster",
            label = "Faster",
            bitOffset = 8,
            bitWidth = 4,
            notes = "Completes a turn exactly 4/3 times as fast if checked.",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
