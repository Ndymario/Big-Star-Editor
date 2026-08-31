-- Ported from NSMB Editor 5's NSMBStageObj.cs case 129.

local images = {}

local function init(ctx)
    images.Lakithunder = ctx.loadImage("images/Lakithunder.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX + 125, y = actor.pixelY - 15, width = 37, height = 45 }
end

local function render(actor, renderer)
    return renderer:draw(images.Lakithunder, grabBounds(actor))
end

return {
    objectId = 129,
    name = "Lakithunder",
    category = "Bosses and cutscenes",
    notes = "Swoops down on the player, and tries to strike the player with Lightning as well. Can only be spawned with a Castle Boss Controller.",
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
