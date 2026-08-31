-- Ported from NSMB Editor 5's NSMBStageObj.cs case 168.

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
    images.StoneBlockSpikes = ctx.loadImage("images/StoneBlockSpikes.png")
    images.StoneBlockTop = ctx.loadImage("images/StoneBlockTop.png")
    images.StoneBlockTopLeft = ctx.loadImage("images/StoneBlockTopLeft.png")
    images.StoneBlockTopRight = ctx.loadImage("images/StoneBlockTopRight.png")
end

local function grabBounds(actor)
    local width = math.max(16 * actor.fields.length, 32)
    local height = 16 * actor.fields.height
    local spikes = actor.fields.spikes

    -- These expansions are inherited selection behavior: vertical spikes
    -- enlarge the width, while horizontal spikes enlarge the height.
    if width == 32 and (spikes == 1 or spikes == 2) then
        width = 48
    end
    if width == 32 and spikes == 3 then
        width = 64
    end
    if height == 16 and (spikes == 4 or spikes == 5) then
        height = 32
    end
    if height == 16 and spikes == 6 then
        height = 48
    end
    return { x = actor.pixelX, y = actor.pixelY, width = width, height = height }
end

local function drawBody(renderer, x, y, columns, rows)
    if rows == 1 then
        renderer:draw(images.StoneBlockFlatHorizLeft, { x = x, y = y })
        for column = 1, columns - 2 do
            renderer:draw(images.StoneBlockFlatHorizMiddle, { x = x + 16 * column, y = y })
        end
        renderer:draw(images.StoneBlockFlatHorizRight, { x = x + 16 * (columns - 1), y = y })
        return
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
end

local function render(actor, renderer)
    local spikes = actor.fields.spikes
    local bodyX, bodyY = actor.pixelX, actor.pixelY
    local columns, rows = actor.fields.length, actor.fields.height

    if spikes == 1 or spikes == 3 then
        rows = rows - 1
        bodyY = bodyY + 16
    end
    if spikes == 2 or spikes == 3 then
        rows = rows - 1
    end
    if spikes == 4 or spikes == 6 then
        columns = columns - 1
    end
    if spikes == 5 or spikes == 6 then
        columns = columns - 1
        bodyX = bodyX + 16
    end
    columns = math.max(columns, 2)
    rows = math.max(rows, 1)

    drawBody(renderer, bodyX, bodyY, columns, rows)

    if spikes == 1 or spikes == 3 then
        for column = 0, columns - 1 do
            renderer:draw(images.StoneBlockSpikes, { x = bodyX + 16 * column, y = bodyY - 16 })
        end
    end
    if spikes == 2 or spikes == 3 then
        for column = 0, actor.fields.length - 1 do
            renderer:draw(images.StoneBlockSpikes, {
                x = bodyX + 16 * column,
                y = bodyY + 16 * rows,
                flipY = true,
            })
        end
    end
    if spikes == 4 or spikes == 6 then
        for row = 0, actor.fields.height - 1 do
            renderer:draw(images.StoneBlockSpikes, {
                x = bodyX + 16 * columns,
                y = bodyY + 16 * row,
                rotation = 90,
            })
        end
    end
    if spikes == 5 or spikes == 6 then
        for row = 0, rows - 1 do
            renderer:draw(images.StoneBlockSpikes, {
                x = bodyX - 16,
                y = bodyY + 16 * row,
                rotation = 90,
                flipX = true,
            })
        end
    end
    return true
end

return {
    objectId = 168,
    name = "Stone wall moving left & right or up & down",
    category = "Platforms and ropes",
    fields = {
        {
            key = "horizontal_movement",
            label = "Horizontal movement",
            bitOffset = 28,
            bitWidth = 4,
            checkboxValue = 1,
        },
        {
            key = "spikes",
            label = "Spikes",
            bitOffset = 24,
            bitWidth = 4,
            choices = {
                { value = 0, label = "None" },
                { value = 1, label = "Top" },
                { value = 2, label = "Bottom" },
                { value = 3, label = "Top and Bottom" },
                { value = 4, label = "Right" },
                { value = 5, label = "Left" },
                { value = 6, label = "Right and left" },
            },
        },
        {
            key = "start_delay",
            label = "Start delay",
            bitOffset = 20,
            bitWidth = 4,
        },
        {
            key = "end_delay",
            label = "End delay",
            bitOffset = 16,
            bitWidth = 4,
        },
        {
            key = "tiles_to_move",
            label = "Tiles to move",
            bitOffset = 12,
            bitWidth = 4,
            notes = "Negative numbers = down/left. Positive numbers up/right. NOT diagonal for example - ticking \"Horizontal movement\" and \"Tiles to move\" to minus -7 will make it move left one block. For vertical movement -8 is no move movement -7 is down one -6 is down two ect",
            signed = true,
        },
        {
            key = "speed",
            label = "Speed",
            bitOffset = 8,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Slowest" },
                { value = 1, label = "Medium" },
                { value = 2, label = "Fast" },
                { value = 3, label = "Fastest" },
            },
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 4,
            bitWidth = 4,
            valueOffset = 1,
        },
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            valueOffset = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
