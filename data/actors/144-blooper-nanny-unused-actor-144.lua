-- Ported from NSMB Editor 5's NSMBStageObj.cs case 144.

local images = {}

local function init(ctx)
    images.BlooperNanny = ctx.loadImage("images/BlooperNanny.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 16, width = 59, height = 32 }
end

local function render(actor, renderer)
    return renderer:draw(images.BlooperNanny, grabBounds(actor))
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
