-- Ported from NSMB Editor 5's NSMBStageObj.cs case 250.

local images = {}

local function init(ctx)
    images.HangingBlockOverrides = ctx.loadImage("images/HangingBlockOverrides.png")
    images.QBlock = ctx.loadImage("images/QBlock.png")
end

local function grabBounds(actor)
    local drop = actor.fields.drop_swing_amount
    if drop == 0 then
        drop = 8
    end
    return { x = actor.pixelX, y = actor.pixelY - 80, width = 16, height = (drop + 1) * 16 }
end

local function render(actor, renderer)
    local drop = actor.fields.drop_swing_amount
    if drop == 0 then
        drop = 8
    end
    local blockY = actor.pixelY + (drop - 5) * 16
    local drawn = false
    drawn = renderer:draw(images.QBlock, { x = actor.pixelX, y = blockY }) or drawn
    if actor.fields.contents <= 8 then
        drawn = renderer:draw(images.HangingBlockOverrides, {
            x = actor.pixelX,
            y = blockY,
            sourceX = actor.fields.contents * 16,
            sourceY = 0,
            sourceWidth = 16,
            sourceHeight = 16,
        }) or drawn
    end
    drawn = renderer:drawLine({ x = actor.pixelX + 7, y = blockY, x2 = actor.pixelX + 7, y2 = actor.pixelY - 80, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = actor.pixelX + 8, y = blockY, x2 = actor.pixelX + 8, y2 = actor.pixelY - 80, color = "#ffffff", strokeWidth = 1 }) or drawn
    return drawn
end

return {
    objectId = 250,
    name = "Hanging Bouncing ? Block",
    category = "Platforms and ropes",
    notes = "Both event bytes are unused. The block starts moving when the event is activated, and turning the event off doesn't affect it.",
    fields = {
        {
            key = "drop_swing_amount",
            label = "Drop / Swing Amount",
            bitOffset = 20,
            bitWidth = 4,
        },
        {
            key = "contents",
            label = "Contents",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Unused.",
            choices = {
                { value = 0, label = "Coin" },
                { value = 1, label = "Fire Flower" },
                { value = 2, label = "Super Star" },
                { value = 3, label = "1-Up Mushroom" },
                { value = 4, label = "Fire Flower" },
                { value = 5, label = "Mini Mushroom" },
                { value = 6, label = "Blue Shell" },
                { value = 7, label = "Mega Mushroom" },
                { value = 8, label = "Super Mushroom" },
            },
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
