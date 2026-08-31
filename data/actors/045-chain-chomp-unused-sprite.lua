-- Ported from NSMB Editor 5's NSMBStageObj.cs case 45.

local images = {}

local function init(ctx)
    images.ChainChomp = ctx.loadImage("images/ChainChomp.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 28
    height = 28
    x = x - 29
    width = 44
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.ChainChomp, RenderX - 29, RenderY - 28) or drawn
    return drawn
end

return {
    objectId = 45,
    name = "Chain Chomp (unused sprite)",
    category = "Enemies - Overground",
    notes = "Spawned by Chain Chomp/Log. When spawned via a sprite it can be defeated by ground-pounding its head for some reason.",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
