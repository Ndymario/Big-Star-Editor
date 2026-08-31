-- Ported from NSMB Editor 5's NSMBStageObj.cs case 301.

local images = {}

local function init(ctx)
    images.SeaWeed = ctx.loadImage("images/SeaWeed.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    x = x - 32
    y = y - 127
    width = 90
    height = 147
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.SeaWeed, RenderX - 32, RenderY - 127) or drawn
    return drawn
end

return {
    objectId = 301,
    name = "Seaweed",
    category = "Effects and decorations",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
