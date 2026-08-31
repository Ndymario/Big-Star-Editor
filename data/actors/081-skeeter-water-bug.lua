-- Ported from NSMB Editor 5's NSMBStageObj.cs case 81.

local images = {}

local function init(ctx)
    images.WaterBug = ctx.loadImage("images/WaterBug.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 16, y = actor.pixelY - 5, width = 41, height = 21 }
end

local function render(actor, renderer)
    return renderer:draw(images.WaterBug, grabBounds(actor))
end

return {
    objectId = 81,
    name = "Skeeter (Water Bug)",
    category = "Enemies - Water",
    fields = {
        {
            key = "mobility",
            label = "Mobility",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Moves around" },
                { value = 1, label = "Stays in place" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
