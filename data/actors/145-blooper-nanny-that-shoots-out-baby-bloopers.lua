-- Ported from NSMB Editor 5's NSMBStageObj.cs case 145.

local images = {}

local function init(ctx)
    images.BlooperWithMini = ctx.loadImage("images/BlooperWithMini.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 8, y = actor.pixelY - 32, width = 42, height = 64 }
end

local function render(actor, renderer)
    return renderer:draw(images.BlooperWithMini, grabBounds(actor))
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
