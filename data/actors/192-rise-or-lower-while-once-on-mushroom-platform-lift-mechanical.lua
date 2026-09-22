-- Ported from NSMB Editor 5's NSMBStageObj.cs case 192.

local images = {}

local function init(ctx)
    images.GreenMushroomEdge = ctx.loadImage("images/GreenMushroomEdge.png")
    images.GreenMushroomMiddle = ctx.loadImage("images/GreenMushroomMiddle.png")
    images.GreenMushroomStalk = ctx.loadImage("images/GreenMushroomStalk.png")
    images.GreenMushroomStalkTop = ctx.loadImage("images/GreenMushroomStalkTop.png")
end

local function grabBounds(actor)
    return {
        x = actor.pixelX - 8 * (actor.fields.width + 1),
        y = actor.pixelY,
        width = 16 * (actor.fields.width + 2),
        height = 16 * actor.fields.stalk_height_and_move_direction + 49,
    }
end

local function render(actor, renderer)
    local box = grabBounds(actor)
    renderer:draw(images.GreenMushroomEdge, { x = box.x, y = box.y })
    for column = 1, actor.fields.width do
        renderer:draw(images.GreenMushroomMiddle, { x = box.x + 16 * column, y = box.y })
    end
    renderer:draw(images.GreenMushroomEdge, {
        x = box.x + 16 * (actor.fields.width + 1),
        y = box.y,
        flipX = true,
    })
    renderer:draw(images.GreenMushroomStalkTop, { x = actor.pixelX, y = actor.pixelY + 24 })
    for segment = 0, actor.fields.stalk_height_and_move_direction - 1 do
        renderer:draw(images.GreenMushroomStalk, { x = actor.pixelX, y = actor.pixelY + 49 + 16 * segment })
    end
    return true
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
