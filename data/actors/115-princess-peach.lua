-- Ported from NSMB Editor 5's NSMBStageObj.cs case 115.

local images = {}

local function init(ctx)
    images.PrincessPeach = ctx.loadImage("images/PrincessPeach.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 22, width = 29, height = 38 }
end

local function render(actor, renderer)
    return renderer:draw(images.PrincessPeach, grabBounds(actor))
end

return {
    objectId = 115,
    name = "Princess Peach",
    category = "Bosses and cutscenes",
    notes = "Seems to only say \"Help!\" when paired with the Final Boss Controller.",
    fields = {
        {
            key = "ignore_cinematics",
            label = "Ignore cinematics",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Used in 1-Castle battle, at least",
            checkboxValue = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
