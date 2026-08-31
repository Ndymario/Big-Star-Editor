-- Ported from NSMB Editor 5's NSMBStageObj.cs case 61.

local images = {}

local function init(ctx)
    images.Infinity = ctx.loadImage("images/Infinity.png")
    images.Shark = ctx.loadImage("images/Shark.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX - 27, y = actor.pixelY - 8, width = 71, height = 26 }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.Shark, grabBounds(actor))
    return renderer:draw(images.Infinity, { x = actor.pixelX - 4, y = actor.pixelY + 1 }) or drawn
end

return {
    objectId = 61,
    name = "Sushi spawner (shark spawner)",
    category = "Enemies - Water",
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
