-- Ported from NSMB Editor 5's NSMBStageObj.cs case 192.

local images = {}

local function init(ctx)
    images.GreenMushroomEdge = ctx.loadImage("images/GreenMushroomEdge.png")
    images.GreenMushroomMiddle = ctx.loadImage("images/GreenMushroomMiddle.png")
    images.GreenMushroomStalk = ctx.loadImage("images/GreenMushroomStalk.png")
    images.GreenMushroomStalkTop = ctx.loadImage("images/GreenMushroomStalkTop.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    width = (actor.fields.width + 2) * 16
    x = x - ((width - 16) // 2)
    height = (d[5] & 0xF0) + 49
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderX2 = RenderX
    local width = 0
    local img
    local drawn = false
    img = images.GreenMushroomEdge
    img = gfx.transform(img, 0, true, false)
    width = actor.fields.width + 1
    RenderX = RenderX - (width * 8)
    drawn = gfx.blit(renderer, images.GreenMushroomEdge, RenderX, RenderY) or drawn
    for l = 0, width - 2 do
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, images.GreenMushroomMiddle, RenderX, RenderY) or drawn
    end
    drawn = gfx.blit(renderer, img, RenderX + 16, RenderY) or drawn
    drawn = gfx.blit(renderer, images.GreenMushroomStalkTop, RenderX2, RenderY + 24) or drawn
    for l = 0, (actor.fields.stalk_height_and_move_direction) - 1 do
        drawn = gfx.blit(renderer, images.GreenMushroomStalk, RenderX2, RenderY + 49) or drawn
        RenderY = RenderY + 16
    end
    return drawn
end

return {
    objectId = 192,
    name = "Rise or lower while/once on Mushroom Platform lift - mechanical",
    category = "Platforms and ropes",
    notes = "Has the same data as sprite 239. Any code differences other than the obvious?",
    fields = {
        {
            key = "distance",
            label = "Distance",
            bitOffset = 8,
            bitWidth = 4,
            notes = "N9-N10 is the actual distance this mushroom will move.",
        },
        {
            key = "stalk_height_and_move_direction",
            label = "Stalk height and move direction",
            bitOffset = 4,
            bitWidth = 4,
            notes = "If N9-10>0, this mushroom will go up, else it'll go down.",
        },
        {
            key = "width",
            label = "Width",
            bitOffset = 0,
            bitWidth = 4,
        },
        {
            key = "returns_to_original_position",
            label = "Returns to original position",
            bitOffset = 12,
            bitWidth = 4,
            notes = "The mushroom will return to its original position once Mario/Luigi gets off it.",
            checkboxValue = 1,
        },
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 36,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 28,
            bitWidth = 8,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
