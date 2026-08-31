-- Ported from NSMB Editor 5's NSMBStageObj.cs case 190.

local images = {}

local function init(ctx)
    images.MushroomEdgePurple = ctx.loadImage("images/MushroomEdgePurple.png")
    images.MushroomSectionPurple = ctx.loadImage("images/MushroomSectionPurple.png")
    images.MushroomStalk = ctx.loadImage("images/MushroomStalk.png")
    images.MushroomStalkTop = ctx.loadImage("images/MushroomStalkTop.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 16 * (actor.fields.length + 1),
        y = actor.pixelY,
        width = 32 * (actor.fields.length + 2),
        height = 112,
    }
end

local function render(actor, renderer)
    renderer:draw(images.MushroomStalkTop, { x = actor.pixelX + 6, y = actor.pixelY + 24 })
    for segment = 0, 4 do
        renderer:draw(images.MushroomStalk, { x = actor.pixelX + 6, y = actor.pixelY + 32 + 16 * segment })
    end

    local box = grabBounds(actor)
    renderer:draw(images.MushroomEdgePurple, { x = box.x, y = box.y })
    for column = 1, actor.fields.length do
        renderer:draw(images.MushroomSectionPurple, { x = box.x + 32 * column, y = box.y })
    end
    return renderer:draw(images.MushroomEdgePurple, {
        x = box.x + 32 * (actor.fields.length + 1),
        y = box.y,
        flipX = true,
    })
end

return {
    objectId = 190,
    name = "Purple Mushroom Platform lift (tilt while stepped on)",
    category = "Platforms and ropes",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 4,
            bitWidth = 2,
        },
        {
            key = "unknown_value_11",
            label = "Unknown value 11",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
