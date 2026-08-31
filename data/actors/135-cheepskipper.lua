-- Ported from NSMB Editor 5's NSMBStageObj.cs case 135.

local images = {}

local function init(ctx)
    images.CheepSkipper = ctx.loadImage("images/CheepSkipper.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 22, y = actor.pixelY - 29, width = 59, height = 48 }
end

local function render(actor, renderer)
    return renderer:draw(images.CheepSkipper, grabBounds(actor))
end

return {
    objectId = 135,
    name = "Cheepskipper",
    category = "Bosses and cutscenes",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
