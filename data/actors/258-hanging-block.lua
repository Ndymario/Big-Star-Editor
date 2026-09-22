-- Ported from NSMB Editor 5's NSMBStageObj.cs case 258.

local images = {}

local function init(ctx)
    images.HangingBlockOverrides = ctx.loadImage("images/HangingBlockOverrides.png")
    images.QBlock = ctx.loadImage("images/QBlock.png")
end

local function grabBounds(actor)
    return { x = actor.pixelX, y = actor.pixelY - 128, width = 16, height = 144 }
end

local function render(actor, renderer)
    local drawn = renderer:draw(images.QBlock, { x = actor.pixelX, y = actor.pixelY })
    if actor.fields.contents < 10 then
        drawn = renderer:draw(images.HangingBlockOverrides, {
            x = actor.pixelX,
            y = actor.pixelY,
            sourceX = actor.fields.contents * 16,
            sourceY = 0,
            sourceWidth = 16,
            sourceHeight = 16,
        }) or drawn
    end
    drawn = renderer:drawLine({ x = actor.pixelX + 7, y = actor.pixelY - 128, x2 = actor.pixelX + 7, y2 = actor.pixelY, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = actor.pixelX + 8, y = actor.pixelY - 128, x2 = actor.pixelX + 8, y2 = actor.pixelY, color = "#ffffff", strokeWidth = 1 }) or drawn
    return drawn
end

return {
    objectId = 258,
    name = "Hanging ? Block",
    category = "Other",
    fields = {
        {
            key = "unknown_5",
            label = "Unknown 5",
            bitOffset = 24,
            bitWidth = 4,
            notes = "Only ever used as 1.",
        },
        {
            key = "affects_contents",
            label = "Affects contents?",
            bitOffset = 4,
            bitWidth = 4,
            notes = "Affects the contents in an unknown way.",
        },
        {
            key = "contents",
            label = "Contents",
            bitOffset = 0,
            bitWidth = 4,
            choices = {
                { value = 0, label = "Coin" },
                { value = 1, label = "Fire Flower" },
                { value = 2, label = "Super Star" },
                { value = 3, label = "1-Up Mushroom" },
                { value = 4, label = "Fire flower" },
                { value = 5, label = "Mini Mushroom" },
                { value = 6, label = "Blue Shell" },
                { value = 7, label = "Mega Mushroom" },
                { value = 8, label = "Super Mushroom" },
                { value = 9, label = "Super Mushroom" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
