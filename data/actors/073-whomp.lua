-- Ported from NSMB Editor 5's NSMBStageObj.cs case 73.

local images = {}

local function init(ctx)
    images.Whomp = ctx.loadImage("images/Whomp.png")
    images.WhompL = ctx.loadImage("images/WhompL.png")
end

local function grabBounds(actor)
    if actor.fields.bigger then
        return { x = actor.pixelX - 38, y = actor.pixelY - 74, width = 96, height = 90 }
    end
    return { x = actor.pixelX - 29, y = actor.pixelY - 57, width = 78, height = 73 }
end

local function render(actor, renderer)
    local image = actor.fields.bigger and images.WhompL or images.Whomp
    return renderer:draw(image, grabBounds(actor))
end

return {
    objectId = 73,
    name = "Whomp",
    category = "Enemies - Castle",
    notes = "Seems to have unused ceiling detection.",
    fields = {
        {
            key = "doesn_t_move",
            label = "Doesn't move",
            bitOffset = 4,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "bigger",
            label = "Bigger",
            bitOffset = 0,
            bitWidth = 4,
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
