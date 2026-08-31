-- Ported from NSMB Editor 5's NSMBStageObj.cs case 115.

local images = {}

local function init(ctx)
    images.PrincessPeach = ctx.loadImage("images/PrincessPeach.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 22
    width = 29
    height = 38
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.PrincessPeach, RenderX, RenderY - 22) or drawn
    return drawn
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
