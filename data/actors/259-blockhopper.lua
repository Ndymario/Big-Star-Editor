-- Ported from NSMB Editor 5's NSMBStageObj.cs case 259.

local images = {}

local function init(ctx)
    images.Brick = ctx.loadImage("images/Brick.png")
    images.JumpingBrickBlock = ctx.loadImage("images/JumpingBrickBlock.png")
    images.JumpingQBlock = ctx.loadImage("images/JumpingQBlock.png")
    images.JumpingQBlockOverrides = ctx.loadImage("images/JumpingQBlockOverrides.png")
end

local function grabBounds(actor)
    local height = actor.fields.height * 16 + 14
    return { x = actor.pixelX, y = actor.pixelY - (height - 16), width = 16, height = height }
end

local function render(actor, renderer)
    local head = actor.fields.brick_head and images.JumpingBrickBlock or images.JumpingQBlock
    local headY = actor.pixelY - 14
    local drawn = renderer:draw(head, { x = actor.pixelX, y = headY })
    if actor.fields.item <= 3 then
        drawn = renderer:draw(images.JumpingQBlockOverrides, {
            x = actor.pixelX,
            y = headY + 3,
            sourceX = actor.fields.item * 16,
            sourceY = 0,
            sourceWidth = 16,
            sourceHeight = 16,
        }) or drawn
    end
    for section = 1, actor.fields.height - 1 do
        drawn = renderer:draw(images.Brick, {
            x = actor.pixelX,
            y = headY - 16 * section,
        }) or drawn
    end
    return drawn
end

return {
    objectId = 259,
    name = "Blockhopper",
    category = "Enemies - Desert",
    notes = "A value of 5 (which is unused) for nybble 9 will yield different results for each region due to a glitch (its reading different invalid data from the memory - different in each region because of the differences in code between regions):\nUS : Starman\nJP : Flower\nEU : Mini-mushroom\nKS : Blue shell\nCH : Mushroom/flower",
    fields = {
        {
            key = "item",
            label = "Item",
            bitOffset = 8,
            bitWidth = 4,
            notes = "5=Star (weird) [US]; Flower [JP]; Mini Mushroom [EU]; Blue Shell [KS]; Mushroom/Flower [CH]. 0, 4 and 5 are unused.",
            choices = {
                { value = 0, label = "Nothing" },
                { value = 1, label = "Coin" },
                { value = 2, label = "Mushroom/Flower" },
                { value = 3, label = "1-Up" },
                { value = 4, label = "Coin" },
                { value = 5, label = "[See comment]" },
                { value = 6, label = "Coin" },
                { value = 7, label = "Mushroom/Flower" },
                { value = 8, label = "Mushroom/Flower" },
                { value = 9, label = "Mushroom (weird)" },
                { value = 10, label = "Mushroom" },
                { value = 11, label = "Coin" },
                { value = 12, label = "Mushroom/Flower" },
                { value = 13, label = "Mushroom/Flower" },
                { value = 14, label = "Flower" },
                { value = 15, label = "Coin" },
            },
        },
        {
            key = "brick_head",
            label = "Brick Head",
            bitOffset = 4,
            bitWidth = 1,
            notes = "Makes Blockhopper a bit like the Pile Driver Micro-Goombas in SMB3. Same behaviour as far as I can tell. Unused.",
            checkboxValue = 1,
        },
        {
            key = "height",
            label = "Height",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Higher values will slow down the game. There is a limit, of something.",
            valueOffset = 1,
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
