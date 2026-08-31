-- Ported from NSMB Editor 5's NSMBStageObj.cs case 145.

local images = {}

local function init(ctx)
    images.BlooperWithMini = ctx.loadImage("images/BlooperWithMini.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 32
    x = x - 8
    width = 42
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BlooperWithMini, RenderX - 8, RenderY - 32) or drawn
    return drawn
end

return {
    objectId = 145,
    name = "Blooper Nanny that shoots out Baby Bloopers",
    category = "Enemies - Water",
    notes = "This one shoots out baby bloopers after a rainbow effect",
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
        {
            key = "avoids_mario",
            label = "Avoids Mario",
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
