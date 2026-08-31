-- Ported from NSMB Editor 5's NSMBStageObj.cs case 63.

local images = {}

local function init(ctx)
    images.ScuttleBug = ctx.loadImage("images/ScuttleBug.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 8, width = 46, height = 24 }
end

local function render(actor, renderer)
    return renderer:draw(images.ScuttleBug, grabBounds(actor))
end

return {
    objectId = 63,
    name = "Scuttlebug spawner",
    category = "Other",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
