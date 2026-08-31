-- Ported from NSMB Editor 5's NSMBStageObj.cs case 259.

local images = {}

local function init(ctx)
    images.Brick = ctx.loadImage("images/Brick.png")
    images.JumpingBrickBlock = ctx.loadImage("images/JumpingBrickBlock.png")
    images.JumpingQBlock = ctx.loadImage("images/JumpingQBlock.png")
    images.JumpingQBlockOverrides = ctx.loadImage("images/JumpingQBlockOverrides.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    height = (d[5] % 0x10) * 16 + 30
    y = y - (height - 16)
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local img
    local drawn = false
    RenderY = RenderY - 14
    img = images.JumpingQBlock
    if (d[5] & 0x10) // 0x10 == 1 then
        img = images.JumpingBrickBlock
    end
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    if (actor.fields.item) <= 3 then
        drawn = gfx.blit(renderer, images.JumpingQBlockOverrides, RenderX, RenderY + 3, nil, nil, { x = (actor.fields.item) * 16, y = 0, width = 16, height = 16 }) or drawn
    end
    for l = 0, (d[5] % 0x10) - 1 do
        RenderY = RenderY - 16
        drawn = gfx.blit(renderer, images.Brick, RenderX, RenderY) or drawn
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
            bitWidth = 4,
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
