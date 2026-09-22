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
