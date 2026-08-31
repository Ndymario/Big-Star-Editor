-- Ported from NSMB Editor 5's NSMBStageObj.cs case 105.

local images = {}

local function init(ctx)
    images.MushroomEdgePurple = ctx.loadImage("images/MushroomEdgePurple.png")
    images.MushroomSectionPurple = ctx.loadImage("images/MushroomSectionPurple.png")
    images.MushroomStalk = ctx.loadImage("images/MushroomStalk.png")
    images.MushroomStalkTop = ctx.loadImage("images/MushroomStalkTop.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 16 * (actor.fields.width + 1),
        y = actor.pixelY,
        width = 32 * (actor.fields.width + 2),
        height = 112,
    }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.MushroomStalkTop, {
        x = actor.pixelX + 6,
        y = actor.pixelY + 24,
    })
    for section = 0, 4 do
        drawn = renderer:draw(images.MushroomStalk, {
            x = actor.pixelX + 6,
            y = actor.pixelY + 32 + 16 * section,
        }) or drawn
    end

    local capX = actor.pixelX - 16 * (actor.fields.width + 1)
    drawn = renderer:draw(images.MushroomEdgePurple, { x = capX, y = actor.pixelY }) or drawn
    for section = 1, actor.fields.width do
        drawn = renderer:draw(images.MushroomSectionPurple, {
            x = capX + 32 * section,
            y = actor.pixelY,
        }) or drawn
    end
    drawn = renderer:draw(images.MushroomEdgePurple, {
        x = capX + 32 * (actor.fields.width + 1),
        y = actor.pixelY,
        flipX = true,
    }) or drawn
    return drawn
end

return {
    objectId = 105,
    name = "Unused Purple Mushroom Platform Lift (tilts if stepped on) (slower)",
    category = "Platforms and ropes",
    notes = "Unused. Tilts slower than used purple tilting mushroom. Fits perfectly with the stalk object in the snow with ice tileset (Hiccup: Is this not true for the other one?)",
    fields = {
        {
            key = "width",
            label = "Width",
            bitOffset = 4,
            bitWidth = 2,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
