-- Ported from NSMB Editor 5's NSMBStageObj.cs case 135.

local images = {}

local function init(ctx)
    images.CheepSkipper = ctx.loadImage("images/CheepSkipper.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x + 22
    y = y - 29
    width = 59
    height = 48
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.CheepSkipper, RenderX + 22, RenderY - 29) or drawn
    return drawn
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
