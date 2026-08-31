-- Ported from NSMB Editor 5's NSMBStageObj.cs case 119.

local images = {}

local function init(ctx)
    images.PeteyPiranha = ctx.loadImage("images/PeteyPiranha.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 16
    y = y - 56
    width = 108
    height = 75
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.PeteyPiranha, RenderX - 16, RenderY - 56) or drawn
    return drawn
end

return {
    objectId = 119,
    name = "Petey Piranha",
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
