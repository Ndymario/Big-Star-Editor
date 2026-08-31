-- Ported from NSMB Editor 5's NSMBStageObj.cs case 207.

local images = {}

local function init(ctx)
    images.GhostGoo = ctx.loadImage("images/GhostGoo.png")
end

local function grabBounds(actor)
    local x, y = actor.pixelX, actor.pixelY
    local width, height = 16, 16
    y = y - 32
    width = 256
    height = 48
    return { x = x, y = y, width = width, height = height }
end

local function render(actor, renderer)
    local RenderX, RenderY = actor.pixelX, actor.pixelY
    local drawn = false
    drawn = gfx.blit(renderer, images.GhostGoo, RenderX, RenderY - 32) or drawn
    return drawn
end

return {
    objectId = 207,
    name = "Ghost house goo",
    category = "Enemies - Ghost House",
    init = init,
    render = render,
    grabBounds = grabBounds,
    renderBounds = grabBounds,
}
