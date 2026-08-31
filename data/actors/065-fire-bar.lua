-- Ported from NSMB Editor 5's NSMBStageObj.cs case 65.

local images = {}

local function init(ctx)
    images.Clockwise = ctx.loadImage("images/Clockwise.png")
    images.FireBarBall = ctx.loadImage("images/FireBarBall.png")
    images.FireBarMiddle = ctx.loadImage("images/FireBarMiddle.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width = actor.fields.radius * 10
    y = y - width
    if actor.fields.enable_second_bar then
        x = x - width
        width = width * 2
    end
    width = width + 12
    return { x = x, y = y, width = width, height = width }
end

local function render(actor, renderer)
    local x, y = actor.pixelX, actor.pixelY
    local radius = actor.fields.radius * 10
    local drawn = false
    if actor.fields.enable_second_bar then
        x = x - radius
        y = y + radius
    end
    while x ~= actor.pixelX + radius + 10 do
        if x == actor.pixelX then
            drawn = renderer:draw(images.FireBarMiddle, { x = x + 4, y = y + 4 }) or drawn
        else
            drawn = renderer:draw(images.FireBarBall, { x = x, y = y }) or drawn
        end
        x = x + 10
        y = y - 10
    end
    return renderer:draw(images.Clockwise, {
        x = actor.pixelX - 8,
        y = actor.pixelY - 8,
        width = 32,
        height = 32,
        flipX = actor.fields.rotation == 0,
    }) or drawn
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
