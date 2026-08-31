-- Ported from NSMB Editor 5's NSMBStageObj.cs case 189.

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
    height = 16 * (d[3] % 0x10 + 4)
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img, img2
    local drawn = false
    RenderY = RenderY + 24
    RenderX = RenderX + 6
    drawn = gfx.blit(renderer, images.MushroomStalkTop, RenderX, RenderY) or drawn
    RenderY = RenderY + 8
    drawn = gfx.blit(renderer, images.MushroomStalk, RenderX, RenderY) or drawn
    for l = 0, d[3] % 0x10 do
        RenderY = RenderY + 16
        drawn = gfx.blit(renderer, images.MushroomStalk, RenderX, RenderY) or drawn
    end
    local RedMax = (d[4] & 0x70) // 0x10
    RenderX = (actor.pixelX / 16) * 16 - 16 * (RedMax + 1)
    RenderY = (actor.pixelY / 16) * 16
    img = images.MushroomEdge
    img2 = images.MushroomSection
    if ((d[3] & 0x80) // 0x80) == 1 then
        img = images.MushroomEdgeGreen
        img2 = images.MushroomSectionGreen
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    RenderX = RenderX + 32
    for l = 0, (RedMax) - 1 do
        drawn = gfx.blit(renderer, img2, RenderX, RenderY) or drawn
        RenderX = RenderX + 32
    end
    img = gfx.transform(img, 0, true, false)
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 189,
    name = "Mushroom Platform lift (tilting and bobbing)",
    category = "Platforms and ropes",
    notes = "\n\t\t\t\tA basic mushroom platform, where Mario/Luigi can stand. There is unused stuff like moving when stood on (huh? -Hiccup). 7 is the maximum height that works with sprite data.  If you want it to be longer, set it to 7 and then use a mushroom stalk object in the tileset to make the mushroom's stalk height look higher.\n\t\t\t\tThis mushroom can be either red or green.\n\t\t\t",
    fields = {
        {
            key = "bobbing",
            label = "Bobbing",
            bitOffset = 21,
            bitWidth = 1,
            checkboxValue = 1,
        },
        {
            key = "color",
            label = "Color",
            bitOffset = 23,
            bitWidth = 1,
            notes = "Green is unused.",
            choices = {
                { value = 0, label = "Red" },
                { value = 1, label = "Green" },
            },
        },
        {
            key = "stalk_height",
            label = "Stalk height",
            bitOffset = 16,
            bitWidth = 3,
            notes = "Max 7. Add tiles under it.",
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 12,
            bitWidth = 3,
        },
        {
            key = "tilt_angle",
            label = "Tilt angle",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 0,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
