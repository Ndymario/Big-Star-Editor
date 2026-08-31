-- Ported from NSMB Editor 5's NSMBStageObj.cs case 51.

local images = {}

local function init(ctx)
    images.TubeBubbles = ctx.loadImage("images/TubeBubbles.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y + 32
    width = 32
    height = 74
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.TubeBubbles
    img = gfx.transform(img, 0, false, true)
    drawn = gfx.blit(renderer, img, RenderX, RenderY + 32) or drawn
    return drawn
end

return {
    objectId = 51,
    name = "Aquatic bubble current (down)",
    category = "Enemies - Water",
    notes = "Setting both input IDs to zero will make the bubbles always run.",
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
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
