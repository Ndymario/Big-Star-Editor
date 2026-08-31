-- Ported from NSMB Editor 5's NSMBStageObj.cs case 250.

local images = {}

local function init(ctx)
    images.HangingBlockOverrides = ctx.loadImage("images/HangingBlockOverrides.png")
    images.QBlock = ctx.loadImage("images/QBlock.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    y = y - 80
    height = actor.fields.drop_swing_amount
    if height == 0 then
        height = 8
    end
    height = (height + 1) * 16
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local RenderY2 = RenderY
    local width, height = 0, 0
    local drawn = false
    RenderY2 = actor.fields.drop_swing_amount
    if RenderY2 == 0 then
        RenderY2 = 8
    end
    drawn = gfx.blit(renderer, images.QBlock, RenderX, RenderY + (RenderY2 - 5) * 16) or drawn
    if actor.fields.contents <= 8 then
        drawn = gfx.blit(renderer, images.HangingBlockOverrides, RenderX, RenderY + (RenderY2 - 5) * 16, nil, nil, { x = (actor.fields.contents) * 16, y = 0, width = 16, height = 16 }) or drawn
    end
    drawn = renderer:drawLine({ x = RenderX + 7, y = RenderY + (RenderY2 - 5) * 16, x2 = RenderX + 7, y2 = RenderY - 80, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = RenderX + 8, y = RenderY + (RenderY2 - 5) * 16, x2 = RenderX + 8, y2 = RenderY - 80, color = "#ffffff", strokeWidth = 1 }) or drawn
    return drawn
end

return {
    objectId = 250,
    name = "Hanging Bouncing ? Block",
    category = "Platforms and ropes",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 40,
            bitWidth = 8,
            notes = "Unused. Starts moving when this is activated. Turning off event doesn't affect it.",
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 32,
            bitWidth = 8,
            notes = "Unused. Starts moving when this is activated. Turning off event doesn't affect it.",
        },
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
