-- Ported from NSMB Editor 5's NSMBStageObj.cs case 144.

local images = {}

local function init(ctx)
    images.BlooperNanny = ctx.loadImage("images/BlooperNanny.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 16
    width = 59
    height = 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.BlooperNanny, RenderX, RenderY - 16) or drawn
    return drawn
end

return {
    objectId = 144,
    name = "Blooper Nanny (unused actor 144)",
    category = "Enemies - Water",
    notes = "\"Only someone with direct access to the database can proceed this modification : Class ID is actually 144, not 2 !\"\nThis one doesn't shoot out baby bloopers.",
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
