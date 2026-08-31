-- Ported from NSMB Editor 5's NSMBStageObj.cs case 107.

local images = {}

local function init(ctx)
    images.FlameChomp = ctx.loadImage("images/FlameChomp.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 1, y = actor.pixelY - 33, width = 53, height = 43 }
end

local function render(actor, renderer)
    return renderer:draw(images.FlameChomp, grabBounds(actor))
end

return {
    objectId = 107,
    name = "Flame Chomp spawner",
    category = "Enemies - Fire",
    notes = "Spawns a Flame Chomp with actor data of: 00 00 00 00 00 00",
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
