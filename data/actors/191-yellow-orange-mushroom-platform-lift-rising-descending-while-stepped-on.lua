-- Ported from NSMB Editor 5's NSMBStageObj.cs case 191.

local images = {}

local function init(ctx)
    images.LoweringMushroomEdge = ctx.loadImage("images/LoweringMushroomEdge.png")
    images.LoweringMushroomMiddle = ctx.loadImage("images/LoweringMushroomMiddle.png")
    images.RiseLowerMushroomStalk = ctx.loadImage("images/RiseLowerMushroomStalk.png")
    images.RiseLowerMushroomStalkTop = ctx.loadImage("images/RiseLowerMushroomStalkTop.png")
    images.RisingMushroomEdge = ctx.loadImage("images/RisingMushroomEdge.png")
    images.RisingMushroomMiddle = ctx.loadImage("images/RisingMushroomMiddle.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - (8 * (actor.fields.width + 1))
    width = 16 * (actor.fields.width + 2)
    height = 16 * (math.max(actor.fields.stalk_height_and_move_direction, actor.fields.distance) + 1) + 32
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderY2 = RenderY
    local img, img2
    local drawn = false
    drawn = gfx.blit(renderer, images.RiseLowerMushroomStalkTop, RenderX, RenderY + 24) or drawn
    RenderY = RenderY + 32
    for l = 0, math.max(actor.fields.stalk_height_and_move_direction, actor.fields.distance) do
        drawn = gfx.blit(renderer, images.RiseLowerMushroomStalk, RenderX, RenderY) or drawn
        RenderY = RenderY + 16
    end
    RenderX = RenderX - ((actor.fields.width + 1) * 8)
    if actor.fields.stalk_height_and_move_direction > actor.fields.distance then
        img = images.LoweringMushroomEdge
        img2 = images.LoweringMushroomMiddle
    else
        img = images.RisingMushroomEdge
        img2 = images.RisingMushroomMiddle
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY2) or drawn
    for l = 0, (actor.fields.width) - 1 do
        RenderX = RenderX + 16
        drawn = gfx.blit(renderer, img2, RenderX, RenderY2) or drawn
    end
    img = gfx.transform(img, 0, true, false)
    drawn = gfx.blit(renderer, img, RenderX + 16, RenderY2) or drawn
    return drawn
end

return {
    objectId = 191,
    name = "Yellow/Orange Mushroom Platform lift (Rising/descending while stepped on)",
    category = "Platforms and ropes",
    notes = "Has the same data as sprite 312.",
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
