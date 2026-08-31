-- Ported from NSMB Editor 5's NSMBStageObj.cs case 84.

local images = {}

local function init(ctx)
    images.Paragoomba = ctx.loadImage("images/Paragoomba.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 12
    width = 24
    height = 28
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.Paragoomba, RenderX, RenderY - 12) or drawn
    return drawn
end

return {
    objectId = 84,
    name = "Paragoomba",
    category = "Enemies - Overground",
    notes = "Unused behaviours:\n- Has ceiling detection [may be used]\n- Has water interaction [may be used, or Goomba leftover]",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
