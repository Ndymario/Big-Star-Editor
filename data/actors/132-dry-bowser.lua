-- Ported from NSMB Editor 5's NSMBStageObj.cs case 132.

local images = {}

local function init(ctx)
    images.DryBowser = ctx.loadImage("images/DryBowser.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 42, y = actor.pixelY - 40, width = 80, height = 57 }
end

local function render(actor, renderer)
    return renderer:draw(images.DryBowser, grabBounds(actor))
end

return {
    objectId = 132,
    name = "Dry Bowser",
    category = "Bosses and cutscenes",
    notes = "Regular Bowser seen in 1-Castle, exept throws bones and is immune to Fireballs.",
    fields = {
        {
            key = "trigger_id",
            label = "Trigger ID",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "bridge_id",
            label = "Bridge ID",
            bitOffset = 40,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
