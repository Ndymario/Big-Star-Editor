-- Ported from NSMB Editor 5's NSMBStageObj.cs case 258.

local images = {}

local function init(ctx)
    images.HangingBlockOverrides = ctx.loadImage("images/HangingBlockOverrides.png")
    images.QBlock = ctx.loadImage("images/QBlock.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local height = 16
    y = y - 128
    height = height + 128
    return { x = x, y = y, width = 16, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local width, height = 0, 0
    local drawn = false
    drawn = gfx.blit(renderer, images.QBlock, RenderX, RenderY) or drawn
    if actor.fields.contents < 10 then
        drawn = gfx.blit(renderer, images.HangingBlockOverrides, RenderX, RenderY, nil, nil, { x = (actor.fields.contents) * 16, y = 0, width = 16, height = 16 }) or drawn
    end
    drawn = renderer:drawLine({ x = RenderX + 7, y = RenderY - 128, x2 = RenderX + 7, y2 = RenderY, color = "#ffffff", strokeWidth = 1 }) or drawn
    drawn = renderer:drawLine({ x = RenderX + 8, y = RenderY - 128, x2 = RenderX + 8, y2 = RenderY, color = "#ffffff", strokeWidth = 1 }) or drawn
    return drawn
end

return {
    objectId = 258,
    name = "Hanging ? Block",
    category = "Other",
    fields = {
        {
            key = "trigger_id_1",
            label = "Trigger ID 1",
            bitOffset = 32,
            bitWidth = 8,
        },
        {
            key = "trigger_id_2",
            label = "Trigger ID 2",
            bitOffset = 40,
            bitWidth = 8,
        },
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
