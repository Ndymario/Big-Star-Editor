-- Ported from NSMB Editor 5's NSMBStageObj.cs case 129.

local images = {}

local function init(ctx)
    images.Lakithunder = ctx.loadImage("images/Lakithunder.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x + 125
    y = y - 15
    width = 37
    height = 45
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Lakithunder, RenderX + 125, RenderY - 15) or drawn
    return drawn
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
