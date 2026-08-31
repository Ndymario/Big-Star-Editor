-- Ported from NSMB Editor 5's NSMBStageObj.cs case 195.

local images = {}

local function init(ctx)
    images.MushroomEdge = ctx.loadImage("images/MushroomEdge.png")
    images.MushroomEdgeGreen = ctx.loadImage("images/MushroomEdgeGreen.png")
    images.MushroomSection = ctx.loadImage("images/MushroomSection.png")
    images.MushroomSectionGreen = ctx.loadImage("images/MushroomSectionGreen.png")
    images.MushroomStalk = ctx.loadImage("images/MushroomStalk.png")
    images.MushroomStalkTop = ctx.loadImage("images/MushroomStalkTop.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - (16 * (((d[4] & 0x70) // 0x10) + 1))
    width = 32 * (((d[4] & 0x70) // 0x10) + 2)
    height = 64
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderY2 = RenderY
    local img, img2
    local drawn = false
    RenderY = RenderY + 24
    RenderX = RenderX + 6
    drawn = gfx.blit(renderer, images.MushroomStalkTop, RenderX, RenderY) or drawn
    RenderY = RenderY + 8
    drawn = gfx.blit(renderer, images.MushroomStalk, RenderX, RenderY) or drawn
    drawn = gfx.blit(renderer, images.MushroomStalk, RenderX, RenderY + 16) or drawn
    local GreenMax = (d[4] & 0x70) // 0x10
    RenderX = (actor.pixelX / 16) * 16 - 16 * (GreenMax + 1)
    RenderY = RenderY2
    img = images.MushroomEdge
    img2 = images.MushroomSection
    if (actor.fields.color) == 1 then
        img = images.MushroomEdgeGreen
        img2 = images.MushroomSectionGreen
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    RenderX = RenderX + 32
    for l = 0, (GreenMax) - 1 do
        drawn = gfx.blit(renderer, img2, RenderX, RenderY) or drawn
        RenderX = RenderX + 32
    end
    img = gfx.transform(img, 0, true, false)
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 195,
    name = "Mushroom Platform lift (Moving, tilting & bobbing)",
    category = "Platforms and ropes",
    notes = "\n\t\t\t\tMoves to the right until it hits a wall, then goes left, until it hits a wall, ect\n\t\t\t\tThis mushroom can be either red or green.\n\t\t\t",
    fields = {
        {
            key = "use_tracks",
            label = "Use tracks",
            bitOffset = 28,
            bitWidth = 1,
            notes = "Unused.",
            checkboxValue = 1,
        },
        {
            key = "horizontal_speed",
            label = "Horizontal speed",
            bitOffset = 24,
            bitWidth = 4,
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 20,
            bitWidth = 1,
            notes = "Green is unused.",
            choices = {
                { value = 0, label = "Red" },
                { value = 1, label = "Green" },
            },
        },
        {
            key = "stalk_height_shift",
            label = "Stalk Height/Shift",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "width",
            label = "Width",
            bitOffset = 12,
            bitWidth = 3,
        },
        {
            key = "tilting_angle",
            label = "Tilting angle",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "bop_speed",
            label = "Bop speed",
            bitOffset = 4,
            bitWidth = 4,
        },
        {
            key = "tilting_speed",
            label = "Tilting speed",
            bitOffset = 0,
            bitWidth = 4,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
