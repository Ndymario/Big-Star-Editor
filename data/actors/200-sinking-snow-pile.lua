-- Ported from NSMB Editor 5's NSMBStageObj.cs case 200.

local images = {}

local function init(ctx)
    images.SinkingSnow = ctx.loadImage("images/SinkingSnow.png")
    images.SinkingSnowEdge = ctx.loadImage("images/SinkingSnowEdge.png")
end

local function grabBounds(actor)
    local d = actor.data
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 16
    height = 35
    width = (actor.fields.length + 3) * 16
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local d = actor.data
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local img
    local drawn = false
    img = images.SinkingSnowEdge
    drawn = gfx.blit(renderer, img, RenderX - 16, RenderY) or drawn
    for l = 0, actor.fields.length do
        drawn = gfx.blit(renderer, images.SinkingSnow, RenderX, RenderY) or drawn
        RenderX = RenderX + 16
    end
    img = gfx.transform(img, 0, true, false)
    drawn = gfx.blit(renderer, img, RenderX, RenderY) or drawn
    return drawn
end

return {
    objectId = 200,
    name = "Sinking-snow pile",
    category = "Enemies - Snow",
    notes = "Length of 0 makes the game crash!",
    fields = {
        {
            key = "length",
            label = "Length",
            bitOffset = 0,
            bitWidth = 4,
            notes = "Game will crash if set to 0!",
        },
    },
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
