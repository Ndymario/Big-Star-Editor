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
    local stalkHeight = math.max(actor.fields.stalk_height_and_move_direction, actor.fields.distance)
    return {
        x = actor.pixelX - 8 * (actor.fields.width + 1),
        y = actor.pixelY,
        width = 16 * (actor.fields.width + 2),
        height = 16 * (stalkHeight + 1) + 32,
    }
end

local function render(actor, renderer)
    local stalkHeight = math.max(actor.fields.stalk_height_and_move_direction, actor.fields.distance)
    renderer:draw(images.RiseLowerMushroomStalkTop, { x = actor.pixelX, y = actor.pixelY + 24 })
    for segment = 0, stalkHeight do
        renderer:draw(images.RiseLowerMushroomStalk, { x = actor.pixelX, y = actor.pixelY + 32 + 16 * segment })
    end

    local edge, middle
    if actor.fields.stalk_height_and_move_direction > actor.fields.distance then
        edge = images.LoweringMushroomEdge
        middle = images.LoweringMushroomMiddle
    else
        edge = images.RisingMushroomEdge
        middle = images.RisingMushroomMiddle
    end

    local box = grabBounds(actor)
    renderer:draw(edge, { x = box.x, y = box.y })
    for column = 1, actor.fields.width do
        renderer:draw(middle, { x = box.x + 16 * column, y = box.y })
    end
    return renderer:draw(edge, {
        x = box.x + 16 * (actor.fields.width + 1),
        y = box.y,
        flipX = true,
    })
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
