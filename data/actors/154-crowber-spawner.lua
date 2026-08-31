-- Ported from NSMB Editor 5's NSMBStageObj.cs case 154.

local images = {}

local function init(ctx)
    images.Crow = ctx.loadImage("images/Crow.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY, width = 22, height = 14 }
end

local function render(actor, renderer)
    return renderer:draw(images.Crow, grabBounds(actor))
end

return {
    objectId = 154,
    name = "Crowber spawner",
    category = "Enemies - Ghost House",
    fields = {
        {
            key = "zone_id",
            label = "Zone ID",
            bitOffset = 8,
            bitWidth = 8,
            notes = "Crobers will spawn when Mario is inside this zone.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
