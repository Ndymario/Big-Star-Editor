-- Ported from NSMB Editor 5's NSMBStageObj.cs case 167.

local images = {}

local function init(ctx)
    images.StoneBlockBottom = ctx.loadImage("images/StoneBlockBottom.png")
    images.StoneBlockBottomLeft = ctx.loadImage("images/StoneBlockBottomLeft.png")
    images.StoneBlockBottomRight = ctx.loadImage("images/StoneBlockBottomRight.png")
    images.StoneBlockFlatHorizLeft = ctx.loadImage("images/StoneBlockFlatHorizLeft.png")
    images.StoneBlockFlatHorizMiddle = ctx.loadImage("images/StoneBlockFlatHorizMiddle.png")
    images.StoneBlockFlatHorizRight = ctx.loadImage("images/StoneBlockFlatHorizRight.png")
    images.StoneBlockLeft = ctx.loadImage("images/StoneBlockLeft.png")
    images.StoneBlockMiddle = ctx.loadImage("images/StoneBlockMiddle.png")
    images.StoneBlockRight = ctx.loadImage("images/StoneBlockRight.png")
    images.StoneBlockTop = ctx.loadImage("images/StoneBlockTop.png")
    images.StoneBlockTopLeft = ctx.loadImage("images/StoneBlockTopLeft.png")
    images.StoneBlockTopRight = ctx.loadImage("images/StoneBlockTopRight.png")
end

local function grabBounds(actor)
    local columns = math.max(actor.fields.length, 2)
    local rows = 2 ^ actor.fields.height
    return { x = actor.pixelX, y = actor.pixelY, width = 16 * columns, height = 16 * rows }
end

local function render(actor, renderer)
    local columns = math.max(actor.fields.length, 2)
    local rows = 2 ^ actor.fields.height
    local x, y = actor.pixelX, actor.pixelY

    if rows == 1 then
        renderer:draw(images.StoneBlockFlatHorizLeft, { x = x, y = y })
        for column = 1, columns - 2 do
            renderer:draw(images.StoneBlockFlatHorizMiddle, { x = x + 16 * column, y = y })
        end
        return renderer:draw(images.StoneBlockFlatHorizRight, { x = x + 16 * (columns - 1), y = y })
    end

    renderer:draw(images.StoneBlockTopLeft, { x = x, y = y })
    for column = 1, columns - 2 do
        renderer:draw(images.StoneBlockTop, { x = x + 16 * column, y = y })
    end
    renderer:draw(images.StoneBlockTopRight, { x = x + 16 * (columns - 1), y = y })
    for row = 1, rows - 2 do
        renderer:draw(images.StoneBlockRight, { x = x + 16 * (columns - 1), y = y + 16 * row })
    end
    renderer:draw(images.StoneBlockBottomRight, {
        x = x + 16 * (columns - 1),
        y = y + 16 * (rows - 1),
    })
    for column = columns - 2, 1, -1 do
        renderer:draw(images.StoneBlockBottom, { x = x + 16 * column, y = y + 16 * (rows - 1) })
    end
    renderer:draw(images.StoneBlockBottomLeft, { x = x, y = y + 16 * (rows - 1) })
    for row = rows - 2, 1, -1 do
        renderer:draw(images.StoneBlockLeft, { x = x, y = y + 16 * row })
    end
    for row = 1, rows - 2 do
        for column = 1, columns - 2 do
            renderer:draw(images.StoneBlockMiddle, { x = x + 16 * column, y = y + 16 * row })
        end
    end
    return true
end

return {
    objectId = 167,
    name = "Stone wall moving left and right (actor 167)",
    category = "Platforms and ropes",
    notes = "Exact duplicate of used horizontal moving stone block.\nSame settings as Actor 166.",
    fields = {
        {
            key = "tiles_to_move",
            label = "Tiles to move",
            bitOffset = 8,
            bitWidth = 4,
        },
        {
            key = "start_direction",
            label = "Start Direction",
            bitOffset = 16,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Right" },
                { value = 1, label = "Left" },
            },
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 12,
            bitWidth = 2,
            notes = "Height formula is 2^(N11 mod 4).",
            choices = {
                { value = 0, label = "1" },
                { value = 1, label = "2" },
                { value = 2, label = "4" },
                { value = 3, label = "8" },
            },
        },
        {
            key = "slower",
            label = "Slower",
            bitOffset = 24,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "enable_trigger_id_check",
            label = "Enable trigger ID check",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "unknown_10",
            label = "Unknown 10",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Used in-game, useless.",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
