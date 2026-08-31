-- Ported from NSMB Editor 5's NSMBStageObj.cs case 105.

local images = {}

local function init(ctx)
    images.MushroomEdgePurple = ctx.loadImage("images/MushroomEdgePurple.png")
    images.MushroomSectionPurple = ctx.loadImage("images/MushroomSectionPurple.png")
    images.MushroomStalk = ctx.loadImage("images/MushroomStalk.png")
    images.MushroomStalkTop = ctx.loadImage("images/MushroomStalkTop.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - (16 * (((d[5] & 0x30) // 0x10) + 1))
    width = 32 * (((d[5] & 0x30) // 0x10) + 2)
    height = 112
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    drawn = gfx.blit(renderer, images.MushroomStalkTop, RenderX + 6, RenderY + 24) or drawn
    RenderY = RenderY + 32
    for l = 0, (5) - 1 do
        drawn = gfx.blit(renderer, images.MushroomStalk, RenderX + 6, RenderY) or drawn
        RenderY = RenderY + 16
    end
    local PurpleMax = (d[5] & 0x30) // 0x10
    RenderX = (actor.pixelX / 16) * 16 - 16 * (PurpleMax + 1)
    RenderY = (actor.pixelY / 16) * 16
    img = images.MushroomEdgePurple
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    RenderX = RenderX + 32
    for l = 0, (PurpleMax) - 1 do
        drawn = gfx.blit(renderer, images.MushroomSectionPurple, RenderX, RenderY) or drawn
        RenderX = RenderX + 32
    end
    img = gfx.transform(img, 0, true, false)
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
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
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
