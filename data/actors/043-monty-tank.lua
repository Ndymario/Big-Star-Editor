-- Ported from NSMB Editor 5's NSMBStageObj.cs case 43.

local images = {}

local function init(ctx)
    images.MontyTank = ctx.loadImage("images/MontyTank.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 374, y = actor.pixelY - 37, width = 72, height = 52 }
end

local function render(actor, renderer)
    return renderer:draw(images.MontyTank, grabBounds(actor))
end

return {
    objectId = 43,
    name = "Monty Tank",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
